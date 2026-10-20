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
    Storage,
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

#[cfg(feature = "system-apps")]
pub(crate) fn load_accounts_file(path: &str) -> Result<(), Error> {
    let stat = crate::vfs::stat(path).map_err(|_| Error::Storage)?;
    if stat.size == 0 || stat.size > 16 * 1024 {
        return Err(Error::Storage);
    }
    let mut contents = alloc::vec![0u8; stat.size];
    let length = crate::vfs::read_all(path, &mut contents).map_err(|_| Error::Storage)?;
    if length != stat.size {
        return Err(Error::Storage);
    }
    let mut authority = Authority::new();
    let group = authority.add_group(b"users").map_err(Error::Identity)?;
    let mut count = 0usize;
    for line in contents[..length].split(|byte| *byte == b'\n') {
        if line.is_empty() || line == b"\r" {
            continue;
        }
        let account = libwoven::accounts::Account::parse(line).ok_or(Error::Storage)?;
        let (salt, hash) = account.credential_material();
        authority
            .add_stored_user(account.name(), group, salt, hash, false)
            .map_err(Error::Identity)?;
        count += 1;
    }
    if count == 0 {
        return Err(Error::Storage);
    }
    install(authority);
    crate::serial::write_line(format_args!(
        "[IDENTITY] loaded {} account credentials from {}",
        count, path
    ));
    Ok(())
}

#[cfg(feature = "system-apps")]
pub(crate) fn load_durable_or_accounts(path: &str) -> Result<(), Error> {
    let mut snapshot = [0u8; libwoven::identity::SNAPSHOT_BYTES];
    if let Ok(sequence) = crate::identity_storage::load(&mut snapshot) {
        let authority = Authority::decode_snapshot(&snapshot).map_err(Error::Identity)?;
        install(authority);
        crate::serial::write_line(format_args!(
            "[IDENTITY] loaded durable WID2 snapshot sequence={}",
            sequence
        ));
        return Ok(());
    }
    load_accounts_file(path)
}

pub(crate) fn release_owner(owner: u64) {
    let mut sessions = [None; MAX_BINDINGS];
    let mut count = 0;
    {
        let mut bindings = BINDINGS.lock();
        for binding in bindings.iter_mut() {
            if binding.is_some_and(|entry| entry.owner == owner) {
                sessions[count] = binding.map(|entry| entry.session);
                count += 1;
                *binding = None;
            }
        }
    }
    let mut authority = AUTHORITY.lock();
    if let Some(authority) = authority.as_mut() {
        for session in sessions[..count].iter().flatten() {
            let _ = authority.close_session(*session);
        }
    }
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

pub(crate) fn open_digest(
    owner: u64,
    name: &[u8],
    digest: &[u8; 32],
) -> Result<SessionId, Error> {
    let binding_slot = BINDINGS
        .lock()
        .iter()
        .position(Option::is_none)
        .ok_or(Error::Full)?;
    let (session, view) = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        let session = authority
            .open_named_session_digest(name, digest)
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

pub(crate) fn unlock_digest(
    owner: u64,
    session: SessionId,
    digest: &[u8; 32],
) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        authority
            .unlock_digest(binding.session, digest)
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
        .position(|entry| {
            entry.is_some_and(|binding| binding.session == session && binding.owner == owner)
        })
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

#[cfg(any(feature = "stage24-4-test", feature = "stage24-5-test"))]
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

pub(crate) fn parse_length_prefixed(
    buffer: &[u8],
    maximum: usize,
) -> Option<(&[u8], &[u8])> {
    let (&length, rest) = buffer.split_first()?;
    let length = length as usize;
    if length == 0 || length > maximum || length > rest.len() {
        return None;
    }
    Some(rest.split_at(length))
}
