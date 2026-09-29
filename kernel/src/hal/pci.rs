use core::{arch::asm, ptr};

use crate::{hal::acpi::McfgAllocation, irq_lock::IrqMutex as Mutex};

pub mod topology;
pub mod vector;
pub mod resource;
pub mod routing;
pub mod bar;
pub mod assignment;
pub mod bridge;
pub mod bridge_transaction;

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;
const MAX_DEVICES: usize = 64;
const MAX_ECAM_REGIONS: usize = crate::hal::acpi::MAX_MCFG_ALLOCATIONS;
const MAX_CAPABILITY_STEPS: usize = 48;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Address {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum BarKind {
    #[default]
    Memory32,
    Memory64,
    Io,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Bar {
    pub valid: bool,
    pub kind: BarKind,
    pub address: u64,
    pub prefetchable: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub power_management: bool,
    pub msi: bool,
    pub msix: bool,
    pub pcie: bool,
    pub malformed: bool,
    pub msi_offset: u16,
    pub msix_offset: u16,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Device {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub revision: u8,
    pub prog_if: u8,
    pub class: u8,
    pub subclass: u8,
    pub header_type: u8,
    pub command: u16,
    pub status: u16,
    pub bars: [Bar; 6],
    pub capabilities: Capabilities,
    pub bridge_buses: Option<topology::BridgeRoute>,
}

#[derive(Clone, Copy, Default)]
pub struct Summary {
    pub discovered: u16,
    pub recorded: u8,
    pub storage: u16,
    pub network: u16,
    pub display: u16,
    pub bridges: u16,
    pub segments: u8,
    pub ecam: bool,
    pub truncated: bool,
}

#[derive(Clone, Copy, Default)]
struct ConfigState {
    ecam: [Option<McfgAllocation>; MAX_ECAM_REGIONS],
    ecam_count: usize,
}

struct Inventory {
    devices: [Option<Device>; MAX_DEVICES],
    summary: Summary,
}

impl Inventory {
    const fn new() -> Self {
        Self {
            devices: [None; MAX_DEVICES],
            summary: Summary {
                discovered: 0,
                recorded: 0,
                storage: 0,
                network: 0,
                display: 0,
                bridges: 0,
                segments: 0,
                ecam: false,
                truncated: false,
            },
        }
    }

    fn record(&mut self, device: Device) {
        self.summary.discovered = self.summary.discovered.saturating_add(1);
        match device.class {
            0x01 => self.summary.storage = self.summary.storage.saturating_add(1),
            0x02 => self.summary.network = self.summary.network.saturating_add(1),
            0x03 => self.summary.display = self.summary.display.saturating_add(1),
            0x06 => self.summary.bridges = self.summary.bridges.saturating_add(1),
            _ => {}
        }
        if let Some(slot) = self.devices.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(device);
            self.summary.recorded = self.summary.recorded.saturating_add(1);
        } else {
            self.summary.truncated = true;
        }
    }
}

/// Serializes the legacy CONFIG_ADDRESS/CONFIG_DATA transaction and PCI config
/// read/modify/write operations across CPUs. Never sleep while this lock is held.
static CONFIG_LOCK: Mutex<()> = Mutex::with_rank((), 10);
// CONFIG_LOCK is the sole owner of both PCI configuration transactions and
// ECAM metadata. Keeping a second rank-10 mutex here would make an ECAM access
// recursively acquire the same lock rank while CONFIG_LOCK is held.
static mut CONFIG: ConfigState = ConfigState {
    ecam: [None; MAX_ECAM_REGIONS],
    ecam_count: 0,
};
struct PublishedState {
    inventory: Inventory,
    topology: topology::Topology,
}

impl PublishedState {
    const fn new() -> Self {
        Self { inventory: Inventory::new(), topology: topology::Topology::new() }
    }
}

// Inventory and topology describe one discovery generation and are therefore
// published under one rank-20 lock. PCI configuration locks (rank 10) are
// never acquired while this lock is held.
static PUBLISHED: Mutex<PublishedState> = Mutex::with_rank(PublishedState::new(), 20);

pub fn configure(allocations: &[McfgAllocation]) {
    let _guard = CONFIG_LOCK.lock();
    let count = core::cmp::min(allocations.len(), MAX_ECAM_REGIONS);
    // SAFETY: CONFIG_LOCK is the sole synchronization authority for CONFIG.
    // Configuration is published before discovery and every later reader also
    // holds CONFIG_LOCK.
    unsafe {
        CONFIG.ecam = [None; MAX_ECAM_REGIONS];
        CONFIG.ecam_count = count;
        let config = core::ptr::addr_of_mut!(CONFIG);
        for (index, allocation) in allocations.iter().copied().take(count).enumerate() {
            // Avoid forming a Rust reference to the mutable static itself.
            // CONFIG_LOCK provides exclusive access while raw-pointer writes
            // initialize the bounded ECAM metadata.
            core::ptr::addr_of_mut!((*config).ecam[index]).write(Some(allocation));
        }
    }
}

pub const MAX_HOST_APERTURES: usize = resource::MAX_APERTURES;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostAperture {
    pub base: u64,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostApertureSet {
    pub ranges: [Option<HostAperture>; MAX_HOST_APERTURES],
    pub count: usize,
}

impl HostApertureSet {
    pub const fn new() -> Self {
        Self { ranges: [None; MAX_HOST_APERTURES], count: 0 }
    }

    pub fn push(&mut self, range: HostAperture) -> Result<(), HostApertureError> {
        if range.size == 0 || range.base.checked_add(range.size).is_none() {
            return Err(HostApertureError::InvalidRange);
        }
        if self.count >= self.ranges.len() {
            return Err(HostApertureError::Capacity);
        }
        let end = range.base.checked_add(range.size).ok_or(HostApertureError::InvalidRange)?;
        for existing in self.ranges[..self.count].iter().flatten() {
            let existing_end = existing.base.checked_add(existing.size)
                .ok_or(HostApertureError::InvalidRange)?;
            if range.base < existing_end && existing.base < end {
                return Err(HostApertureError::Overlap);
            }
        }
        self.ranges[self.count] = Some(range);
        self.count += 1;
        Ok(())
    }

    fn resource_ranges(&self) -> Result<([resource::Range; MAX_HOST_APERTURES], usize), HostApertureError> {
        let mut output = [resource::Range { base: 0, size: 0 }; MAX_HOST_APERTURES];
        for (index, aperture) in self.ranges[..self.count].iter().enumerate() {
            let aperture = aperture.ok_or(HostApertureError::InvalidRange)?;
            output[index] = resource::Range { base: aperture.base, size: aperture.size };
        }
        Ok((output, self.count))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostApertures {
    pub io: HostApertureSet,
    pub memory: HostApertureSet,
    pub prefetch: HostApertureSet,
}

static HOST_APERTURES: Mutex<Option<HostApertures>> = Mutex::with_rank(None, 10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostApertureError {
    Unavailable,
    InvalidRange,
    Capacity,
    Overlap,
    Allocation(resource::Error),
}

/// Publish firmware-authoritative PCI root-bridge apertures. The caller must
/// obtain these from platform resource descriptors (for ACPI systems, _CRS);
/// ECAM/MCFG ranges are configuration space and are intentionally rejected as
/// an implicit source of BAR allocation space.
pub fn configure_host_apertures(apertures: HostApertures) -> Result<(), HostApertureError> {
    fn validate(set: &HostApertureSet) -> Result<(), HostApertureError> {
        if set.count > set.ranges.len() { return Err(HostApertureError::Capacity); }
        let mut checked = HostApertureSet::new();
        for range in set.ranges[..set.count].iter().copied() {
            checked.push(range.ok_or(HostApertureError::InvalidRange)?)?;
        }
        Ok(())
    }
    validate(&apertures.io)?;
    validate(&apertures.memory)?;
    validate(&apertures.prefetch)?;
    *HOST_APERTURES.lock() = Some(apertures);
    Ok(())
}

fn configured_assignment_apertures() -> Result<assignment::Apertures, HostApertureError> {
    let configured = HOST_APERTURES.lock().ok_or(HostApertureError::Unavailable)?;
    let (io, io_count) = configured.io.resource_ranges()?;
    let (memory, memory_count) = configured.memory.resource_ranges()?;
    let (prefetch, prefetch_count) = configured.prefetch.resource_ranges()?;
    assignment::Apertures::from_ranges(
        &io[..io_count],
        &memory[..memory_count],
        &prefetch[..prefetch_count],
    ).map_err(HostApertureError::Allocation)
}

fn scan_inventory() -> Inventory {
    let config = {
        let _guard = CONFIG_LOCK.lock();
        // SAFETY: CONFIG_LOCK serializes CONFIG reads and writes.
        unsafe { CONFIG }
    };
    let mut inventory = Inventory::new();
    inventory.summary.ecam = config.ecam_count != 0;

    if config.ecam_count == 0 {
        scan_reachable_buses(&mut inventory, 0, 0, u8::MAX);
        inventory.summary.segments = 1;
    } else {
        let mut seen_segments = [None; MAX_ECAM_REGIONS];
        let mut seen_count = 0usize;
        for allocation in config.ecam[..config.ecam_count].iter().flatten().copied() {
            scan_reachable_buses(
                &mut inventory,
                allocation.segment_group,
                allocation.start_bus,
                allocation.end_bus,
            );
            if !seen_segments[..seen_count].contains(&Some(allocation.segment_group))
                && seen_count < seen_segments.len()
            {
                seen_segments[seen_count] = Some(allocation.segment_group);
                seen_count += 1;
            }
        }
        inventory.summary.segments = u8::try_from(seen_count).unwrap_or(u8::MAX);
    }

    inventory
}

/// Initial PCI discovery. This is only valid before runtime ownership/leases
/// exist; later refreshes must use the driver-aware hotplug coordinator.
pub fn discover() -> Summary {
    let inventory = scan_inventory();
    let mut topology = topology::Topology::new();
    for device in inventory.devices.iter().flatten() {
        if topology.insert(topology::FunctionDescriptor {
            address: topology::FunctionAddress {
                segment: device.segment, bus: device.bus, device: device.device, function: device.function,
            },
            bridge: device.bridge_buses,
        }).is_err() {
            let mut failed = inventory;
            failed.summary.truncated = true;
            return failed.summary;
        }
    }
    let summary = inventory.summary;
    let mut published = PUBLISHED.lock();
    // Refuse to replace a live topology. Runtime refresh must first coordinate
    // removals through WovenDriver so generations and subordinate authority are
    // not silently invalidated.
    if published.topology.count() != 0 {
        return published.inventory.summary;
    }
    published.topology = topology;
    published.inventory = inventory;
    summary
}

/// Publish a fresh hardware inventory after the caller has already detected
/// and torn down disappeared functions. Existing handles/owners/leases for
/// still-present functions are preserved transactionally.
#[cfg(any(
    feature = "stage13-1-test",
    feature = "stage13-2-test",
    feature = "stage13-3-test",
    feature = "stage13-4-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-8-test",
    feature = "stage13-9-test"
))]
pub(crate) fn reconcile_after_teardown() -> Summary {
    let mut inventory = scan_inventory();
    let mut published = PUBLISHED.lock();
    let mut candidate = published.topology.clone();
    for device in inventory.devices.iter().flatten() {
        if candidate.reconcile(topology::FunctionDescriptor {
            address: topology::FunctionAddress {
                segment: device.segment, bus: device.bus, device: device.device, function: device.function,
            },
            bridge: device.bridge_buses,
        }).is_err() {
            inventory.summary.truncated = true;
            return inventory.summary;
        }
    }
    let summary = inventory.summary;
    published.topology = candidate;
    published.inventory = inventory;
    summary
}

fn scan_reachable_buses(
    inventory: &mut Inventory,
    segment: u16,
    start_bus: u8,
    end_bus: u8,
) {
    // MCFG commonly describes the entire 0..=255 ECAM aperture. Mapping and
    // probing every possible function would touch up to 65,536 4-KiB ECAM
    // pages even when only a handful of buses are reachable. Start at the
    // firmware-described root bus and follow each discovered PCI-to-PCI
    // bridge's secondary bus instead. This is both bounded and topology-aware.
    let mut pending = [0_u8; 256];
    let mut visited = [false; 256];
    let mut head = 0_usize;
    let mut tail = 1_usize;
    pending[0] = start_bus;

    while head < tail {
        let bus = pending[head];
        head += 1;
        if bus < start_bus || bus > end_bus || visited[usize::from(bus)] {
            continue;
        }
        visited[usize::from(bus)] = true;

        for device in 0_u8..32 {
            let address = Address {
                segment,
                bus,
                device,
                function: 0,
            };
            let Some(identity) = read_config(address, 0) else {
                continue;
            };
            if identity as u16 == 0xffff {
                continue;
            }
            let header = read_config(address, 0x0c).unwrap_or(u32::MAX);
            let functions = if ((header >> 16) as u8) & 0x80 != 0 { 8 } else { 1 };
            for function in 0..functions {
                let address = Address { function, ..address };
                let Some(found) = probe(address) else {
                    continue;
                };
                if let Some(route) = found.bridge_buses {
                    let secondary = route.secondary;
                    if secondary >= start_bus
                        && secondary <= end_bus
                        && !visited[usize::from(secondary)]
                        && !pending[head..tail].contains(&secondary)
                    {
                        if tail < pending.len() {
                            pending[tail] = secondary;
                            tail += 1;
                        } else {
                            inventory.summary.truncated = true;
                        }
                    }
                }
                inventory.record(found);
            }
        }
    }
}

pub fn device(index: usize) -> Option<Device> {
    PUBLISHED.lock().inventory.devices.get(index).copied().flatten()
}

#[cfg(any(
    feature = "stage13-1-test",
    feature = "stage13-2-test",
    feature = "stage13-3-test",
    feature = "stage13-4-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-8-test",
    feature = "stage13-9-test"
))]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct RescanRemoval {
    pub function: topology::FunctionHandle,
    pub address: Address,
}

