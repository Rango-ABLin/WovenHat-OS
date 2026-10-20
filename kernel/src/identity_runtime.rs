//! Owner-bound kernel identity sessions.
//!
//! The policy authority remains in `libwoven::identity`; this layer binds its
//! session tokens to a process ID and applies credentials only after the
//! identity lock is released. It is the kernel seam used by the forthcoming
//! userspace identity syscalls.

use libwoven::identity::{Authority, Error as IdentityError, SessionId, SessionView};
use core::sync::atomic::{AtomicBool, Ordering};

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
    name: [u8; libwoven::accounts::MAX_NAME],
    name_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LockoutEntry {
    name: [u8; libwoven::accounts::MAX_NAME],
    name_len: usize,
    failures: u8,
    locked_until: u64,
}

const MAX_BINDINGS: usize = libwoven::identity::MAX_SESSIONS;
const MAX_LOCKOUT_FAILURES: u8 = 5;
const LOCKOUT_TICKS: u64 = 500;
static AUTHORITY: IrqMutex<Option<Authority>> = IrqMutex::with_rank(None, 4);
static BINDINGS: IrqMutex<[Option<Binding>; MAX_BINDINGS]> =
    IrqMutex::with_rank([None; MAX_BINDINGS], 5);
static LOCKOUTS: IrqMutex<[Option<LockoutEntry>; libwoven::identity::MAX_USERS]> =
    IrqMutex::with_rank([None; libwoven::identity::MAX_USERS], 6);
static LOCKOUT_SATURATED: AtomicBool = AtomicBool::new(false);

pub(crate) fn install(authority: Authority) {
    *AUTHORITY.lock() = Some(authority);
    BINDINGS.lock().fill(None);
    LOCKOUTS.lock().fill(None);
    LOCKOUT_SATURATED.store(false, Ordering::Release);
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

#[cfg(feature = "system-apps")]
pub(crate) fn change_pin(
    owner: u64,
    name: &[u8],
    old_pin: &[u8],
    new_pin: &[u8],
) -> Result<(), Error> {
    if !BINDINGS.lock().iter().flatten().any(|binding| binding.owner == owner) {
        return Err(Error::Owner);
    }
    let mut salt = [0u8; 16];
    for chunk in salt.chunks_mut(8) {
        chunk.copy_from_slice(&crate::entropy::random_u64().to_le_bytes()[..chunk.len()]);
    }
    {
        let mut authority = AUTHORITY.lock();
        authority
            .as_mut()
            .ok_or(Error::NotReady)?
            .change_password(name, old_pin, &salt, new_pin)
            .map_err(Error::Identity)?;
    }
    if crate::accounts::change_pin(name, old_pin, new_pin).is_ok() {
        return Ok(());
    }
    let mut authority = AUTHORITY.lock();
    if let Some(authority) = authority.as_mut() {
        let _ = authority.change_password(name, new_pin, &salt, old_pin);
    }
    Err(Error::Storage)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_add_user(
    owner: u64,
    name: &[u8],
    password: &[u8],
    administrator: bool,
) -> Result<libwoven::identity::UserId, Error> {
    let session = owner_session(owner)?;
    let mut salt = [0u8; 16];
    for chunk in salt.chunks_mut(8) {
        chunk.copy_from_slice(&crate::entropy::random_u64().to_le_bytes()[..chunk.len()]);
    }
    let mut authority = AUTHORITY.lock();
    let authority = authority.as_mut().ok_or(Error::NotReady)?;
    let group = authority.group_id(b"users").map_err(Error::Identity)?;
    authority
        .admin_add_user(session, name, group, &salt, password, administrator)
        .map_err(Error::Identity)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_delete_user(owner: u64, user: libwoven::identity::UserId) -> Result<(), Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    authority
        .as_mut()
        .ok_or(Error::NotReady)?
        .admin_delete_user(session, user)
        .map_err(Error::Identity)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_add_group(
    owner: u64,
    name: &[u8],
) -> Result<libwoven::identity::GroupId, Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    authority
        .as_mut()
        .ok_or(Error::NotReady)?
        .admin_add_group(session, name)
        .map_err(Error::Identity)
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
    check_lockout(name)?;
    let binding_slot = BINDINGS
        .lock()
        .iter()
        .position(Option::is_none)
        .ok_or(Error::Full)?;
    let (session, view) = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        let session = match authority.open_named_session(name, password) {
            Ok(session) => session,
            Err(IdentityError::WrongPassword) => {
                note_failure(name);
                return Err(Error::Identity(IdentityError::WrongPassword));
            }
            Err(error) => return Err(Error::Identity(error)),
        };
        let view = authority.view(session).map_err(Error::Identity)?;
        (session, view)
    };
    clear_failures(name);
    BINDINGS.lock()[binding_slot] = Some(binding(session, owner, name));
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
    check_lockout(name)?;
    let binding_slot = BINDINGS
        .lock()
        .iter()
        .position(Option::is_none)
        .ok_or(Error::Full)?;
    let (session, view) = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        let session = match authority.open_named_session_digest(name, digest) {
            Ok(session) => session,
            Err(IdentityError::WrongPassword) => {
                note_failure(name);
                return Err(Error::Identity(IdentityError::WrongPassword));
            }
            Err(error) => return Err(Error::Identity(error)),
        };
        let view = authority.view(session).map_err(Error::Identity)?;
        (session, view)
    };
    clear_failures(name);
    BINDINGS.lock()[binding_slot] = Some(binding(session, owner, name));
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
    check_lockout(&binding.name[..binding.name_len])?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        match authority.unlock(binding.session, password) {
            Ok(()) => {}
            Err(IdentityError::WrongPassword) => {
                note_failure(&binding.name[..binding.name_len]);
                return Err(Error::Identity(IdentityError::WrongPassword));
            }
            Err(error) => return Err(Error::Identity(error)),
        }
        authority.view(binding.session).map_err(Error::Identity)?
    };
    clear_failures(&binding.name[..binding.name_len]);
    apply_view(owner, view)
}

