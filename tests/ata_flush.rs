#[path = "../kernel/src/block.rs"]
mod block;
#[path = "../kernel/src/block_cache.rs"]
mod block_cache;
#[path = "../kernel/src/ata/flush.rs"]
mod flush;
#[path = "../kernel/src/partition.rs"]
mod partition;

use block::{BlockDevice, Error, RamDisk, SECTOR_SIZE};
use flush::{Command, Transport};
use std::collections::VecDeque;

const READY: u8 = 0x40;
const BUSY: u8 = 0x80;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Status(u8),
    Select,
    Settle,
    Issue(u8),
    Relax,
    Write(u64),
}

struct Io {
    statuses: VecDeque<u8>,
    last: u8,
    trace: Vec<Event>,
    clock: u64,
    step: u64,
    scripted_ticks: VecDeque<u64>,
}

impl Io {
    fn new(statuses: &[u8]) -> Self {
        Self {
            statuses: statuses.iter().copied().collect(),
            last: READY,
            trace: Vec::new(),
            clock: 0,
            step: 1,
            scripted_ticks: VecDeque::new(),
        }
    }
    fn commands(&self) -> Vec<u8> {
        self.trace
            .iter()
            .filter_map(|event| match event {
                Event::Issue(command) => Some(*command),
                _ => None,
            })
            .collect()
    }
}

impl Transport for Io {
    fn status(&mut self) -> u8 {
        self.last = self.statuses.pop_front().unwrap_or(self.last);
        self.trace.push(Event::Status(self.last));
        self.last
    }
    fn select_master(&mut self) {
        self.trace.push(Event::Select);
    }
    fn issue(&mut self, command: u8) {
        self.trace.push(Event::Issue(command));
    }
    fn settle(&mut self) {
        self.trace.push(Event::Settle);
    }
    fn ticks(&mut self) -> u64 {
        if let Some(now) = self.scripted_ticks.pop_front() {
            self.clock = now;
            return now;
        }
        let now = self.clock;
        self.clock = self.clock.wrapping_add(self.step);
        now
    }
    fn relax(&mut self) {
        self.trace.push(Event::Relax);
    }
}

#[test]
fn legacy_command_is_ordered_and_cannot_skip_completion() {
    let mut io = Io::new(&[READY, READY, BUSY, READY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20).is_ok());
    assert_eq!(
        io.trace,
        [
            Event::Status(READY),
            Event::Select,
            Event::Settle,
            Event::Status(READY),
            Event::Issue(0xe7),
            Event::Settle,
            Event::Status(BUSY),
            Event::Relax,
            Event::Status(READY)
        ]
    );
}

#[test]
fn extended_command_requires_valid_identify_and_both_support_bits() {
    for validity in [0, 0x4000, 0x8000, 0xc000] {
        for (support, expected) in [
            (0, Command::Legacy),
            (0x400, Command::Legacy),
            (0x2000, Command::Legacy),
            (0x2400, Command::Extended),
        ] {
            let selected = Command::from_identify(validity | support);
            let expected = if validity == 0x4000 {
                expected
            } else {
                Command::Legacy
            };
            assert_eq!(selected, expected);
        }
    }
    assert_eq!(Command::from_identify(0xffff), Command::Legacy);
    let mut io = Io::new(&[READY, READY, READY]);
    assert!(flush::run(&mut io, Command::Extended, 5, 20).is_ok());
    assert_eq!(io.commands(), [0xea]);
}

#[test]
fn bsy_hides_stale_error_bits_until_completion() {
    let mut io = Io::new(&[BUSY | 0x21, READY, READY, BUSY | 0x21, READY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20).is_ok());
    assert_eq!(io.commands(), [0xe7]);
}

#[test]
fn each_bad_completion_fails_without_automatic_reissue() {
    for status in [READY | 1, READY | 0x20, READY | 8, 0, 0xff] {
        let mut io = Io::new(&[READY, READY, status]);
        assert!(flush::run(&mut io, Command::Legacy, 5, 20) == Err(Error::DeviceFault));
        assert_eq!(io.commands(), [0xe7]);
    }
}

#[test]
fn absent_or_busy_channel_never_gets_a_command() {
    for status in [0, 0xff, BUSY, READY | 8] {
        let mut io = Io::new(&[status]);
        assert!(flush::run(&mut io, Command::Legacy, 5, 20) == Err(Error::DeviceFault));
        assert!(io.commands().is_empty());
    }
}

#[test]
fn selected_device_must_be_idle_before_issuing() {
    let mut io = Io::new(&[READY, BUSY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20) == Err(Error::DeviceFault));
    assert!(io.commands().is_empty());
    assert!(io.trace.contains(&Event::Select));
}

#[test]
fn stale_precommand_errors_do_not_prevent_a_fresh_flush() {
    let mut io = Io::new(&[READY | 1, READY | 0x20, READY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20).is_ok());
    assert_eq!(io.commands(), [0xe7]);
}

#[test]
fn busy_completion_timeout_is_not_success_or_a_retry() {
    let mut io = Io::new(&[READY, READY, BUSY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20) == Err(Error::DeviceFault));
    assert_eq!(io.commands(), [0xe7]);
}

#[test]
fn stopped_timer_is_bounded_at_any_initial_tick() {
    for start in [0, 7, u64::MAX] {
        let mut io = Io::new(&[READY, READY, BUSY]);
        io.clock = start;
        io.step = 0;
        assert!(flush::run(&mut io, Command::Legacy, 5, 3) == Err(Error::DeviceFault));
        assert_eq!(
            io.trace
                .iter()
                .filter(|e| **e == Event::Status(BUSY))
                .count(),
            3
        );
        assert_eq!(io.commands(), [0xe7]);
    }
}

