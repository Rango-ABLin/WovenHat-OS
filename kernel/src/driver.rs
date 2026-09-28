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
                entry.pci = Some(PciBinding { function: handle, owner });
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
