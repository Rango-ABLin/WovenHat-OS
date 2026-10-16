//! Bounds of the Ring-3 address space, checked before any user copy.
//!
//! A syscall pointer is untrusted. Before the kernel walks page tables for
//! it, the whole range must lie in the canonical lower half. This rejects
//! kernel-half addresses outright, rather than relying on leaf page flags
//! alone, and keeps non-canonical addresses away from
//! `x86_64::VirtAddr::new`, which panics on them.

/// Exclusive upper bound of user virtual addresses: the first non-canonical
/// address above the lower half with 4-level paging.
pub const USER_ADDRESS_LIMIT: u64 = 0x0000_8000_0000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    /// `start + length` overflows `u64`.
    Overflow,
    /// Part of the range lies at or above `USER_ADDRESS_LIMIT`.
    OutsideUser,
}

/// Accept `[start, start + length)` only when it lies entirely in user space.
/// An empty range touches no memory and is always accepted, as before.
pub fn check_user_range(start: u64, length: usize) -> Result<(), RangeError> {
    if length == 0 {
        return Ok(());
    }
    let length = u64::try_from(length).map_err(|_| RangeError::Overflow)?;
    let end = start.checked_add(length).ok_or(RangeError::Overflow)?;
    if end > USER_ADDRESS_LIMIT {
        return Err(RangeError::OutsideUser);
    }
    Ok(())
}