/// Detect functions that disappeared from configuration space without
/// publishing a replacement inventory/topology. Callers must teardown each
/// returned owner first; only then may a later discover() publish fresh state.
#[cfg(any(
    feature = "stage13-1-test",
    feature = "stage13-2-test",
    feature = "stage13-3-test",
    feature = "stage13-4-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-8-test",
    feature = "stage13-9-test"
))]
pub fn rescan_removed(
    removed: &mut [Option<RescanRemoval>; MAX_DEVICES],
) -> usize {
    *removed = [None; MAX_DEVICES];

    // Snapshot topology identity while holding rank 20, then release it before
    // any configuration-space access (rank 10). This preserves global lock
    // ordering and prevents a topology -> config inversion during hotplug.
    let mut candidates = [None; topology::MAX_FUNCTIONS];
    {
        let topology = PUBLISHED.lock().topology;
        for (slot, candidate) in candidates.iter_mut().enumerate() {
            let Some(function) = topology.handle_at(slot) else { continue; };
            let Ok(snapshot) = topology.snapshot(function) else { continue; };
            *candidate = Some(RescanRemoval {
                function,
                address: Address {
                    segment: snapshot.address.segment,
                    bus: snapshot.address.bus,
                    device: snapshot.address.device,
                    function: snapshot.address.function,
                },
            });
        }
    }

    let mut count = 0usize;
    for candidate in candidates.iter().flatten().copied() {
        let present = read_config(candidate.address, 0)
            .is_some_and(|identity| identity as u16 != 0xffff);
        if !present && count < removed.len() {
            removed[count] = Some(candidate);
            count += 1;
        }
    }
    count
}


