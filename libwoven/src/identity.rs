//! Bounded local identity authority for WovenIdentity.
//!
//! This module owns identity state without an allocator: users, groups,
//! sessions, lock state, profiles, and credential hashes all have explicit
//! limits.  It is deliberately a policy core, not a process credential
//! switch; the kernel must still authorize any effective-credential change
//! and audit it before integrating this with task creation.

use crate::accounts::{MAX_NAME, MAX_PIN, hash_pin};

pub const MAX_USERS: usize = 16;
pub const MAX_GROUPS: usize = 16;
pub const MAX_GROUP_MEMBERS: usize = 8;
pub const MAX_SESSIONS: usize = 16;
pub const MAX_PROFILE_BYTES: usize = 64;
pub const MAX_SALT_BYTES: usize = 32;
pub const DEFAULT_LOCKOUT_FAILURES: u8 = 5;
pub const MAX_LOCKOUT_FAILURES: u8 = 32;
pub const ADMIN_QUERY_USERS: u8 = 0;
pub const ADMIN_QUERY_GROUPS: u8 = 1;
pub const ADMIN_QUERY_VERSION: u8 = 1;
pub const ADMIN_QUERY_HEADER_BYTES: usize = 4;
pub const ADMIN_USER_RECORD_BYTES: usize = 4 + 4 + 4 + 1 + 1 + MAX_NAME;
pub const ADMIN_GROUP_RECORD_BYTES: usize = 4 + 1 + MAX_NAME + 1 + MAX_GROUP_MEMBERS * 4;
pub const ADMIN_QUERY_MAX_BYTES: usize =
    ADMIN_QUERY_HEADER_BYTES + MAX_USERS * ADMIN_GROUP_RECORD_BYTES;
pub const SNAPSHOT_BYTES: usize = 4
    + 1
    + 1
    + MAX_USERS * (4 + 4 + 4 + 1 + MAX_NAME + 1 + MAX_SALT_BYTES + 32 + 1 + 1 + MAX_PROFILE_BYTES)
    + MAX_GROUPS * (4 + 1 + MAX_NAME + MAX_GROUP_MEMBERS * 4)
    + 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UserId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GroupId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SessionId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    Full,
    Invalid,
    NotFound,
    AlreadyExists,
    Locked,
    WrongPassword,
    Unauthorized,
    BufferTooSmall,
    Corrupt,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
    Active,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UserProfile {
    display_name: [u8; MAX_PROFILE_BYTES],
    display_name_len: usize,
}

impl UserProfile {
    pub const EMPTY: Self = Self {
        display_name: [0; MAX_PROFILE_BYTES],
        display_name_len: 0,
    };

    pub fn new(display_name: &[u8]) -> Result<Self, Error> {
        if display_name.len() > MAX_PROFILE_BYTES
            || !display_name.iter().all(|byte| (b' '..=b'~').contains(byte))
        {
            return Err(Error::Invalid);
        }
        let mut profile = Self::EMPTY;
        profile.display_name[..display_name.len()].copy_from_slice(display_name);
        profile.display_name_len = display_name.len();
        Ok(profile)
    }

