#[allow(dead_code)]
#[path = "../kernel/src/hal/pci/bridge.rs"]
mod bridge;
#[path = "../kernel/src/hal/pci/routing.rs"]
mod routing;

use bridge::Window;
use routing::{Bridge, Demand, Error, Plan};

#[test]
fn nested_bridge_propagates_child_demand_to_parent() {
    let mut plan = Plan::new();
    plan.add_bridge(Bridge { id: 1, parent: None }).unwrap();
    plan.add_bridge(Bridge { id: 2, parent: Some(1) }).unwrap();
    plan.add_demand(2, Demand {
        io: Some(Window { base: 0x2800, size: 0x800 }),
        memory: Some(Window { base: 0x8123_4000, size: 0x2000 }),
        prefetch: Some(Window { base: 0x2_1234_5000, size: 0x3000 }),
    }).unwrap();
    plan.solve().unwrap();

    let child = plan.windows(2).unwrap();
    let parent = plan.windows(1).unwrap();
    assert_eq!(child, parent);
    assert_eq!(child.io, Some(Window { base: 0x2000, size: 0x1000 }));
    assert_eq!(child.memory, Some(Window { base: 0x8120_0000, size: 0x10_0000 }));
    assert_eq!(child.prefetch, Some(Window { base: 0x2_1230_0000, size: 0x10_0000 }));
    assert_eq!(plan.depth(1), Ok(0));
    assert_eq!(plan.depth(2), Ok(1));
}

#[test]
fn siblings_expand_parent_without_expanding_each_other() {
    let mut plan = Plan::new();
    plan.add_bridge(Bridge { id: 1, parent: None }).unwrap();
    plan.add_bridge(Bridge { id: 2, parent: Some(1) }).unwrap();
    plan.add_bridge(Bridge { id: 3, parent: Some(1) }).unwrap();
    plan.add_demand(2, Demand { memory: Some(Window { base: 0x8000_0000, size: 0x10_0000 }), ..Demand::default() }).unwrap();
    plan.add_demand(3, Demand { memory: Some(Window { base: 0x8040_0000, size: 0x10_0000 }), ..Demand::default() }).unwrap();
    plan.solve().unwrap();
    assert_eq!(plan.windows(2).unwrap().memory.unwrap().size, 0x10_0000);
    assert_eq!(plan.windows(3).unwrap().memory.unwrap().size, 0x10_0000);
    assert_eq!(plan.windows(1).unwrap().memory, Some(Window { base: 0x8000_0000, size: 0x50_0000 }));
}

#[test]
fn rejects_cycles_and_missing_parents() {
    let mut cycle = Plan::new();
    cycle.add_bridge(Bridge { id: 1, parent: Some(2) }).unwrap();
    cycle.add_bridge(Bridge { id: 2, parent: Some(1) }).unwrap();
    assert_eq!(cycle.solve(), Err(Error::InvalidParent));

    let mut missing = Plan::new();
    missing.add_bridge(Bridge { id: 1, parent: Some(9) }).unwrap();
    assert_eq!(missing.solve(), Err(Error::InvalidParent));
}
