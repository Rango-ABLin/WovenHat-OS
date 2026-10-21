//! Bounded WovenDriver manager: discovery, matching, binding, and power state.
#![cfg_attr(
    any(not(feature = "qemu-test"), feature = "stage13-12-test"),
    expect(
        dead_code,
        reason = "production binds through register_pci/bind_pci/unbind_pci_resources; power, interrupt and hotplug paths are exercised by the Stage 13.1/13.2 gates until production drivers call them"
    )
)]
use crate::device::DeviceKind;
use crate::irq_lock::IrqMutex as Mutex;
const MAX: usize = 32;
const PCI_BAR_COUNT: usize = 6;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Registered,
    Bound,
    Suspended,
    Unbinding,
}
#[derive(Clone, Copy)]
struct Entry {
    name: &'static str,
    kind: DeviceKind,
    pci_match: Option<PciMatch>,
    state: State,
    pci: Option<PciBinding>,
}

/// Bounded PCI identity matching criteria. `None` leaves a field
/// unconstrained; an all-unconstrained matcher is rejected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PciMatch {
    pub vendor_id: Option<u16>,
    pub device_id: Option<u16>,
    pub class: Option<u8>,
    pub subclass: Option<u8>,
    pub prog_if: Option<u8>,
}

impl PciMatch {
    fn constrained(self) -> bool {
        self.vendor_id.is_some()
            || self.device_id.is_some()
            || self.class.is_some()
            || self.subclass.is_some()
            || self.prog_if.is_some()
    }

    pub(crate) fn matches(self, device: crate::hal::pci::Device) -> bool {
        self.vendor_id.is_none_or(|value| value == device.vendor_id)
            && self.device_id.is_none_or(|value| value == device.device_id)
            && self.class.is_none_or(|value| value == device.class)
            && self.subclass.is_none_or(|value| value == device.subclass)
            && self.prog_if.is_none_or(|value| value == device.prog_if)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PciBinding {
    function: crate::hal::pci::topology::FunctionHandle,
    address: crate::hal::pci::Address,
    owner: u32,
    bars: [Option<crate::hal::pci::topology::MmioLease>; PCI_BAR_COUNT],
    interrupt: Option<PciInterrupt>,
    bar_rebalanced: bool,
    suspended_command: Option<crate::hal::pci::PowerCheckpoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PciInterrupt {
    Msi(crate::hal::pci::MsiLease),
    Msix(crate::hal::pci::msix::MsixLease),
}
/// Bounded driver binding table. Device discovery happens before this lock;
/// rank 10 therefore covers only the local binding transaction.
static TABLE: Mutex<[Option<Entry>; MAX]> = Mutex::with_rank([None; MAX], 10);

/// A successful, bounded driver-quiescence transaction. Names are static
/// registration identities; a suspended entry cannot be unbound or rebound
/// until this plan is resumed.
#[derive(Clone, Copy)]
pub struct SuspendPlan {
    names: [&'static str; MAX],
    count: usize,
}

/// Suspend every currently bound driver, rolling back already-suspended
/// drivers in reverse registration order if any participant refuses. The
/// table is sampled before any device transition so no table lock is held
/// across driver or PCI hardware operations.
pub fn suspend_all() -> Option<SuspendPlan> {
    let mut names = [""; MAX];
    let count = {
        let table = TABLE.lock();
        let mut count = 0;
        for entry in table.iter().flatten() {
            if entry.state == State::Bound {
                if count == MAX {
                    return None;
                }
                names[count] = entry.name;
                count += 1;
            }
        }
        count
    };
    for index in 0..count {
        if !suspend(names[index]) {
            for rollback in (0..index).rev() {
                let _ = resume(names[rollback]);
            }
            return None;
        }
    }
    Some(SuspendPlan { names, count })
}

/// Resume a previously successful suspend plan. Every participant is
/// attempted even if one resume fails so later drivers are not abandoned
/// silently.
pub fn resume_all(plan: SuspendPlan) -> bool {
    let mut restored = true;
    for index in (0..plan.count).rev() {
        if !resume(plan.names[index]) {
            restored = false;
        }
    }
    restored
}
pub fn register(name: &'static str, kind: DeviceKind) -> bool {
    register_inner(name, kind, None)
}

fn register_inner(name: &'static str, kind: DeviceKind, pci_match: Option<PciMatch>) -> bool {
    let mut t = TABLE.lock();
    if t.iter().flatten().any(|d| d.name == name) {
        return false;
    }
    let Some(s) = t.iter_mut().find(|d| d.is_none()) else {
        return false;
    };
    *s = Some(Entry {
        name,
        kind,
        pci_match,
        state: State::Registered,
        pci: None,
    });
    true
}

/// Register a driver with an explicit PCI identity matcher. Registration does
/// not claim hardware until [`bind_matching_pci`] is called.
#[cfg_attr(
    not(any(
        not(feature = "qemu-test"),
        feature = "stage13-12-test",
        feature = "stage13-1-test",
        feature = "stage13-2-test",
        feature = "stage13-3-test",
        feature = "stage13-4-test",
        feature = "stage13-5-test",
        feature = "stage13-6-test",
        feature = "stage13-7-test",
        feature = "stage13-8-test",
        feature = "stage13-9-test"
    )),
    expect(dead_code, reason = "used by production PCI driver registration")
)]
pub fn register_pci(name: &'static str, kind: DeviceKind, matcher: PciMatch) -> bool {
    matcher.constrained() && register_inner(name, kind, Some(matcher))
}

/// Return the first registered, unbound driver whose matcher accepts a PCI
/// function. Registration order is deterministic and bounded.
#[cfg_attr(
    not(any(
        not(feature = "qemu-test"),
        feature = "stage13-12-test",
        feature = "stage13-1-test",
        feature = "stage13-2-test",
        feature = "stage13-3-test",
        feature = "stage13-4-test",
        feature = "stage13-5-test",
        feature = "stage13-6-test",
        feature = "stage13-7-test",
        feature = "stage13-8-test",
        feature = "stage13-9-test"
    )),
    expect(dead_code, reason = "used by production PCI hotplug matching")
)]
pub fn matching_pci_driver(address: crate::hal::pci::Address) -> Option<&'static str> {
    let device = (0..crate::hal::pci::topology::MAX_FUNCTIONS)
        .filter_map(crate::hal::pci::device)
        .find(|device| {
            device.segment == address.segment
                && device.bus == address.bus
                && device.device == address.device
                && device.function == address.function
        })?;
    TABLE
        .lock()
        .iter()
        .flatten()
        .find(|entry| {
            entry.state == State::Registered
                && entry.pci.is_none()
                && entry
                    .pci_match
                    .is_some_and(|matcher| matcher.matches(device))
        })
        .map(|entry| entry.name)
}

