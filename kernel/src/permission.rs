//! Bounded runtime permission requests.
//!
//! A request is an authority-bearing object, not a boolean stored in a user
//! process.  It has an owner, a generation-tagged handle, an explicit state,
//! and a bounded lifetime.  Decisions still pass through `task::grant`, so
//! WovenGuard domain and sandbox ceilings remain authoritative.

use crate::{capability::Capability, irq_lock::IrqMutex, task::TaskId};

pub const MAX_REQUESTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Permission {
    Camera = 0,
    Microphone = 1,
    Network = 2,
    FilesystemRead = 3,
    FilesystemWrite = 4,
    Gpu = 5,
    Usb = 6,
    Clipboard = 7,
    Location = 8,
    Processes = 9,
    Ipc = 10,
    Storage = 11,
    Display = 12,
    Input = 13,
    Ai = 14,
    Secrets = 15,
}

impl Permission {
    pub const fn from_u64(value: u64) -> Option<Self> {
        Some(match value {
            0 => Self::Camera,
            1 => Self::Microphone,
            2 => Self::Network,
            3 => Self::FilesystemRead,
            4 => Self::FilesystemWrite,
            5 => Self::Gpu,
            6 => Self::Usb,
            7 => Self::Clipboard,
            8 => Self::Location,
            9 => Self::Processes,
            10 => Self::Ipc,
            11 => Self::Storage,
            12 => Self::Display,
            13 => Self::Input,
            14 => Self::Ai,
            15 => Self::Secrets,
            _ => return None,
        })
    }

