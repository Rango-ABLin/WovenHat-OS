#[allow(dead_code)]
#[path = "../kernel/src/hal/pci/topology.rs"]
mod topology;

use topology::{
    BridgeRoute, Error, FunctionAddress, FunctionDescriptor, Topology, MAX_FUNCTIONS,
};

fn endpoint(bus: u8, device: u8) -> FunctionDescriptor {
    FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus, device, function: 0 },
        bridge: None,
    }
}

#[test]
fn nested_bridge_chooses_narrowest_parent() {
    let mut t = Topology::new();
    let root = t.insert(FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus: 0, device: 1, function: 0 },
        bridge: Some(BridgeRoute { primary: 0, secondary: 1, subordinate: 20 }),
    }).unwrap();
    let nested = t.insert(FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus: 1, device: 0, function: 0 },
        bridge: Some(BridgeRoute { primary: 1, secondary: 4, subordinate: 8 }),
    }).unwrap();
    let child = t.insert(endpoint(5, 0)).unwrap();
    assert_eq!(t.snapshot(nested).unwrap().parent, Some(root));
    assert_eq!(t.snapshot(child).unwrap().parent, Some(nested));
}

#[test]
fn teardown_invalidates_owner_handle_and_mmio_before_reuse() {
    let mut t = Topology::new();
    let h = t.insert(endpoint(2, 0)).unwrap();
    t.claim(h, 41).unwrap();
    let lease = t.lease_mmio(h, 41, 0, 0x8000_0000, 0x4000).unwrap();
    assert!(t.validate_mmio(lease, 41));
    t.teardown(h, 41).unwrap();
    assert!(!t.validate_mmio(lease, 41));
    assert_eq!(t.snapshot(h), Err(Error::InvalidHandle));
    let replacement = t.insert(endpoint(2, 0)).unwrap();
    assert_eq!(replacement.slot, h.slot);
    assert_ne!(replacement.generation, h.generation);
}

#[test]
fn ownership_and_partial_failure_are_bounded() {
    let mut t = Topology::new();
    let h = t.insert(endpoint(2, 0)).unwrap();
    assert_eq!(t.claim(h, 0), Err(Error::InvalidOwner));
    t.claim(h, 7).unwrap();
    assert_eq!(t.claim(h, 8), Err(Error::AlreadyOwned));
    assert_eq!(t.release(h, 8), Err(Error::NotOwner));
    assert_eq!(t.teardown(h, 8), Err(Error::NotOwner));
    assert_eq!(t.snapshot(h).unwrap().owner, Some(7));
}

#[test]
fn capacity_and_duplicate_rejection_preserve_count() {
    let mut t = Topology::new();
    let first = endpoint(1, 0);
    t.insert(first).unwrap();
    assert_eq!(t.insert(first), Err(Error::Duplicate));
    assert_eq!(t.count(), 1);
    for i in 1..MAX_FUNCTIONS {
        let bus = 1 + (i / 31) as u8;
        let device = (i % 31) as u8;
        t.insert(endpoint(bus, device)).unwrap();
    }
    assert_eq!(t.count(), MAX_FUNCTIONS);
    assert_eq!(t.insert(endpoint(10, 1)), Err(Error::Capacity));
    assert_eq!(t.count(), MAX_FUNCTIONS);
}