/// Match and bind one PCI function. Unknown hardware returns `None` and does
/// not change ownership; a matched driver's admission result is returned.
#[cfg_attr(
    not(any(
        feature = "stage13-1-test",
        feature = "stage13-2-test",
        feature = "stage13-3-test",
        feature = "stage13-4-test",
        feature = "stage13-5-test",
        feature = "stage13-6-test",
        feature = "stage13-7-test",
        feature = "stage13-8-test",
        feature = "stage13-9-test"
    )),
    expect(dead_code, reason = "used by production PCI hotplug matching")
)]
pub fn bind_matching_pci(
    address: crate::hal::pci::Address,
    owner: u32,
) -> Option<Result<(), crate::hal::pci::topology::Error>> {
    let name = matching_pci_driver(address)?;
    Some(bind_pci_with_bars(name, address, owner))
}
pub fn bind(name: &'static str) -> bool {
    let Some(device) = crate::device::find(name) else {
        return false;
    };
    let mut t = TABLE.lock();
    let Some(d) = t
        .iter_mut()
        .flatten()
        .find(|d| d.name == name && d.kind == device.kind)
    else {
        return false;
    };
    d.state = State::Bound;
    true
}
pub fn suspend(name: &'static str) -> bool {
    let pci = {
        let table = TABLE.lock();
        let Some(entry) = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
        else {
            return false;
        };
        match entry.pci {
            Some(binding) if binding.interrupt.is_some() => return false,
            Some(binding) => Some(binding),
            None => None,
        }
    };
    let Some(binding) = pci else {
        let mut table = TABLE.lock();
        let Some(entry) = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
        else {
            return false;
        };
        entry.state = State::Suspended;
        return true;
    };
    let saved = match crate::hal::pci::suspend_owned(binding.function, binding.owner) {
        Ok(command) => command,
        Err(_) => return false,
    };
    let mut table = TABLE.lock();
    let Some(entry) = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
    else {
        drop(table);
        let _ = crate::hal::pci::resume_owned(binding.function, binding.owner, saved);
        return false;
    };
    let Some(current) = entry.pci.as_mut() else {
        return false;
    };
    if *current != binding || current.suspended_command.is_some() {
        drop(table);
        let _ = crate::hal::pci::resume_owned(binding.function, binding.owner, saved);
        return false;
    }
    current.suspended_command = Some(saved);
    entry.state = State::Suspended;
    true
}
pub fn resume(name: &'static str) -> bool {
    let pci = {
        let table = TABLE.lock();
        let Some(entry) = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Suspended)
        else {
            return false;
        };
        match entry.pci {
            Some(binding) if binding.interrupt.is_some() => return false,
            Some(binding) => {
                let Some(saved) = binding.suspended_command else {
                    return false;
                };
                Some((binding, saved))
            }
            None => None,
        }
    };
    let Some((binding, saved)) = pci else {
        let mut table = TABLE.lock();
        let Some(entry) = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Suspended)
        else {
            return false;
        };
        entry.state = State::Bound;
        return true;
    };
    if crate::hal::pci::resume_owned(binding.function, binding.owner, saved).is_err() {
        return false;
    }
    let mut table = TABLE.lock();
    let Some(entry) = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Suspended)
    else {
        let _ = crate::hal::pci::suspend_owned(binding.function, binding.owner);
        return false;
    };
    let Some(current) = entry.pci.as_mut() else {
        return false;
    };
    if *current != binding || current.suspended_command != Some(saved) {
        drop(table);
        let _ = crate::hal::pci::suspend_owned(binding.function, binding.owner);
        return false;
    }
    current.suspended_command = None;
    entry.state = State::Bound;
    true
}

/// Claim a discovered PCI function for a registered WovenDriver. The topology
/// claim is acquired before the binding is published; publication failure is
/// rolled back so no owner is stranded.
fn bind_pci_claim(
    name: &'static str,
    address: crate::hal::pci::Address,
    owner: u32,
) -> Result<(), crate::hal::pci::topology::Error> {
    if owner == crate::hal::pci::topology::NO_OWNER {
        return Err(crate::hal::pci::topology::Error::InvalidOwner);
    }
    let handle = crate::hal::pci::topology_handle(address)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    crate::hal::pci::claim_function(handle, owner)?;

    let published = {
        let mut table = TABLE.lock();
        table
            .iter_mut()
            .flatten()
            .find(|entry| {
                entry.name == name && entry.state == State::Registered && entry.pci.is_none()
            })
            .is_some_and(|entry| {
                entry.pci = Some(PciBinding {
                    function: handle,
                    address,
                    owner,
                    bars: [None; PCI_BAR_COUNT],
                    interrupt: None,
                    bar_rebalanced: false,
                    suspended_command: None,
                });
                entry.state = State::Bound;
                true
            })
    };
    if !published {
        let _ = crate::hal::pci::release_function(handle, owner);
        return Err(crate::hal::pci::topology::Error::AlreadyOwned);
    }
    Ok(())
}

/// Admit a discovered PCI function into WovenDriver ownership.  Public PCI
/// admission always retains every currently programmed MMIO BAR so a bound
/// driver cannot exist with untracked subordinate address-space authority.
pub fn bind_pci(
    name: &'static str,
    address: crate::hal::pci::Address,
    owner: u32,
) -> Result<(), crate::hal::pci::topology::Error> {
    bind_pci_with_bars(name, address, owner)
}

