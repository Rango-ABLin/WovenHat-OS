#[allow(dead_code)]
#[path = "../kernel/src/hal/pci/bar.rs"]
mod bar;
#[allow(dead_code)]
#[path = "../kernel/src/hal/pci/resource.rs"]
mod resource;
#[path = "../kernel/src/hal/pci/assignment.rs"]
mod assignment;

use assignment::{release_all, Apertures, Error, Plan, Request};
use bar::{Kind, Probe};
use resource::Range;

fn probe(kind: Kind, size: u64) -> Probe {
    Probe { kind, size, prefetchable: false, low_flags: if kind == Kind::Io { 1 } else { 0 } }
}

#[test]
fn separate_domains_allocate_independently() {
    let mut apertures = Apertures::new(
        Range { base: 0x1000, size: 0x10000 },
        Range { base: 0x8000_0000, size: 0x1000_0000 },
        Range { base: 0x1_0000_0000, size: 0x1000_0000 },
    );
    let assignments = {
        let mut plan = Plan::new(&mut apertures);
        let io = plan.reserve(Request { index: 0, probe: Some(probe(Kind::Io, 0x100)) }).unwrap().unwrap();
        let m32 = plan.reserve(Request { index: 1, probe: Some(probe(Kind::Memory32, 0x1000)) }).unwrap().unwrap();
        let m64 = plan.reserve(Request {
            index: 2,
            probe: Some(Probe {
                kind: Kind::Memory64,
                size: 0x20_0000,
                prefetchable: true,
                low_flags: 0x0c,
            }),
        }).unwrap().unwrap();
        assert_eq!(io.reservation.range.base, 0x1000);
        assert_eq!(m32.reservation.range.base, 0x8000_0000);
        assert_eq!(m64.reservation.range.base, 0x1_0000_0000);
        plan.commit()
    };
    assert_eq!(assignments.iter().flatten().count(), 3);
    assert_eq!(apertures.io.active(), 1);
    assert_eq!(apertures.mmio32.active(), 1);
    assert_eq!(apertures.mmio64.active(), 1);
}

#[test]
fn failure_rolls_back_every_resource_domain() {
    let mut apertures = Apertures::new(
        Range { base: 0x1000, size: 0x1000 },
        Range { base: 0x8000_0000, size: 0x1000 },
        Range { base: 0x1_0000_0000, size: 0x1000 },
    );
    {
        let mut plan = Plan::new(&mut apertures);
        plan.reserve(Request { index: 0, probe: Some(probe(Kind::Io, 0x100)) }).unwrap();
        plan.reserve(Request { index: 1, probe: Some(probe(Kind::Memory32, 0x1000)) }).unwrap();
        assert!(plan.reserve(Request { index: 2, probe: Some(probe(Kind::Memory64, 0x2000)) }).is_err());
    }
    assert_eq!(apertures.io.active(), 0);
    assert_eq!(apertures.mmio32.active(), 0);
    assert_eq!(apertures.mmio64.active(), 0);
}

#[test]
fn duplicate_and_invalid_bar_indices_fail_closed() {
    let mut apertures = Apertures::new(
        Range { base: 0x1000, size: 0x1000 },
        Range { base: 0x8000_0000, size: 0x10000 },
        Range { base: 0x1_0000_0000, size: 0x10000 },
    );
    let mut plan = Plan::new(&mut apertures);
    plan.reserve(Request { index: 1, probe: Some(probe(Kind::Memory32, 0x1000)) }).unwrap();
    assert_eq!(plan.reserve(Request { index: 1, probe: Some(probe(Kind::Memory32, 0x1000)) }), Err(Error::DuplicateIndex));
    assert_eq!(plan.reserve(Request { index: 6, probe: Some(probe(Kind::Memory32, 0x1000)) }), Err(Error::InvalidIndex));
}

#[test]
fn committed_assignments_can_be_released_after_hardware_failure() {
    let mut apertures = Apertures::new(
        Range { base: 0x1000, size: 0x1000 },
        Range { base: 0x8000_0000, size: 0x10000 },
        Range { base: 0x1_0000_0000, size: 0x10000 },
    );
    let assignments = {
        let mut plan = Plan::new(&mut apertures);
        plan.reserve(Request { index: 0, probe: Some(probe(Kind::Io, 0x100)) }).unwrap();
        plan.reserve(Request { index: 1, probe: Some(probe(Kind::Memory32, 0x1000)) }).unwrap();
        plan.commit()
    };
    assert_eq!(apertures.io.active(), 1);
    assert_eq!(apertures.mmio32.active(), 1);
    release_all(&mut apertures, &assignments).unwrap();
    assert_eq!(apertures.io.active(), 0);
    assert_eq!(apertures.mmio32.active(), 0);
    assert_eq!(apertures.mmio64.active(), 0);
}

#[test]
fn non_prefetchable_64_bit_bar_uses_32_bit_routing_aperture() {
    let mut apertures = Apertures::new(
        Range { base: 0x1000, size: 0x1000 },
        Range { base: 0x9000_0000, size: 0x0100_0000 },
        Range { base: 0x2_0000_0000, size: 0x0100_0000 },
    );
    let assignment = {
        let mut plan = Plan::new(&mut apertures);
        let item = plan.reserve(Request {
            index: 0,
            probe: Some(Probe {
                kind: Kind::Memory64,
                size: 0x20_0000,
                prefetchable: false,
                low_flags: 0x04,
            }),
        }).unwrap().unwrap();
        let _ = plan.commit();
        item
    };
    assert_eq!(assignment.reservation.range.base, 0x9000_0000);
    assert_eq!(apertures.mmio32.active(), 1);
    assert_eq!(apertures.mmio64.active(), 0);
}
