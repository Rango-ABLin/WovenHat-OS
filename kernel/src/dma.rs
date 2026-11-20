#![cfg_attr(
    not(feature = "m5-dma-test"),
    expect(
        dead_code,
        reason = "the device handoff API has no production caller yet; the M5 DMA gate exercises it"
    )
)]
//! Shared DMA buffer primitive for WovenHat drivers.
//!
//! [`DmaPage`] is one exclusively owned 4 KiB frame. The CPU reaches it through
//! the bootloader direct map, and the device reaches the same bytes at the
//! frame's physical address, so there is no copy. Ownership is explicit: the
//! buffer is either CPU-owned or device-owned, and the transitions in
//! [`DmaPage::hand_to_device`] and [`DmaPage::reclaim`] are checked against the
//! current owner (see `dma/ownership.rs`).
//!
//! Safety invariant for the whole module: a `DmaPage` never returns its frame to
//! the physical allocator while a device may still write it. Dropping a
//! device-owned page quarantines the frame instead of reusing it, so a late
//! device write cannot corrupt a later allocation.

use core::sync::atomic::{Ordering, fence};

use x86_64::structures::paging::{PhysFrame, Size4KiB};

use crate::{memory, paging, serial};

mod ownership;

pub use ownership::Owner;
use ownership::{Ownership, check_access, check_device_range};

pub const DMA_PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmaMemoryError {
    OutOfFrames,
    DirectMapUnavailable,
    AddressOverflow,
    WrongOwner,
    DeviceRangeRejected,
}

/// One kernel-owned 4 KiB frame suitable for a device DMA buffer.
///
/// The frame remains exclusively owned by this value and is returned to the
/// physical allocator on Drop (unless the device still owns it, see the module
/// invariant). The virtual address is the bootloader direct mapping of the same
/// physical frame, so CPU and device refer to one backing allocation.
pub struct DmaPage {
    frame: PhysFrame<Size4KiB>,
    physical_address: u64,
    virtual_address: u64,
    ownership: Ownership,
}

impl DmaPage {
    pub fn allocate_zeroed() -> Result<Self, DmaMemoryError> {
        let frame = memory::allocate_frame().ok_or(DmaMemoryError::OutOfFrames)?;
        let physical_address = frame.start_address().as_u64();
        let Some(offset) = paging::physical_memory_offset() else {
            let _ = memory::deallocate_frame(frame);
            return Err(DmaMemoryError::DirectMapUnavailable);
        };
        let Some(virtual_address) = offset.checked_add(physical_address) else {
            let _ = memory::deallocate_frame(frame);
            return Err(DmaMemoryError::AddressOverflow);
        };

        // SAFETY: `frame` is exclusively owned by this DmaPage and the
        // bootloader direct map makes the complete 4 KiB frame writable at
        // `virtual_address`.
        unsafe {
            core::ptr::write_bytes(virtual_address as *mut u8, 0, DMA_PAGE_SIZE);
        }
        Ok(Self {
            frame,
            physical_address,
            virtual_address,
            ownership: Ownership::new_cpu(),
        })
    }

    pub const fn physical_address(&self) -> u64 {
        self.physical_address
    }
    pub const fn virtual_address(&self) -> u64 {
        self.virtual_address
    }
    pub const fn len(&self) -> usize {
        DMA_PAGE_SIZE
    }
    pub const fn is_empty(&self) -> bool {
        false
    }
    pub const fn owner(&self) -> Owner {
        self.ownership.owner()
    }

    /// Give the page to the device. Returns the device-visible physical address.
    ///
    /// The device-addressability `limit` is exclusive. The range check runs
    /// before ownership changes, so a rejected handoff leaves the page with the
    /// CPU. CPU writes are published before the transition.
    pub fn hand_to_device(&mut self, device_limit: u64) -> Result<u64, DmaMemoryError> {
        if self.ownership.require(Owner::Cpu).is_err() {
            return Err(DmaMemoryError::WrongOwner);
        }
        check_device_range(self.physical_address, DMA_PAGE_SIZE, device_limit)
            .map_err(|_| DmaMemoryError::DeviceRangeRejected)?;
        self.ownership
            .hand_to_device()
            .map_err(|_| DmaMemoryError::WrongOwner)?;
        dma_publish();
        Ok(self.physical_address)
    }

