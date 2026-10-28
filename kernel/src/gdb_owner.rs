//! Generation-bound single-owner authority for a future live GDB session.
//!
//! This module owns no global state and does not identify or stop tasks. A
//! future debugger loop can keep the value beside its session and require the
//! token on every asynchronous event, preventing stale attachment reuse.

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AttachmentToken {
    generation: u64,
    task: u64,
}

pub struct Owner {
    generation: u64,
    token: Option<AttachmentToken>,
}

impl Owner {
    pub const fn new() -> Self {
        Self {
            generation: 0,
            token: None,
        }
    }

    pub const fn attached(&self) -> bool {
        self.token.is_some()
    }

    /// Attach exactly one nonzero opaque task identity.
    pub fn attach(&mut self, task: u64) -> Option<AttachmentToken> {
        if task == 0 || self.token.is_some() {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        let token = AttachmentToken { generation, task };
        self.generation = generation;
        self.token = Some(token);
        Some(token)
    }

    pub fn matches(&self, token: AttachmentToken) -> bool {
        match self.token {
            Some(current) => current == token,
            None => false,
        }
    }

    /// Detach only the current generation-bound owner.
    pub fn detach(&mut self, token: AttachmentToken) -> bool {
        if !self.matches(token) {
            return false;
        }
        self.token = None;
        true
    }
}

impl Default for Owner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "stage32-13-test"))]
pub fn self_test() -> bool {
    let mut owner = Owner::new();
    if owner.attached() || owner.attach(0).is_some() {
        return false;
    }
    let Some(first) = owner.attach(7) else {
        return false;
    };
    if !owner.attached() || !owner.matches(first) || owner.attach(8).is_some() {
        return false;
    }
    let stale = AttachmentToken {
        generation: first.generation,
        task: 8,
    };
    if owner.detach(stale) || !owner.detach(first) || owner.attached() {
        return false;
    }
    let Some(second) = owner.attach(8) else {
        return false;
    };
    second.generation != first.generation && !owner.matches(first) && owner.matches(second)
}

#[cfg(test)]
mod tests {
    use super::Owner;

    #[test]
    fn stale_token_cannot_detach_reused_owner() {
        let mut owner = Owner::new();
        let first = owner.attach(1).expect("first attachment");
        assert!(owner.detach(first));
        let second = owner.attach(1).expect("second attachment");
        assert!(!owner.matches(first));
        assert!(owner.matches(second));
    }
}
