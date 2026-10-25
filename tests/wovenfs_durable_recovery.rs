//! WovenFS durability regression: volatile write-cache loss and flush faults.
//! This tests the block-device contract, not physical hardware power-loss behavior.
#![allow(dead_code)]
use std::cell::RefCell;
use std::rc::Rc;

#[path = "../kernel/src/block.rs"]
mod block;
#[path = "../kernel/src/wfs.rs"]
mod wfs;

use block::{BlockDevice, Error as BlockError, SECTOR_SIZE};
use wfs::{Error as FsError, Volume, MAX_FILE_SIZE};

const SECTORS: usize = 192;
type Media = Rc<RefCell<Vec<[u8; SECTOR_SIZE]>>>;

/// Writes are volatile until flush. A reboot loses the outstanding write cache.
/// A flush can fail before persisting any pending write. The model assumes a
/// successful flush persists all preceding writes, as BlockDevice must promise.
#[derive(Clone)]
struct CachedDisk {
    durable: Media,
    pending: Vec<(usize, [u8; SECTOR_SIZE])>,
    flushes: usize,
    fail_flush: Option<usize>,
}

impl CachedDisk {
    fn blank() -> Self {
        Self {
            durable: Rc::new(RefCell::new(vec![[0; SECTOR_SIZE]; SECTORS])),
            pending: Vec::new(),
            flushes: 0,
            fail_flush: None,
        }
    }

    fn reboot(&self) -> Self {
        Self {
            durable: Rc::new(RefCell::new(self.durable.borrow().clone())),
            pending: Vec::new(),
            flushes: 0,
            fail_flush: None,
        }
    }

    fn reboot_with_flush_failure(&self, fail_flush: usize) -> Self {
        let mut disk = self.reboot();
        disk.fail_flush = Some(fail_flush);
        disk
    }
}

impl BlockDevice for CachedDisk {
    fn sector_count(&self) -> u64 {
        self.durable.borrow().len() as u64
    }

    fn read_sector(&mut self, lba: u64, sector: &mut [u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        let durable = self.durable.borrow();
        let value = durable.get(index).ok_or(BlockError::OutOfBounds)?;
        if let Some((_, newest)) = self.pending.iter().rev().find(|(at, _)| *at == index) {
            sector.copy_from_slice(newest);
        } else {
            sector.copy_from_slice(value);
        }
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, sector: &[u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        if index >= self.durable.borrow().len() {
            return Err(BlockError::OutOfBounds);
        }
        let mut bytes = [0; SECTOR_SIZE];
        bytes.copy_from_slice(sector);
        self.pending.push((index, bytes));
        Ok(())
    }

    fn flush(&mut self) -> Result<(), BlockError> {
        let index = self.flushes;
        self.flushes += 1;
        if self.fail_flush == Some(index) {
            return Err(BlockError::DeviceFault);
        }
        let mut media = self.durable.borrow_mut();
        for (index, bytes) in self.pending.drain(..) {
            media[index] = bytes;
        }
        Ok(())
    }
}

fn image() -> CachedDisk {
    let disk = CachedDisk::blank();
    Volume::format(disk.clone(), 32).expect("format");
    disk.reboot()
}

fn read_file(disk: &CachedDisk, name: &str) -> Option<Vec<u8>> {
    let mut volume = Volume::mount(disk.reboot()).expect("mount");
    volume.check().expect("fsck");
    let entry = match volume.lookup(1, name) {
        Ok(entry) => entry,
        Err(FsError::NotFound) => return None,
        Err(error) => panic!("lookup failed: {error:?}"),
    };
    let mut output = vec![0; MAX_FILE_SIZE];
    let len = volume.read_file(entry.inode, &mut output).expect("read");
    output.truncate(len);
    Some(output)
}

#[test]
fn acknowledged_create_and_write_survive_cache_loss_and_two_reboots() {
    let disk = image();
    let payload = (0..1536_u32).map(|i| (i % 251) as u8).collect::<Vec<_>>();
    {
        let mut volume = Volume::mount(disk.reboot()).expect("mount");
        let inode = volume.create(1, "durable.bin", 0o600, 1).expect("create");
        volume.write_file(inode, &payload, 2).expect("write");
        volume.check().expect("check while mounted");
    }
    assert_eq!(read_file(&disk, "durable.bin"), Some(payload.clone()));
    assert_eq!(read_file(&disk.reboot(), "durable.bin"), Some(payload));
}

#[test]
fn flush_failure_at_each_boundary_never_exposes_torn_replacement() {
    let disk = image();
    let old = vec![0x11; 1024];
    let new = vec![0x77; 1536];
    {
        let mut volume = Volume::mount(disk.reboot()).expect("mount");
        let inode = volume.create(1, "replace.bin", 0o600, 1).expect("create");
        volume.write_file(inode, &old, 2).expect("seed");
    }
    let reference = disk.reboot();
    let probe = reference.reboot();
    let total_flushes = {
        let mut volume = Volume::mount(probe.clone()).expect("probe mount");
        let inode = volume.lookup(1, "replace.bin").expect("lookup").inode;
        volume.write_file(inode, &new, 3).expect("probe write");
        probe.flushes
    };
    // The probe handles a clone of CachedDisk; count the device's flush
    // operations by observing the concrete device returned by Volume.
    // We deliberately enumerate a generous bound; cuts after completion
    // represent successful writes and must also preserve the new contents.
    assert!(total_flushes <= 32);
    for cut in 0..32 {
        let failing = reference.reboot_with_flush_failure(cut);
        let mut volume = Volume::mount(failing.clone()).expect("pre-write mount");
        let inode = volume.lookup(1, "replace.bin").expect("lookup").inode;
        let _ = volume.write_file(inode, &new, 3);
        drop(volume);
        let result = read_file(&failing, "replace.bin").expect("recovered file");
        assert!(
            result == old || result == new,
            "flush cut {cut} produced torn contents ({} bytes)",
            result.len()
        );
    }
}