/// Claim a discovered PCI function and retain its currently programmed MMIO
/// BARs as one driver-admission transaction. If any BAR lease cannot be
/// retained, ordered teardown rolls the partial binding back before returning.
pub fn bind_pci_with_bars(
    name: &'static str,
    address: crate::hal::pci::Address,
    owner: u32,
) -> Result<(), crate::hal::pci::topology::Error> {
    if !crate::hal::pci::admission_ready() {
        return Err(crate::hal::pci::topology::Error::InvalidHandle);
    }
    bind_pci_claim(name, address, owner)?;
    let bar_rebalanced = match crate::hal::pci::prepare_pci_admission(address) {
        Ok(rebalanced) => rebalanced,
        Err(_) => {
            let _ = unbind_pci_resources(name);
            return Err(crate::hal::pci::topology::Error::InvalidBar);
        }
    };
    if bar_rebalanced {
        let marked = {
            let mut table = TABLE.lock();
            table
                .iter_mut()
                .flatten()
                .find(|entry| entry.name == name && entry.state == State::Bound)
                .and_then(|entry| entry.pci.as_mut())
                .is_some_and(|binding| {
                    if binding.address == address && !binding.bar_rebalanced {
                        binding.bar_rebalanced = true;
                        true
                    } else {
                        false
                    }
                })
        };
        if !marked {
            let _ = crate::hal::pci::release_bars_from_firmware(address);
            let _ = unbind_pci_resources(name);
            return Err(crate::hal::pci::topology::Error::InvalidBar);
        }
        if crate::hal::pci::route_discovered_bridges().is_err() {
            let _ = unbind_pci_resources(name);
            return Err(crate::hal::pci::topology::Error::InvalidBar);
        }
    }
    let binding = {
        let table = TABLE.lock();
        match table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .and_then(|entry| entry.pci)
        {
            Some(binding) => binding,
            None => {
                drop(table);
                // The outer claim has already been published. Never return
                // without attempting ordered rollback, otherwise an admission
                // bookkeeping failure can strand function ownership.
                let _ = unbind_pci_resources(name);
                return Err(crate::hal::pci::topology::Error::InvalidHandle);
            }
        }
    };

    for bar_index in 0..PCI_BAR_COUNT {
        match crate::hal::pci::lease_bar(binding.function, binding.owner, bar_index as u8) {
            Ok(lease) => {
                if attach_pci_bar(name, lease).is_err() {
                    let _ = crate::hal::pci::release_bar_lease(lease, binding.owner);
                    let _ = unbind_pci_resources(name);
                    return Err(crate::hal::pci::topology::Error::InvalidBar);
                }
            }
            // I/O, absent, and unassigned BAR slots are not MMIO authority and
            // therefore need no driver-table lease.
            Err(crate::hal::pci::topology::Error::InvalidBar) => {}
            Err(error) => {
                let _ = unbind_pci_resources(name);
                return Err(error);
            }
        }
    }

    Ok(())
}

/// Detach a PCI binding after the caller has quiesced device interrupts and
/// released any MSI/MSI-X and BAR leases. The driver is marked Unbinding
/// before topology ownership is released so new work cannot observe it Bound.
pub fn unbind_pci(name: &'static str) -> Result<(), crate::hal::pci::topology::Error> {
    let binding = {
        let mut table = TABLE.lock();
        let entry = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
        let binding = entry
            .pci
            .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
        // Function ownership is the outer authority. It must outlive every
        // subordinate BAR/interrupt lease; resource-aware teardown detaches
        // those leases before it can reach this release path.
        if binding.bars.iter().any(Option::is_some) || binding.interrupt.is_some() {
            return Err(crate::hal::pci::topology::Error::AlreadyOwned);
        }
        entry.state = State::Unbinding;
        binding
    };

    if let Err(error) = crate::hal::pci::release_function(binding.function, binding.owner) {
        let mut table = TABLE.lock();
        if let Some(entry) = table.iter_mut().flatten().find(|entry| entry.name == name) {
            entry.state = State::Bound;
        }
        return Err(error);
    }

    let mut table = TABLE.lock();
    let entry = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    entry.pci = None;
    entry.state = State::Registered;
    Ok(())
}

/// Attach one generation-safe MMIO BAR lease to its architectural BAR slot.
/// Distinct BARs can be retained concurrently; replacing an existing slot is
/// rejected so authority cannot be silently lost.
pub fn attach_pci_bar(
    name: &'static str,
    bar: crate::hal::pci::topology::MmioLease,
) -> Result<(), crate::hal::pci::topology::Error> {
    let index = usize::from(bar.bar);
    if index >= PCI_BAR_COUNT {
        return Err(crate::hal::pci::topology::Error::InvalidBar);
    }
    let mut table = TABLE.lock();
    let entry = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry
        .pci
        .as_mut()
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.bars[index].is_some()
        || bar.function != binding.function
        || !crate::hal::pci::validate_bar_lease(bar, binding.owner)
    {
        return Err(crate::hal::pci::topology::Error::InvalidBar);
    }
    binding.bars[index] = Some(bar);
    Ok(())
}

/// Map one page within a retained PCI BAR under the driver's owner token.
///
/// The returned mapping is revoked automatically by
/// [`unbind_pci_resources`] after interrupt quiescence and before BAR/function
/// authority is released. Raw `paging::map_mmio` must not be used for a page
/// admitted through this API because raw mappings are permanently shared.
pub fn map_pci_mmio_page(
    name: &'static str,
    bar_index: u8,
    offset: u64,
) -> Result<crate::paging::OwnedMmioMapping, crate::paging::MapRangeError> {
    if usize::from(bar_index) >= PCI_BAR_COUNT {
        return Err(crate::paging::MapRangeError::InvalidRange);
    }
    let (base, size, owner) = {
        let table = TABLE.lock();
        let entry = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::paging::MapRangeError::InvalidRange)?;
        let binding = entry
            .pci
            .ok_or(crate::paging::MapRangeError::InvalidRange)?;
        let bar = binding.bars[usize::from(bar_index)]
            .ok_or(crate::paging::MapRangeError::InvalidRange)?;
        (bar.base, bar.size, binding.owner)
    };
    let page_offset = offset & !(4096_u64 - 1);
    if page_offset >= size {
        return Err(crate::paging::MapRangeError::InvalidRange);
    }
    let physical = base
        .checked_add(page_offset)
        .ok_or(crate::paging::MapRangeError::InvalidRange)?;
    crate::paging::map_mmio_owned(physical, owner)
}

