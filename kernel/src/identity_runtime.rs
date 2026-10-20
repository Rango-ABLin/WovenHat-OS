//! Owner-bound kernel identity sessions.
//!
//! The policy authority remains in `libwoven::identity`; this layer binds its
//! session tokens to a process ID and applies credentials only after the
//! identity lock is released. It is the kernel seam used by the forthcoming
//! userspace identity syscalls.

use libwoven::identity::{Authority, Error as IdentityError, SessionId, SessionView};

use crate::irq_lock::IrqMutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Error {
    NotReady,
    Full,
    Owner,
    Identity(IdentityError),
    Task,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Binding {
    session: SessionId,
    owner: u64,
}

const MAX_BINDINGS: usize = libwoven::identity::MAX_SESSIONS;
static AUTHORITY: IrqMutex<Option<Authority>> = IrqMutex::with_rank(None, 4);
static BINDINGS: IrqMutex<[Option<Binding>; MAX_BINDINGS]> =
    IrqMutex::with_rank([None; MAX_BINDINGS], 5);

pub(crate) fn install(authority: Authority) {
    *AUTHORITY.lock() = Some(authority);
    BINDINGS.lock().fill(None);
}

pub(crate) fn open(owner: u64, name: &[u8], password: &[u8]) -> Result<SessionId, Error> {
    let binding_slot = BINDINGS
        .lock()
        .iter()
        .position(Option::is_none)
        .ok_or(Error::Full)?;
    let (session, view) = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        let session = authority
            .open_named_session(name, password)
            .map_err(Error::Identity)?;
        let view = authority.view(session).map_err(Error::Identity)?;
        (session, view)
    };
    BINDINGS.lock()[binding_slot] = Some(Binding { session, owner });
    if apply_view(owner, view).is_err() {
        let _ = close(owner, session);
        return Err(Error::Task);
    }
    Ok(session)
}

pub(crate) fn lock(owner: u64, session: SessionId) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        authority.lock(binding.session).map_err(Error::Identity)?;
        authority.view(binding.session).map_err(Error::Identity)?
    };
    apply_view(owner, view)
}

pub(crate) fn unlock(owner: u64, session: SessionId, password: &[u8]) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        authority
            .unlock(binding.session, password)
            .map_err(Error::Identity)?;
        authority.view(binding.session).map_err(Error::Identity)?
    };
    apply_view(owner, view)
}

pub(crate) fn elevate(owner: u64, session: SessionId, password: &[u8]) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        authority
            .elevate(binding.session, password)
            .map_err(Error::Identity)?;
        authority.view(binding.session).map_err(Error::Identity)?
    };
    apply_view(owner, view)
}

pub(crate) fn close(owner: u64, session: SessionId) -> Result<(), Error> {
    let slot = BINDINGS
        .lock()
        .iter()
        .position(|entry| entry.is_some_and(|binding| binding.session == session && binding.owner == owner))
        .ok_or(Error::Owner)?;
    {
        let mut authority = AUTHORITY.lock();
        authority
            .as_mut()
            .ok_or(Error::NotReady)?
            .close_session(session)
            .map_err(Error::Identity)?;
    }
    BINDINGS.lock()[slot] = None;
    if crate::task::current_process_id() != owner
        || !crate::task::set_current_credentials(crate::task::Credentials::USERSPACE)
    {
        return Err(Error::Task);
    }
    Ok(())
}

fn owned_binding(owner: u64, session: SessionId) -> Result<Binding, Error> {
    BINDINGS
        .lock()
        .iter()
        .flatten()
        .find(|binding| binding.session == session && binding.owner == owner)
        .copied()
        .ok_or(Error::Owner)
}

fn apply_view(owner: u64, view: SessionView) -> Result<(), Error> {
    if view.id.0 == 0 || crate::task::current_process_id() != owner {
        return Err(Error::Task);
    }
    if !crate::task::set_current_credentials(crate::task::Credentials {
        uid: view.uid,
        gid: view.gid.0,
    }) {
        return Err(Error::Task);
    }
    Ok(())
}

pub(crate) fn self_test() -> bool {
    let mut authority = Authority::new();
    let Ok(group) = authority.add_group(b"users") else {
        return false;
    };
    if authority
        .add_administrator(b"alice", group, b"salt", b"1234")
        .is_err()
    {
        return false;
    }
    install(authority);
    // The bootstrap process has no PROCESS_TABLE entry, so exercise the
    // owner/token/lock policy directly and verify the expected task failure is
    // fail-closed until the syscall path has a live userspace owner.
    matches!(open(1, b"alice", b"1234"), Err(Error::Task))
        && matches!(lock(1, SessionId(1)), Err(Error::Owner))
}

pub(crate) fn parse_open_buffer(buffer: &[u8]) -> Option<(&[u8], &[u8])> {
    let (&name_len, rest) = buffer.split_first()?;
    let name_len = name_len as usize;
    let (name, rest) = rest.split_at_checked(name_len)?;
    let (&password_len, rest) = rest.split_first()?;
    let password_len = password_len as usize;
    let (password, rest) = rest.split_at_checked(password_len)?;
    (rest.is_empty()
        && !name.is_empty()
        && name.len() <= libwoven::accounts::MAX_NAME
        && !password.is_empty()
        && password.len() <= libwoven::accounts::MAX_PIN)
        .then_some((name, password))
}

pub(crate) fn parse_password_buffer(buffer: &[u8]) -> Option<&[u8]> {
    let (&length, rest) = buffer.split_first()?;
    let length = length as usize;
    let (password, rest) = rest.split_at_checked(length)?;
    (rest.is_empty() && !password.is_empty() && password.len() <= libwoven::accounts::MAX_PIN)
        .then_some(password)
}
