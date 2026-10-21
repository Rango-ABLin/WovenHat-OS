//! Owner-bound kernel identity sessions.
//!
//! The policy authority remains in `libwoven::identity`; this layer binds its
//! session tokens to a process ID and applies credentials only after the
//! identity lock is released. It is the kernel seam used by the forthcoming
//! userspace identity syscalls.

use core::sync::atomic::{AtomicBool, Ordering};
use libwoven::identity::{Authority, Error as IdentityError, SessionId, SessionView};

use crate::irq_lock::IrqMutex;
use crate::tpm2::CredentialDecision;

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
    hardware_generation: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LockoutEntry {
    name: [u8; libwoven::accounts::MAX_NAME],
    name_len: usize,
    failures: u8,
    locked_until: u64,
}

const MAX_BINDINGS: usize = libwoven::identity::MAX_SESSIONS;
const LOCKOUT_TICKS: u64 = 500;
static AUTHORITY: IrqMutex<Option<Authority>> = IrqMutex::with_rank(None, 4);
static BINDINGS: IrqMutex<[Option<Binding>; MAX_BINDINGS]> =
    IrqMutex::with_rank([None; MAX_BINDINGS], 5);
static PENDING_HARDWARE_DECISION: IrqMutex<Option<(u64, SessionId, CredentialDecision)>> =
    IrqMutex::with_rank(None, 7);
static LOCKOUTS: IrqMutex<[Option<LockoutEntry>; libwoven::identity::MAX_USERS]> =
    IrqMutex::with_rank([None; libwoven::identity::MAX_USERS], 6);
static LOCKOUT_SATURATED: AtomicBool = AtomicBool::new(false);

pub(crate) fn install(authority: Authority) {
    *AUTHORITY.lock() = Some(authority);
    BINDINGS.lock().fill(None);
    LOCKOUTS.lock().fill(None);
    LOCKOUT_SATURATED.store(false, Ordering::Release);
}

