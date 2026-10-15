//! Modern (virtio 1.x) PCI transport, Stage 20.14.
//!
//! virtio-net in this kernel uses the legacy I/O-port transport. Devices that
//! exist only as modern virtio, such as virtio-gpu, are configured through
//! vendor-specific PCI capabilities that point into memory BARs:
//! - common configuration: features, status, and queue setup;
//! - notification: where to write to kick a queue;
//! - ISR status;
//! - device-specific configuration.
//!
//! This module finds those regions, negotiates `VIRTIO_F_VERSION_1`, and sets
//! up split virtqueues that complete requests synchronously with a bounded
//! poll.

use core::{
    ptr,
    sync::atomic::{fence, Ordering},
};

use crate::{hal::pci, memory, paging};

const PAGE_SIZE: usize = 4096;
const CAP_VENDOR: u8 = 0x09;
const CFG_COMMON: u8 = 1;
const CFG_NOTIFY: u8 = 2;
const CFG_ISR: u8 = 3;
const CFG_DEVICE: u8 = 4;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_FAILED: u8 = 128;

/// Feature bit 32: the device follows virtio 1.x.
const FEATURE_VERSION_1: u64 = 1 << 32;

/// Descriptors per queue: enough for a request and its response, with room.
pub const QUEUE_SIZE: u16 = 16;
const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;
const POLL_LIMIT: usize = 20_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    MissingCapability,
    MmioUnavailable,
    DmaUnavailable,
    FeaturesRejected,
    QueueUnavailable,
    Timeout,
}

/// A page of memory the device may read or write, owned by the driver.
#[derive(Clone, Copy)]
pub struct DmaPages {
    pub physical: u64,
    pub virtual_address: u64,
    pub pages: usize,
}

impl DmaPages {
    pub fn allocate(pages: usize) -> Result<Self, Error> {
        let frame = memory::allocate_contiguous_frames(pages).ok_or(Error::DmaUnavailable)?;
        let physical = frame.start_address().as_u64();
        let virtual_address = paging::physical_memory_offset()
            .and_then(|offset| offset.checked_add(physical))
            .ok_or(Error::DmaUnavailable)?;
        // SAFETY: the frames were just allocated to this object.
        unsafe { ptr::write_bytes(virtual_address as *mut u8, 0, pages * PAGE_SIZE) };
        Ok(Self { physical, virtual_address, pages })
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: owned, mapped through the physical-memory window, and
        // exclusively borrowed.
        unsafe { core::slice::from_raw_parts_mut(self.virtual_address as *mut u8, self.pages * PAGE_SIZE) }
    }

    pub fn bytes(&self) -> &[u8] {
        // SAFETY: as above, shared.
        unsafe { core::slice::from_raw_parts(self.virtual_address as *const u8, self.pages * PAGE_SIZE) }
    }
}

/// A capability's region: BAR address plus offset, mapped page by page.
#[derive(Clone, Copy)]
struct Region {
    physical: u64,
}

impl Region {
    fn address(&self, offset: u64) -> Result<u64, Error> {
        let physical = self.physical + offset;
        let page = paging::map_mmio(physical & !0xfff).map_err(|_| Error::MmioUnavailable)?;
        Ok(page + (physical & 0xfff))
    }

    fn read8(&self, offset: u64) -> Result<u8, Error> {
        // SAFETY: a mapped device register.
        Ok(unsafe { ptr::read_volatile(self.address(offset)? as *const u8) })
    }
    fn read16(&self, offset: u64) -> Result<u16, Error> {
        // SAFETY: a mapped, aligned device register.
        Ok(unsafe { ptr::read_volatile(self.address(offset)? as *const u16) })
    }
    fn read32(&self, offset: u64) -> Result<u32, Error> {
        // SAFETY: a mapped, aligned device register.
        Ok(unsafe { ptr::read_volatile(self.address(offset)? as *const u32) })
    }
    fn write8(&self, offset: u64, value: u8) -> Result<(), Error> {
        // SAFETY: a mapped device register.
        unsafe { ptr::write_volatile(self.address(offset)? as *mut u8, value) };
        Ok(())
    }
    fn write16(&self, offset: u64, value: u16) -> Result<(), Error> {
        // SAFETY: a mapped, aligned device register.
        unsafe { ptr::write_volatile(self.address(offset)? as *mut u16, value) };
        Ok(())
    }
    fn write32(&self, offset: u64, value: u32) -> Result<(), Error> {
        // SAFETY: a mapped, aligned device register.
        unsafe { ptr::write_volatile(self.address(offset)? as *mut u32, value) };
        Ok(())
    }
    fn write64(&self, offset: u64, value: u64) -> Result<(), Error> {
        self.write32(offset, value as u32)?;
        self.write32(offset + 4, (value >> 32) as u32)
    }
}

