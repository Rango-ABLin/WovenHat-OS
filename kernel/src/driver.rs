//! Bounded WovenDriver manager: discovery, matching, binding, and power state.
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
    state: State,
    pci: Option<PciBinding>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PciBinding {
    function: crate::hal::pci::topology::FunctionHandle,
    owner: u32,
    bars: [Option<crate::hal::pci::topology::MmioLease>; PCI_BAR_COUNT],
    interrupt: Option<PciInterrupt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PciInterrupt {
    Msi(crate::hal::pci::msi::MsiLease),
    Msix(crate::hal::pci::msix::MsixLease),
}
/// Bounded driver binding table. Device discovery happens before this lock;
/// rank 10 therefore covers only the local binding transaction.
static TABLE: Mutex<[Option<Entry>; MAX]> = Mutex::with_rank([None; MAX], 10);
pub fn register(name: &'static str, kind: DeviceKind) -> bool {
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
        state: State::Registered,
        pci: None,
    });
    true
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
    let mut t = TABLE.lock();
    let Some(d) = t
        .iter_mut()
        .flatten()
        .find(|d| d.name == name && d.state == State::Bound)
    else {
        return false;
    };
    d.state = State::Suspended;
    true
}
pub fn resume(name: &'static str) -> bool {
    let mut t = TABLE.lock();
    let Some(d) = t
        .iter_mut()
        .flatten()
        .find(|d| d.name == name && d.state == State::Suspended)
    else {
        return false;
    };
    d.state = State::Bound;
    true
}