    pub fn display_name(&self) -> &[u8] {
        &self.display_name[..self.display_name_len]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Credential {
    salt: [u8; MAX_SALT_BYTES],
    salt_len: usize,
    hash: [u8; 32],
}

impl Credential {
    fn new(salt: &[u8], password: &[u8]) -> Result<Self, Error> {
        if salt.is_empty()
            || salt.len() > MAX_SALT_BYTES
            || password.is_empty()
            || password.len() > MAX_PIN
        {
            return Err(Error::Invalid);
        }
        let mut credential = Self {
            salt: [0; MAX_SALT_BYTES],
            salt_len: salt.len(),
            hash: [0; 32],
        };
        credential.salt[..salt.len()].copy_from_slice(salt);
        credential.hash = hash_pin(salt, password);
        Ok(credential)
    }

    fn verify(&self, password: &[u8]) -> bool {
        if password.is_empty() || password.len() > MAX_PIN {
            return false;
        }
        let candidate = hash_pin(&self.salt[..self.salt_len], password);
        let mut difference = 0u8;
        for (candidate, stored) in candidate.iter().zip(self.hash.iter()) {
            difference |= candidate ^ stored;
        }
        difference == 0
    }

    fn verify_digest(&self, digest: &[u8; 32]) -> bool {
        let mut difference = 0u8;
        for (candidate, stored) in digest.iter().zip(self.hash.iter()) {
            difference |= candidate ^ stored;
        }
        difference == 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct User {
    id: UserId,
    uid: u32,
    gid: GroupId,
    name: [u8; MAX_NAME],
    name_len: usize,
    credential: Credential,
    profile: UserProfile,
    administrator: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Group {
    id: GroupId,
    name: [u8; MAX_NAME],
    name_len: usize,
    members: [Option<UserId>; MAX_GROUP_MEMBERS],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Session {
    id: SessionId,
    user: UserId,
    state: SessionState,
    elevated: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SessionView {
    pub id: SessionId,
    pub user: UserId,
    pub uid: u32,
    pub gid: GroupId,
    pub state: SessionState,
    pub elevated: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdminUserRecord {
    pub id: UserId,
    pub uid: u32,
    pub gid: GroupId,
    pub administrator: bool,
    name: [u8; MAX_NAME],
    name_len: usize,
}

impl AdminUserRecord {
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdminGroupRecord {
    pub id: GroupId,
    name: [u8; MAX_NAME],
    name_len: usize,
    pub members: [Option<UserId>; MAX_GROUP_MEMBERS],
}

impl AdminGroupRecord {
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdminQuery<'a> {
    kind: u8,
    records: &'a [u8],
    count: usize,
}

impl<'a> AdminQuery<'a> {
    pub fn decode(input: &'a [u8]) -> Result<Self, Error> {
        if input.len() < ADMIN_QUERY_HEADER_BYTES
            || input[0] != ADMIN_QUERY_VERSION
            || input[1] > ADMIN_QUERY_GROUPS
        {
            return Err(Error::Corrupt);
        }
        let kind = input[1];
        let count = u16::from_le_bytes([input[2], input[3]]) as usize;
        let record_bytes = if kind == ADMIN_QUERY_USERS {
            ADMIN_USER_RECORD_BYTES
        } else {
            ADMIN_GROUP_RECORD_BYTES
        };
        if count > MAX_USERS.max(MAX_GROUPS)
            || ADMIN_QUERY_HEADER_BYTES
                .checked_add(count.checked_mul(record_bytes).ok_or(Error::Corrupt)?)
                != Some(input.len())
        {
            return Err(Error::Corrupt);
        }
        Ok(Self {
            kind,
            records: &input[ADMIN_QUERY_HEADER_BYTES..],
            count,
        })
    }

    pub const fn kind(&self) -> u8 {
        self.kind
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn user(&self, index: usize) -> Option<AdminUserRecord> {
        if self.kind != ADMIN_QUERY_USERS || index >= self.count {
            return None;
        }
        let start = index * ADMIN_USER_RECORD_BYTES;
        let record = self.records.get(start..start + ADMIN_USER_RECORD_BYTES)?;
        let name_len = record[13] as usize;
        if name_len > MAX_NAME || record[12] > 1 {
            return None;
        }
        Some(AdminUserRecord {
            id: UserId(u32::from_le_bytes(record[0..4].try_into().ok()?)),
            uid: u32::from_le_bytes(record[4..8].try_into().ok()?),
            gid: GroupId(u32::from_le_bytes(record[8..12].try_into().ok()?)),
            administrator: record[12] != 0,
            name: record[14..14 + MAX_NAME].try_into().ok()?,
            name_len,
        })
    }

    pub fn group(&self, index: usize) -> Option<AdminGroupRecord> {
        if self.kind != ADMIN_QUERY_GROUPS || index >= self.count {
            return None;
        }
        let start = index * ADMIN_GROUP_RECORD_BYTES;
        let record = self.records.get(start..start + ADMIN_GROUP_RECORD_BYTES)?;
        let name_len = record[4] as usize;
        let member_count = record[5 + MAX_NAME] as usize;
        if name_len > MAX_NAME || member_count > MAX_GROUP_MEMBERS {
            return None;
        }
        let mut members = [None; MAX_GROUP_MEMBERS];
        for (index, member) in members.iter_mut().enumerate() {
            let offset = 6 + MAX_NAME + index * 4;
            let id = u32::from_le_bytes(record[offset..offset + 4].try_into().ok()?);
            *member = (id != 0).then_some(UserId(id));
        }
        Some(AdminGroupRecord {
            id: GroupId(u32::from_le_bytes(record[0..4].try_into().ok()?)),
            name: record[5..5 + MAX_NAME].try_into().ok()?,
            name_len,
            members,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Authority {
    users: [Option<User>; MAX_USERS],
    groups: [Option<Group>; MAX_GROUPS],
    sessions: [Option<Session>; MAX_SESSIONS],
    next_user: u32,
    next_group: u32,
    next_session: u64,
    lockout_failures: u8,
}

impl Authority {
    pub const fn new() -> Self {
        Self {
            users: [None; MAX_USERS],
            groups: [None; MAX_GROUPS],
            sessions: [None; MAX_SESSIONS],
            next_user: 1000,
            next_group: 1000,
            next_session: 1,
            lockout_failures: DEFAULT_LOCKOUT_FAILURES,
        }
    }

    pub fn add_group(&mut self, name: &[u8]) -> Result<GroupId, Error> {
        let (name, name_len) = bounded_name(name)?;
        if self
            .groups
            .iter()
            .flatten()
            .any(|group| group.name[..group.name_len] == name[..name_len])
        {
            return Err(Error::AlreadyExists);
        }
        let slot = self
            .groups
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        let id = GroupId(self.next_group);
        self.next_group = self.next_group.checked_add(1).ok_or(Error::Full)?;
        self.groups[slot] = Some(Group {
            id,
            name,
            name_len,
            members: [None; MAX_GROUP_MEMBERS],
        });
        Ok(id)
    }

    pub fn add_user(
        &mut self,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        password: &[u8],
    ) -> Result<UserId, Error> {
        self.add_user_with_role(name, gid, salt, password, false)
    }

    pub fn add_administrator(
        &mut self,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        password: &[u8],
    ) -> Result<UserId, Error> {
        self.add_user_with_role(name, gid, salt, password, true)
    }

    /// Import an already-derived credential without accepting plaintext.
    pub fn add_stored_user(
        &mut self,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        hash: &[u8; 32],
        administrator: bool,
    ) -> Result<UserId, Error> {
        let (salt, salt_len) = bounded_salt(salt)?;
        self.add_user_with_credential(
            name,
            gid,
            Credential {
                salt,
                salt_len,
                hash: *hash,
            },
            administrator,
        )
    }

    /// Replace one user's credential after verifying the current password.
    /// The caller supplies the new bounded salt so the policy core remains
    /// deterministic and does not depend on a platform RNG.
    pub fn change_password(
        &mut self,
        name: &[u8],
        old_password: &[u8],
        salt: &[u8],
        new_password: &[u8],
    ) -> Result<(), Error> {
        let user = self
            .users
            .iter()
            .flatten()
            .find(|user| user.name[..user.name_len] == *name)
            .copied()
            .ok_or(Error::NotFound)?;
        if !user.credential.verify(old_password) {
            return Err(Error::WrongPassword);
        }
        let credential = Credential::new(salt, new_password)?;
        self.find_user_mut(user.id)
            .ok_or(Error::NotFound)?
            .credential = credential;
        Ok(())
    }

    fn add_user_with_role(
        &mut self,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        password: &[u8],
        administrator: bool,
    ) -> Result<UserId, Error> {
        let credential = Credential::new(salt, password)?;
        self.add_user_with_credential(name, gid, credential, administrator)
    }

    fn add_user_with_credential(
        &mut self,
        name: &[u8],
        gid: GroupId,
        credential: Credential,
        administrator: bool,
    ) -> Result<UserId, Error> {
        let (name, name_len) = bounded_name(name)?;
        if self.find_group(gid).is_none()
            || !self.group_has_member_slot(gid)
            || self
                .users
                .iter()
                .flatten()
                .any(|user| user.name[..user.name_len] == name[..name_len])
        {
            return Err(if self.find_group(gid).is_none() {
                Error::NotFound
            } else if !self.group_has_member_slot(gid) {
                Error::Full
            } else {
                Error::AlreadyExists
            });
        }
        let slot = self
            .users
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        let id = UserId(self.next_user);
        self.next_user = self.next_user.checked_add(1).ok_or(Error::Full)?;
        self.users[slot] = Some(User {
            id,
            uid: id.0,
            gid,
            name,
            name_len,
            credential,
            profile: UserProfile::EMPTY,
            administrator,
        });
        self.add_member(gid, id)?;
        Ok(id)
    }

    pub fn set_profile(&mut self, user: UserId, profile: UserProfile) -> Result<(), Error> {
        self.find_user_mut(user).ok_or(Error::NotFound)?.profile = profile;
        Ok(())
    }

    /// Resolve a bounded group name for an administrative caller.
    pub fn group_id(&self, name: &[u8]) -> Result<GroupId, Error> {
        self.groups
            .iter()
            .flatten()
            .find(|group| group.name[..group.name_len] == *name)
            .map(|group| group.id)
            .ok_or(Error::NotFound)
    }

    pub fn user_name(&self, user: UserId) -> Result<&[u8], Error> {
        let user = self.find_user(user).ok_or(Error::NotFound)?;
        Ok(&user.name[..user.name_len])
    }

    pub fn credential_material(&self, name: &[u8]) -> Result<(&[u8], &[u8; 32]), Error> {
        let user = self
            .users
            .iter()
            .flatten()
            .find(|user| user.name[..user.name_len] == *name)
            .ok_or(Error::NotFound)?;
        Ok((
            &user.credential.salt[..user.credential.salt_len],
            &user.credential.hash,
        ))
    }

    /// Create a user only from an active, elevated administrator session.
    pub fn admin_add_user(
        &mut self,
        session: SessionId,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        password: &[u8],
        administrator: bool,
    ) -> Result<UserId, Error> {
        self.require_administrator(session)?;
        self.add_user_with_role(name, gid, salt, password, administrator)
    }

    /// Return the durable maximum failed-authentication count before the
    /// kernel imposes its bounded cooldown.
    pub fn lockout_failures(&self) -> u8 {
        self.lockout_failures
    }

    /// Change the lockout threshold through an active elevated administrator
    /// session. The cooldown duration remains a kernel time-base policy.
    pub fn admin_set_lockout_failures(
        &mut self,
        session: SessionId,
        failures: u8,
    ) -> Result<(), Error> {
        self.require_administrator(session)?;
        if !(1..=MAX_LOCKOUT_FAILURES).contains(&failures) {
            return Err(Error::Invalid);
        }
        self.lockout_failures = failures;
        Ok(())
    }

    /// Add a group only from an active, elevated administrator session.
    pub fn admin_add_group(&mut self, session: SessionId, name: &[u8]) -> Result<GroupId, Error> {
        self.require_administrator(session)?;
        self.add_group(name)
    }

    pub fn admin_add_group_member(
        &mut self,
        session: SessionId,
        user: UserId,
        gid: GroupId,
    ) -> Result<(), Error> {
        self.require_administrator(session)?;
        if self.find_user(user).is_none() || self.find_group(gid).is_none() {
            return Err(Error::NotFound);
        }
        let group = self.find_group_mut(gid).ok_or(Error::NotFound)?;
        if group.members.iter().flatten().any(|member| *member == user) {
            return Err(Error::AlreadyExists);
        }
        let slot = group
            .members
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        group.members[slot] = Some(user);
        Ok(())
    }

    pub fn admin_remove_group_member(
        &mut self,
        session: SessionId,
        user: UserId,
        gid: GroupId,
    ) -> Result<(), Error> {
        self.require_administrator(session)?;
        let account = self.find_user(user).ok_or(Error::NotFound)?;
        if account.gid == gid {
            return Err(Error::Invalid);
        }
        let group = self.find_group_mut(gid).ok_or(Error::NotFound)?;
        let slot = group
            .members
            .iter()
            .position(|member| *member == Some(user))
            .ok_or(Error::NotFound)?;
        group.members[slot] = None;
        Ok(())
    }

    /// Encode non-secret account or group records for an elevated administrator.
    /// Credentials, profiles, and active session tokens never cross this
    /// boundary. The output is a bounded versioned record list.
    pub fn admin_query(
        &self,
        session: SessionId,
        kind: u8,
        output: &mut [u8],
    ) -> Result<usize, Error> {
        self.require_administrator(session)?;
        let (record_bytes, count) = match kind {
            ADMIN_QUERY_USERS => (ADMIN_USER_RECORD_BYTES, self.users.iter().flatten().count()),
            ADMIN_QUERY_GROUPS => (
                ADMIN_GROUP_RECORD_BYTES,
                self.groups.iter().flatten().count(),
            ),
            _ => return Err(Error::Invalid),
        };
        let required = ADMIN_QUERY_HEADER_BYTES
            .checked_add(
                count
                    .checked_mul(record_bytes)
                    .ok_or(Error::BufferTooSmall)?,
            )
            .ok_or(Error::BufferTooSmall)?;
        if output.len() < required {
            return Err(Error::BufferTooSmall);
        }
        let mut writer = SnapshotWriter::new(&mut output[..required]);
        writer.u8(ADMIN_QUERY_VERSION)?;
        writer.u8(kind)?;
        writer.fixed(&(count as u16).to_le_bytes())?;
        match kind {
            ADMIN_QUERY_USERS => {
                for user in self.users.iter().flatten() {
                    writer.u32(user.id.0)?;
                    writer.u32(user.uid)?;
                    writer.u32(user.gid.0)?;
                    writer.u8(u8::from(user.administrator))?;
                    writer.u8(user.name_len as u8)?;
                    writer.fixed(&user.name)?;
                }
            }
            ADMIN_QUERY_GROUPS => {
                for group in self.groups.iter().flatten() {
                    writer.u32(group.id.0)?;
                    writer.u8(group.name_len as u8)?;
                    writer.fixed(&group.name)?;
                    writer.u8(group.members.iter().flatten().count() as u8)?;
                    for member in group.members {
                        writer.u32(member.map_or(0, |user| user.0))?;
                    }
                }
            }
            _ => return Err(Error::Invalid),
        }
        Ok(required)
    }

    /// Delete an account only from an active, elevated administrator session.
    /// Active sessions and the administrator's own account cannot be removed.
    pub fn admin_delete_user(&mut self, session: SessionId, target: UserId) -> Result<(), Error> {
        let administrator = self.require_administrator(session)?;
        if administrator == target {
            return Err(Error::Unauthorized);
        }
        if self
            .sessions
            .iter()
            .flatten()
            .any(|active| active.user == target)
        {
            return Err(Error::Locked);
        }
        if self.find_user(target).is_none() {
            return Err(Error::NotFound);
        }
        for group in self.groups.iter_mut().flatten() {
            for member in &mut group.members {
                if *member == Some(target) {
                    *member = None;
                }
            }
        }
        let slot = self
            .users
            .iter()
            .position(|user| user.is_some_and(|user| user.id == target))
            .ok_or(Error::NotFound)?;
        self.users[slot] = None;
        Ok(())
    }

    fn require_administrator(&self, session: SessionId) -> Result<UserId, Error> {
        let current = self.find_session(session).ok_or(Error::NotFound)?;
        if current.state != SessionState::Active || !current.elevated {
            return Err(Error::Unauthorized);
        }
        let user = self.find_user(current.user).ok_or(Error::NotFound)?;
        user.administrator
            .then_some(user.id)
            .ok_or(Error::Unauthorized)
    }

    pub fn open_session(&mut self, user: UserId, password: &[u8]) -> Result<SessionId, Error> {
        let account = self.find_user(user).ok_or(Error::NotFound)?;
        if !account.credential.verify(password) {
            return Err(Error::WrongPassword);
        }
        let slot = self
            .sessions
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        let id = SessionId(self.next_session);
        self.next_session = self.next_session.checked_add(1).ok_or(Error::Full)?;
        self.sessions[slot] = Some(Session {
            id,
            user,
            state: SessionState::Active,
            elevated: false,
        });
        Ok(id)
    }

    /// Authenticate a local account by its bounded display name.
    pub fn open_named_session(&mut self, name: &[u8], password: &[u8]) -> Result<SessionId, Error> {
        let user = self
            .users
            .iter()
            .flatten()
            .find(|user| user.name[..user.name_len] == *name)
            .ok_or(Error::NotFound)?
            .id;
        self.open_session(user, password)
    }

    /// Open a session after a trusted login manager has verified the PIN.
    /// The authority still compares the supplied digest to its own record.
    pub fn open_named_session_digest(
        &mut self,
        name: &[u8],
        digest: &[u8; 32],
    ) -> Result<SessionId, Error> {
        let user = self
            .users
            .iter()
            .flatten()
            .find(|user| user.name[..user.name_len] == *name)
            .ok_or(Error::NotFound)?;
        if !user.credential.verify_digest(digest) {
            return Err(Error::WrongPassword);
        }
        self.open_session_unchecked(user.id)
    }

    fn open_session_unchecked(&mut self, user: UserId) -> Result<SessionId, Error> {
        let slot = self
            .sessions
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        let id = SessionId(self.next_session);
        self.next_session = self.next_session.checked_add(1).ok_or(Error::Full)?;
        self.sessions[slot] = Some(Session {
            id,
            user,
            state: SessionState::Active,
            elevated: false,
        });
        Ok(id)
    }

    pub fn lock(&mut self, session: SessionId) -> Result<(), Error> {
        let session = self.find_session_mut(session).ok_or(Error::NotFound)?;
        session.state = SessionState::Locked;
        session.elevated = false;
        Ok(())
    }

    pub fn unlock(&mut self, session: SessionId, password: &[u8]) -> Result<(), Error> {
        let user = self.find_session(session).ok_or(Error::NotFound)?.user;
        if !self
            .find_user(user)
            .ok_or(Error::NotFound)?
            .credential
            .verify(password)
        {
            return Err(Error::WrongPassword);
        }
        let session = self.find_session_mut(session).ok_or(Error::NotFound)?;
        session.state = SessionState::Active;
        Ok(())
    }

    pub fn unlock_digest(&mut self, session: SessionId, digest: &[u8; 32]) -> Result<(), Error> {
        let user = self.find_session(session).ok_or(Error::NotFound)?.user;
        if !self
            .find_user(user)
            .ok_or(Error::NotFound)?
            .credential
            .verify_digest(digest)
        {
            return Err(Error::WrongPassword);
        }
        let session = self.find_session_mut(session).ok_or(Error::NotFound)?;
        session.state = SessionState::Active;
        Ok(())
    }

    pub fn elevate(&mut self, session: SessionId, password: &[u8]) -> Result<(), Error> {
        let current = self.find_session(session).ok_or(Error::NotFound)?;
        if current.state != SessionState::Active {
            return Err(Error::Locked);
        }
        let user = self.find_user(current.user).ok_or(Error::NotFound)?;
        if !user.administrator {
            return Err(Error::Unauthorized);
        }
        if !user.credential.verify(password) {
            return Err(Error::WrongPassword);
        }
        self.find_session_mut(session)
            .ok_or(Error::NotFound)?
            .elevated = true;
        Ok(())
    }

    pub fn close_session(&mut self, session: SessionId) -> Result<(), Error> {
        let slot = self
            .sessions
            .iter()
            .position(|entry| entry.is_some_and(|value| value.id == session))
            .ok_or(Error::NotFound)?;
        self.sessions[slot] = None;
        Ok(())
    }

    pub fn view(&self, session: SessionId) -> Result<SessionView, Error> {
        let session = self.find_session(session).ok_or(Error::NotFound)?;
        let user = self.find_user(session.user).ok_or(Error::NotFound)?;
        Ok(SessionView {
            id: session.id,
            user: user.id,
            uid: if session.elevated { 0 } else { user.uid },
            gid: user.gid,
            state: session.state,
            elevated: session.elevated,
        })
    }

    /// Encode users and groups into an authenticated fixed-size snapshot.
    /// Active sessions are intentionally excluded: authentication never
    /// survives a reboot. The checksum is FNV-1a over the header and records.
    pub fn encode_snapshot(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < SNAPSHOT_BYTES {
            return Err(Error::BufferTooSmall);
        }
        let position = {
            let mut writer = SnapshotWriter::new(output);
            writer.bytes(b"WID1")?;
            writer.u8(2)?;
            writer.u8(self.users.iter().flatten().count() as u8)?;
            writer.u8(self.groups.iter().flatten().count() as u8)?;
            writer.u8(self.lockout_failures)?;
            for user in self.users.iter().flatten() {
                writer.u32(user.id.0)?;
                writer.u32(user.uid)?;
                writer.u32(user.gid.0)?;
                writer.u8(user.name_len as u8)?;
                writer.fixed(&user.name)?;
                writer.u8(user.credential.salt_len as u8)?;
                writer.fixed(&user.credential.salt)?;
                writer.fixed(&user.credential.hash)?;
                writer.u8(u8::from(user.administrator))?;
                writer.u8(user.profile.display_name_len as u8)?;
                writer.fixed(&user.profile.display_name)?;
            }
            for group in self.groups.iter().flatten() {
                writer.u32(group.id.0)?;
                writer.u8(group.name_len as u8)?;
                writer.fixed(&group.name)?;
                for member in group.members {
                    writer.u32(member.map_or(0, |user| user.0))?;
                }
            }
            writer.position
        };
        output[position..SNAPSHOT_BYTES - 4].fill(0);
        let checksum = fnv1a(&output[..SNAPSHOT_BYTES - 4]);
        output[SNAPSHOT_BYTES - 4..].copy_from_slice(&checksum.to_le_bytes());
        Ok(SNAPSHOT_BYTES)
    }

    /// Decode and authenticate a snapshot into a fresh authority. No existing
    /// authority state is changed when validation fails.
    pub fn decode_snapshot(input: &[u8]) -> Result<Self, Error> {
        if input.len() != SNAPSHOT_BYTES {
            return Err(Error::Corrupt);
        }
        let stored_checksum = u32::from_le_bytes(input[SNAPSHOT_BYTES - 4..].try_into().unwrap());
        if fnv1a(&input[..SNAPSHOT_BYTES - 4]) != stored_checksum {
            return Err(Error::Corrupt);
        }
        let mut reader = SnapshotReader::new(&input[..SNAPSHOT_BYTES - 4]);
        if reader.bytes(4)? != b"WID1" {
            return Err(Error::Corrupt);
        }
        let version = reader.u8()?;
        let user_count = reader.u8()? as usize;
        let group_count = reader.u8()? as usize;
        let lockout_failures = reader.u8()?;
        let lockout_failures = match version {
            1 if lockout_failures == 0 => DEFAULT_LOCKOUT_FAILURES,
            2 if (1..=MAX_LOCKOUT_FAILURES).contains(&lockout_failures) => lockout_failures,
            _ => return Err(Error::Corrupt),
        };
        if user_count > MAX_USERS || group_count > MAX_GROUPS {
            return Err(Error::Corrupt);
        }
        let mut authority = Self::new();
        authority.lockout_failures = lockout_failures;
        for slot in 0..user_count {
            let id = UserId(reader.u32()?);
            let uid = reader.u32()?;
            let gid = GroupId(reader.u32()?);
            let name_len = reader.u8()? as usize;
            let name = read_fixed::<MAX_NAME>(&mut reader)?;
            let salt_len = reader.u8()? as usize;
            let salt = read_fixed::<MAX_SALT_BYTES>(&mut reader)?;
            let hash = read_fixed::<32>(&mut reader)?;
            let administrator = match reader.u8()? {
                0 => false,
                1 => true,
                _ => return Err(Error::Corrupt),
            };
            let profile_len = reader.u8()? as usize;
            let profile_name = read_fixed::<MAX_PROFILE_BYTES>(&mut reader)?;
            if id.0 == 0
                || uid == 0
                || gid.0 == 0
                || bounded_name(&name[..name_len]).is_err()
                || salt_len == 0
                || salt_len > MAX_SALT_BYTES
                || profile_len > MAX_PROFILE_BYTES
                || !profile_name[..profile_len]
                    .iter()
                    .all(|byte| (b' '..=b'~').contains(byte))
                || authority.find_user(id).is_some()
                || authority.users[..slot]
                    .iter()
                    .flatten()
                    .any(|user| user.name[..user.name_len] == name[..name_len])
            {
                return Err(Error::Corrupt);
            }
            let mut profile = UserProfile::EMPTY;
            profile.display_name[..profile_len].copy_from_slice(&profile_name[..profile_len]);
            profile.display_name_len = profile_len;
            authority.users[slot] = Some(User {
                id,
                uid,
                gid,
                name,
                name_len,
                credential: Credential {
                    salt,
                    salt_len,
                    hash,
                },
                profile,
                administrator,
            });
            authority.next_user = authority.next_user.max(id.0.saturating_add(1));
        }
        // Groups are serialized after users, but users reference group IDs.
        // Decode them into temporary slots first, then validate memberships.
        for slot in 0..group_count {
            let id = GroupId(reader.u32()?);
            let name_len = reader.u8()? as usize;
            let name = read_fixed::<MAX_NAME>(&mut reader)?;
            let mut members = [None; MAX_GROUP_MEMBERS];
            for member in &mut members {
                let raw = reader.u32()?;
                *member = (raw != 0).then_some(UserId(raw));
            }
            if id.0 == 0
                || bounded_name(&name[..name_len]).is_err()
                || authority.find_group(id).is_some()
                || authority.groups[..slot]
                    .iter()
                    .flatten()
                    .any(|group| group.name[..group.name_len] == name[..name_len])
            {
                return Err(Error::Corrupt);
            }
            authority.groups[slot] = Some(Group {
                id,
                name,
                name_len,
                members,
            });
            authority.next_group = authority.next_group.max(id.0.saturating_add(1));
        }
        if reader
            .bytes(SNAPSHOT_BYTES - 4 - reader.position)?
            .iter()
            .any(|byte| *byte != 0)
            || reader.position != SNAPSHOT_BYTES - 4
            || authority
                .users
                .iter()
                .flatten()
                .any(|user| authority.find_group(user.gid).is_none())
            || authority.groups.iter().flatten().any(|group| {
                group
                    .members
                    .iter()
                    .flatten()
                    .any(|member| authority.find_user(*member).is_none())
            })
        {
            return Err(Error::Corrupt);
        }
        Ok(authority)
    }

    fn add_member(&mut self, gid: GroupId, user: UserId) -> Result<(), Error> {
        let group = self.find_group_mut(gid).ok_or(Error::NotFound)?;
        let slot = group
            .members
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        group.members[slot] = Some(user);
        Ok(())
    }

    fn group_has_member_slot(&self, gid: GroupId) -> bool {
        self.find_group(gid)
            .is_some_and(|group| group.members.iter().any(Option::is_none))
    }

    fn find_user(&self, id: UserId) -> Option<&User> {
        self.users.iter().flatten().find(|user| user.id == id)
    }
    fn find_user_mut(&mut self, id: UserId) -> Option<&mut User> {
        self.users.iter_mut().flatten().find(|user| user.id == id)
    }
    fn find_group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().flatten().find(|group| group.id == id)
    }
    fn find_group_mut(&mut self, id: GroupId) -> Option<&mut Group> {
        self.groups
            .iter_mut()
            .flatten()
            .find(|group| group.id == id)
    }
    fn find_session(&self, id: SessionId) -> Option<&Session> {
        self.sessions
            .iter()
            .flatten()
            .find(|session| session.id == id)
    }
    fn find_session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .flatten()
            .find(|session| session.id == id)
    }
}

struct SnapshotWriter<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl SnapshotWriter<'_> {
    const fn new(output: &mut [u8]) -> SnapshotWriter<'_> {
        SnapshotWriter {
            output,
            position: 0,
        }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.fixed(bytes)
    }

    fn fixed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or(Error::BufferTooSmall)?;
        if end > self.output.len() {
            return Err(Error::BufferTooSmall);
        }
        self.output[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), Error> {
        self.fixed(&[value])
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.fixed(&value.to_le_bytes())
    }
}

struct SnapshotReader<'a> {
    input: &'a [u8],
    position: usize,
}

impl SnapshotReader<'_> {
    const fn new(input: &[u8]) -> SnapshotReader<'_> {
        SnapshotReader { input, position: 0 }
    }

    fn bytes(&mut self, length: usize) -> Result<&[u8], Error> {
        let end = self.position.checked_add(length).ok_or(Error::Corrupt)?;
        if end > self.input.len() {
            return Err(Error::Corrupt);
        }
        let bytes = &self.input[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(*self.bytes(1)?.first().ok_or(Error::Corrupt)?)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().map_err(|_| Error::Corrupt)?,
        ))
    }
}

fn read_fixed<const N: usize>(reader: &mut SnapshotReader<'_>) -> Result<[u8; N], Error> {
    reader.bytes(N)?.try_into().map_err(|_| Error::Corrupt)
}

fn fnv1a(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Open a kernel-owned identity session using a bounded copied request.
pub fn session_open(name: &[u8], password: &[u8]) -> Result<SessionId, crate::Error> {
    if name.is_empty() || name.len() > MAX_NAME || password.is_empty() || password.len() > MAX_PIN {
        return Err(crate::Error::Failed);
    }
    let mut packed = [0u8; 2 + MAX_NAME + MAX_PIN];
    packed[0] = name.len() as u8;
    packed[1..1 + name.len()].copy_from_slice(name);
    let password_offset = 1 + name.len();
    packed[password_offset] = password.len() as u8;
    packed[password_offset + 1..password_offset + 1 + password.len()].copy_from_slice(password);
    let length = password_offset + 1 + password.len();
    // SAFETY: `packed` is a bounded stack buffer whose exact initialized
    // length is passed to the kernel.
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_OPEN,
            packed.as_ptr() as u64,
            length as u64,
            0,
        )
    })
    .map(SessionId)
}

/// Open an account session using a digest verified by the trusted login
/// manager. The kernel still compares it against its authority record.
pub fn session_open_verified(name: &[u8], digest: &[u8; 32]) -> Result<SessionId, crate::Error> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(crate::Error::Failed);
    }
    let mut packed = [0u8; 1 + MAX_NAME + 32];
    packed[0] = name.len() as u8;
    packed[1..1 + name.len()].copy_from_slice(name);
    packed[1 + name.len()..1 + name.len() + 32].copy_from_slice(digest);
    let length = 1 + name.len() + 32;
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_OPEN_VERIFIED,
            packed.as_ptr() as u64,
            length as u64,
            0,
        )
    })
    .map(SessionId)
}