    /// Take the page back after the device has completed. Device writes are
    /// observed before the CPU reads them.
    pub fn reclaim(&mut self) -> Result<(), DmaMemoryError> {
        self.ownership
            .reclaim()
            .map_err(|_| DmaMemoryError::WrongOwner)?;
        dma_consume();
        Ok(())
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) -> Result<(), DmaMemoryError> {
        self.check_cpu_word(offset)?;
        // SAFETY: check_cpu_word validated the 4-byte aligned in-page offset and
        // this DmaPage is CPU-owned, so the device does not write the word.
        unsafe {
            ((self.virtual_address + offset as u64) as *mut u32).write_volatile(value);
        }
        Ok(())
    }

    pub fn read_u32(&self, offset: usize) -> Result<u32, DmaMemoryError> {
        self.check_cpu_word(offset)?;
        // SAFETY: check_cpu_word validated the offset and CPU ownership; the
        // direct mapping remains live while self owns the physical frame.
        Ok(unsafe { ((self.virtual_address + offset as u64) as *const u32).read_volatile() })
    }

    fn check_cpu_word(&self, offset: usize) -> Result<(), DmaMemoryError> {
        check_access(DMA_PAGE_SIZE, offset, 4, 4).map_err(|_| DmaMemoryError::AddressOverflow)?;
        self.ownership
            .require(Owner::Cpu)
            .map_err(|_| DmaMemoryError::WrongOwner)
    }
}

impl Drop for DmaPage {
    fn drop(&mut self) {
        if self.ownership.owner() == Owner::Device {
            serial::write_line(format_args!(
                "[DMA] quarantined device-owned frame {:#x}",
                self.physical_address
            ));
            return;
        }
        // The allocator rejects duplicate returns; ownership of `frame` is
        // confined to this value, so a normal Drop returns it exactly once.
        let _ = memory::deallocate_frame(self.frame);
    }
}

/// Publish CPU writes before handing DMA ownership to a device.
pub fn dma_publish() {
    fence(Ordering::Release);
}

/// Observe device DMA writes before the CPU consumes a completed descriptor.
pub fn dma_consume() {
    fence(Ordering::Acquire);
}

#[cfg(feature = "m5-dma-test")]
pub fn self_test() -> bool {
    const DEVICE_LIMIT_32: u64 = 0x1_0000_0000;

    let Ok(mut page) = DmaPage::allocate_zeroed() else {
        return false;
    };
    if page.is_empty()
        || page.len() != DMA_PAGE_SIZE
        || page.virtual_address() == 0
        || page.owner() != Owner::Cpu
        || page.read_u32(DMA_PAGE_SIZE - 4) != Ok(0)
    {
        return false;
    }
    if page.write_u32(DMA_PAGE_SIZE - 4, 0xa5a5_5a5a) != Ok(())
        || page.read_u32(DMA_PAGE_SIZE - 4) != Ok(0xa5a5_5a5a)
    {
        return false;
    }
    if page.write_u32(DMA_PAGE_SIZE, 1) != Err(DmaMemoryError::AddressOverflow)
        || page.write_u32(2, 1) != Err(DmaMemoryError::AddressOverflow)
    {
        return false;
    }
    if page.hand_to_device(0).is_ok() {
        return false;
    }
    let Ok(device_address) = page.hand_to_device(DEVICE_LIMIT_32) else {
        return false;
    };
    if device_address != page.physical_address() || page.owner() != Owner::Device {
        return false;
    }
    if page.write_u32(0, 7) != Err(DmaMemoryError::WrongOwner)
        || page.read_u32(0) != Err(DmaMemoryError::WrongOwner)
        || page.hand_to_device(DEVICE_LIMIT_32) != Err(DmaMemoryError::WrongOwner)
    {
        return false;
    }
    if page.reclaim().is_err() || page.owner() != Owner::Cpu {
        return false;
    }
    if page.reclaim() != Err(DmaMemoryError::WrongOwner) {
        return false;
    }
    if page.write_u32(0, 0x1234_5678).is_err() || page.read_u32(0) != Ok(0x1234_5678) {
        return false;
    }
    serial::write_line(format_args!("[M5 DMA] shared DMA ownership: PASSED"));
    true
}