/// Claim a discovered PCI function for a registered WovenDriver. The topology
/// claim is acquired before the binding is published; publication failure is
/// rolled back so no owner is stranded.
pub fn bind_pci(
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
        table.iter_mut().flatten()
            .find(|entry| entry.name == name && entry.state == State::Registered && entry.pci.is_none())
            .is_some_and(|entry| {
                entry.pci = Some(PciBinding { function: handle, owner, bars: [None; PCI_BAR_COUNT], interrupt: None });
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

/// Detach a PCI binding after the caller has quiesced device interrupts and
/// released any MSI/MSI-X and BAR leases. The driver is marked Unbinding
/// before topology ownership is released so new work cannot observe it Bound.
pub fn unbind_pci(name: &'static str) -> Result<(), crate::hal::pci::topology::Error> {
    let binding = {
        let mut table = TABLE.lock();
        let entry = table.iter_mut().flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
        let binding = entry.pci.ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
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
    let entry = table.iter_mut().flatten()
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
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry.pci.as_mut().ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.bars[index].is_some() || bar.function != binding.function
        || !crate::hal::pci::validate_bar_lease(bar, binding.owner)
    {
        return Err(crate::hal::pci::topology::Error::InvalidBar);
    }
    binding.bars[index] = Some(bar);
    Ok(())
}

pub fn attach_pci_msi(
    name: &'static str,
    lease: crate::hal::pci::msi::MsiLease,
) -> Result<(), crate::hal::pci::topology::Error> {
    let mut table = TABLE.lock();
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry.pci.as_mut().ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
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
        let entry = table.iter().flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(crate::hal::pci::msix::LifecycleError::Msix(
                crate::hal::pci::msix::Error::InvalidLease,
            ))?;
        let binding = entry.pci.ok_or(crate::hal::pci::msix::LifecycleError::Msix(
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
            attach_pci_msix(name, lease)
                .map_err(|_| crate::hal::pci::msix::LifecycleError::Msix(
                    crate::hal::pci::msix::Error::InvalidLease,
                ))?;
            Ok(lease)
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
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry.pci.as_mut().ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.interrupt.is_some() || lease.function != binding.function
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
    Msi(crate::hal::pci::msi::MsiLifecycleError),
    Msix(crate::hal::pci::msix::LifecycleError),
    Topology(crate::hal::pci::topology::Error),
}

/// Resource-aware PCI teardown. Interrupt delivery is quiesced before its
/// vector can be recycled; only after subordinate authority is gone may the
/// outer function ownership be released.
pub fn unbind_pci_resources(name: &'static str) -> Result<(), PciUnbindError> {
    {
        let mut table = TABLE.lock();
        let entry = table.iter_mut().flatten()
            .find(|entry| entry.name == name
                && matches!(entry.state, State::Bound | State::Unbinding))
            .ok_or(PciUnbindError::InvalidBinding)?;
        if entry.pci.is_none() { return Err(PciUnbindError::InvalidBinding); }
        entry.state = State::Unbinding;
    }

    // Each successful phase is checkpointed in the binding before continuing.
    // A retry therefore resumes from the first still-owned resource instead of
    // replaying teardown against a stale vector or BAR generation.
    let binding = TABLE.lock().iter().flatten()
        .find(|entry| entry.name == name).and_then(|entry| entry.pci)
        .ok_or(PciUnbindError::InvalidBinding)?;

    if let Some(interrupt) = binding.interrupt {
        let result = match interrupt {
            PciInterrupt::Msi(lease) =>
                crate::hal::pci::msi::disable_owned_msi(lease, binding.owner)
                    .map_err(PciUnbindError::Msi),
            PciInterrupt::Msix(lease) =>
                crate::hal::pci::msix::disable_owned(lease, binding.owner)
                    .map_err(PciUnbindError::Msix),
        };
        if let Err(error) = result {
            return Err(error);
        }
        let mut table = TABLE.lock();
        let entry = table.iter_mut().flatten().find(|entry| entry.name == name)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let current = entry.pci.as_mut().ok_or(PciUnbindError::InvalidBinding)?;
        current.interrupt = None;
    }

    // MSI-X attachment makes its table BAR a persistent retained BAR, so after
    // interrupt quiescence every remaining authority is represented by bars[].
    for index in 0..PCI_BAR_COUNT {
        let bar = {
            let table = TABLE.lock();
            table.iter().flatten().find(|entry| entry.name == name)
                .and_then(|entry| entry.pci)
                .and_then(|current| current.bars[index])
        };
        let Some(bar) = bar else { continue; };
        crate::hal::pci::release_bar_lease(bar, binding.owner)
            .map_err(PciUnbindError::Topology)?;
        let mut table = TABLE.lock();
        let entry = table.iter_mut().flatten().find(|entry| entry.name == name)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let current = entry.pci.as_mut().ok_or(PciUnbindError::InvalidBinding)?;
        current.bars[index] = None;
    }

    crate::hal::pci::release_function(binding.function, binding.owner)
        .map_err(PciUnbindError::Topology)?;

    let mut table = TABLE.lock();
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name)
        .ok_or(PciUnbindError::InvalidBinding)?;
    entry.pci = None;
    entry.state = State::Registered;
    Ok(())
}

/// Return the bound driver name for one generation-safe PCI function.
pub fn pci_driver_name(function: crate::hal::pci::topology::FunctionHandle) -> Option<&'static str> {
    TABLE.lock().iter().flatten()
        .find(|entry| entry.pci.is_some_and(|binding| binding.function == function))
        .map(|entry| entry.name)
}

/// Hot-remove a bound PCI function using the same ordered resource teardown as
/// explicit driver unbind, then invalidate the topology generation before the
/// slot can be reused.
pub fn remove_pci_function(
    function: crate::hal::pci::topology::FunctionHandle,
) -> Result<(), PciUnbindError> {
    let name = pci_driver_name(function).ok_or(PciUnbindError::InvalidBinding)?;
    unbind_pci_resources(name)?;
    crate::hal::pci::teardown_function(function, crate::hal::pci::topology::NO_OWNER)
        .map_err(PciUnbindError::Topology)
}


/// Driver-aware PCI hotplug reconciliation. The HAL only detects removals;
/// this layer owns driver teardown policy, preserving dependency direction.
pub fn hotplug_rescan() -> Result<crate::hal::pci::Summary, PciUnbindError> {
    let mut removed = [None; crate::hal::pci::MAX_DEVICES];
    let count = crate::hal::pci::rescan_removed(&mut removed);
    for removal in removed[..count].iter().flatten().copied() {
        if pci_driver_name(removal.function).is_some() {
            remove_pci_function(removal.function)?;
        } else {
            let owner = crate::hal::pci::topology_owner(removal.function)
                .map_err(PciUnbindError::Topology)?;
            crate::hal::pci::teardown_function(removal.function, owner)
                .map_err(PciUnbindError::Topology)?;
        }
    }
    Ok(crate::hal::pci::reconcile_after_teardown())
}
