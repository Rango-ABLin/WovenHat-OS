//! Bounded WovenAssistant action planning for Stage 23.6.
//!
//! The assistant may propose actions, but execution remains an explicit
//! capability/permission decision. This module owns only fixed-size plan and
//! target validation; platform executors remain responsible for performing
//! the action after the decision has been granted.

pub const MAX_ACTIONS: usize = 8;
pub const MAX_TARGET_BYTES: usize = 96;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    OpenApplication = 1,
    ReadFile = 2,
    WriteFile = 3,
    ChangeSetting = 4,
    SendMessage = 5,
    RunCommand = 6,
    NetworkRequest = 7,
}

impl ActionKind {
    pub const fn required_permission(self) -> crate::Permission {
        match self {
            Self::OpenApplication => crate::Permission::Processes,
            Self::ReadFile => crate::Permission::FilesystemRead,
            Self::WriteFile => crate::Permission::FilesystemWrite,
            Self::ChangeSetting => crate::Permission::FilesystemWrite,
            Self::SendMessage => crate::Permission::Ipc,
            Self::RunCommand => crate::Permission::Processes,
            Self::NetworkRequest => crate::Permission::Network,
        }
    }

    pub const fn requires_confirmation(self) -> bool {
        matches!(
            self,
            Self::OpenApplication
                | Self::WriteFile
                | Self::ChangeSetting
                | Self::SendMessage
                | Self::RunCommand
                | Self::NetworkRequest
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyTarget,
    TargetTooLong,
    InvalidTarget,
    PlanFull,
    NotFound,
    PermissionDenied,
    ConfirmationRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub target: [u8; MAX_TARGET_BYTES],
    pub target_len: u8,
}

impl Action {
    pub fn new(kind: ActionKind, target: &[u8]) -> Result<Self, Error> {
        if target.is_empty() {
            return Err(Error::EmptyTarget);
        }
        if target.len() > MAX_TARGET_BYTES {
            return Err(Error::TargetTooLong);
        }
        if core::str::from_utf8(target).is_err() || target.contains(&0) {
            return Err(Error::InvalidTarget);
        }
        let mut output = [0; MAX_TARGET_BYTES];
        output[..target.len()].copy_from_slice(target);
        Ok(Self {
            kind,
            target: output,
            target_len: target.len() as u8,
        })
    }

    pub fn target(&self) -> &[u8] {
        &self.target[..self.target_len as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    actions: [Option<Action>; MAX_ACTIONS],
    length: usize,
}

impl Plan {
    pub const fn new() -> Self {
        Self {
            actions: [None; MAX_ACTIONS],
            length: 0,
        }
    }

    pub fn push(&mut self, action: Action) -> Result<usize, Error> {
        if self.length == MAX_ACTIONS {
            return Err(Error::PlanFull);
        }
        let index = self.length;
        self.actions[index] = Some(action);
        self.length += 1;
        Ok(index)
    }

    pub fn get(&self, index: usize) -> Result<Action, Error> {
        self.actions
            .get(index)
            .copied()
            .flatten()
            .ok_or(Error::NotFound)
    }

    pub fn authorize(
        &self,
        index: usize,
        granted: &[crate::Permission],
        confirmed: bool,
    ) -> Result<Action, Error> {
        let action = self.get(index)?;
        if !granted.contains(&action.kind.required_permission()) {
            return Err(Error::PermissionDenied);
        }
        if action.kind.requires_confirmation() && !confirmed {
            return Err(Error::ConfirmationRequired);
        }
        Ok(action)
    }
}

impl Default for Plan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, ActionKind, Error, MAX_ACTIONS, Plan};
    use crate::Permission;

    #[test]
    fn assistant_requires_explicit_permission_and_confirmation() {
        let mut plan = Plan::new();
        let index = plan
            .push(Action::new(ActionKind::WriteFile, b"/home/note.txt").unwrap())
            .unwrap();
        assert_eq!(
            plan.authorize(index, &[Permission::FilesystemWrite], false),
            Err(Error::ConfirmationRequired)
        );
        assert!(
            plan.authorize(index, &[Permission::FilesystemWrite], true)
                .is_ok()
        );
        assert_eq!(
            plan.authorize(index, &[Permission::FilesystemRead], true),
            Err(Error::PermissionDenied)
        );
    }

    #[test]
    fn action_and_plan_bounds_are_fail_closed() {
        assert_eq!(
            Action::new(ActionKind::RunCommand, b""),
            Err(Error::EmptyTarget)
        );
        assert_eq!(
            Action::new(ActionKind::RunCommand, &[b'a'; super::MAX_TARGET_BYTES + 1]),
            Err(Error::TargetTooLong)
        );
        let action = Action::new(ActionKind::ReadFile, b"/tmp/x").unwrap();
        let mut plan = Plan::new();
        for _ in 0..MAX_ACTIONS {
            assert!(plan.push(action).is_ok());
        }
        assert_eq!(plan.push(action), Err(Error::PlanFull));
    }
}