pub(crate) fn unlock_digest(
    owner: u64,
    session: SessionId,
    digest: &[u8; 32],
) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    check_lockout(&binding.name[..binding.name_len])?;
    let view = {
        let mut authority = AUTHORITY.lock();
        let authority = authority.as_mut().ok_or(Error::NotReady)?;
        match authority.unlock_digest(binding.session, digest) {
            Ok(()) => {}
            Err(IdentityError::WrongPassword) => {
                note_failure(&binding.name[..binding.name_len]);
                return Err(Error::Identity(IdentityError::WrongPassword));
            }
            Err(error) => return Err(Error::Identity(error)),
        }
        authority.view(binding.session).map_err(Error::Identity)?
    };
    clear_failures(&binding.name[..binding.name_len]);
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

#[cfg(feature = "system-apps")]
fn owner_session(owner: u64) -> Result<SessionId, Error> {
    BINDINGS
        .lock()
        .iter()
        .flatten()
        .find(|binding| binding.owner == owner)
        .map(|binding| binding.session)
        .ok_or(Error::Owner)
}

fn binding(session: SessionId, owner: u64, name: &[u8]) -> Binding {
    let mut stored = [0u8; libwoven::accounts::MAX_NAME];
    stored[..name.len()].copy_from_slice(name);
    Binding {
        session,
        owner,
        name: stored,
        name_len: name.len(),
    }
}

fn check_lockout(name: &[u8]) -> Result<(), Error> {
    let now = crate::timer::ticks();
    if LOCKOUT_SATURATED.load(Ordering::Acquire) {
        return Err(Error::Identity(IdentityError::Locked));
    }
    let mut lockouts = LOCKOUTS.lock();
    let Some(entry) = lockouts
        .iter_mut()
        .flatten()
        .find(|entry| entry.name[..entry.name_len] == *name)
    else {
        return Ok(());
    };
    if entry.locked_until > now {
        return Err(Error::Identity(IdentityError::Locked));
    }
    if entry.locked_until != 0 {
        entry.failures = 0;
        entry.locked_until = 0;
    }
    Ok(())
}

fn note_failure(name: &[u8]) {
    let now = crate::timer::ticks();
    let mut lockouts = LOCKOUTS.lock();
    let entry = if let Some(entry) = lockouts
        .iter_mut()
        .flatten()
        .find(|entry| entry.name[..entry.name_len] == *name)
    {
        entry
    } else if let Some(slot) = lockouts.iter_mut().find(|entry| entry.is_none()) {
        let mut stored = [0u8; libwoven::accounts::MAX_NAME];
        stored[..name.len()].copy_from_slice(name);
        *slot = Some(LockoutEntry {
            name: stored,
            name_len: name.len(),
            failures: 0,
            locked_until: 0,
        });
        slot.as_mut().expect("just initialized lockout entry")
    } else {
        LOCKOUT_SATURATED.store(true, Ordering::Release);
        return;
    };
    entry.failures = entry.failures.saturating_add(1);
    if entry.failures >= MAX_LOCKOUT_FAILURES {
        entry.locked_until = now.saturating_add(LOCKOUT_TICKS);
    }
}

fn clear_failures(name: &[u8]) {
    if let Some(entry) = LOCKOUTS
        .lock()
        .iter_mut()
        .flatten()
        .find(|entry| entry.name[..entry.name_len] == *name)
    {
        entry.failures = 0;
        entry.locked_until = 0;
    }
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
    for _ in 0..MAX_LOCKOUT_FAILURES {
        if !matches!(open(1, b"alice", b"wrong"), Err(Error::Identity(IdentityError::WrongPassword))) {
            return false;
        }
    }
    if !matches!(open(1, b"alice", b"1234"), Err(Error::Identity(IdentityError::Locked))) {
        return false;
    }
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