pub fn session_lock(session: SessionId) -> Result<(), crate::Error> {
    // SAFETY: integer-only syscall; the kernel owner-binds the token.
    crate::check(unsafe { crate::syscall::invoke(crate::syscall::IDENTITY_LOCK, session.0, 0, 0) })
        .map(|_| ())
}

fn session_password_action(
    syscall: u64,
    session: SessionId,
    password: &[u8],
) -> Result<(), crate::Error> {
    if password.is_empty() || password.len() > MAX_PIN {
        return Err(crate::Error::Failed);
    }
    let mut packed = [0u8; 1 + MAX_PIN];
    packed[0] = password.len() as u8;
    packed[1..1 + password.len()].copy_from_slice(password);
    // SAFETY: `packed` is a bounded stack buffer with an exact initialized
    // length, and the kernel copies it before using the password.
    crate::check(unsafe {
        crate::syscall::invoke(
            syscall,
            session.0,
            packed.as_ptr() as u64,
            1 + password.len() as u64,
        )
    })
    .map(|_| ())
}

pub fn session_unlock(session: SessionId, password: &[u8]) -> Result<(), crate::Error> {
    session_password_action(crate::syscall::IDENTITY_UNLOCK, session, password)
}

pub fn session_unlock_verified(session: SessionId, digest: &[u8; 32]) -> Result<(), crate::Error> {
    let mut packed = [0u8; 32];
    packed.copy_from_slice(digest);
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_UNLOCK_VERIFIED,
            session.0,
            packed.as_ptr() as u64,
            packed.len() as u64,
        )
    })
    .map(|_| ())
}