/// Activate MSI for a bound PCI driver and checkpoint the resulting lease.
/// Hardware programming happens outside the driver-table lock. If the binding
/// changes before publication, immediately quiesce the just-enabled MSI path
/// so an unreachable live vector cannot escape driver ownership.
pub fn enable_pci_msi(
    name: &'static str,
    destination_apic_id: u32,
) -> Result<crate::hal::pci::MsiLease, crate::hal::pci::MsiLifecycleError> {
    let (function, owner) = {
        let table = TABLE.lock();
        let entry = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::hal::pci::MsiLifecycleError::InvalidFunction)?;
        let binding = entry
            .pci
            .ok_or(crate::hal::pci::MsiLifecycleError::InvalidFunction)?;
        if binding.interrupt.is_some() {
            return Err(crate::hal::pci::MsiLifecycleError::InvalidFunction);
        }
        (binding.function, binding.owner)
    };

    let lease = crate::hal::pci::enable_owned_msi(function, owner, destination_apic_id)?;
    if attach_pci_msi(name, lease).is_ok() {
        return Ok(lease);
    }

    // Publication raced with a lifecycle change. The lease is still local to
    // this call, so ordered disable is the only safe rollback. If hardware
    // quiesce fails, preserve the concrete lease in ActivationRetained so the
    // caller can retry teardown instead of losing a live vector obligation.
    match crate::hal::pci::disable_owned_msi(lease, owner) {
        Ok(()) => Err(crate::hal::pci::MsiLifecycleError::InvalidFunction),
        Err(crate::hal::pci::MsiLifecycleError::ActivationRetained { error, .. }) => {
            Err(crate::hal::pci::MsiLifecycleError::ActivationRetained { error, lease })
        }
        Err(error) => Err(error),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciInterruptLease {
    Msi(crate::hal::pci::MsiLease),
    Msix(crate::hal::pci::msix::MsixLease),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciInterruptError {
    Msi(crate::hal::pci::MsiLifecycleError),
    Msix(crate::hal::pci::msix::LifecycleError),
}

/// Activate the best supported interrupt mode for a bound function. MSI-X is
/// attempted only when the published capability is present and a retained BAR
/// candidate exists; clean MSI-X failures fall back to MSI. An
/// `ActivationRetained` failure is returned immediately because the device may
/// already own a live vector/table entry that must be torn down explicitly.
#[cfg_attr(
    not(any(
        not(feature = "qemu-test"),
        feature = "stage13-12-test",
        feature = "stage13-1-test",
        feature = "stage13-2-test",
        feature = "stage13-3-test",
        feature = "stage13-4-test",
        feature = "stage13-5-test",
        feature = "stage13-6-test",
        feature = "stage13-7-test",
        feature = "stage13-8-test",
        feature = "stage13-9-test"
    )),
    expect(dead_code, reason = "used by production driver interrupt activation")
)]
pub fn enable_pci_interrupt(
    name: &'static str,
    destination_apic_id: u32,
    entry_index: u16,
) -> Result<PciInterruptLease, PciInterruptError> {
    let (bars, supports_msix) = {
        let table = TABLE.lock();
        let entry = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(PciInterruptError::Msi(
                crate::hal::pci::MsiLifecycleError::InvalidFunction,
            ))?;
        let binding = entry.pci.ok_or(PciInterruptError::Msi(
            crate::hal::pci::MsiLifecycleError::InvalidFunction,
        ))?;
        let supports_msix = (0..crate::hal::pci::topology::MAX_FUNCTIONS)
            .filter_map(crate::hal::pci::device)
            .find(|device| {
                device.segment == binding.address.segment
                    && device.bus == binding.address.bus
                    && device.device == binding.address.device
                    && device.function == binding.address.function
            })
            .is_some_and(|device| device.capabilities.msix);
        (binding.bars, supports_msix)
    };

    if supports_msix {
        for bar in bars.iter().flatten().copied() {
            match enable_pci_msix(name, bar, destination_apic_id, entry_index) {
                Ok(lease) => return Ok(PciInterruptLease::Msix(lease)),
                Err(crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease }) => {
                    return Err(PciInterruptError::Msix(
                        crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease },
                    ));
                }
                Err(_) => {}
            }
        }
    }

    enable_pci_msi(name, destination_apic_id)
        .map(PciInterruptLease::Msi)
        .map_err(PciInterruptError::Msi)
}

pub fn attach_pci_msi(
    name: &'static str,
    lease: crate::hal::pci::MsiLease,
) -> Result<(), crate::hal::pci::topology::Error> {
    let mut table = TABLE.lock();
    let entry = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry
        .pci
        .as_mut()
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.interrupt.is_some() || lease.function != binding.function {
        return Err(crate::hal::pci::topology::Error::InvalidHandle);
    }
    binding.interrupt = Some(PciInterrupt::Msi(lease));
    Ok(())
}

pub fn enable_pci_msix(
    name: &'static str,
    bar: crate::hal::pci::topology::MmioLease,
    destination_apic_id: u32,
    entry_index: u16,
) -> Result<crate::hal::pci::msix::MsixLease, crate::hal::pci::msix::LifecycleError> {
    let (function, owner) = {
        let table = TABLE.lock();
        let entry = table
            .iter()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::hal::pci::msix::LifecycleError::Msix(
                crate::hal::pci::msix::Error::InvalidLease,
            ))?;
        let binding = entry
            .pci
            .ok_or(crate::hal::pci::msix::LifecycleError::Msix(
                crate::hal::pci::msix::Error::InvalidLease,
            ))?;
        (binding.function, binding.owner)
    };

    match crate::hal::pci::msix::enable_owned(
        function,
        bar,
        owner,
        destination_apic_id,
        entry_index,
    ) {
        Ok(lease) => {
            if attach_pci_msix(name, lease).is_ok() {
                return Ok(lease);
            }
            // The hardware activation succeeded but the binding changed before
            // we could checkpoint the lease.  Never discard that teardown
            // obligation: quiesce and recycle it immediately.  If cleanup
            // itself cannot complete, return ActivationRetained so the caller
            // still owns the live lease and can retry teardown explicitly.
            match crate::hal::pci::msix::disable_owned(lease, owner) {
                Ok(()) => Err(crate::hal::pci::msix::LifecycleError::Msix(
                    crate::hal::pci::msix::Error::InvalidLease,
                )),
                Err(crate::hal::pci::msix::LifecycleError::Msix(error)) => {
                    Err(crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease })
                }
                Err(crate::hal::pci::msix::LifecycleError::ActivationRetained {
                    error, ..
                }) => {
                    Err(crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease })
                }
                Err(crate::hal::pci::msix::LifecycleError::Vector(error)) => {
                    Err(crate::hal::pci::msix::LifecycleError::Vector(error))
                }
            }
        }
        Err(crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease }) => {
            // Activation failed only after the device-visible table held the
            // new vector. Persist both the table BAR and interrupt lease so the
            // normal ordered unbind path can quiesce and recycle them safely.
            if attach_pci_msix(name, lease).is_err() {
                return Err(crate::hal::pci::msix::LifecycleError::ActivationRetained {
                    error,
                    lease,
                });
            }
            Err(crate::hal::pci::msix::LifecycleError::ActivationRetained { error, lease })
        }
        Err(error) => Err(error),
    }
}

