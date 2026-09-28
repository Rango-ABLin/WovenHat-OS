#[path = "../kernel/src/hal/pci/resource.rs"]
mod resource;

use resource::{Allocator, Error};

#[test]
fn aligned_first_fit_reuses_released_space() {
    let mut allocator = Allocator::new(0x1000, 0x10000);
    let a = allocator.reserve(0x1800, 0x1000).unwrap();
    let b = allocator.reserve(0x1000, 0x2000).unwrap();
    assert_eq!(a.range.base, 0x1000);
    assert_eq!(b.range.base, 0x4000);
    allocator.release(a).unwrap();
    let c = allocator.reserve(0x1000, 0x1000).unwrap();
    assert_eq!(c.range.base, 0x1000);
}

#[test]
fn explicit_release_recovers_partial_reservations_after_failure() {
    let mut allocator = Allocator::new(0x8000_0000, 0x4000);
    let first = allocator.reserve(0x1000, 0x1000).unwrap();
    let second = allocator.reserve(0x2000, 0x2000).unwrap();
    assert_eq!(allocator.reserve(0x4000, 0x4000), Err(Error::Exhausted));
    allocator.release(second).unwrap();
    allocator.release(first).unwrap();
    assert_eq!(allocator.active(), 0);
    assert_eq!(allocator.reserve(0x4000, 0x4000).unwrap().range.base, 0x8000_0000);
}

#[test]
fn live_reservations_remain_owned_until_explicit_release() {
    let mut allocator = Allocator::new(0x1_0000_0000, 0x10000);
    let first = allocator.reserve(0x1000, 0x1000).unwrap();
    let second = allocator.reserve(0x2000, 0x2000).unwrap();
    assert_eq!(allocator.active(), 2);
    allocator.release(second).unwrap();
    allocator.release(first).unwrap();
    assert_eq!(allocator.active(), 0);
}

#[test]
fn stale_reservation_cannot_release_reused_slot() {
    let mut allocator = Allocator::new(0x1000, 0x8000);
    let stale = allocator.reserve(0x1000, 0x1000).unwrap();
    allocator.release(stale).unwrap();
    let current = allocator.reserve(0x1000, 0x1000).unwrap();
    assert_ne!(stale, current);
    assert_eq!(allocator.release(stale), Err(Error::InvalidReservation));
    allocator.release(current).unwrap();
}

#[test]
fn invalid_alignment_and_exhaustion_fail_closed() {
    let mut allocator = Allocator::new(0x1000, 0x3000);
    assert_eq!(allocator.reserve(0x1000, 3), Err(Error::InvalidAlignment));
    allocator.reserve(0x2000, 0x1000).unwrap();
    assert_eq!(allocator.reserve(0x2000, 0x1000), Err(Error::Exhausted));
}