pub fn session_elevate(session: SessionId, password: &[u8]) -> Result<(), crate::Error> {
    session_password_action(crate::syscall::IDENTITY_ELEVATE, session, password)
}

/// Add a regular or administrator account through the elevated identity
/// session. The kernel assigns the account to the bounded `users` group.
pub fn admin_add_user(
    name: &[u8],
    password: &[u8],
    administrator: bool,
) -> Result<UserId, crate::Error> {
    if name.is_empty() || name.len() > MAX_NAME || password.is_empty() || password.len() > MAX_PIN {
        return Err(crate::Error::Failed);
    }
    let mut packed = [0u8; 3 + MAX_NAME + MAX_PIN];
    let mut offset = 0;
    for field in [name, password] {
        packed[offset] = field.len() as u8;
        offset += 1;
        packed[offset..offset + field.len()].copy_from_slice(field);
        offset += field.len();
    }
    packed[offset] = u8::from(administrator);
    offset += 1;
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_ADD_USER,
            packed.as_ptr() as u64,
            offset as u64,
            0,
        )
    })
    .map(|value| UserId(value as u32))
}

pub fn admin_delete_user(user: UserId) -> Result<(), crate::Error> {
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_DELETE_USER,
            user.0 as u64,
            0,
            0,
        )
    })
    .map(|_| ())
}

