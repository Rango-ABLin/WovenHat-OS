#[path = "../kernel/src/hal/pci/vector.rs"]
mod vector;

#[test]
fn allocates_only_the_reserved_pci_device_band() {
    let mut allocator = vector::Allocator::new();
    let first = allocator.allocate(7).unwrap();
    assert_eq!(first.vector, vector::FIRST_VECTOR);
    assert!(first.vector >= 0x20);
    assert_ne!(first.vector, 0x80);
    assert!(first.vector < 0xd0);
}

#[test]
fn release_invalidates_stale_generation_before_reuse() {
    let mut allocator = vector::Allocator::new();
    let old = allocator.allocate(11).unwrap();
    allocator.release(old, 11).unwrap();
    assert_eq!(allocator.validate(old, 11), Err(vector::Error::InvalidLease));
    let replacement = allocator.allocate(22).unwrap();
    assert_eq!(replacement.vector, old.vector);
    assert_eq!(allocator.validate(old, 11), Err(vector::Error::InvalidLease));
    assert_eq!(allocator.validate(replacement, 22), Ok(()));
}

#[test]
fn ownership_and_exhaustion_are_enforced() {
    let mut allocator = vector::Allocator::new();
    assert_eq!(allocator.allocate(0), Err(vector::Error::InvalidOwner));
    let lease = allocator.allocate(1).unwrap();
    assert_eq!(allocator.release(lease, 2), Err(vector::Error::NotOwner));
    for owner in 2..=vector::VECTOR_COUNT as u32 {
        allocator.allocate(owner).unwrap();
    }
    assert_eq!(allocator.allocate(999), Err(vector::Error::Exhausted));
}