/// A split virtqueue in one DMA page: descriptors, available ring, used ring.
pub struct Queue {
    index: u16,
    notify: u64,
    memory: DmaPages,
    next_avail: u16,
    last_used: u16,
}

const DESC_OFFSET: usize = 0;
const AVAIL_OFFSET: usize = 256;
const USED_OFFSET: usize = 512;

impl Queue {
    fn write_desc(&mut self, index: u16, address: u64, length: u32, flags: u16, next: u16) {
        let offset = DESC_OFFSET + usize::from(index) * 16;
        let bytes = &mut self.memory.bytes_mut()[offset..offset + 16];
        bytes[0..8].copy_from_slice(&address.to_le_bytes());
        bytes[8..12].copy_from_slice(&length.to_le_bytes());
        bytes[12..14].copy_from_slice(&flags.to_le_bytes());
        bytes[14..16].copy_from_slice(&next.to_le_bytes());
    }
}

pub struct Transport {
    address: pci::Address,
    common: Region,
    notify: Region,
    notify_multiplier: u32,
    _isr: Region,
    pub device: Region2,
}

/// The device-specific configuration region, for the device driver.
#[derive(Clone, Copy)]
pub struct Region2(Region);

impl Region2 {
    pub fn read32(&self, offset: u64) -> Result<u32, Error> {
        self.0.read32(offset)
    }
}