pub fn attach_pci_msix(
    name: &'static str,
    lease: crate::hal::pci::msix::MsixLease,
) -> Result<(), crate::hal::pci::topology::Error> {
    let mut table = TABLE.lock();
    let entry = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry
        .pci
        .as_mut()
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.interrupt.is_some()
        || lease.function != binding.function
        || lease.bar.function != binding.function
    {
        return Err(crate::hal::pci::topology::Error::InvalidHandle);
    }
    let bar_index = usize::from(lease.bar.bar);
    if bar_index >= PCI_BAR_COUNT {
        return Err(crate::hal::pci::topology::Error::InvalidBar);
    }
    match binding.bars[bar_index] {
        Some(retained) if retained != lease.bar => {
            return Err(crate::hal::pci::topology::Error::InvalidBar);
        }
        None => binding.bars[bar_index] = Some(lease.bar),
        Some(_) => {}
    }
    binding.interrupt = Some(PciInterrupt::Msix(lease));
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciUnbindError {
    InvalidBinding,
    /// A new device appeared at an existing BDF before the old owner was
    /// quiesced. Configuration writes cannot safely be directed to either
    /// generation until platform hotplug ordering supplies that boundary.
    ReplacementRequiresQuiescence,
    Mmio(crate::paging::MapRangeError),
    Msi(crate::hal::pci::MsiLifecycleError),
    Msix(crate::hal::pci::msix::LifecycleError),
    Topology(crate::hal::pci::topology::Error),
    Bar(crate::hal::pci::PersistentBarError),
}

/// Resource-aware PCI teardown. Interrupt delivery is quiesced before its
/// vector can be recycled; only after subordinate authority is gone may the
/// outer function ownership be released.
pub fn unbind_pci_resources(name: &'static str) -> Result<(), PciUnbindError> {
    {
        let mut table = TABLE.lock();
        let entry = table
            .iter_mut()
            .flatten()
            .find(|entry| {
                entry.name == name && matches!(entry.state, State::Bound | State::Unbinding)
            })
            .ok_or(PciUnbindError::InvalidBinding)?;
        if entry.pci.is_none() {
            return Err(PciUnbindError::InvalidBinding);
        }
        entry.state = State::Unbinding;
    }

    // Each successful phase is checkpointed in the binding before continuing.
    // A retry therefore resumes from the first still-owned resource instead of
    // replaying teardown against a stale vector or BAR generation.
    let binding = TABLE
        .lock()
        .iter()
        .flatten()
        .find(|entry| entry.name == name)
        .and_then(|entry| entry.pci)
        .ok_or(PciUnbindError::InvalidBinding)?;

    if let Some(interrupt) = binding.interrupt {
        let result = match interrupt {
            PciInterrupt::Msi(lease) => crate::hal::pci::disable_owned_msi(lease, binding.owner)
                .map_err(PciUnbindError::Msi),
            PciInterrupt::Msix(lease) => crate::hal::pci::msix::disable_owned(lease, binding.owner)
                .map_err(PciUnbindError::Msix),
        };
        result?;
        let mut table = TABLE.lock();
        let entry = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Unbinding)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let current = entry.pci.as_mut().ok_or(PciUnbindError::InvalidBinding)?;
        if current.function != binding.function
            || current.owner != binding.owner
            || current.interrupt != Some(interrupt)
        {
            return Err(PciUnbindError::InvalidBinding);
        }
        current.interrupt = None;
    }

    // Device-register mappings are revoked only after interrupt delivery is
    // quiesced. A failure leaves the binding in Unbinding so teardown can be
    // retried without releasing BAR or function authority prematurely.
    crate::paging::revoke_mmio_owner(binding.owner).map_err(PciUnbindError::Mmio)?;

    // MSI-X attachment makes its table BAR a persistent retained BAR, so after
    // interrupt quiescence every remaining authority is represented by bars[].
    for index in 0..PCI_BAR_COUNT {
        let bar = {
            let table = TABLE.lock();
            table
                .iter()
                .flatten()
                .find(|entry| entry.name == name)
                .and_then(|entry| entry.pci)
                .and_then(|current| current.bars[index])
        };
        let Some(bar) = bar else {
            continue;
        };
        crate::hal::pci::release_bar_lease(bar, binding.owner).map_err(PciUnbindError::Topology)?;
        let mut table = TABLE.lock();
        let entry = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Unbinding)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let current = entry.pci.as_mut().ok_or(PciUnbindError::InvalidBinding)?;
        if current.function != binding.function
            || current.owner != binding.owner
            || current.bars[index] != Some(bar)
        {
            return Err(PciUnbindError::InvalidBinding);
        }
        current.bars[index] = None;
    }

    if binding.bar_rebalanced {
        crate::hal::pci::release_bars_from_firmware(binding.address)
            .map_err(PciUnbindError::Bar)?;
        let mut table = TABLE.lock();
        let entry = table
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name && entry.state == State::Unbinding)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let current = entry.pci.as_mut().ok_or(PciUnbindError::InvalidBinding)?;
        if current.address != binding.address || !current.bar_rebalanced {
            return Err(PciUnbindError::InvalidBinding);
        }
        current.bar_rebalanced = false;
        drop(table);

        // Clearing a BAR removes this function's demand from its bridge
        // windows. Recompute the complete forwarding chain before releasing
        // the function generation; a routing failure leaves teardown in
        // `Unbinding` so the operation can be retried safely.
        crate::hal::pci::route_discovered_bridges().map_err(|_| PciUnbindError::InvalidBinding)?;
    }

    crate::hal::pci::release_function(binding.function, binding.owner)
        .map_err(PciUnbindError::Topology)?;

    let mut table = TABLE.lock();
    let entry = table
        .iter_mut()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Unbinding)
        .ok_or(PciUnbindError::InvalidBinding)?;
    let current = entry.pci.ok_or(PciUnbindError::InvalidBinding)?;
    if current.function != binding.function
        || current.owner != binding.owner
        || current.interrupt.is_some()
        || current.bars.iter().any(Option::is_some)
    {
        return Err(PciUnbindError::InvalidBinding);
    }
    entry.pci = None;
    entry.state = State::Registered;
    Ok(())
}

