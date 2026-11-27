//! DMA buffer ownership and bounds rules.
//!
//! This module is pure logic with no memory or device access, so the host
//! tests in `tests/dma_ownership.rs` include it directly. A DMA buffer is owned
//! by exactly one side at a time, the CPU or the device. Transitions are explicit
//! and rejected when the current owner does not match. Every offset, length and
//! device address is checked with checked arithmetic before it is used.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Cpu,
    Device,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    WrongOwner { expected: Owner, actual: Owner },
    OutOfBounds,
    Misaligned,
    EmptyRange,
    AddressOverflow,
    AddressOutOfRange,
}

/// Single-owner state of one DMA buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ownership {
    owner: Owner,
}

impl Ownership {
    pub const fn new_cpu() -> Self {
        Self { owner: Owner::Cpu }
    }

    pub const fn owner(&self) -> Owner {
        self.owner
    }

    pub fn require(&self, expected: Owner) -> Result<(), Error> {
        if self.owner == expected {
            Ok(())
        } else {
            Err(Error::WrongOwner {
                expected,
                actual: self.owner,
            })
        }
    }

    /// Give the buffer to the device. Only the CPU may do this.
    pub fn hand_to_device(&mut self) -> Result<(), Error> {
        self.require(Owner::Cpu)?;
        self.owner = Owner::Device;
        Ok(())
    }

    /// Take the buffer back from the device. Only the device may give it up.
    pub fn reclaim(&mut self) -> Result<(), Error> {
        self.require(Owner::Device)?;
        self.owner = Owner::Cpu;
        Ok(())
    }
}

/// Validate a `width`-byte access at `offset` inside a `capacity`-byte buffer.
///
/// `align` must be a non-zero power of two. The end of the access is computed
/// with checked addition, so an overflowing offset is rejected as out of bounds.
pub fn check_access(
    capacity: usize,
    offset: usize,
    width: usize,
    align: usize,
) -> Result<(), Error> {
    if align == 0 || !align.is_power_of_two() {
        return Err(Error::Misaligned);
    }
    if width == 0 {
        return Err(Error::EmptyRange);
    }
    if offset & (align - 1) != 0 {
        return Err(Error::Misaligned);
    }
    let end = offset.checked_add(width).ok_or(Error::OutOfBounds)?;
    if end > capacity {
        return Err(Error::OutOfBounds);
    }
    Ok(())
}

/// Check that the device-visible range `[physical, physical + length)` lies
/// entirely below the exclusive device-addressability `limit`. Returns the
/// exclusive end address on success.
pub fn check_device_range(physical: u64, length: usize, limit: u64) -> Result<u64, Error> {
    if length == 0 {
        return Err(Error::EmptyRange);
    }
    let length = u64::try_from(length).map_err(|_| Error::AddressOverflow)?;
    let end = physical
        .checked_add(length)
        .ok_or(Error::AddressOverflow)?;
    if end > limit {
        return Err(Error::AddressOutOfRange);
    }
    Ok(end)
}
