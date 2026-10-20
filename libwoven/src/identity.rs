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

pub struct Authority {
    users: [Option<User>; MAX_USERS],
    groups: [Option<Group>; MAX_GROUPS],
    sessions: [Option<Session>; MAX_SESSIONS],
    next_user: u32,
    next_group: u32,
    next_session: u64,
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

    fn add_user_with_role(
        &mut self,
        name: &[u8],
        gid: GroupId,
        salt: &[u8],
        password: &[u8],
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
        let credential = Credential::new(salt, password)?;
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
}
