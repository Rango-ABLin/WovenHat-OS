use core::arch::asm;

use crate::irq_lock::IrqMutex as Mutex;

use crate::block::{BlockDevice, Error, SECTOR_SIZE};

const DATA: u16 = 0x1f0;
const SECTOR_COUNT: u16 = 0x1f2;
const LBA_LOW: u16 = 0x1f3;
const LBA_MID: u16 = 0x1f4;
const LBA_HIGH: u16 = 0x1f5;
const DRIVE: u16 = 0x1f6;
const STATUS_COMMAND: u16 = 0x1f7;
const ALT_STATUS: u16 = 0x3f6;

const COMMAND_IDENTIFY: u8 = 0xec;
const COMMAND_READ_SECTORS: u8 = 0x20;
const COMMAND_WRITE_SECTORS: u8 = 0x30;
const ATA_RETRIES: usize = 3;
const STATUS_ERR: u8 = 1;
const STATUS_DRQ: u8 = 1 << 3;
const STATUS_DF: u8 = 1 << 5;
const STATUS_BSY: u8 = 1 << 7;
/// Spin bound used only before the PIT is running, during drive
/// identification. Once the timer ticks, `poll_until` switches to the
/// wall-clock deadline below.
const POLL_LIMIT: usize = 50_000_000;
/// Wall-clock bound for one ATA command, in timer ticks.
///
/// An iteration count is not a time bound. QEMU's file-backed disks complete a
/// write before a handful of status reads, but real media does not: a USB flash
/// device buffers the first few hundred sectors and then stalls for tens of
/// milliseconds per write while it programs. A 100_000-iteration wait mistook
/// that for a device fault, which only physical hardware revealed -- format of
/// an 8 GB USB stick failed at the 320th sector write while the same code
/// passed against an emulated image.
const POLL_DEADLINE_TICKS: u64 = 5 * crate::timer::FREQUENCY_HZ as u64;
const LBA28_SECTORS: u64 = 1 << 28;

/// The cached ATA device handed to `with_primary_master` callers.
///
/// Public because it appears in that function's callback signature, so any
/// caller naming the parameter type needs to see it.
pub type BufferedAta = crate::block_cache::CachedDevice<AtaPio, 64>;

/// The primary PIO device is serialized for each complete command sequence.
/// Rank 10 lets block workers own the device while higher-ranked cache or
/// metadata locks are acquired by callers that need them.
static PRIMARY_MASTER: Mutex<Option<BufferedAta>> = Mutex::with_rank(None, 10);

pub struct AtaPio {
    sectors: u64,
}

impl AtaPio {
    fn identify() -> Option<Self> {
        unsafe {
            outb(DRIVE, 0xa0);
            io_delay();
            outb(SECTOR_COUNT, 0);
            outb(LBA_LOW, 0);
            outb(LBA_MID, 0);
            outb(LBA_HIGH, 0);
            outb(STATUS_COMMAND, COMMAND_IDENTIFY);

            if inb(STATUS_COMMAND) == 0 || !poll_data_ready() {
                return None;
            }
            if inb(LBA_MID) != 0 || inb(LBA_HIGH) != 0 {
                return None;
            }

            let mut words = [0_u16; 256];
            for word in &mut words {
                *word = inw(DATA);
            }
            identify_sector_count(&words).map(|sectors| Self { sectors })
        }
    }
}

impl BlockDevice for AtaPio {
    fn sector_count(&self) -> u64 {
        self.sectors
    }

    fn read_sector(&mut self, lba: u64, sector: &mut [u8]) -> Result<(), Error> {
        if sector.len() != SECTOR_SIZE {
            return Err(Error::InvalidBuffer);
        }
        if lba >= self.sectors || lba >= LBA28_SECTORS {
            return Err(Error::OutOfBounds);
        }

        for _ in 0..ATA_RETRIES {
            if unsafe { read_sector_once(lba, sector) }.is_ok() {
                return Ok(());
            }
            unsafe { io_delay() };
        }
        Err(Error::DeviceFault)
    }

    fn write_sector(&mut self, lba: u64, sector: &[u8]) -> Result<(), Error> {
        if sector.len() != SECTOR_SIZE {
            return Err(Error::InvalidBuffer);
        }
        if lba >= self.sectors || lba >= LBA28_SECTORS {
            return Err(Error::OutOfBounds);
        }

        for _ in 0..ATA_RETRIES {
            if unsafe { write_sector_once(lba, sector) }.is_ok() {
                return Ok(());
            }
            unsafe { io_delay() };
        }
        Err(Error::DeviceFault)
    }
}