    const fn capability(self) -> Capability {
        match self {
            Self::Camera => Capability::Camera,
            Self::Microphone => Capability::Microphone,
            Self::Network => Capability::NetworkIo,
            Self::FilesystemRead => Capability::FileRead,
            Self::FilesystemWrite => Capability::FileWrite,
            Self::Gpu => Capability::Gpu,
            Self::Usb => Capability::Usb,
            Self::Clipboard => Capability::Clipboard,
            Self::Location => Capability::Location,
            Self::Processes => Capability::ProcessCreate,
            Self::Ipc => Capability::Ipc,
            Self::Storage => Capability::StorageIo,
            Self::Display => Capability::DisplayIo,
            Self::Input => Capability::InputIo,
            Self::Ai => Capability::Ai,
            Self::Secrets => Capability::Secrets,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    Resolving,
    Granted,
    Denied,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    InvalidHandle,
    NotOwner,
    NotPending,
    Unauthorized,
    GrantFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    generation: u64,
    owner: TaskId,
    permission: Permission,
    reason: u64,
    state: State,
}

#[derive(Clone, Copy)]
struct Broker {
    entries: [Option<Entry>; MAX_REQUESTS],
    next_generation: u64,
}

impl Broker {
    const fn empty() -> Self {
        Self {
            entries: [None; MAX_REQUESTS],
            next_generation: 1,
        }
    }

    fn encode(slot: usize, generation: u64) -> u64 {
        (generation << 8) | slot as u64
    }

    fn locate(&self, handle: u64) -> Result<(usize, Entry), Error> {
        let slot = (handle & 0xff) as usize;
        if slot >= MAX_REQUESTS {
            return Err(Error::InvalidHandle);
        }
        let generation = handle >> 8;
        let entry = self.entries[slot].ok_or(Error::InvalidHandle)?;
        if entry.generation != generation {
            return Err(Error::InvalidHandle);
        }
        Ok((slot, entry))
    }

    fn request(
        &mut self,
        owner: TaskId,
        permission: Permission,
        reason: u64,
    ) -> Result<u64, Error> {
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Capacity)?;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.entries[slot] = Some(Entry {
            generation,
            owner,
            permission,
            reason,
            state: State::Pending,
        });
        Ok(Self::encode(slot, generation))
    }
}

static BROKER: IrqMutex<Broker> = IrqMutex::with_rank(Broker::empty(), 20);

pub fn request(owner: TaskId, permission: Permission, reason: u64) -> Result<u64, Error> {
    let handle = BROKER.lock().request(owner, permission, reason)?;
    crate::audit::record_detail(
        owner.as_u64(),
        crate::audit::Action::PermissionRequest,
        handle,
        permission as u64,
        true,
    );
    Ok(handle)
}

pub fn request_current(permission: Permission, reason: u64) -> Result<u64, Error> {
    request(crate::task::current_task_id(), permission, reason)
}

pub fn state(owner: TaskId, handle: u64) -> Result<State, Error> {
    let broker = BROKER.lock();
    let (_, entry) = broker.locate(handle)?;
    if entry.owner != owner {
        return Err(Error::NotOwner);
    }
    Ok(entry.state)
}

pub fn state_current(handle: u64) -> Result<State, Error> {
    state(crate::task::current_task_id(), handle)
}

/// Resolve a request. The caller must already be an authorized policy
/// service (`TaskControl`); the actual capability grant remains WovenGuard's
/// decision, and therefore cannot be bypassed by this broker.
pub fn decide(handle: u64, allow: bool) -> Result<(), Error> {
    if !crate::task::current_has(crate::capability::Capability::TaskControl) {
        return Err(Error::Unauthorized);
    }
    let (slot, entry) = {
        let mut broker = BROKER.lock();
        let (slot, entry) = broker.locate(handle)?;
        if entry.state != State::Pending {
            return Err(Error::NotPending);
        }
        broker.entries[slot].as_mut().expect("located entry").state = State::Resolving;
        (slot, entry)
    };
    let result = if allow {
        crate::task::grant(entry.owner, entry.permission.capability())
            .map_err(|_| Error::GrantFailed)
    } else {
        Err(Error::GrantFailed)
    };
    let granted = allow && result.is_ok();
    {
        let mut broker = BROKER.lock();
        if let Some(current) = broker.entries[slot].as_mut() {
            current.state = if granted {
                State::Granted
            } else {
                State::Denied
            };
        }
    }
    crate::audit::record_detail(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::PermissionDecision,
        handle,
        entry.permission as u64,
        granted,
    );
    if granted {
        Ok(())
    } else {
        Err(Error::GrantFailed)
    }
}

pub fn revoke(handle: u64) -> Result<(), Error> {
    if !crate::task::current_has(crate::capability::Capability::TaskControl) {
        return Err(Error::Unauthorized);
    }
    let (slot, entry) = {
        let mut broker = BROKER.lock();
        let (slot, entry) = broker.locate(handle)?;
        if entry.state != State::Granted {
            return Err(Error::NotPending);
        }
        broker.entries[slot].as_mut().expect("located entry").state = State::Resolving;
        (slot, entry)
    };
    let result = crate::task::revoke(entry.owner, entry.permission.capability())
        .map_err(|_| Error::GrantFailed);
    let revoked = result.is_ok();
    if let Some(current) = BROKER.lock().entries[slot].as_mut() {
        current.state = if revoked {
            State::Revoked
        } else {
            State::Granted
        };
    }
    crate::audit::record_detail(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::PermissionRevoke,
        handle,
        entry.permission as u64,
        revoked,
    );
    if revoked {
        Ok(())
    } else {
        Err(Error::GrantFailed)
    }
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    let owner = TaskId::from_u64(7);
    let mut broker = Broker::empty();
    let handle = broker.request(owner, Permission::Camera, 0xabc).ok();
    let Some(handle) = handle else {
        return false;
    };
    let Ok((slot, entry)) = broker.locate(handle) else {
        return false;
    };
    entry.owner == owner
        && entry.permission == Permission::Camera
        && entry.reason == 0xabc
        && entry.state == State::Pending
        && broker.entries[slot].take().is_some()
        && broker.locate(handle) == Err(Error::InvalidHandle)
}
