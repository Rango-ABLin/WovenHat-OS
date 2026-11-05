//! Bounded authenticated administration and transactional policy rollback.
//!
//! This is the protocol/state boundary used by a remote administration
//! service.  It deliberately does not contain a socket listener or a private
//! key: the service supplies authentication and transport, while this module
//! guarantees replay protection, bounded records, and atomic rollback.

pub const MAX_POLICY_BYTES: usize = 1024;
pub const MAX_COMMAND_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    TooLarge,
    Unauthorized,
    Replay,
    NoPreviousPolicy,
    GenerationMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    ApplyPolicy,
    RollbackPolicy,
    QueryPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub command: Command,
    pub nonce: u64,
    pub expected_generation: u64,
    pub payload_len: usize,
}

pub trait Authorizer {
    fn authorize(&mut self, request: &Request, payload: &[u8]) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyView {
    pub generation: u64,
    pub length: usize,
}

pub struct PolicyJournal {
    active: [u8; MAX_POLICY_BYTES],
    active_len: usize,
    previous: [u8; MAX_POLICY_BYTES],
    previous_len: usize,
    generation: u64,
    previous_generation: u64,
    last_nonce: u64,
}

impl PolicyJournal {
    pub const fn new(initial: &[u8]) -> Result<Self, Error> {
        if initial.len() > MAX_POLICY_BYTES {
            return Err(Error::TooLarge);
        }
        let mut active = [0; MAX_POLICY_BYTES];
        let mut index = 0;
        while index < initial.len() {
            active[index] = initial[index];
            index += 1;
        }
        Ok(Self {
            active,
            active_len: initial.len(),
            previous: [0; MAX_POLICY_BYTES],
            previous_len: 0,
            generation: 1,
            previous_generation: 0,
            last_nonce: 0,
        })
    }

    pub const fn view(&self) -> PolicyView {
        PolicyView {
            generation: self.generation,
            length: self.active_len,
        }
    }

    pub fn copy_active(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < self.active_len {
            return Err(Error::TooLarge);
        }
        output[..self.active_len].copy_from_slice(&self.active[..self.active_len]);
        Ok(self.active_len)
    }

    pub fn apply<A: Authorizer>(
        &mut self,
        authorizer: &mut A,
        nonce: u64,
        expected_generation: u64,
        policy: &[u8],
    ) -> Result<PolicyView, Error> {
        let request = Request {
            command: Command::ApplyPolicy,
            nonce,
            expected_generation,
            payload_len: policy.len(),
        };
        self.authorize(authorizer, &request, policy)?;
        if expected_generation != self.generation {
            return Err(Error::GenerationMismatch);
        }
        if policy.is_empty() || policy.len() > MAX_POLICY_BYTES {
            return Err(Error::TooLarge);
        }
        self.previous[..self.active_len].copy_from_slice(&self.active[..self.active_len]);
        self.previous_len = self.active_len;
        self.previous_generation = self.generation;
        self.active[..policy.len()].copy_from_slice(policy);
        self.active_len = policy.len();
        self.generation = self.generation.wrapping_add(1).max(1);
        Ok(self.view())
    }

    pub fn rollback<A: Authorizer>(
        &mut self,
        authorizer: &mut A,
        nonce: u64,
        expected_generation: u64,
    ) -> Result<PolicyView, Error> {
        let request = Request {
            command: Command::RollbackPolicy,
            nonce,
            expected_generation,
            payload_len: 0,
        };
        self.authorize(authorizer, &request, &[])?;
        if expected_generation != self.generation {
            return Err(Error::GenerationMismatch);
        }
        if self.previous_len == 0 {
            return Err(Error::NoPreviousPolicy);
        }
        let old_len = self.active_len;
        let old = self.active;
        self.active[..self.previous_len].copy_from_slice(&self.previous[..self.previous_len]);
        self.active_len = self.previous_len;
        self.previous[..old_len].copy_from_slice(&old[..old_len]);
        self.previous_len = old_len;
        self.previous_generation = self.generation;
        self.generation = self.generation.wrapping_add(1).max(1);
        Ok(self.view())
    }

    fn authorize<A: Authorizer>(
        &mut self,
        authorizer: &mut A,
        request: &Request,
        payload: &[u8],
    ) -> Result<(), Error> {
        if request.nonce == 0 || request.nonce <= self.last_nonce {
            return Err(Error::Replay);
        }
        if request.payload_len > MAX_COMMAND_BYTES || request.payload_len != payload.len() {
            return Err(Error::TooLarge);
        }
        if !authorizer.authorize(request, payload) {
            return Err(Error::Unauthorized);
        }
        self.last_nonce = request.nonce;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Admin;
    impl Authorizer for Admin {
        fn authorize(&mut self, request: &Request, _payload: &[u8]) -> bool {
            request.nonce != 99
        }
    }

    #[test]
    fn applies_atomically_and_rolls_back() {
        let mut journal = PolicyJournal::new(b"safe").unwrap();
        let mut admin = Admin;
        let applied = journal.apply(&mut admin, 1, 1, b"locked").unwrap();
        assert_eq!(applied.generation, 2);
        let mut output = [0; 16];
        assert_eq!(journal.copy_active(&mut output), Ok(6));
        assert_eq!(&output[..6], b"locked");
        let rolled = journal.rollback(&mut admin, 2, 2).unwrap();
        assert_eq!(rolled.generation, 3);
        assert_eq!(journal.copy_active(&mut output), Ok(4));
        assert_eq!(&output[..4], b"safe");
    }

    #[test]
    fn rejects_replay_unauthorized_and_stale_generation() {
        let mut journal = PolicyJournal::new(b"safe").unwrap();
        let mut admin = Admin;
        assert_eq!(
            journal.apply(&mut admin, 99, 1, b"bad"),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            journal.apply(&mut admin, 1, 1, b"ok"),
            Ok(PolicyView {
                generation: 2,
                length: 2
            })
        );
        assert_eq!(
            journal.apply(&mut admin, 2, 1, b"stale"),
            Err(Error::GenerationMismatch)
        );
        assert_eq!(journal.rollback(&mut admin, 2, 2), Err(Error::Replay));
    }
}
