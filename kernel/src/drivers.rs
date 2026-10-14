//! Boot-time driver bring-up (Stage 13.12).
//!
//! Every boot registers the storage, USB and audio drivers with the WovenDriver
//! manager, matches them against the discovered PCI functions, binds the first
//! function each driver accepts (claiming it and its MMIO BARs for the driver's
//! owner token), and only then initializes the controller at that address. A
//! driver whose controller fails to initialize is unbound again, so a failed
//! device never keeps PCI ownership. A missing device or failed driver is
//! reported and never stops the boot.
//!
//! One function per driver: a second controller of the same class stays
//! unbound.
use crate::{
    device::DeviceKind,
    driver::{self, PciMatch},
    hal::pci,
};

/// Owner token for boot-bound drivers ("WD" for WovenDriver, then 13.12).
const OWNER: u32 = 0x5744_0d0c;

/// What happened to one driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No PCI function matched.
    Absent,
    /// Bound and initialized; the value is driver-specific (sectors for
    /// storage, port count for USB, codec address for audio).
    Ready(u64),
    /// Bound by an earlier bring-up; left as it is.
    AlreadyBound,
    /// A function matched but binding failed; nothing is owned.
    BindFailed,
    /// Bound, but the controller failed to initialize; the binding was
    /// rolled back.
    InitFailed,
}

/// One driver's bring-up result and the function it matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Driver {
    pub name: &'static str,
    pub address: Option<pci::Address>,
    pub outcome: Outcome,
}

/// The drivers bring-up knows, in the order they are tried.
pub const COUNT: usize = 4;

pub struct Report {
    pub drivers: [Driver; COUNT],
}

struct Spec {
    name: &'static str,
    kind: DeviceKind,
    matcher: PciMatch,
    init: fn(pci::Device) -> Result<u64, ()>,
}

const fn class(class: u8, subclass: u8, prog_if: Option<u8>) -> PciMatch {
    PciMatch {
        vendor_id: None,
        device_id: None,
        class: Some(class),
        subclass: Some(subclass),
        prog_if,
    }
}

const SPECS: [Spec; COUNT] = [
    Spec {
        name: "nvme0",
        kind: DeviceKind::Block,
        matcher: class(0x01, 0x08, Some(0x02)),
        init: |device| crate::nvme::init_at(device).map_err(|_| ()),
    },
    Spec {
        name: "ahci0",
        kind: DeviceKind::Block,
        matcher: class(0x01, 0x06, Some(0x01)),
        init: |device| crate::ahci::init_at(device).map_err(|_| ()),
    },
    Spec {
        name: "xhci0",
        kind: DeviceKind::UsbHost,
        matcher: class(0x0c, 0x03, Some(0x30)),
        init: |device| {
            let (_, ports, _, _) = crate::xhci::init_at(device).map_err(|_| ())?;
            // Keyboards and mice into WovenInput. Input is optional: a
            // controller whose input cannot start still serves the bus.
            match crate::xhci::start_input("xhci0") {
                Ok(input) => crate::serial::write_line(format_args!(
                    "[DRIVERS] xhci0 input: keyboards={} mice={} vector={:#x}",
                    input.keyboards, input.mice, input.vector
                )),
                Err(error) => {
                    crate::serial::write_line(format_args!("[DRIVERS] xhci0 input unavailable: {:?}", error))
                }
            }
            Ok(u64::from(ports))
        },
    },
    Spec {
        name: "hda0",
        kind: DeviceKind::Audio,
        matcher: class(0x04, 0x03, None),
        init: |device| {
            crate::hda::init_at(device)
                .map(|summary| u64::from(summary.output_streams))
                .map_err(|_| ())
        },
    },
];

fn address_of(device: &pci::Device) -> pci::Address {
    pci::Address {
        segment: device.segment,
        bus: device.bus,
        device: device.device,
        function: device.function,
    }
}

/// The discovered function at `address`, read again after binding: admission
/// may have moved its BARs.
fn device_at(address: pci::Address) -> Option<pci::Device> {
    (0..pci::topology::MAX_FUNCTIONS)
        .filter_map(pci::device)
        .find(|device| address_of(device) == address)
}

