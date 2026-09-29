#[allow(dead_code)]
#[path = "../kernel/src/hal/pci/bridge.rs"]
mod bridge;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Address { pub segment: u16, pub bus: u8, pub device: u8, pub function: u8 }
#[path = "../kernel/src/hal/pci/bridge_transaction.rs"]
mod bridge_transaction;

use bridge_transaction::{Chain, Entry, Error, MAX_BRIDGES};

fn entry(bus: u8, depth: u8) -> Entry {
    Entry { address: Address { segment: 0, bus, device: 0, function: 0 }, header_type: 1, depth, windows: bridge::Windows::default() }
}

#[test]
fn sorts_deepest_bridge_first_and_keeps_siblings_stable() {
    let mut chain = Chain::new();
    chain.push(entry(1, 0)).unwrap();
    chain.push(entry(2, 2)).unwrap();
    chain.push(entry(3, 1)).unwrap();
    chain.push(entry(4, 2)).unwrap();
    chain.sort_deepest_first();
    assert_eq!(chain.get(0).unwrap().address.bus, 2);
    assert_eq!(chain.get(1).unwrap().address.bus, 4);
    assert_eq!(chain.get(2).unwrap().address.bus, 3);
    assert_eq!(chain.get(3).unwrap().address.bus, 1);
}

#[test]
fn rejects_duplicate_and_capacity_overflow() {
    let mut duplicate = Chain::new();
    duplicate.push(entry(1, 0)).unwrap();
    assert_eq!(duplicate.push(entry(1, 1)), Err(Error::Duplicate));

    let mut full = Chain::new();
    for index in 0..MAX_BRIDGES {
        full.push(Entry {
            address: Address { segment: index as u16, bus: 0, device: 0, function: 0 },
            header_type: 1,
            depth: 0,
            windows: bridge::Windows::default(),
        }).unwrap();
    }
    assert_eq!(full.len(), MAX_BRIDGES);
    assert_eq!(full.push(Entry { address: Address { segment: u16::MAX, bus: 0, device: 0, function: 0 }, header_type: 1, depth: 0, windows: bridge::Windows::default() }), Err(Error::Capacity));
}
