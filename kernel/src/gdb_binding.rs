//! Generation-checked binding between a debugger owner and stopped context.

use crate::{
    gdb_debugger::RegisterContext,
    gdb_frame::CapturedFrame,
    gdb_owner::{AttachmentToken, Owner},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TaskBinding {
    token: AttachmentToken,
    context: RegisterContext,
}

impl TaskBinding {
    pub fn capture(owner: &Owner, token: AttachmentToken, frame: CapturedFrame) -> Option<Self> {
        if !owner.matches(token) {
            return None;
        }
        Some(Self {
            token,
            context: frame.register_context(),
        })
    }

    pub fn is_current(self, owner: &Owner) -> bool {
        owner.matches(self.token)
    }

    pub fn context(self, owner: &Owner) -> Option<RegisterContext> {
        self.is_current(owner).then_some(self.context)
    }
}

#[cfg(any(test, feature = "stage32-15-test"))]
pub fn self_test() -> bool {
    let mut owner = Owner::new();
    let Some(token) = owner.attach(9) else {
        return false;
    };
    let mut frame = CapturedFrame::ZERO;
    frame.rip = 0xfeed;
    let Some(binding) = TaskBinding::capture(&owner, token, frame) else {
        return false;
    };
    if binding.context(&owner).map(|context| context.values[16]) != Some(0xfeed) {
        return false;
    }
    if !owner.detach(token) || binding.context(&owner).is_some() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::TaskBinding;
    use crate::{gdb_frame::CapturedFrame, gdb_owner::Owner};

    #[test]
    fn rejects_binding_for_unowned_token() {
        let owner = Owner::new();
        let mut other = Owner::new();
        let token = other.attach(1).expect("other owner token");
        assert!(TaskBinding::capture(&owner, token, CapturedFrame::ZERO).is_none());
    }
}