unsafe fn select_lba(lba: u64) -> Result<(), Error> {
    if !poll_not_busy() {
        return Err(Error::DeviceFault);
    }
    outb(DRIVE, 0xe0 | ((lba >> 24) as u8 & 0x0f));
    io_delay();
    outb(SECTOR_COUNT, 1);
    outb(LBA_LOW, lba as u8);
    outb(LBA_MID, (lba >> 8) as u8);
    outb(LBA_HIGH, (lba >> 16) as u8);
    Ok(())
}

unsafe fn read_sector_once(lba: u64, sector: &mut [u8]) -> Result<(), Error> {
    select_lba(lba)?;
    outb(STATUS_COMMAND, COMMAND_READ_SECTORS);
    io_delay();
    if !poll_data_ready() {
        return Err(Error::DeviceFault);
    }
    for index in 0..SECTOR_SIZE / 2 {
        let bytes = inw(DATA).to_le_bytes();
        sector[index * 2] = bytes[0];
        sector[index * 2 + 1] = bytes[1];
    }
    Ok(())
}

unsafe fn write_sector_once(lba: u64, sector: &[u8]) -> Result<(), Error> {
    select_lba(lba)?;
    outb(STATUS_COMMAND, COMMAND_WRITE_SECTORS);
    io_delay();
    if !poll_data_ready() {
        return Err(Error::DeviceFault);
    }
    for index in 0..SECTOR_SIZE / 2 {
        let word = u16::from_le_bytes([sector[index * 2], sector[index * 2 + 1]]);
        outw(DATA, word);
    }
    io_delay();
    if !poll_not_busy() {
        return Err(Error::DeviceFault);
    }
    let status = inb(STATUS_COMMAND);
    if status & (STATUS_ERR | STATUS_DF) != 0 {
        return Err(Error::DeviceFault);
    }
    Ok(())
}

pub fn init() -> Option<u64> {
    let mut primary = PRIMARY_MASTER.lock();
    if let Some(disk) = primary.as_ref() {
        return Some(disk.sector_count());
    }
    let disk = AtaPio::identify()?;
    let sectors = disk.sectors;
    *primary = Some(BufferedAta::new(disk));
    Some(sectors)
}

pub fn with_primary_master<R>(operation: impl FnOnce(&mut BufferedAta) -> R) -> Option<R> {
    let mut disk = PRIMARY_MASTER.lock();
    disk.as_mut().map(operation)
}

fn identify_sector_count(words: &[u16; 256]) -> Option<u64> {
    if words[49] & (1 << 9) == 0 {
        return None;
    }
    let sectors = u32::from(words[60]) | (u32::from(words[61]) << 16);
    (sectors != 0).then_some(u64::from(sectors).min(LBA28_SECTORS))
}

pub fn self_test() -> bool {
    let mut words = [0_u16; 256];
    words[49] = 1 << 9;
    words[60] = 0x5678;
    words[61] = 0x1234;
    let valid = identify_sector_count(&words) == Some(0x1234_5678_u64.min(LBA28_SECTORS));
    words[49] = 0;
    let requires_lba = identify_sector_count(&words).is_none();
    words[49] = 1 << 9;
    words[60] = 0;
    words[61] = 0;
    valid && requires_lba && identify_sector_count(&words).is_none()
}

/// Poll the status register until `ready`, an error bit, or the deadline.
///
/// The wall clock is the real bound: whenever the timer has advanced past
/// `POLL_DEADLINE_TICKS` the command is abandoned. The spin count only governs
/// when the PIT is not yet running, which is the case while the drive is first
/// identified.
///
/// # Safety
/// Caller must hold the channel and be permitted to read its status port.
unsafe fn poll_until(ready: impl Fn(u8) -> bool) -> bool {
    let start = crate::timer::ticks();
    let mut spins: usize = 0;
    loop {
        let status = unsafe { inb(STATUS_COMMAND) };
        if status & (STATUS_ERR | STATUS_DF) != 0 {
            return false;
        }
        if ready(status) {
            return true;
        }
        let elapsed = crate::timer::ticks().wrapping_sub(start);
        if elapsed >= POLL_DEADLINE_TICKS {
            return false;
        }
        if elapsed == 0 {
            spins = spins.saturating_add(1);
            if spins >= POLL_LIMIT {
                return false;
            }
        }
        core::hint::spin_loop();
    }
}

unsafe fn poll_not_busy() -> bool {
    unsafe { poll_until(|status| status & STATUS_BSY == 0) }
}

unsafe fn poll_data_ready() -> bool {
    unsafe { poll_until(|status| status & STATUS_BSY == 0 && status & STATUS_DRQ != 0) }
}

unsafe fn io_delay() {
    for _ in 0..4 {
        let _ = inb(ALT_STATUS);
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", in("dx") port, out("ax") value, options(nomem, nostack, preserves_flags));
    value
}

unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
}