impl Transport {
    /// Find the device's virtio capabilities, reset it and negotiate
    /// virtio 1.x with `wanted` device features (only those the device
    /// offers are accepted).
    pub fn new(device: pci::Device, wanted: u64) -> Result<Self, Error> {
        let address = pci::Address { segment: device.segment, bus: device.bus, device: device.device, function: device.function };
        if !pci::enable_memory_bus_master(address) {
            return Err(Error::MmioUnavailable);
        }
        let read = |offset: u8| pci::read_config_dword(device.bus, device.device, device.function, offset);
        let bar_base = |bar: u8| -> Option<u64> {
            let entry = device.bars.get(usize::from(bar))?;
            entry.valid.then_some(entry.address)
        };
        let (mut common, mut notify, mut isr, mut config) = (None, None, None, None);
        let mut multiplier = 0;
        // Capability list from the status register's capabilities bit.
        let mut pointer = (read(0x34) & 0xfc) as u8;
        let mut steps = 0;
        while pointer != 0 && steps < 48 {
            steps += 1;
            let header = read(pointer);
            let (id, next) = (header as u8, (header >> 8) as u8);
            if id == CAP_VENDOR {
                let cfg_type = (header >> 24) as u8;
                let word = read(pointer + 4);
                let bar = word as u8;
                let offset = u64::from(read(pointer + 8));
                if let Some(base) = bar_base(bar) {
                    let region = Some(Region { physical: base + offset });
                    match cfg_type {
                        CFG_COMMON if common.is_none() => common = region,
                        CFG_NOTIFY if notify.is_none() => {
                            notify = region;
                            multiplier = read(pointer + 16);
                        }
                        CFG_ISR if isr.is_none() => isr = region,
                        CFG_DEVICE if config.is_none() => config = region,
                        _ => {}
                    }
                }
            }
            pointer = next & 0xfc;
        }
        let transport = Self {
            address,
            common: common.ok_or(Error::MissingCapability)?,
            notify: notify.ok_or(Error::MissingCapability)?,
            notify_multiplier: multiplier,
            _isr: isr.ok_or(Error::MissingCapability)?,
            device: Region2(config.ok_or(Error::MissingCapability)?),
        };
        let common = transport.common;
        // Reset, then acknowledge.
        common.write8(0x14, 0)?;
        for _ in 0..POLL_LIMIT {
            if common.read8(0x14)? == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        common.write8(0x14, STATUS_ACKNOWLEDGE)?;
        common.write8(0x14, STATUS_ACKNOWLEDGE | STATUS_DRIVER)?;
        let offered = {
            common.write32(0x00, 0)?;
            let low = u64::from(common.read32(0x04)?);
            common.write32(0x00, 1)?;
            low | (u64::from(common.read32(0x04)?) << 32)
        };
        if offered & FEATURE_VERSION_1 == 0 {
            common.write8(0x14, STATUS_FAILED)?;
            return Err(Error::FeaturesRejected);
        }
        let accepted = offered & (wanted | FEATURE_VERSION_1);
        common.write32(0x08, 0)?;
        common.write32(0x0c, accepted as u32)?;
        common.write32(0x08, 1)?;
        common.write32(0x0c, (accepted >> 32) as u32)?;
        let status = STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK;
        common.write8(0x14, status)?;
        if common.read8(0x14)? & STATUS_FEATURES_OK == 0 {
            common.write8(0x14, STATUS_FAILED)?;
            return Err(Error::FeaturesRejected);
        }
        Ok(transport)
    }

    /// Set up queue `index` with [`QUEUE_SIZE`] descriptors.
    pub fn queue(&mut self, index: u16) -> Result<Queue, Error> {
        let common = self.common;
        common.write16(0x16, index)?;
        let maximum = common.read16(0x18)?;
        if maximum == 0 {
            return Err(Error::QueueUnavailable);
        }
        let size = QUEUE_SIZE.min(maximum);
        if size != QUEUE_SIZE {
            return Err(Error::QueueUnavailable);
        }
        let memory = DmaPages::allocate(1)?;
        common.write16(0x18, size)?;
        common.write64(0x20, memory.physical + DESC_OFFSET as u64)?;
        common.write64(0x28, memory.physical + AVAIL_OFFSET as u64)?;
        common.write64(0x30, memory.physical + USED_OFFSET as u64)?;
        let notify_off = u64::from(common.read16(0x1e)?);
        common.write16(0x1c, 1)?;
        let notify = self.notify.physical + notify_off * u64::from(self.notify_multiplier);
        Ok(Queue { index, notify, memory, next_avail: 0, last_used: 0 })
    }

    /// Tell the device the driver is ready.
    pub fn ready(&self) -> Result<(), Error> {
        let status = STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK;
        self.common.write8(0x14, status)
    }

    /// Send a device-readable request and wait for the device-writable
    /// response; returns the bytes the device wrote.
    pub fn exchange(&self, queue: &mut Queue, request: (u64, u32), response: (u64, u32)) -> Result<u32, Error> {
        queue.write_desc(0, request.0, request.1, DESC_F_NEXT, 1);
        queue.write_desc(1, response.0, response.1, DESC_F_WRITE, 0);
        let slot = usize::from(queue.next_avail % QUEUE_SIZE);
        {
            let bytes = queue.memory.bytes_mut();
            let entry = AVAIL_OFFSET + 4 + slot * 2;
            bytes[entry..entry + 2].copy_from_slice(&0u16.to_le_bytes());
            fence(Ordering::Release);
            let next = queue.next_avail.wrapping_add(1);
            bytes[AVAIL_OFFSET + 2..AVAIL_OFFSET + 4].copy_from_slice(&next.to_le_bytes());
        }
        queue.next_avail = queue.next_avail.wrapping_add(1);
        fence(Ordering::SeqCst);
        let page = paging::map_mmio(queue.notify & !0xfff).map_err(|_| Error::MmioUnavailable)?;
        // SAFETY: the queue's mapped notification register.
        unsafe { ptr::write_volatile((page + (queue.notify & 0xfff)) as *mut u16, queue.index) };
        for _ in 0..POLL_LIMIT {
            fence(Ordering::Acquire);
            let bytes = queue.memory.bytes();
            let used = u16::from_le_bytes([bytes[USED_OFFSET + 2], bytes[USED_OFFSET + 3]]);
            if used != queue.last_used {
                let slot = usize::from(queue.last_used % QUEUE_SIZE);
                let entry = USED_OFFSET + 4 + slot * 8;
                let written = u32::from_le_bytes([bytes[entry + 4], bytes[entry + 5], bytes[entry + 6], bytes[entry + 7]]);
                queue.last_used = queue.last_used.wrapping_add(1);
                return Ok(written);
            }
            core::hint::spin_loop();
        }
        Err(Error::Timeout)
    }

    pub fn pci_address(&self) -> pci::Address {
        self.address
    }
}