fn bring_up_one(spec: &Spec) -> Driver {
    let absent = Driver {
        name: spec.name,
        address: None,
        outcome: Outcome::Absent,
    };
    // Registration is idempotent across calls: a name already registered
    // keeps its matcher.
    let _ = driver::register_pci(spec.name, spec.kind, spec.matcher);
    if driver::pci_binding(spec.name).is_some() {
        return Driver {
            address: driver::pci_binding(spec.name),
            outcome: Outcome::AlreadyBound,
            ..absent
        };
    }
    let Some(address) = (0..pci::topology::MAX_FUNCTIONS)
        .filter_map(pci::device)
        .map(|device| address_of(&device))
        .find(|address| driver::matching_pci_driver(*address) == Some(spec.name))
    else {
        return absent;
    };
    let mut result = Driver {
        address: Some(address),
        ..absent
    };
    if driver::bind_pci(spec.name, address, OWNER).is_err() {
        result.outcome = Outcome::BindFailed;
        return result;
    }
    result.outcome = match device_at(address).ok_or(()).and_then(spec.init) {
        Ok(value) => {
            // Listed with the other devices; a name already present (an
            // earlier bring-up) is left as it is.
            let _ = crate::device::register(crate::device::Device {
                name: spec.name,
                kind: spec.kind,
                irq: None,
            });
            Outcome::Ready(value)
        }
        Err(()) => {
            // The controller is quiesced by its driver's own failure path;
            // give the function back.
            let _ = driver::unbind_pci_resources(spec.name);
            Outcome::InitFailed
        }
    };
    result
}

/// Bind and initialize every known driver once. Call after PCI discovery.
pub fn bring_up() -> Report {
    let mut report = Report {
        drivers: [Driver {
            name: "",
            address: None,
            outcome: Outcome::Absent,
        }; COUNT],
    };
    for (slot, spec) in report.drivers.iter_mut().zip(SPECS.iter()) {
        *slot = bring_up_one(spec);
        log(slot);
    }
    report
}

fn log(driver: &Driver) {
    match (driver.address, driver.outcome) {
        (None, _) => crate::serial::write_line(format_args!("[DRIVERS] {}: absent", driver.name)),
        (Some(at), outcome) => crate::serial::write_line(format_args!(
            "[DRIVERS] {} at {:04x}:{:02x}:{:02x}.{}: {:?}",
            driver.name, at.segment, at.bus, at.device, at.function, outcome
        )),
    }
}

/// Stage 13.12 acceptance: bring-up, checked against the driver manager and
/// the device registry rather than against bring-up's own report alone.
#[cfg(feature = "stage13-12-test")]
pub fn self_test() -> bool {
    use crate::block::BlockDevice;

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S13.12] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    /// Write a pattern to the last sector, read it back, restore it.
    fn round_trip(disk: &mut dyn BlockDevice) -> bool {
        let lba = disk.sector_count().saturating_sub(1);
        let mut original = [0u8; crate::block::SECTOR_SIZE];
        let mut pattern = [0u8; crate::block::SECTOR_SIZE];
        for (index, byte) in pattern.iter_mut().enumerate() {
            *byte = (index as u8) ^ 0x5a;
        }
        let mut back = [0u8; crate::block::SECTOR_SIZE];
        disk.read_sector(lba, &mut original).is_ok()
            && disk.write_sector(lba, &pattern).is_ok()
            && disk.flush().is_ok()
            && disk.read_sector(lba, &mut back).is_ok()
            && back == pattern
            && disk.write_sector(lba, &original).is_ok()
            && disk.flush().is_ok()
    }

    let first = bring_up();
    let mut ok = true;
    for (driver, spec) in first.drivers.iter().zip(SPECS.iter()) {
        let registered = crate::device::find(spec.name);
        let consistent = match driver.outcome {
            Outcome::Ready(_) => {
                let class_ok = driver
                    .address
                    .and_then(device_at)
                    .is_some_and(|device| spec.matcher.matches(device));
                class_ok
                    && driver::pci_binding(spec.name) == driver.address
                    && driver::pci_bar_leases(spec.name) >= 1
                    && registered.is_some_and(|device| device.kind == spec.kind)
            }
            // Nothing owned and nothing listed.
            Outcome::Absent | Outcome::InitFailed | Outcome::BindFailed => {
                driver::pci_binding(spec.name).is_none() && registered.is_none()
            }
            Outcome::AlreadyBound => false,
        };
        let label = alloc::format!("{} {:?} matches the driver manager and registry", spec.name, driver.outcome);
        ok &= step(&label, consistent);
    }
    if first.drivers[0].outcome != Outcome::Absent {
        let io = matches!(first.drivers[0].outcome, Outcome::Ready(_))
            && crate::nvme::with_controller(|disk| round_trip(disk)) == Some(true);
        ok &= step("nvme0 block I/O through the bound controller", io);
    }
    if matches!(first.drivers[1].outcome, Outcome::Ready(_)) {
        let io = crate::ahci::with_controller(|disk| round_trip(disk)) == Some(true);
        ok &= step("ahci0 block I/O through the bound controller", io);
    }
    // A second bring-up claims nothing twice.
    let second = bring_up();
    let idempotent = first.drivers.iter().zip(second.drivers.iter()).all(|(a, b)| match a.outcome {
        Outcome::Ready(_) => b.outcome == Outcome::AlreadyBound && b.address == a.address,
        other => b.outcome == other,
    });
    ok &= step("a second bring-up binds nothing twice", idempotent);
    ok
}
