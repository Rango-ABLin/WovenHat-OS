//! Bounded WovenDriver manager: discovery, matching, binding, and power state.
use crate::device::DeviceKind;
use crate::irq_lock::IrqMutex as Mutex;
const MAX: usize = 32;
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
    bar: Option<crate::hal::pci::topology::MmioLease>,
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
                entry.pci = Some(PciBinding { function: handle, owner, bar: None, interrupt: None });
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
        if binding.bar.is_some() || binding.interrupt.is_some() {
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


/// Attach one generation-safe MMIO BAR lease to the driver's PCI binding.
/// A binding may retain only one primary MMIO authority until multi-BAR
/// ownership is modeled explicitly.
pub fn attach_pci_bar(
    name: &'static str,
    bar: crate::hal::pci::topology::MmioLease,
) -> Result<(), crate::hal::pci::topology::Error> {
    let mut table = TABLE.lock();
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name && entry.state == State::Bound)
        .ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    let binding = entry.pci.as_mut().ok_or(crate::hal::pci::topology::Error::InvalidHandle)?;
    if binding.bar.is_some() || bar.function != binding.function
        || !crate::hal::pci::validate_bar_lease(bar, binding.owner)
    {
        return Err(crate::hal::pci::topology::Error::InvalidBar);
    }
    binding.bar = Some(bar);
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
    let binding = {
        let mut table = TABLE.lock();
        let entry = table.iter_mut().flatten()
            .find(|entry| entry.name == name && entry.state == State::Bound)
            .ok_or(PciUnbindError::InvalidBinding)?;
        let binding = entry.pci.ok_or(PciUnbindError::InvalidBinding)?;
        entry.state = State::Unbinding;
        binding
    };

    let interrupt_result = match binding.interrupt {
        Some(PciInterrupt::Msi(lease)) =>
            crate::hal::pci::msi::disable_owned_msi(lease, binding.owner)
                .map_err(PciUnbindError::Msi),
        Some(PciInterrupt::Msix(lease)) =>
            crate::hal::pci::msix::disable_owned(lease, binding.owner)
                .map_err(PciUnbindError::Msix),
        None => Ok(()),
    };
    if let Err(error) = interrupt_result {
        let mut table = TABLE.lock();
        if let Some(entry) = table.iter_mut().flatten().find(|entry| entry.name == name) {
            entry.state = State::Bound;
        }
        return Err(error);
    }

    // MSI-X teardown above still needs its BAR authority, so BAR release must
    // follow interrupt quiescence and precede outer function ownership release.
    if let Some(bar) = binding.bar {
        if let Err(error) = crate::hal::pci::release_bar_lease(bar, binding.owner) {
            return Err(PciUnbindError::Topology(error));
        }
    } else if let Some(PciInterrupt::Msix(lease)) = binding.interrupt {
        // MSI-X can carry its table BAR directly even when it was not also
        // registered as the driver's primary BAR.
        if let Err(error) = crate::hal::pci::release_bar_lease(lease.bar, binding.owner) {
            return Err(PciUnbindError::Topology(error));
        }
    }

    if let Err(error) = crate::hal::pci::release_function(binding.function, binding.owner) {
        // Interrupts are already quiesced and their vector released. Keep the
        // driver out of Bound state: restoring Bound here would falsely imply
        // an operational interrupt path.
        return Err(PciUnbindError::Topology(error));
    }

    let mut table = TABLE.lock();
    let entry = table.iter_mut().flatten()
        .find(|entry| entry.name == name)
        .ok_or(PciUnbindError::InvalidBinding)?;
    entry.pci = None;
    entry.state = State::Registered;
    Ok(())
}