pub fn admin_add_group(name: &[u8]) -> Result<GroupId, crate::Error> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(crate::Error::Failed);
    }
    let mut packed = [0u8; 1 + MAX_NAME];
    packed[0] = name.len() as u8;
    packed[1..1 + name.len()].copy_from_slice(name);
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_ADD_GROUP,
            packed.as_ptr() as u64,
            (1 + name.len()) as u64,
            0,
        )
    })
    .map(|value| GroupId(value as u32))
}

pub fn admin_add_group_member(user: UserId, group: GroupId) -> Result<(), crate::Error> {
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_ADD_MEMBER,
            user.0 as u64,
            group.0 as u64,
            0,
        )
    })
    .map(|_| ())
}

pub fn admin_remove_group_member(user: UserId, group: GroupId) -> Result<(), crate::Error> {
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_REMOVE_MEMBER,
            user.0 as u64,
            group.0 as u64,
            0,
        )
    })
    .map(|_| ())
}

/// Set the durable maximum failed-authentication count before cooldown.
pub fn admin_set_lockout_failures(failures: u8) -> Result<(), crate::Error> {
    if !(1..=MAX_LOCKOUT_FAILURES).contains(&failures) {
        return Err(crate::Error::Failed);
    }
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_SET_LOCKOUT,
            u64::from(failures),
            0,
            0,
        )
    })
    .map(|_| ())
}