#[cfg(any(feature = "stage20-12-test", feature = "system-apps"))]
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
            .add_stored_user(
                account.name(),
                group,
                salt,
                hash,
                cfg!(feature = "hardware-demo") && count == 0,
            )
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
    if !BINDINGS
        .lock()
        .iter()
        .flatten()
        .any(|binding| binding.owner == owner)
    {
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
    if crate::accounts::change_pin_with_salt(name, old_pin, new_pin, &salt).is_ok() {
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
    let previous = *authority;
    let user = authority
        .admin_add_user(session, name, group, &salt, password, administrator)
        .map_err(Error::Identity)?;
    let (stored_salt, stored_hash) = authority
        .credential_material(name)
        .map_err(Error::Identity)?;
    if persist_authority(authority).is_err() {
        *authority = previous;
        return Err(Error::Storage);
    }
    if crate::accounts::add_stored_user(name, stored_salt, stored_hash).is_err() {
        #[cfg(feature = "hardware-demo")]
        {
            // The disposable hardware-demo image has no durable WovenFS
            // volume. Keep this fixture's live authority usable for UI
            // acceptance; production builds remain fail-closed below.
            return Ok(user);
        }
        #[cfg(not(feature = "hardware-demo"))]
        {
            let _ = persist_authority(&previous);
            *authority = previous;
            return Err(Error::Storage);
        }
    }
    Ok(user)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_delete_user(owner: u64, user: libwoven::identity::UserId) -> Result<(), Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    let authority = authority.as_mut().ok_or(Error::NotReady)?;
    let previous = *authority;
    let name = authority.user_name(user).map_err(Error::Identity)?;
    let mut stored_name = [0u8; libwoven::accounts::MAX_NAME];
    let name_len = name.len();
    stored_name[..name_len].copy_from_slice(name);
    authority
        .admin_delete_user(session, user)
        .map_err(Error::Identity)?;
    if persist_authority(authority).is_err() {
        *authority = previous;
        return Err(Error::Storage);
    }
    if crate::accounts::remove_user(&stored_name[..name_len]).is_err() {
        #[cfg(feature = "hardware-demo")]
        {
            return Ok(());
        }
        #[cfg(not(feature = "hardware-demo"))]
        {
            let _ = persist_authority(&previous);
            *authority = previous;
            return Err(Error::Storage);
        }
    }
    Ok(())
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_add_group(
    owner: u64,
    name: &[u8],
) -> Result<libwoven::identity::GroupId, Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    let authority = authority.as_mut().ok_or(Error::NotReady)?;
    let previous = *authority;
    let group = authority
        .admin_add_group(session, name)
        .map_err(Error::Identity)?;
    if persist_authority(authority).is_err() {
        *authority = previous;
        return Err(Error::Storage);
    }
    Ok(group)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_group_member(
    owner: u64,
    user: libwoven::identity::UserId,
    group: libwoven::identity::GroupId,
    add: bool,
) -> Result<(), Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    let authority = authority.as_mut().ok_or(Error::NotReady)?;
    let previous = *authority;
    let result = if add {
        authority.admin_add_group_member(session, user, group)
    } else {
        authority.admin_remove_group_member(session, user, group)
    };
    result.map_err(Error::Identity)?;
    if persist_authority(authority).is_err() {
        *authority = previous;
        return Err(Error::Storage);
    }
    Ok(())
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_query(owner: u64, kind: u8, output: &mut [u8]) -> Result<usize, Error> {
    let session = owner_session(owner)?;
    let authority = AUTHORITY.lock();
    authority
        .as_ref()
        .ok_or(Error::NotReady)?
        .admin_query(session, kind, output)
        .map_err(Error::Identity)
}

#[cfg(feature = "system-apps")]
pub(crate) fn admin_set_lockout(owner: u64, failures: u8) -> Result<(), Error> {
    let session = owner_session(owner)?;
    let mut authority = AUTHORITY.lock();
    let authority = authority.as_mut().ok_or(Error::NotReady)?;
    let previous = *authority;
    authority
        .admin_set_lockout_failures(session, failures)
        .map_err(Error::Identity)?;
    if persist_authority(authority).is_err() {
        *authority = previous;
        return Err(Error::Storage);
    }
    Ok(())
}

#[cfg(feature = "system-apps")]
fn persist_authority(authority: &Authority) -> Result<(), Error> {
    let mut snapshot = [0u8; libwoven::identity::SNAPSHOT_BYTES];
    authority
        .encode_snapshot(&mut snapshot)
        .map_err(Error::Identity)?;
    let writer = crate::wfs::AuthorizedWfsWriter::after_capability_check();
    match crate::identity_storage::persist(&writer, &snapshot, crate::timer::ticks()) {
        Ok(_) => Ok(()),
        Err(_) if cfg!(feature = "hardware-demo") => {
            // hardware-demo is a disposable no-disk fixture. Its authority
            // remains live in RAM, while production builds retain durable
            // publication as a hard requirement.
            crate::serial::write_line(format_args!(
                "[IDENTITY] durable authority unavailable; using volatile hardware-demo state"
            ));
            Ok(())
        }
        Err(_) => Err(Error::Storage),
    }
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
                note_failure(name, authority.lockout_failures());
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

pub(crate) fn open_digest(owner: u64, name: &[u8], digest: &[u8; 32]) -> Result<SessionId, Error> {
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
                note_failure(name, authority.lockout_failures());
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
                note_failure(
                    &binding.name[..binding.name_len],
                    authority.lockout_failures(),
                );
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
                note_failure(
                    &binding.name[..binding.name_len],
                    authority.lockout_failures(),
                );
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

/// Consume one move-only TPM credential decision for an already-owned live
/// identity session. The decision is bound to the account name used to create
/// its challenge; it cannot be redirected to another account or process.
/// Userspace never receives the decision object or any TPM-derived material.
pub(crate) fn consume_hardware_decision(
    owner: u64,
    session: SessionId,
    decision: CredentialDecision,
) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    if !decision.matches_context(&binding.name[..binding.name_len]) {
        return Err(Error::Identity(IdentityError::Unauthorized));
    }
    {
        let authority = AUTHORITY.lock();
        let authority = authority.as_ref().ok_or(Error::NotReady)?;
        let view = authority.view(binding.session).map_err(Error::Identity)?;
        if view.state != libwoven::identity::SessionState::Active {
            return Err(Error::Identity(IdentityError::Locked));
        }
    }
    let mut bindings = BINDINGS.lock();
    let entry = bindings
        .iter_mut()
        .flatten()
        .find(|entry| entry.session == session && entry.owner == owner)
        .ok_or(Error::Owner)?;
    if entry.hardware_generation.is_some() {
        return Err(Error::Identity(IdentityError::Unauthorized));
    }
    entry.hardware_generation = Some(decision.root_generation());
    Ok(())
}

/// Publish one verified decision for the owner/session that requested it.
/// This is a kernel-internal handoff from the TPM credential broker; the
/// pending slot is bounded to one entry and cannot be populated by userspace.
#[allow(dead_code)]
pub fn publish_hardware_decision(
    owner: u64,
    session: SessionId,
    decision: CredentialDecision,
) -> Result<(), Error> {
    let mut pending = PENDING_HARDWARE_DECISION.lock();
    if pending.is_some() {
        return Err(Error::Full);
    }
    *pending = Some((owner, session, decision));
    Ok(())
}

pub(crate) fn consume_pending_hardware_decision(
    owner: u64,
    session: SessionId,
) -> Result<(), Error> {
    let decision = {
        let mut pending = PENDING_HARDWARE_DECISION.lock();
        let Some((pending_owner, pending_session, decision)) = pending.take() else {
            return Err(Error::NotReady);
        };
        if pending_owner != owner || pending_session != session {
            *pending = Some((pending_owner, pending_session, decision));
            return Err(Error::Owner);
        }
        decision
    };
    consume_hardware_decision(owner, session, decision)
}

/// Ask the installed physical TPM broker to authenticate the exact account
/// bound to this live session. The account name is copied from the kernel
/// binding, never accepted from userspace.
#[cfg(feature = "system-apps")]
pub(crate) fn request_hardware_decision(owner: u64, session: SessionId) -> Result<(), Error> {
    let binding = owned_binding(owner, session)?;
    {
        let authority = AUTHORITY.lock();
        let authority = authority.as_ref().ok_or(Error::NotReady)?;
        let view = authority.view(binding.session).map_err(Error::Identity)?;
        if view.state != libwoven::identity::SessionState::Active {
            return Err(Error::Identity(IdentityError::Locked));
        }
    }
    crate::tpm2::issue_hardware_credential(owner, session, &binding.name[..binding.name_len])
        .map_err(|_| Error::NotReady)
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
        hardware_generation: None,
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

fn note_failure(name: &[u8], threshold: u8) {
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
    if entry.failures >= threshold {
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
    for _ in 0..libwoven::identity::DEFAULT_LOCKOUT_FAILURES {
        if !matches!(
            open(1, b"alice", b"wrong"),
            Err(Error::Identity(IdentityError::WrongPassword))
        ) {
            return false;
        }
    }
    if !matches!(
        open(1, b"alice", b"1234"),
        Err(Error::Identity(IdentityError::Locked))
    ) {
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

pub(crate) fn parse_length_prefixed(buffer: &[u8], maximum: usize) -> Option<(&[u8], &[u8])> {
    let (&length, rest) = buffer.split_first()?;
    let length = length as usize;
    if length == 0 || length > maximum || length > rest.len() {
        return None;
    }
    Some(rest.split_at(length))
}