#[test]
fn timer_that_stalls_after_progress_also_has_a_bounded_wait() {
    let mut io = Io::new(&[READY, READY, BUSY]);
    io.scripted_ticks.extend([100, 101]);
    io.step = 0;
    assert!(flush::run(&mut io, Command::Legacy, 10, 3) == Err(Error::DeviceFault));
    assert_eq!(
        io.trace
            .iter()
            .filter(|e| **e == Event::Status(BUSY))
            .count(),
        4
    );
    assert_eq!(io.commands(), [0xe7]);
}

#[test]
fn deadline_handles_tick_wrap_without_an_unbounded_wait() {
    let mut io = Io::new(&[READY, READY, BUSY]);
    io.clock = u64::MAX - 1;
    assert!(flush::run(&mut io, Command::Legacy, 4, 20) == Err(Error::DeviceFault));
    assert_eq!(io.commands(), [0xe7]);
    assert_eq!(
        io.trace
            .iter()
            .filter(|e| **e == Event::Status(BUSY))
            .count(),
        4
    );
}

#[test]
fn invalid_budget_touches_no_registers() {
    for (ticks, spins) in [(0, 3), (5, 0), (0, 0)] {
        let mut io = Io::new(&[]);
        assert!(flush::run(&mut io, Command::Legacy, ticks, spins) == Err(Error::DeviceFault));
        assert!(io.trace.is_empty());
    }
}

#[test]
fn explicit_retry_reacquires_idle_and_issues_a_fresh_command() {
    let mut io = Io::new(&[READY, READY, READY | 1]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20).is_err());
    io.statuses.extend([READY | 1, READY | 1, BUSY, READY]);
    assert!(flush::run(&mut io, Command::Legacy, 5, 20).is_ok());
    assert_eq!(io.commands(), [0xe7, 0xe7]);
}

struct Device {
    disk: RamDisk<4>,
    io: Io,
    fail_write: bool,
}

impl BlockDevice for Device {
    fn sector_count(&self) -> u64 {
        self.disk.sector_count()
    }
    fn read_sector(&mut self, lba: u64, data: &mut [u8]) -> Result<(), Error> {
        self.disk.read_sector(lba, data)
    }
    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), Error> {
        if self.fail_write {
            return Err(Error::DeviceFault);
        }
        self.io.trace.push(Event::Write(lba));
        self.disk.write_sector(lba, data)
    }
    fn flush(&mut self) -> Result<(), Error> {
        flush::run(&mut self.io, Command::Legacy, 5, 20)
    }
}

#[test]
fn cache_writes_precede_device_flush_and_errors_propagate_through_partition() {
    let mut device = Device {
        disk: RamDisk::new(),
        io: Io::new(&[READY, READY, READY | 1]),
        fail_write: false,
    };
    {
        let mut cache = block_cache::CachedDevice::<_, 2>::new(&mut device);
        let part = partition::Partition {
            start_lba: 1,
            sectors: 2,
            kind: 0x0b,
        };
        {
            let mut view = partition::PartitionDevice::new(&mut cache, part)
                .ok()
                .unwrap();
            assert!(view.write_sector(0, &[7; SECTOR_SIZE]).is_ok());
            assert!(view.write_sector(1, &[9; SECTOR_SIZE]).is_ok());
            assert!(view.flush() == Err(Error::DeviceFault));
        }
        // Data reached the device, but that alone is NOT physical durability.
        assert_eq!(cache.stats().dirty, 0);
        assert_eq!(cache.stats().writebacks, 2);
    }
    assert_eq!(&device.io.trace[..2], &[Event::Write(1), Event::Write(2)]);
    assert_eq!(device.io.commands(), [0xe7]);
    device.io.statuses.extend([READY, READY, READY]);
    // Even an empty software cache must forward an explicit flush retry.
    assert!(block_cache::CachedDevice::<_, 2>::new(&mut device)
        .flush()
        .is_ok());
    assert_eq!(device.io.commands(), [0xe7, 0xe7]);
    let mut out = [0; SECTOR_SIZE];
    assert!(device.disk.read_sector(1, &mut out).is_ok());
    assert_eq!(out, [7; SECTOR_SIZE]);
    assert!(partition::self_test());
}

#[test]
fn software_write_failure_does_not_issue_a_device_flush_or_drop_dirty_data() {
    let mut device = Device {
        disk: RamDisk::new(),
        io: Io::new(&[]),
        fail_write: true,
    };
    {
        let mut cache = block_cache::CachedDevice::<_, 2>::new(&mut device);
        assert!(cache.write_sector(0, &[3; SECTOR_SIZE]).is_ok());
        assert!(cache.flush() == Err(Error::DeviceFault));
        assert_eq!(cache.stats().dirty, 1);
        let mut out = [0; SECTOR_SIZE];
        assert!(cache.read_sector(0, &mut out).is_ok());
        assert_eq!(out, [3; SECTOR_SIZE]);
    }
    assert!(device.io.commands().is_empty());
}

#[test]
fn existing_block_and_cache_contracts_remain_exercised() {
    assert!(block::self_test());
    assert!(block_cache::self_test());
    let mut disk = RamDisk::<4>::new();
    let mut cache = block_cache::CachedDevice::<_, 2>::new(&mut disk);
    let mut data = [0; SECTOR_SIZE];
    assert!(cache.read_sector(0, &mut data).is_ok());
    assert!(cache.read_sector(0, &mut data).is_ok());
    let stats = cache.stats();
    assert_eq!((stats.hits, stats.misses, stats.capacity), (1, 1, 2));
}