/// Query bounded non-secret account or group records. The kernel requires the
/// calling process to hold an active elevated administrator identity session.
pub fn admin_query(
    kind: u8,
    output: &mut [u8; ADMIN_QUERY_MAX_BYTES],
) -> Result<AdminQuery<'_>, crate::Error> {
    if kind > ADMIN_QUERY_GROUPS {
        return Err(crate::Error::Failed);
    }
    let length = crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::IDENTITY_ADMIN_QUERY,
            output.as_mut_ptr() as u64,
            output.len() as u64,
            u64::from(kind),
        )
    })
    .map_err(|_| crate::Error::Failed)?;
    let length = usize::try_from(length).map_err(|_| crate::Error::Failed)?;
    AdminQuery::decode(output.get(..length).ok_or(crate::Error::Failed)?)
        .map_err(|_| crate::Error::Failed)
}

pub fn session_close(session: SessionId) -> Result<(), crate::Error> {
    // SAFETY: integer-only syscall; the kernel owner-binds the token.
    crate::check(unsafe { crate::syscall::invoke(crate::syscall::IDENTITY_CLOSE, session.0, 0, 0) })
        .map(|_| ())
}

/// Request a hardware-backed credential decision for this active session.
/// The kernel derives the account context from its owner-bound binding.
pub fn session_request_hardware(session: SessionId) -> Result<(), crate::Error> {
    crate::check(unsafe {
        crate::syscall::invoke(crate::syscall::IDENTITY_REQUEST_HARDWARE, session.0, 0, 0)
    })
    .map(|_| ())
}