/// Return the bound driver name for one generation-safe PCI function.
pub fn pci_driver_name(
    function: crate::hal::pci::topology::FunctionHandle,
) -> Option<&'static str> {
    TABLE
        .lock()
        .iter()
        .flatten()
        .find(|entry| {
            entry
                .pci
                .is_some_and(|binding| binding.function == function)
        })
        .map(|entry| entry.name)
}

/// The PCI function a driver is bound to, if it is bound to one.
pub fn pci_binding(name: &'static str) -> Option<crate::hal::pci::Address> {
    TABLE
        .lock()
        .iter()
        .flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .and_then(|entry| entry.pci)
        .map(|binding| binding.address)
}

/// How many MMIO BAR leases a bound driver holds.
pub fn pci_bar_leases(name: &'static str) -> usize {
    TABLE
        .lock()
        .iter()
        .flatten()
        .find(|entry| entry.name == name)
        .and_then(|entry| entry.pci)
        .map_or(0, |binding| binding.bars.iter().flatten().count())
}

/// Hot-remove a bound PCI function using the same ordered resource teardown as
/// explicit driver unbind, then invalidate the topology generation before the
/// slot can be reused.
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
pub fn remove_pci_function(
    function: crate::hal::pci::topology::FunctionHandle,
) -> Result<(), PciUnbindError> {
    let name = pci_driver_name(function).ok_or(PciUnbindError::InvalidBinding)?;
    unbind_pci_resources(name)?;

    // unbind_pci_resources releases the outer function claim but deliberately
    // leaves the topology node alive. Revalidate the same generation before
    // invalidating it so a concurrent lifecycle transition cannot make this
    // hot-remove act on a recycled slot.
    let owner = crate::hal::pci::topology_owner(function).map_err(PciUnbindError::Topology)?;
    if owner != crate::hal::pci::topology::NO_OWNER {
        return Err(PciUnbindError::Topology(
            crate::hal::pci::topology::Error::AlreadyOwned,
        ));
    }
    crate::hal::pci::teardown_function(function, owner).map_err(PciUnbindError::Topology)
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
#[derive(Clone, Copy)]
pub struct PciHotplugReport {
    pub summary: crate::hal::pci::Summary,
    pub additions:
        [Option<crate::hal::pci::RescanAddition>; crate::hal::pci::topology::MAX_FUNCTIONS],
    pub added_count: usize,
}

/// Preserve the established summary-only ABI while exposing verified
/// insertion events to a future driver matcher.
/// Validate the driver-table state without touching PCI hardware.
#[cfg(feature = "stage13-2-test")]
fn stage13_2_entry_valid(entry: &Entry) -> bool {
    match (entry.state, entry.pci) {
        (State::Registered | State::Suspended, None) => true,
        (State::Bound, Some(binding)) => {
            binding.owner != crate::hal::pci::topology::NO_OWNER
                && binding.bars.iter().enumerate().all(|(index, lease)| {
                    lease.is_none_or(|bar| {
                        usize::from(bar.bar) == index && bar.function == binding.function
                    })
                })
                && binding.interrupt.is_none_or(|interrupt| match interrupt {
                    PciInterrupt::Msi(lease) => lease.function == binding.function,
                    PciInterrupt::Msix(lease) => {
                        let index = usize::from(lease.bar.bar);
                        lease.function == binding.function
                            && lease.bar.function == binding.function
                            && index < PCI_BAR_COUNT
                            && binding.bars[index] == Some(lease.bar)
                    }
                })
        }
        (State::Bound, None) => true, // Non-PCI driver.
        (State::Unbinding, _) | (State::Registered | State::Suspended, Some(_)) => false,
    }
}

/// Stage 13.2 driver lifecycle snapshot and negative-state regression.
/// The synthetic cases verify that unfinished teardown is rejected; real
/// ownership release, hotplug and interrupt quiescence require separate tests.
#[cfg(feature = "stage13-2-test")]
pub fn stage13_2_legacy_lifecycle_self_test() -> bool {
    let synthetic = Entry {
        name: "stage13-2-audit",
        kind: DeviceKind::Timer,
        state: State::Registered,
        pci: None,
        pci_match: None,
    };
    if !stage13_2_entry_valid(&synthetic) {
        return false;
    }
    let invalid = Entry {
        state: State::Unbinding,
        ..synthetic
    };
    if stage13_2_entry_valid(&invalid) {
        return false;
    }
    // Exercise the real generation-safe topology state machine in isolation:
    // a retained BAR blocks outer-owner release, stale leases fail after
    // release, and a reused function slot cannot honor its old handle.
    let mut topology = crate::hal::pci::topology::Topology::new();
    let descriptor = crate::hal::pci::topology::FunctionDescriptor {
        address: crate::hal::pci::topology::FunctionAddress {
            segment: 0,
            bus: 0,
            device: 1,
            function: 0,
        },
        bridge: None,
    };
    let Ok(handle) = topology.insert(descriptor) else {
        return false;
    };
    const OWNER: u32 = 0x132;
    if topology.claim(handle, OWNER).is_err()
        || topology.claim(handle, OWNER) != Err(crate::hal::pci::topology::Error::AlreadyOwned)
    {
        return false;
    }
    let Ok(lease) = topology.lease_mmio(handle, OWNER, 0, 0x1000, 0x1000) else {
        return false;
    };
    if !topology.validate_mmio(lease, OWNER)
        || topology.release(handle, OWNER) != Err(crate::hal::pci::topology::Error::AlreadyOwned)
        || topology.release_mmio(lease, OWNER + 1).is_ok()
        || topology.release_mmio(lease, OWNER).is_err()
        || topology.validate_mmio(lease, OWNER)
        || topology.release_mmio(lease, OWNER).is_ok()
        || topology.release(handle, OWNER).is_err()
        || topology
            .teardown(handle, crate::hal::pci::topology::NO_OWNER)
            .is_err()
        || topology.snapshot(handle).is_ok()
    {
        return false;
    }
    let Ok(reused) = topology.insert(descriptor) else {
        return false;
    };
    if reused == handle
        || topology.claim(handle, OWNER).is_ok()
        || topology.claim(reused, OWNER).is_err()
    {
        return false;
    }
    TABLE.lock().iter().flatten().all(stage13_2_entry_valid)
}

/// Driver-aware PCI hotplug reconciliation. The HAL only detects removals;
/// this layer owns driver teardown policy, preserving dependency direction.
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
pub fn hotplug_rescan() -> Result<crate::hal::pci::Summary, PciUnbindError> {
    Ok(hotplug_rescan_report()?.summary)
}

/// Driver-aware PCI hotplug reconciliation. Removal teardown is completed
/// before publication; newly present functions are returned only after the
/// refreshed topology and bridge routing have succeeded. Unknown functions
/// remain unclaimed.
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
pub fn hotplug_rescan_report() -> Result<PciHotplugReport, PciUnbindError> {
    let mut additions = [None; crate::hal::pci::topology::MAX_FUNCTIONS];
    let added_count = crate::hal::pci::rescan_added(&mut additions)
        .map_err(|_| PciUnbindError::InvalidBinding)?;
    let mut removed = [None; crate::hal::pci::topology::MAX_FUNCTIONS];
    let count = crate::hal::pci::rescan_removed(&mut removed);

    // A same-slot replacement is not equivalent to an ordinary disappearance:
    // the configuration address now names the replacement device. Refuse the
    // entire batch before any teardown so unbind cannot write command/BAR
    // state to the wrong generation.
    if removed[..count].iter().flatten().any(|removal| {
        additions[..added_count]
            .iter()
            .flatten()
            .any(|addition| addition.address == removal.address)
    }) {
        return Err(PciUnbindError::ReplacementRequiresQuiescence);
    }

    // Validate the complete removal set before mutating ownership.  A stale
    // generation in the rescan snapshot must abort the batch rather than
    // tearing down an arbitrary prefix and leaving hotplug reconciliation in
    // a half-applied state.
    for removal in removed[..count].iter().flatten().copied() {
        crate::hal::pci::topology_owner(removal.function).map_err(PciUnbindError::Topology)?;
    }

    for removal in removed[..count].iter().flatten().copied() {
        if pci_driver_name(removal.function).is_some() {
            remove_pci_function(removal.function)?;
        } else {
            let owner = crate::hal::pci::topology_owner(removal.function)
                .map_err(PciUnbindError::Topology)?;
            // A function may have been admitted and BAR-rebalanced before a
            // matching driver was registered. Release that persistent
            // reservation before invalidating the topology generation. An
            // unallocated or unconfigured resource domain is benign here;
            // every other failure keeps the removal retryable.
            match crate::hal::pci::release_bars_from_firmware(removal.address) {
                Ok(()) | Err(crate::hal::pci::PersistentBarError::Unavailable) => {}
                Err(error) => return Err(PciUnbindError::Bar(error)),
            }
            crate::hal::pci::teardown_function(removal.function, owner)
                .map_err(PciUnbindError::Topology)?;
        }
    }
    let summary =
        crate::hal::pci::reconcile_after_teardown().map_err(|_| PciUnbindError::InvalidBinding)?;

    // Reconciliation may add a newly reachable function behind an existing
    // bridge. Recompute and publish the complete forwarding hierarchy only
    // after all disappeared functions have completed ordered teardown. The
    // bridge layer performs one rollback-capable config-space transaction, so
    // a failed route update cannot leave a partially programmed chain.
    crate::hal::pci::route_discovered_bridges().map_err(|_| PciUnbindError::InvalidBinding)?;

    for addition in additions[..added_count].iter().flatten().copied() {
        if crate::hal::pci::topology_handle(addition.address).is_none() {
            return Err(PciUnbindError::InvalidBinding);
        }
    }
    Ok(PciHotplugReport {
        summary,
        additions,
        added_count,
    })
}

/// Exercise the complete PCI ownership transaction against one discovered
/// function without activating a device-specific driver.  This is deliberately
/// a test-only probe: production boot must not claim arbitrary hardware merely
/// because it is present, while the Stage 13.2 gate must still prove that the
/// WovenDriver owner is wired to PCI generations and BAR teardown.
#[cfg(feature = "stage13-2-test")]
pub fn stage13_2_lifecycle_self_test() -> bool {
    const NAME: &str = "stage13-2-pci-lifecycle";
    const OWNER: u32 = 0xD132_0001;

    let mut selected = None;
    let mut interrupt_candidate = None;
    let mut interrupt_candidate_is_msix = false;
    for index in 0..crate::hal::pci::topology::MAX_FUNCTIONS {
        let Some(device) = crate::hal::pci::device(index) else {
            continue;
        };
        crate::serial::write_line(format_args!(
            "[S13.2] inventory {:04x}:{:02x}:{:02x}.{} id={:04x}:{:04x} class={:02x}/{:02x}/{:02x} msi={} msix={}",
            device.segment,
            device.bus,
            device.device,
            device.function,
            device.vendor_id,
            device.device_id,
            device.class,
            device.subclass,
            device.prog_if,
            device.capabilities.msi,
            device.capabilities.msix,
        ));
        if device.capabilities.msix {
            crate::serial::write_line(format_args!(
                "[S13.2] msix-layout={:?}",
                crate::hal::pci::msix::capability(device)
                    .ok()
                    .map(|capability| {
                        (
                            capability.table.bir,
                            device.bars[usize::from(capability.table.bir)].address,
                            device.bars[usize::from(capability.table.bir)].valid,
                        )
                    })
            ));
        }
        let has_bar = device.bars.iter().any(|bar| {
            bar.valid
                && matches!(
                    bar.kind,
                    crate::hal::pci::BarKind::Memory32 | crate::hal::pci::BarKind::Memory64
                )
                && bar.address != 0
        });
        if has_bar && selected.is_none() {
            selected = Some(device);
        }
        let mappable_bar = device.bars.iter().any(|bar| {
            has_bar
                && bar.valid
                && matches!(
                    bar.kind,
                    crate::hal::pci::BarKind::Memory32 | crate::hal::pci::BarKind::Memory64
                )
                && bar.address != 0
        });
        let msix_table_mappable = if device.capabilities.msix {
            crate::hal::pci::msix::capability(device)
                .ok()
                .is_some_and(|capability| {
                    device
                        .bars
                        .get(usize::from(capability.table.bir))
                        .is_some_and(|bar| {
                            bar.valid
                                && matches!(
                                    bar.kind,
                                    crate::hal::pci::BarKind::Memory32
                                        | crate::hal::pci::BarKind::Memory64
                                )
                                && bar.address != 0
                        })
                })
        } else {
            false
        };
        if mappable_bar && (device.capabilities.msi || msix_table_mappable) {
            // Prefer a complete MSI-X fixture. Fall back to MSI only when no
            // MSI-X table is usable; MSI-X and MSI have distinct BAR and
            // cleanup obligations and should not be conflated by selection.
            if msix_table_mappable {
                if !interrupt_candidate_is_msix {
                    interrupt_candidate = Some(device);
                    interrupt_candidate_is_msix = true;
                }
            } else if device.capabilities.msi && interrupt_candidate.is_none() {
                interrupt_candidate = Some(device);
            }
        }
    }
    if let Some(device) = interrupt_candidate {
        selected = Some(device);
    }
    let Some(device) = selected else {
        return false;
    };
    crate::serial::write_line(format_args!(
        "[S13.2] owner-mmio candidate {:04x}:{:02x}:{:02x}.{} bars={:x}/{:x}/{:x}/{:x}/{:x}/{:x}",
        device.segment,
        device.bus,
        device.device,
        device.function,
        device.bars[0].address,
        device.bars[1].address,
        device.bars[2].address,
        device.bars[3].address,
        device.bars[4].address,
        device.bars[5].address,
    ));
    let registered = register_pci(
        NAME,
        DeviceKind::Block,
        PciMatch {
            vendor_id: Some(device.vendor_id),
            device_id: Some(device.device_id),
            ..PciMatch::default()
        },
    );
    if !registered {
        return false;
    }

    let address = crate::hal::pci::Address {
        segment: device.segment,
        bus: device.bus,
        device: device.device,
        function: device.function,
    };
    let matched = matching_pci_driver(address) == Some(NAME);
    crate::serial::write_line(format_args!("[S13.2] driver-match={}", matched));
    let bound = matched && bind_matching_pci(address, OWNER).is_some_and(|result| result.is_ok());
    crate::serial::write_line(format_args!("[S13.2] owner-mmio bind={}", bound));
    let interrupt_ready = if bound
        && interrupt_candidate.is_some_and(|candidate| {
            candidate.segment == device.segment
                && candidate.bus == device.bus
                && candidate.device == device.device
                && candidate.function == device.function
        }) {
        match enable_pci_interrupt(NAME, 0, 0) {
            Ok(_) => {
                crate::serial::write_line(format_args!("[S13.2] interrupt-selection=active"));
                true
            }
            Err(error) => {
                crate::serial::write_line(format_args!(
                    "[S13.2] interrupt-selection=failed {:?}",
                    error
                ));
                false
            }
        }
    } else {
        true
    };
    // Activate interrupts before retaining an arbitrary register mapping.
    // An MSI-X table may share the first page of a BAR; holding that page here
    // would make the activation appear to fail with AlreadyMapped.
    let owned_mapping = if bound {
        let mut mapped = None;
        for bar in 0..PCI_BAR_COUNT as u8 {
            match map_pci_mmio_page(NAME, bar, 0) {
                Ok(mapping) => {
                    crate::serial::write_line(format_args!(
                        "[S13.2] owner-mmio mapped bar={} physical={:#x} virtual={:#x}",
                        bar,
                        mapping.physical_address(),
                        mapping.virtual_address(),
                    ));
                    mapped = Some(mapping);
                    break;
                }
                Err(error) => crate::serial::write_line(format_args!(
                    "[S13.2] owner-mmio bar={} map={:?}",
                    bar, error,
                )),
            }
        }
        mapped
    } else {
        None
    };
    let clean = bound && unbind_pci_resources(NAME).is_ok();
    crate::serial::write_line(format_args!(
        "[S13.2] interrupt-selection teardown={} ready={}",
        clean, interrupt_ready
    ));
    crate::serial::write_line(format_args!("[S13.2] owner-mmio unbind={}", clean));
    let high_mmio = if clean {
        match crate::paging::map_mmio_owned(0x2000_0000_0000, OWNER.wrapping_add(3)) {
            Ok(mapping) => {
                let released = crate::paging::release_mmio_owned(mapping).is_ok();
                crate::serial::write_line(format_args!(
                    "[S13.2] high-mmio virtual={:#x} released={}",
                    mapping.virtual_address(),
                    released
                ));
                released
            }
            Err(error) => {
                crate::serial::write_line(format_args!("[S13.2] high-mmio failed {:?}", error));
                false
            }
        }
    } else {
        false
    };
    let remapped = match owned_mapping {
        Some(mapping) if clean => {
            let Ok(replacement) =
                crate::paging::map_mmio_owned(mapping.physical_address(), OWNER.wrapping_add(1))
            else {
                return false;
            };
            crate::paging::release_mmio_owned(replacement).is_ok()
        }
        Some(_) => false,
        None => {
            // QEMU's UEFI misc device may expose a firmware BAR outside the
            // kernel's direct-map range. Exercise the mapping primitive on a
            // reserved, non-RAM test page so the lifecycle gate still proves
            // revoke/remap without touching an invalid firmware address.
            let synthetic = crate::paging::map_mmio_owned(0x4000_0000, OWNER.wrapping_add(2));
            match synthetic {
                Ok(mapping) => crate::paging::release_mmio_owned(mapping).is_ok(),
                Err(crate::paging::MapRangeError::AlreadyMapped) => false,
                Err(_) => false,
            }
        }
    };
    if !clean || !remapped || !interrupt_ready || !high_mmio {
        return false;
    }
    let rescan = hotplug_rescan().is_ok();
    crate::serial::write_line(format_args!("[S13.2] owner-mmio rescan={}", rescan));
    if !rescan {
        return false;
    }
    let empty = {
        let mut table = TABLE.lock();
        let Some(slot) = table
            .iter_mut()
            .find(|entry| entry.is_some_and(|entry| entry.name == NAME))
        else {
            return false;
        };
        let empty =
            slot.is_some_and(|entry| entry.state == State::Registered && entry.pci.is_none());
        *slot = None;
        empty
    };
    bound && empty
}