#[expect(dead_code)]
pub fn topology_handle(address: Address) -> Option<topology::FunctionHandle> {
    let published = PUBLISHED.lock();
    for slot in 0..topology::MAX_FUNCTIONS {
        let Some(handle) = published.topology.handle_at(slot) else { continue; };
        if published.topology.snapshot(handle).ok().is_some_and(|node| {
            node.address == topology::FunctionAddress {
                segment: address.segment, bus: address.bus, device: address.device, function: address.function,
            }
        }) {
            return Some(handle);
        }
    }
    None
}

#[expect(dead_code)]
pub(crate) fn function_device(
    handle: topology::FunctionHandle,
) -> Option<(topology::NodeSnapshot, Device)> {
    let published = PUBLISHED.lock();
    let snapshot = published.topology.snapshot(handle).ok()?;
    let device = published.inventory.devices.iter().flatten().find(|device| {
        device.segment == snapshot.address.segment && device.bus == snapshot.address.bus
            && device.device == snapshot.address.device && device.function == snapshot.address.function
    }).copied()?;
    Some((snapshot, device))
}

#[expect(dead_code)]
pub fn claim_function(handle: topology::FunctionHandle, owner: u32) -> Result<(), topology::Error> {
    PUBLISHED.lock().topology.claim(handle, owner)
}

#[cfg(any(
    feature = "stage13-1-test",
    feature = "stage13-2-test",
    feature = "stage13-3-test",
    feature = "stage13-4-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-8-test",
    feature = "stage13-9-test"
))]
pub fn topology_owner(handle: topology::FunctionHandle) -> Result<u32, topology::Error> {
    Ok(PUBLISHED.lock().topology.snapshot(handle)?.owner.unwrap_or(topology::NO_OWNER))
}

#[expect(dead_code)]
pub fn release_function(handle: topology::FunctionHandle, owner: u32) -> Result<(), topology::Error> {
    PUBLISHED.lock().topology.release(handle, owner)
}

#[expect(dead_code)]
pub fn lease_bar(
    handle: topology::FunctionHandle,
    owner: u32,
    bar_index: u8,
) -> Result<topology::MmioLease, topology::Error> {
    let (snapshot, device) = {
        let published = PUBLISHED.lock();
        let snapshot = published.topology.snapshot(handle)?;
        let Some(device) = published.inventory.devices.iter().flatten().find(|device| {
            device.segment == snapshot.address.segment && device.bus == snapshot.address.bus
                && device.device == snapshot.address.device && device.function == snapshot.address.function
        }).copied() else { return Err(topology::Error::InvalidHandle); };
        (snapshot, device)
    };
    let Some(bar) = device.bars.get(bar_index as usize).copied() else {
        return Err(topology::Error::InvalidBar);
    };
    if !bar.valid || bar.kind == BarKind::Io || bar.address == 0 {
        return Err(topology::Error::InvalidBar);
    }
    let address = Address {
        segment: snapshot.address.segment,
        bus: snapshot.address.bus,
        device: snapshot.address.device,
        function: snapshot.address.function,
    };
    let probe = probe_bar_size(address, device.header_type, bar_index)
        .map_err(|_| topology::Error::InvalidBar)?;
    PUBLISHED.lock().topology.lease_mmio(handle, owner, bar_index, bar.address, probe.size)
}

#[expect(dead_code)]
pub fn validate_bar_lease(lease: topology::MmioLease, owner: u32) -> bool {
    PUBLISHED.lock().topology.validate_mmio(lease, owner)
}

#[expect(dead_code)]
pub fn release_bar_lease(
    lease: topology::MmioLease,
    owner: u32,
) -> Result<(), topology::Error> {
    PUBLISHED.lock().topology.release_mmio(lease, owner)
}

// Wired by the Stage 13.2 hotplug/unbind increment; keep the lifecycle entry
// point explicit until removal events have a real caller.
#[expect(dead_code)]
pub fn teardown_function(handle: topology::FunctionHandle, owner: u32) -> Result<(), topology::Error> {
    PUBLISHED.lock().topology.teardown(handle, owner)
}

#[allow(dead_code)]
pub fn read_config_dword(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    read_config(
        Address {
            segment: 0,
            bus,
            device,
            function,
        },
        u16::from(offset),
    )
    .unwrap_or(u32::MAX)
}

#[cfg(feature = "stage13-2-test")]
pub fn write_config_dword(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let _ = write_config(
        Address {
            segment: 0,
            bus,
            device,
            function,
        },
        u16::from(offset),
        value,
    );
}
fn read_command_unlocked(address: Address) -> Option<u16> {
    Some((read_config_unlocked(address, 0x04)? & 0xffff) as u16)
}

fn write_command_unlocked(address: Address, command: u16) -> bool {
    // PCI Status occupies the high 16 bits and contains write-one-to-clear bits.
    // Writing zeros there preserves pending status while updating Command only.
    write_config_unlocked(address, 0x04, u32::from(command))
}

pub fn enable_io_bus_master(bus: u8, device: u8, function: u8) {
    let address = Address {
        segment: 0,
        bus,
        device,
        function,
    };
    let _guard = CONFIG_LOCK.lock();
    if let Some(command) = read_command_unlocked(address) {
        // PCI command: bit0 I/O space, bit2 bus master.
        let _ = write_command_unlocked(address, command | 0x0005);
    }
}

/// Enable MMIO decoding and DMA bus mastering for a PCI/PCIe function.
/// Returns false if the configuration transaction cannot be completed.
#[cfg(any(
    feature = "stage13-3-test",
    feature = "stage13-4-test",
    feature = "stage13-5-test",
    feature = "stage13-6-test",
    feature = "stage13-7-test",
    feature = "stage13-8-test",
    feature = "stage13-9-test"
))]
pub fn enable_memory_bus_master(address: Address) -> bool {
    let _guard = CONFIG_LOCK.lock();
    let Some(command) = read_command_unlocked(address) else {
        return false;
    };
    // PCI command: bit1 memory space, bit2 bus master.
    write_command_unlocked(address, command | 0x0006)
}

pub fn bar0_io_base(bus: u8, device: u8, function: u8) -> Option<u16> {
    let bar = read_config(
        Address {
            segment: 0,
            bus,
            device,
            function,
        },
        0x10,
    )?;
    if bar & 1 == 0 {
        return None;
    }
    let base = bar & 0xffff_fffc;
    u16::try_from(base).ok().filter(|base| *base != 0)
}