/// Consume the one-shot decision previously published by the kernel TPM
/// broker for this session.
pub fn session_consume_hardware(session: SessionId) -> Result<(), crate::Error> {
    crate::check(unsafe {
        crate::syscall::invoke(crate::syscall::IDENTITY_CONSUME_HARDWARE, session.0, 0, 0)
    })
    .map(|_| ())
}

impl Default for Authority {
    fn default() -> Self {
        Self::new()
    }
}

fn bounded_name(input: &[u8]) -> Result<([u8; MAX_NAME], usize), Error> {
    if input.is_empty()
        || input.len() > MAX_NAME
        || !input
            .iter()
            .all(|byte| (b' '..=b'~').contains(byte) && *byte != b':')
    {
        return Err(Error::Invalid);
    }
    let mut name = [0; MAX_NAME];
    name[..input.len()].copy_from_slice(input);
    Ok((name, input.len()))
}

fn bounded_salt(input: &[u8]) -> Result<([u8; MAX_SALT_BYTES], usize), Error> {
    if input.is_empty() || input.len() > MAX_SALT_BYTES {
        return Err(Error::Invalid);
    }
    let mut salt = [0u8; MAX_SALT_BYTES];
    salt[..input.len()].copy_from_slice(input);
    Ok((salt, input.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_covers_lock_unlock_elevation_and_profile() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let user = authority
            .add_administrator(b"alice", group, b"salt", b"1234")
            .unwrap();
        authority
            .set_profile(user, UserProfile::new(b"Alice Example").unwrap())
            .unwrap();
        let session = authority.open_session(user, b"1234").unwrap();
        assert_eq!(authority.view(session).unwrap().uid, 1000);
        authority.lock(session).unwrap();
        assert_eq!(authority.elevate(session, b"1234"), Err(Error::Locked));
        authority.unlock(session, b"1234").unwrap();
        authority.elevate(session, b"1234").unwrap();
        let view = authority.view(session).unwrap();
        assert_eq!(view.uid, 0);
        assert!(view.elevated);
    }

    #[test]
    fn rejects_wrong_credentials_duplicates_and_capacity() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let user = authority
            .add_user(b"alice", group, b"salt", b"1234")
            .unwrap();
        assert_eq!(
            authority.open_session(user, b"wrong"),
            Err(Error::WrongPassword)
        );
        assert_eq!(
            authority.add_user(b"alice", group, b"salt2", b"1234"),
            Err(Error::AlreadyExists)
        );
        assert_eq!(authority.add_group(b"bad:name"), Err(Error::Invalid));
    }

    #[test]
    fn ordinary_users_cannot_elevate_and_full_groups_do_not_partially_create() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let user = authority
            .add_user(b"alice", group, b"salt", b"1234")
            .unwrap();
        let session = authority.open_session(user, b"1234").unwrap();
        assert_eq!(
            authority.elevate(session, b"1234"),
            Err(Error::Unauthorized)
        );
        for index in 1..MAX_GROUP_MEMBERS {
            let mut name = [b'a'; MAX_NAME];
            name[0] = b'a' + index as u8;
            let length = 1;
            authority
                .add_user(&name[..length], group, b"salt", b"1234")
                .unwrap();
        }
        assert_eq!(
            authority.add_user(b"overflow", group, b"salt", b"1234"),
            Err(Error::Full)
        );
        assert!(authority.open_session(user, b"1234").is_ok());
    }

    #[test]
    fn stored_credential_import_authenticates_without_plaintext() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let salt = b"account-file-salt";
        let digest = hash_pin(salt, b"1234");
        let user = authority
            .add_stored_user(b"ada", group, salt, &digest, false)
            .unwrap();
        let session = authority.open_session(user, b"1234").unwrap();
        assert_eq!(authority.view(session).unwrap().uid, 1000);
        assert_eq!(
            authority.open_session(user, b"wrong"),
            Err(Error::WrongPassword)
        );
    }

    #[test]
    fn password_rotation_is_atomic_and_invalidates_old_password() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        authority
            .add_user(b"ada", group, b"old-salt", b"1234")
            .unwrap();
        assert_eq!(
            authority.change_password(b"ada", b"wrong", b"new-salt", b"5678"),
            Err(Error::WrongPassword)
        );
        assert!(
            authority
                .change_password(b"ada", b"1234", b"new-salt", b"5678")
                .is_ok()
        );
        let session = authority.open_named_session(b"ada", b"5678").unwrap();
        assert_eq!(
            authority.open_named_session(b"ada", b"1234"),
            Err(Error::WrongPassword)
        );
        assert_eq!(authority.view(session).unwrap().uid, 1000);
    }

    #[test]
    fn administrative_user_and_group_changes_require_elevated_admin() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let admin = authority
            .add_administrator(b"admin", group, b"salt", b"1234")
            .unwrap();
        let ordinary = authority
            .add_user(b"ada", group, b"salt2", b"5678")
            .unwrap();
        let admin_session = authority.open_session(admin, b"1234").unwrap();
        let ordinary_session = authority.open_session(ordinary, b"5678").unwrap();
        assert_eq!(
            authority.admin_add_group(ordinary_session, b"staff"),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            authority.admin_add_user(ordinary_session, b"lin", group, b"salt3", b"9999", false),
            Err(Error::Unauthorized)
        );
        authority.elevate(admin_session, b"1234").unwrap();
        let staff = authority.admin_add_group(admin_session, b"staff").unwrap();
        let created = authority
            .admin_add_user(admin_session, b"lin", staff, b"salt3", b"9999", false)
            .unwrap();
        assert_eq!(
            authority.admin_add_group_member(admin_session, ordinary, staff),
            Ok(())
        );
        assert_eq!(
            authority.admin_remove_group_member(admin_session, ordinary, group),
            Err(Error::Invalid)
        );
        authority
            .admin_remove_group_member(admin_session, ordinary, staff)
            .unwrap();
        let created_session = authority.open_session(created, b"9999").unwrap();
        assert_eq!(
            authority.admin_delete_user(admin_session, created),
            Err(Error::Locked)
        );
        authority.close_session(created_session).unwrap();
        authority.admin_delete_user(admin_session, created).unwrap();
        assert_eq!(authority.group_id(b"staff"), Ok(staff));
    }

    #[test]
    fn administrative_query_exposes_only_bounded_non_secret_records() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let admin = authority
            .add_administrator(b"admin", group, b"salt", b"1234")
            .unwrap();
        let session = authority.open_session(admin, b"1234").unwrap();
        assert_eq!(
            authority.admin_query(session, ADMIN_QUERY_USERS, &mut [0; ADMIN_QUERY_MAX_BYTES]),
            Err(Error::Unauthorized)
        );
        authority.elevate(session, b"1234").unwrap();
        let mut bytes = [0; ADMIN_QUERY_MAX_BYTES];
        let length = authority
            .admin_query(session, ADMIN_QUERY_USERS, &mut bytes)
            .unwrap();
        let query = AdminQuery::decode(&bytes[..length]).unwrap();
        let user = query.user(0).unwrap();
        assert_eq!(user.name(), b"admin");
        assert!(user.administrator);
        assert_eq!(query.user(1), None);
        assert!(
            authority
                .admin_query(
                    session,
                    ADMIN_QUERY_USERS,
                    &mut [0; ADMIN_USER_RECORD_BYTES]
                )
                .is_err()
        );
    }

    #[test]
    fn administrator_lockout_policy_is_bounded_and_durable() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let admin = authority
            .add_administrator(b"admin", group, b"salt", b"1234")
            .unwrap();
        let session = authority.open_session(admin, b"1234").unwrap();
        assert_eq!(authority.lockout_failures(), DEFAULT_LOCKOUT_FAILURES);
        assert_eq!(
            authority.admin_set_lockout_failures(session, 2),
            Err(Error::Unauthorized)
        );
        authority.elevate(session, b"1234").unwrap();
        assert_eq!(
            authority.admin_set_lockout_failures(session, 0),
            Err(Error::Invalid)
        );
        assert_eq!(
            authority.admin_set_lockout_failures(session, MAX_LOCKOUT_FAILURES + 1),
            Err(Error::Invalid)
        );
        authority.admin_set_lockout_failures(session, 2).unwrap();
        let mut snapshot = [0; SNAPSHOT_BYTES];
        authority.encode_snapshot(&mut snapshot).unwrap();
        let restored = Authority::decode_snapshot(&snapshot).unwrap();
        assert_eq!(restored.lockout_failures(), 2);
    }

    #[test]
    fn snapshot_round_trip_excludes_sessions_and_rejects_tampering() {
        let mut authority = Authority::new();
        let group = authority.add_group(b"users").unwrap();
        let user = authority
            .add_administrator(b"alice", group, b"salt", b"1234")
            .unwrap();
        authority
            .set_profile(user, UserProfile::new(b"Alice Example").unwrap())
            .unwrap();
        assert!(authority.open_session(user, b"1234").is_ok());
        let mut snapshot = [0u8; SNAPSHOT_BYTES];
        assert_eq!(
            authority.encode_snapshot(&mut snapshot).unwrap(),
            SNAPSHOT_BYTES
        );
        let mut restored = Authority::decode_snapshot(&snapshot).unwrap();
        assert_eq!(restored.view(SessionId(1)), Err(Error::NotFound));
        assert!(restored.open_session(user, b"1234").is_ok());
        snapshot[SNAPSHOT_BYTES - 1] ^= 1;
        assert!(matches!(
            Authority::decode_snapshot(&snapshot),
            Err(Error::Corrupt)
        ));
    }
}
