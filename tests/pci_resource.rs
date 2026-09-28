#[path = "../kernel/src/hal/pci/resource.rs"]
mod resource;

use resource::{Allocator, Error, Transaction};

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
fn rollback_releases_every_partial_reservation() {
    let mut allocator = Allocator::new(0x8000_0000, 0x4000);
    {
        let mut tx = Transaction::new(&mut allocator);
        tx.reserve(0x1000, 0x1000).unwrap();
        tx.reserve(0x2000, 0x2000).unwrap();
        assert_eq!(tx.reserve(0x4000, 0x4000), Err(Error::Exhausted));
    }
    assert_eq!(allocator.active(), 0);
    assert_eq!(allocator.reserve(0x4000, 0x4000).unwrap().range.base, 0x8000_0000);
}

#[test]
fn committed_transaction_keeps_reservations_live() {
    let mut allocator = Allocator::new(0x1_0000_0000, 0x10000);
    let reservations = {
        let mut tx = Transaction::new(&mut allocator);
        tx.reserve(0x1000, 0x1000).unwrap();
        tx.reserve(0x2000, 0x2000).unwrap();
        tx.commit()
    };
    assert_eq!(allocator.active(), 2);
    assert!(allocator.contains(reservations[0].unwrap()));
    assert!(allocator.contains(reservations[1].unwrap()));
}

#[test]
fn stale_reservation_cannot_release_reused_slot() {
    let mut allocator = Allocator::new(0x1000, 0x8000);
    let stale = allocator.reserve(0x1000, 0x1000).unwrap();
    allocator.release(stale).unwrap();
    let current = allocator.reserve(0x1000, 0x1000).unwrap();
    assert_ne!(stale, current);
    assert_eq!(allocator.release(stale), Err(Error::InvalidReservation));
    assert!(allocator.contains(current));
}

#[test]
fn invalid_alignment_and_exhaustion_fail_closed() {
    let mut allocator = Allocator::new(0x1000, 0x3000);
    assert_eq!(allocator.reserve(0x1000, 3), Err(Error::InvalidAlignment));
    allocator.reserve(0x2000, 0x1000).unwrap();
    assert_eq!(allocator.reserve(0x2000, 0x1000), Err(Error::Exhausted));
}