fn probe(address: Address) -> Option<Device> {
    let identity = read_config(address, 0)?;
    let vendor_id = identity as u16;
    if vendor_id == 0xffff {
        return None;
    }
    let class_revision = read_config(address, 0x08)?;
    let header = read_config(address, 0x0c)?;
    let command_status = read_config(address, 0x04)?;
    let header_type = (header >> 16) as u8;
    Some(Device {
        segment: address.segment,
        bus: address.bus,
        device: address.device,
        function: address.function,
        vendor_id,
        device_id: (identity >> 16) as u16,
        revision: class_revision as u8,
        prog_if: (class_revision >> 8) as u8,
        subclass: (class_revision >> 16) as u8,
        class: (class_revision >> 24) as u8,
        header_type,
        command: command_status as u16,
        status: (command_status >> 16) as u16,
        bars: read_bars(address, header_type),
        capabilities: read_capabilities(address, (command_status >> 16) as u16, header_type),
        bridge_buses: read_bridge_route(address, header_type),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeProgramError {
    NotBridge,
    InvalidWindow(bridge::Error),
    ConfigUnavailable,
    WriteFailed,
    VerifyFailed,
    RestoreFailed,
}

#[derive(Clone, Copy)]
struct BridgeRegisterSnapshot {
    command: u16,
    io_low: u16,
    memory: u32,
    prefetch_low: u32,
    prefetch_base_upper: u32,
    prefetch_limit_upper: u32,
    io_upper: u32,
}

fn bridge_snapshot_unlocked(address: Address) -> Option<BridgeRegisterSnapshot> {
    Some(BridgeRegisterSnapshot {
        command: read_command_unlocked(address)?,
        io_low: (read_config_unlocked(address, 0x1c)? & 0xffff) as u16,
        memory: read_config_unlocked(address, 0x20)?,
        prefetch_low: read_config_unlocked(address, 0x24)?,
        prefetch_base_upper: read_config_unlocked(address, 0x28)?,
        prefetch_limit_upper: read_config_unlocked(address, 0x2c)?,
        io_upper: read_config_unlocked(address, 0x30)?,
    })
}

fn restore_bridge_unlocked(address: Address, saved: BridgeRegisterSnapshot) -> bool {
    // Keep forwarding disabled until every original window has been restored.
    write_command_unlocked(address, saved.command & !0x3)
        && write_config_unlocked(address, 0x1c, u32::from(saved.io_low))
        && write_config_unlocked(address, 0x20, saved.memory)
        && write_config_unlocked(address, 0x24, saved.prefetch_low)
        && write_config_unlocked(address, 0x28, saved.prefetch_base_upper)
        && write_config_unlocked(address, 0x2c, saved.prefetch_limit_upper)
        && write_config_unlocked(address, 0x30, saved.io_upper)
        && write_command_unlocked(address, saved.command)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
pub enum BridgeChainError {
    NotBridge,
    InvalidWindow(bridge::Error),
    ConfigUnavailable,
    WriteFailed,
    VerifyFailed,
    RestoreFailed,
}

#[derive(Clone, Copy)]
#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
struct BridgeChainState {
    entry: bridge_transaction::Entry,
    registers: bridge::Registers,
    saved: BridgeRegisterSnapshot,
}

fn write_bridge_registers_unlocked(address: Address, registers: bridge::Registers) -> bool {
    write_config_unlocked(address, 0x1c, registers.io_low & 0xffff)
        && write_config_unlocked(address, 0x20, registers.memory)
        && write_config_unlocked(address, 0x24, registers.prefetch_low)
        && write_config_unlocked(address, 0x28, registers.prefetch_base_upper)
        && write_config_unlocked(address, 0x2c, registers.prefetch_limit_upper)
        && write_config_unlocked(address, 0x30, registers.io_upper)
}

fn verify_bridge_registers_unlocked(address: Address, registers: bridge::Registers) -> bool {
    read_config_unlocked(address, 0x1c).is_some_and(|value| value & 0xffff == registers.io_low & 0xffff)
        && read_config_unlocked(address, 0x20) == Some(registers.memory)
        && read_config_unlocked(address, 0x24) == Some(registers.prefetch_low)
        && read_config_unlocked(address, 0x28) == Some(registers.prefetch_base_upper)
        && read_config_unlocked(address, 0x2c) == Some(registers.prefetch_limit_upper)
        && read_config_unlocked(address, 0x30) == Some(registers.io_upper)
}

fn bridge_forwarding_command(saved: u16, windows: bridge::Windows) -> u16 {
    let mut command = saved & !0x3;
    if windows.io.is_some() { command |= 0x1; }
    if windows.memory.is_some() || windows.prefetch.is_some() { command |= 0x2; }
    command
}

#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
fn restore_bridge_chain_unlocked(
    states: &[Option<BridgeChainState>; bridge_transaction::MAX_BRIDGES],
    count: usize,
) -> bool {
    let mut restored = true;
    let mut index = count;
    while index > 0 {
        index -= 1;
        if let Some(state) = states[index] {
            restored = restore_bridge_unlocked(state.entry.address, state.saved) && restored;
        }
    }
    restored
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
pub enum BridgePlanError {
    Routing(routing::Error),
    Transaction(bridge_transaction::Error),
    Probe(BarProbeError),
    Program(BridgeChainError),
    Overflow,
}

/// Build bridge forwarding windows from the currently published PCI generation.
/// Existing BAR addresses are treated as demand; host aperture allocation remains
/// a separate policy input and is never invented here.
#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
pub fn build_bridge_chain() -> Result<bridge_transaction::Chain, BridgePlanError> {
    let mut nodes = [None; topology::MAX_FUNCTIONS];
    let devices;
    {
        let published = PUBLISHED.lock();
        devices = published.inventory.devices;
        for (slot, node) in nodes.iter_mut().enumerate() {
            let Some(handle) = published.topology.handle_at(slot) else { continue; };
            *node = published.topology.snapshot(handle).ok();
        }
    }

    let mut plan = routing::Plan::new();
    for node in nodes.iter().flatten().copied().filter(|node| node.bridge.is_some()) {
        plan.add_bridge(routing::Bridge {
            id: node.handle.slot,
            parent: node.parent.map(|parent| parent.slot),
        }).map_err(BridgePlanError::Routing)?;
    }

    // Probe BAR sizes only after dropping PUBLISHED (rank 20), because BAR
    // sizing performs config-space transactions under rank 10.
    for node in nodes.iter().flatten().copied() {
        let Some(parent) = node.parent else { continue; };
        let Some(device) = devices.iter().flatten().find(|device| {
            device.segment == node.address.segment && device.bus == node.address.bus
                && device.device == node.address.device && device.function == node.address.function
        }).copied() else { continue; };

        let mut demand = routing::Demand::default();
        let mut index = 0usize;
        while index < device.bars.len() {
            let bar = device.bars[index];
            if !bar.valid || bar.address == 0 {
                index += 1;
                continue;
            }
            let probe = probe_bar_size(
                Address {
                    segment: device.segment, bus: device.bus,
                    device: device.device, function: device.function,
                },
                device.header_type,
                index as u8,
            ).map_err(BridgePlanError::Probe)?;
            let window = bridge::Window { base: bar.address, size: probe.size };
            match bar.kind {
                BarKind::Io => merge_route_window(&mut demand.io, window, 0x1000)?,
                BarKind::Memory32 | BarKind::Memory64 if bar.prefetchable =>
                    merge_route_window(&mut demand.prefetch, window, 0x10_0000)?,
                BarKind::Memory32 | BarKind::Memory64 =>
                    merge_route_window(&mut demand.memory, window, 0x10_0000)?,
            }
            index += if bar.kind == BarKind::Memory64 { 2 } else { 1 };
        }
        plan.add_demand(parent.slot, demand).map_err(BridgePlanError::Routing)?;
    }

    plan.solve().map_err(BridgePlanError::Routing)?;
    let mut chain = bridge_transaction::Chain::new();
    for node in nodes.iter().flatten().copied().filter(|node| node.bridge.is_some()) {
        let Some(device) = devices.iter().flatten().find(|device| {
            device.segment == node.address.segment && device.bus == node.address.bus
                && device.device == node.address.device && device.function == node.address.function
        }).copied() else { continue; };
        chain.push(bridge_transaction::Entry {
            address: Address {
                segment: node.address.segment, bus: node.address.bus,
                device: node.address.device, function: node.address.function,
            },
            header_type: device.header_type,
            depth: plan.depth(node.handle.slot).map_err(BridgePlanError::Routing)?,
            windows: plan.windows(node.handle.slot).map_err(BridgePlanError::Routing)?,
        }).map_err(BridgePlanError::Transaction)?;
    }
    Ok(chain)
}

/// Plan the complete forwarding hierarchy first, then apply it as one
/// rollback-capable hardware transaction. No bridge register is touched unless
/// topology discovery, BAR probing, demand aggregation, and encoding all pass.
#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
pub fn route_discovered_bridges() -> Result<(), BridgePlanError> {
    let chain = build_bridge_chain()?;
    program_bridge_chain(chain).map_err(BridgePlanError::Program)
}

#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
fn merge_route_window(
    target: &mut Option<bridge::Window>,
    incoming: bridge::Window,
    granularity: u64,
) -> Result<(), BridgePlanError> {
    let incoming_end = incoming.base.checked_add(incoming.size).ok_or(BridgePlanError::Overflow)?;
    let mut base = incoming.base & !(granularity - 1);
    let mut end = incoming_end.checked_add(granularity - 1).ok_or(BridgePlanError::Overflow)?
        & !(granularity - 1);
    if let Some(current) = *target {
        let current_end = current.base.checked_add(current.size).ok_or(BridgePlanError::Overflow)?;
        base = core::cmp::min(base, current.base);
        end = core::cmp::max(end, current_end);
    }
    *target = Some(bridge::Window {
        base,
        size: end.checked_sub(base).ok_or(BridgePlanError::Overflow)?,
    });
    Ok(())
}

#[expect(dead_code, reason = "Stage 13.2 bridge routing becomes live at device admission/hotplug integration")]
pub fn program_bridge_chain(
    mut chain: bridge_transaction::Chain,
) -> Result<(), BridgeChainError> {
    chain.sort_deepest_first();
    let mut states: [Option<BridgeChainState>; bridge_transaction::MAX_BRIDGES] =
        [None; bridge_transaction::MAX_BRIDGES];

    // Encode the complete transaction before acquiring CONFIG_LOCK or touching
    // hardware. Invalid windows therefore cannot leave a partial bridge chain.
    for (index, slot) in states.iter_mut().enumerate().take(chain.len()) {
        let entry = chain.get(index).ok_or(BridgeChainError::ConfigUnavailable)?;
        if entry.header_type & 0x7f != 0x01 { return Err(BridgeChainError::NotBridge); }
        let registers = bridge::encode_windows(entry.windows).map_err(BridgeChainError::InvalidWindow)?;
        *slot = Some(BridgeChainState {
            entry,
            registers,
            saved: BridgeRegisterSnapshot {
                command: 0, io_low: 0, memory: 0, prefetch_low: 0,
                prefetch_base_upper: 0, prefetch_limit_upper: 0, io_upper: 0,
            },
        });
    }

    let _guard = CONFIG_LOCK.lock();

    // Snapshot every target before the first mutation. This is the rollback
    // boundary for the whole bridge chain, not an individual bridge.
    for slot in states.iter_mut().take(chain.len()) {
        let mut state = slot.ok_or(BridgeChainError::ConfigUnavailable)?;
        state.saved = bridge_snapshot_unlocked(state.entry.address)
            .ok_or(BridgeChainError::ConfigUnavailable)?;
        *slot = Some(state);
    }

    // Disable forwarding across the whole target set while routing registers
    // are inconsistent. Preserve bus-master and unrelated command bits.
    for index in 0..chain.len() {
        let state = states[index].ok_or(BridgeChainError::ConfigUnavailable)?;
        if !write_command_unlocked(state.entry.address, state.saved.command & !0x3) {
            return if restore_bridge_chain_unlocked(&states, chain.len()) {
                Err(BridgeChainError::WriteFailed)
            } else {
                Err(BridgeChainError::RestoreFailed)
            };
        }
    }

    // The chain is sorted leaf-to-root. Program and verify each bridge while
    // forwarding remains disabled everywhere.
    for index in 0..chain.len() {
        let state = states[index].ok_or(BridgeChainError::ConfigUnavailable)?;
        if !write_bridge_registers_unlocked(state.entry.address, state.registers) {
            return if restore_bridge_chain_unlocked(&states, chain.len()) {
                Err(BridgeChainError::WriteFailed)
            } else {
                Err(BridgeChainError::RestoreFailed)
            };
        }
        if !verify_bridge_registers_unlocked(state.entry.address, state.registers) {
            return if restore_bridge_chain_unlocked(&states, chain.len()) {
                Err(BridgeChainError::VerifyFailed)
            } else {
                Err(BridgeChainError::RestoreFailed)
            };
        }
    }

    // Re-enable forwarding root-to-leaf so an upstream path exists before a
    // child bridge starts forwarding traffic.
    let mut index = chain.len();
    while index > 0 {
        index -= 1;
        let state = states[index].ok_or(BridgeChainError::ConfigUnavailable)?;
        let command = bridge_forwarding_command(state.saved.command, state.entry.windows);
        if !write_command_unlocked(state.entry.address, command) {
            return if restore_bridge_chain_unlocked(&states, chain.len()) {
                Err(BridgeChainError::WriteFailed)
            } else {
                Err(BridgeChainError::RestoreFailed)
            };
        }
    }
    Ok(())
}

#[expect(dead_code)]
pub fn program_bridge_windows(
    address: Address,
    header_type: u8,
    windows: bridge::Windows,
) -> Result<(), BridgeProgramError> {
    if header_type & 0x7f != 0x01 { return Err(BridgeProgramError::NotBridge); }
    let registers = bridge::encode_windows(windows).map_err(BridgeProgramError::InvalidWindow)?;
    let _guard = CONFIG_LOCK.lock();
    let saved = bridge_snapshot_unlocked(address).ok_or(BridgeProgramError::ConfigUnavailable)?;
    if !write_command_unlocked(address, saved.command & !0x3) { return Err(BridgeProgramError::WriteFailed); }
    if !write_bridge_registers_unlocked(address, registers) {
        return if restore_bridge_unlocked(address, saved) { Err(BridgeProgramError::WriteFailed) } else { Err(BridgeProgramError::RestoreFailed) };
    }
    if !verify_bridge_registers_unlocked(address, registers) {
        return if restore_bridge_unlocked(address, saved) { Err(BridgeProgramError::VerifyFailed) } else { Err(BridgeProgramError::RestoreFailed) };
    }
    if !write_command_unlocked(address, bridge_forwarding_command(saved.command, windows)) {
        return if restore_bridge_unlocked(address, saved) { Err(BridgeProgramError::WriteFailed) } else { Err(BridgeProgramError::RestoreFailed) };
    }
    Ok(())
}

fn read_bridge_route(address: Address, header_type: u8) -> Option<topology::BridgeRoute> {
    if header_type & 0x7f != 0x01 { return None; }
    let buses = read_config(address, 0x18)?;
    let route = topology::BridgeRoute {
        primary: buses as u8,
        secondary: (buses >> 8) as u8,
        subordinate: (buses >> 16) as u8,
    };
    (route.secondary != 0 && route.secondary <= route.subordinate).then_some(route)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarProbeError {
    InvalidIndex,
    ConfigUnavailable,
    Probe(bar::Error),
    RestoreFailed,
}

#[expect(dead_code)]
pub fn probe_bar_size(address: Address, header_type: u8, bar_index: u8) -> Result<bar::Probe, BarProbeError> {
    let count = match header_type & 0x7f {
        0x00 => 6usize,
        0x01 => 2usize,
        _ => 0usize,
    };
    let index = bar_index as usize;
    if index >= count {
        return Err(BarProbeError::InvalidIndex);
    }
    let offset = 0x10 + (index as u16 * 4);
    let _guard = CONFIG_LOCK.lock();
    let command = read_command_unlocked(address).ok_or(BarProbeError::ConfigUnavailable)?;
    let original_low = read_config_unlocked(address, offset).ok_or(BarProbeError::ConfigUnavailable)?;
    if original_low == u32::MAX {
        return Err(BarProbeError::ConfigUnavailable);
    }
    let is_64 = original_low & 1 == 0 && ((original_low >> 1) & 0x3) == 2;
    if is_64 && index + 1 >= count {
        return Err(BarProbeError::InvalidIndex);
    }
    let original_high = if is_64 {
        Some(read_config_unlocked(address, offset + 4).ok_or(BarProbeError::ConfigUnavailable)?)
    } else {
        None
    };

    // Disable I/O and memory decoding while BARs contain the sizing pattern.
    // Bus mastering is preserved because no DMA address is changed here.
    let decode_disabled = command & !0x3;
    if !write_command_unlocked(address, decode_disabled)
        || !write_config_unlocked(address, offset, u32::MAX)
        || (is_64 && !write_config_unlocked(address, offset + 4, u32::MAX))
    {
        let _ = write_config_unlocked(address, offset, original_low);
        if let Some(high) = original_high {
            let _ = write_config_unlocked(address, offset + 4, high);
        }
        let _ = write_command_unlocked(address, command);
        return Err(BarProbeError::RestoreFailed);
    }

    let mask_low = read_config_unlocked(address, offset).ok_or(BarProbeError::ConfigUnavailable);
    let mask_high = if is_64 {
        read_config_unlocked(address, offset + 4).ok_or(BarProbeError::ConfigUnavailable).map(Some)
    } else {
        Ok(None)
    };

    let restored = write_config_unlocked(address, offset, original_low)
        && original_high.is_none_or(|high| write_config_unlocked(address, offset + 4, high))
        && write_command_unlocked(address, command);
    if !restored {
        return Err(BarProbeError::RestoreFailed);
    }
    let probe = bar::decode_probe(original_low, mask_low?, mask_high?).map_err(BarProbeError::Probe)?;
    Ok(probe)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarProgramError {
    InvalidIndex,
    ConfigUnavailable,
    Encode(bar::Error),
    WriteFailed,
    VerifyFailed,
    RestoreFailed,
}

#[expect(dead_code)]
pub fn program_bar(
    address: Address,
    header_type: u8,
    bar_index: u8,
    probe: bar::Probe,
    base: u64,
) -> Result<(), BarProgramError> {
    let count = match header_type & 0x7f {
        0x00 => 6usize,
        0x01 => 2usize,
        _ => 0usize,
    };
    let index = bar_index as usize;
    let is_64 = probe.kind == bar::Kind::Memory64;
    if index >= count || (is_64 && index + 1 >= count) {
        return Err(BarProgramError::InvalidIndex);
    }
    let (encoded_low, encoded_high) = bar::encode(probe, base).map_err(BarProgramError::Encode)?;
    let offset = 0x10 + (index as u16 * 4);
    let _guard = CONFIG_LOCK.lock();
    let command = read_command_unlocked(address).ok_or(BarProgramError::ConfigUnavailable)?;
    let original_low = read_config_unlocked(address, offset).ok_or(BarProgramError::ConfigUnavailable)?;
    let original_high = if is_64 {
        Some(read_config_unlocked(address, offset + 4).ok_or(BarProgramError::ConfigUnavailable)?)
    } else {
        None
    };

    let restore = |low: u32, high: Option<u32>| -> bool {
        write_config_unlocked(address, offset, low)
            && high.is_none_or(|value| write_config_unlocked(address, offset + 4, value))
            && write_command_unlocked(address, command)
    };

    if !write_command_unlocked(address, command & !0x3)
        || !write_config_unlocked(address, offset, encoded_low)
        || encoded_high.is_some_and(|value| !write_config_unlocked(address, offset + 4, value))
    {
        return if restore(original_low, original_high) {
            Err(BarProgramError::WriteFailed)
        } else {
            Err(BarProgramError::RestoreFailed)
        };
    }

    let verified = read_config_unlocked(address, offset) == Some(encoded_low)
        && encoded_high.is_none_or(|value| read_config_unlocked(address, offset + 4) == Some(value));
    if !verified {
        return if restore(original_low, original_high) {
            Err(BarProgramError::VerifyFailed)
        } else {
            Err(BarProgramError::RestoreFailed)
        };
    }
    if !write_command_unlocked(address, command) {
        return if restore(original_low, original_high) {
            Err(BarProgramError::WriteFailed)
        } else {
            Err(BarProgramError::RestoreFailed)
        };
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebalanceError {
    Probe(BarProbeError),
    Allocation(assignment::Error),
    Program(BarProgramError),
    RollbackFailed,
}

#[expect(dead_code, reason = "called by device admission once ACPI _CRS apertures are published")]
pub fn rebalance_bars_from_firmware(
    address: Address,
    header_type: u8,
) -> Result<[Option<assignment::Assignment>; 6], RebalanceError> {
    let mut apertures = configured_assignment_apertures()
        .map_err(|_| RebalanceError::Allocation(assignment::Error::Resource(resource::Error::InvalidRange)))?;
    rebalance_bars(address, header_type, &mut apertures)
}

#[expect(dead_code)]
pub fn rebalance_bars(
    address: Address,
    header_type: u8,
    apertures: &mut assignment::Apertures,
) -> Result<[Option<assignment::Assignment>; 6], RebalanceError> {
    let count = match header_type & 0x7f {
        0x00 => 6usize,
        0x01 => 2usize,
        _ => 0usize,
    };
    let mut probes = [None; 6];
    let mut index = 0usize;
    while index < count {
        let probe = probe_bar_size(address, header_type, index as u8).map_err(RebalanceError::Probe)?;
        probes[index] = Some(probe);
        index += if probe.kind == bar::Kind::Memory64 { 2 } else { 1 };
    }

    let assignments = {
        let mut plan = assignment::Plan::new(apertures);
        let mut slot = 0usize;
        while slot < count {
            if let Some(probe) = probes[slot] {
                plan.reserve(assignment::Request { index: slot as u8, probe: Some(probe) })
                    .map_err(RebalanceError::Allocation)?;
                slot += if probe.kind == bar::Kind::Memory64 { 2 } else { 1 };
            } else {
                slot += 1;
            }
        }
        plan.commit()
    };

    // Encode every assignment before touching hardware. Once config-space
    // programming starts, I/O and memory decoding stay disabled until every BAR
    // has been written and verified.
    let mut encoded = [None; 6];
    for item in assignments.iter().flatten().copied() {
        let probe = probes[item.index as usize].ok_or(RebalanceError::RollbackFailed)?;
        let value = match bar::encode(probe, item.reservation.range.base) {
            Ok(value) => value,
            Err(error) => {
                let _ = assignment::release_all(apertures, &assignments);
                return Err(RebalanceError::Program(BarProgramError::Encode(error)));
            }
        };
        encoded[item.index as usize] = Some(value);
    }

    let programming = {
        let _guard = CONFIG_LOCK.lock();
        let command = read_command_unlocked(address);
        let mut original = [0u32; 6];
        let mut raw = 0usize;
        let mut snapshot_ok = command.is_some();
        while raw < count {
            if let Some(value) = read_config_unlocked(address, 0x10 + raw as u16 * 4) {
                original[raw] = value;
            } else {
                snapshot_ok = false;
                break;
            }
            raw += 1;
        }
        let command = command.unwrap_or(0);

        let restore = || -> bool {
            let mut slot = 0usize;
            let mut ok = true;
            while slot < count {
                ok &= write_config_unlocked(address, 0x10 + slot as u16 * 4, original[slot]);
                slot += 1;
            }
            ok && write_command_unlocked(address, command)
        };

        if !snapshot_ok {
            Err(RebalanceError::Program(BarProgramError::ConfigUnavailable))
        } else if !write_command_unlocked(address, command & !0x3) {
            Err(RebalanceError::Program(BarProgramError::WriteFailed))
        } else {
            let mut write_failed = false;
            for item in assignments.iter().flatten().copied() {
                let offset = 0x10 + u16::from(item.index) * 4;
                let Some((low, high)) = encoded[item.index as usize] else {
                    write_failed = true;
                    break;
                };
                if !write_config_unlocked(address, offset, low)
                    || high.is_some_and(|value| !write_config_unlocked(address, offset + 4, value))
                {
                    write_failed = true;
                    break;
                }
            }
            if write_failed {
                if restore() {
                    Err(RebalanceError::Program(BarProgramError::WriteFailed))
                } else {
                    Err(RebalanceError::RollbackFailed)
                }
            } else {
                let mut verified = true;
                for item in assignments.iter().flatten().copied() {
                    let offset = 0x10 + u16::from(item.index) * 4;
                    let Some((low, high)) = encoded[item.index as usize] else {
                        verified = false;
                        break;
                    };
                    if read_config_unlocked(address, offset) != Some(low)
                        || high.is_some_and(|value| read_config_unlocked(address, offset + 4) != Some(value))
                    {
                        verified = false;
                        break;
                    }
                }
                if !verified {
                    if restore() {
                        Err(RebalanceError::Program(BarProgramError::VerifyFailed))
                    } else {
                        Err(RebalanceError::RollbackFailed)
                    }
                } else if !write_command_unlocked(address, command) {
                    if restore() {
                        Err(RebalanceError::Program(BarProgramError::WriteFailed))
                    } else {
                        Err(RebalanceError::RollbackFailed)
                    }
                } else {
                    Ok(())
                }
            }
        }
    };

    if let Err(error) = programming {
        if assignment::release_all(apertures, &assignments).is_err() {
            return Err(RebalanceError::RollbackFailed);
        }
        return Err(error);
    }
    Ok(assignments)
}

fn read_bars(address: Address, header_type: u8) -> [Bar; 6] {
    let mut bars = [Bar::default(); 6];
    let count = if header_type & 0x7f == 0x00 {
        6
    } else if header_type & 0x7f == 0x01 {
        2
    } else {
        0
    };
    let mut index = 0usize;
    while index < count {
        let offset = 0x10 + (index as u16 * 4);
        let Some(low) = read_config(address, offset) else {
            break;
        };
        if low == 0 || low == u32::MAX {
            index += 1;
            continue;
        }
        if low & 1 != 0 {
            bars[index] = Bar {
                valid: true,
                kind: BarKind::Io,
                address: u64::from(low & !3),
                prefetchable: false,
            };
            index += 1;
            continue;
        }
        let memory_type = (low >> 1) & 3;
        let prefetchable = low & 8 != 0;
        if memory_type == 2 && index + 1 < count {
            if let Some(high) = read_config(address, offset + 4) {
                bars[index] = Bar {
                    valid: true,
                    kind: BarKind::Memory64,
                    address: (u64::from(high) << 32) | u64::from(low & !0xf),
                    prefetchable,
                };
                index += 2;
                continue;
            }
        }
        if memory_type == 0 {
            bars[index] = Bar {
                valid: true,
                kind: BarKind::Memory32,
                address: u64::from(low & !0xf),
                prefetchable,
            };
        }
        index += 1;
    }
    bars
}

fn read_capabilities(address: Address, status: u16, header_type: u8) -> Capabilities {
    let mut capabilities = Capabilities::default();
    if status & (1 << 4) == 0 {
        return capabilities;
    }
    let pointer_register = if header_type & 0x7f == 0x02 {
        0x14
    } else {
        0x34
    };
    let Some(mut pointer) =
        read_config(address, pointer_register).map(|value| (value & 0xfc) as u16)
    else {
        capabilities.malformed = true;
        return capabilities;
    };
    let mut visited = [0_u8; MAX_CAPABILITY_STEPS];
    let mut visited_count = 0usize;
    while pointer != 0 {
        if !(0x40..=0xfc).contains(&pointer)
            || pointer & 3 != 0
            || visited[..visited_count].contains(&(pointer as u8))
        {
            capabilities.malformed = true;
            break;
        }
        if visited_count == visited.len() {
            capabilities.malformed = true;
            break;
        }
        visited[visited_count] = pointer as u8;
        visited_count += 1;
        let Some(value) = read_config(address, pointer) else {
            capabilities.malformed = true;
            break;
        };
        match value as u8 {
            0x01 => capabilities.power_management = true,
            0x05 => {
                capabilities.msi = true;
                capabilities.msi_offset = pointer;
            }
            0x10 => capabilities.pcie = true,
            0x11 => {
                capabilities.msix = true;
                capabilities.msix_offset = pointer;
            }
            _ => {}
        }
        pointer = ((value >> 8) & 0xfc) as u16;
    }
    capabilities
}

// Generic MSI lifecycle is part of Stage 13.2. The existing AX200 path
// continues to use the same programming primitives under its feature gate.
mod msi;
pub mod msix;
pub use msi::*;
fn read_config(address: Address, offset: u16) -> Option<u32> {
    let _guard = CONFIG_LOCK.lock();
    read_config_unlocked(address, offset)
}

#[cfg(feature = "stage13-2-test")]
fn write_config(address: Address, offset: u16, value: u32) -> bool {
    let _guard = CONFIG_LOCK.lock();
    write_config_unlocked(address, offset, value)
}
fn read_config_unlocked(address: Address, offset: u16) -> Option<u32> {
    if offset & 3 != 0 || offset > 0xffc || address.device >= 32 || address.function >= 8 {
        return None;
    }
    if let Some(physical) = ecam_physical(address, offset) {
        let virtual_address = crate::paging::map_mmio(physical).ok()?;
        return Some(unsafe { ptr::read_volatile(virtual_address as *const u32) });
    }
    if address.segment != 0 || offset > 0xfc {
        return None;
    }
    unsafe {
        outl(
            CONFIG_ADDRESS,
            config_address(address.bus, address.device, address.function, offset as u8),
        );
        Some(inl(CONFIG_DATA))
    }
}

fn write_config_unlocked(address: Address, offset: u16, value: u32) -> bool {
    if offset & 3 != 0 || offset > 0xffc || address.device >= 32 || address.function >= 8 {
        return false;
    }
    if let Some(physical) = ecam_physical(address, offset) {
        let Ok(virtual_address) = crate::paging::map_mmio(physical) else {
            return false;
        };
        unsafe { ptr::write_volatile(virtual_address as *mut u32, value) };
        return true;
    }
    if address.segment != 0 || offset > 0xfc {
        return false;
    }
    unsafe {
        outl(
            CONFIG_ADDRESS,
            config_address(address.bus, address.device, address.function, offset as u8),
        );
        outl(CONFIG_DATA, value);
    }
    true
}

fn ecam_physical(address: Address, offset: u16) -> Option<u64> {
    // Caller holds CONFIG_LOCK (all callers are *_config_unlocked helpers).
    // Copy the fixed-capacity state so no reference to mutable static storage
    // escapes this critical section.
    let state = unsafe { CONFIG };
    let allocation = state.ecam[..state.ecam_count]
        .iter()
        .flatten()
        .find(|allocation| {
            allocation.segment_group == address.segment
                && (allocation.start_bus..=allocation.end_bus).contains(&address.bus)
        })?;
    let bus = u64::from(address.bus - allocation.start_bus);
    allocation
        .base_address
        .checked_add(bus << 20)?
        .checked_add(u64::from(address.device) << 15)?
        .checked_add(u64::from(address.function) << 12)?
        .checked_add(u64::from(offset))
}

const fn config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    1 << 31
        | (bus as u32) << 16
        | (device as u32) << 11
        | (function as u32) << 8
        | (offset as u32 & 0xfc)
}

pub fn self_test() -> bool {
    let allocation = McfgAllocation {
        base_address: 0xe000_0000,
        segment_group: 0,
        start_bus: 0x20,
        end_bus: 0x2f,
    };
    let ecam_math = ecam_address_for(
        allocation,
        Address {
            segment: 0,
            bus: 0x21,
            device: 3,
            function: 4,
        },
        0x100,
    ) == Some(0xe000_0000 + (1 << 20) + (3 << 15) + (4 << 12) + 0x100);
    config_address(2, 3, 4, 0x0b) == 0x8002_1c08
        && ecam_math
        && PUBLISHED.lock().inventory.summary.recorded as usize <= MAX_DEVICES
        && device(MAX_DEVICES).is_none()
        && topology_self_test()
        && routing_self_test()
        && bridge_transaction_self_test()
        && vector_self_test()
        && stage13_10aa_msi_self_test()
        && msix::stage13_2_msix_self_test()
}

fn topology_self_test() -> bool {
    use topology::{BridgeRoute, FunctionAddress, FunctionDescriptor, Topology};
    let mut topology = Topology::new();
    let Ok(bridge) = topology.insert(FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus: 0, device: 1, function: 0 },
        bridge: Some(BridgeRoute { primary: 0, secondary: 1, subordinate: 8 }),
    }) else { return false; };
    let Ok(function) = topology.insert(FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus: 2, device: 0, function: 0 },
        bridge: None,
    }) else { return false; };
    if topology.snapshot(function).ok().and_then(|node| node.parent) != Some(bridge)
        || topology.claim(function, 7).is_err() { return false; }
    let Ok(lease) = topology.lease_mmio(function, 7, 0, 0x8000_0000, 0x4000) else { return false; };
    if !topology.validate_mmio(lease, 7) || topology.teardown(function, 7).is_err()
        || topology.validate_mmio(lease, 7) || topology.snapshot(function).is_ok() { return false; }
    let Ok(reused) = topology.insert(FunctionDescriptor {
        address: FunctionAddress { segment: 0, bus: 2, device: 0, function: 0 },
        bridge: None,
    }) else { return false; };
    if reused.slot != function.slot || reused.generation == function.generation {
        return false;
    }

    // Use count/handle_at through a local topology without mutating the
    // published inventory topology during boot validation.
    let _ = topology.count();
    let _ = topology.handle_at(reused.slot as usize);
    true
}


fn routing_self_test() -> bool {
    use bridge::Window;
    use routing::{Bridge, Demand, Plan};

    let mut plan = Plan::new();
    if plan.add_bridge(Bridge { id: 1, parent: None }).is_err()
        || plan.add_bridge(Bridge { id: 2, parent: Some(1) }).is_err()
        || plan.add_demand(
            2,
            Demand {
                io: Some(Window { base: 0x2800, size: 0x800 }),
                memory: Some(Window { base: 0x8123_4000, size: 0x2000 }),
                prefetch: Some(Window { base: 0x2_1234_5000, size: 0x3000 }),
            },
        ).is_err()
        || plan.solve().is_err()
    {
        return false;
    }

    let Ok(child) = plan.windows(2) else { return false; };
    let Ok(parent) = plan.windows(1) else { return false; };
    child == parent
        && child.io == Some(Window { base: 0x2000, size: 0x1000 })
        && child.memory == Some(Window { base: 0x8120_0000, size: 0x10_0000 })
        && child.prefetch == Some(Window { base: 0x2_1230_0000, size: 0x10_0000 })
        && plan.depth(1) == Ok(0)
        && plan.depth(2) == Ok(1)
        && plan.bridge_at(0) == Some(Bridge { id: 1, parent: None })
        && plan.bridge_at(1) == Some(Bridge { id: 2, parent: Some(1) })
        && plan.bridge_at(routing::MAX_ROUTES).is_none()
}


fn bridge_transaction_self_test() -> bool {
    use bridge_transaction::{Chain, Entry};
    let mut chain = Chain::new();
    for (bus, depth) in [(1u8, 0u8), (2, 2), (3, 1)] {
        if chain.push(Entry {
            address: Address { segment: 0, bus, device: 0, function: 0 },
            header_type: 1,
            depth,
            windows: bridge::Windows::default(),
        }).is_err() {
            return false;
        }
    }
    chain.sort_deepest_first();
    chain.len() == 3
        && chain.get(0).is_some_and(|entry| entry.depth == 2)
        && chain.get(1).is_some_and(|entry| entry.depth == 1)
        && chain.get(2).is_some_and(|entry| entry.depth == 0)
}

fn vector_self_test() -> bool {
    let mut allocator = vector::Allocator::new();
    let Ok(first) = allocator.allocate(1) else { return false; };
    if first.vector != vector::FIRST_VECTOR || allocator.validate(first, 1).is_err() {
        return false;
    }
    if allocator.release(first, 1).is_err() || allocator.validate(first, 1).is_ok() {
        return false;
    }
    let Ok(reused) = allocator.allocate(2) else { return false; };
    reused.vector == first.vector && allocator.validate(reused, 2).is_ok()
}

fn ecam_address_for(allocation: McfgAllocation, address: Address, offset: u16) -> Option<u64> {
    if allocation.segment_group != address.segment
        || !(allocation.start_bus..=allocation.end_bus).contains(&address.bus)
        || address.device >= 32
        || address.function >= 8
        || offset > 0xfff
    {
        return None;
    }
    allocation
        .base_address
        .checked_add(u64::from(address.bus - allocation.start_bus) << 20)?
        .checked_add(u64::from(address.device) << 15)?
        .checked_add(u64::from(address.function) << 12)?
        .checked_add(u64::from(offset))
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    unsafe {
        asm!("in eax, dx", in("dx") port, out("eax") value, options(nomem, nostack, preserves_flags));
    }
    value
}

unsafe fn outl(port: u16, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}
