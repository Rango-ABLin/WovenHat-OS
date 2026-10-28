//! Advisory WovenAI security-monitor summaries for Stage 23.7.
//!
//! This module consumes already-recorded security events and produces bounded
//! explanations. It has no capability-grant, revoke, or enforcement API;
//! WovenGuard remains the deterministic authority.

pub const MAX_EXPLANATION_BYTES: usize = 64;
pub const MAX_FINDINGS: usize = 16;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum Severity {
    Info = 1,
    Notice = 2,
    Warning = 3,
    Critical = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finding {
    pub sequence: u64,
    pub action: crate::security::AuditAction,
    pub severity: Severity,
    pub allowed: bool,
    pub explanation: [u8; MAX_EXPLANATION_BYTES],
    pub explanation_len: u8,
}

pub fn explain(event: &crate::security::AuditEvent) -> Finding {
    let (severity, text): (Severity, &[u8]) = match (event.action, event.allowed) {
        (crate::security::AuditAction::WovenGuardDeny, _)
        | (crate::security::AuditAction::SandboxProfileDeny, _)
        | (crate::security::AuditAction::SandboxDeviceAccess, false) => {
            (Severity::Warning, b"WovenGuard denied an authority request")
        }
        (crate::security::AuditAction::SecretStore, true)
        | (crate::security::AuditAction::CapabilityGrant, true) => (
            Severity::Notice,
            b"Sensitive authority was granted or stored",
        ),
        (crate::security::AuditAction::ProcessFault, _) => {
            (Severity::Critical, b"A process fault was recorded")
        }
        (_, false) => (Severity::Notice, b"A security operation was denied"),
        (_, true) => (Severity::Info, b"A security operation was recorded"),
    };
    let mut explanation = [0; MAX_EXPLANATION_BYTES];
    explanation[..text.len()].copy_from_slice(text);
    Finding {
        sequence: event.sequence,
        action: event.action,
        severity,
        allowed: event.allowed,
        explanation,
        explanation_len: text.len() as u8,
    }
}

pub struct Monitor {
    findings: [Option<Finding>; MAX_FINDINGS],
    length: usize,
}

impl Monitor {
    pub const fn new() -> Self {
        Self {
            findings: [None; MAX_FINDINGS],
            length: 0,
        }
    }

    pub fn observe(&mut self, event: &crate::security::AuditEvent) -> bool {
        if self.length == MAX_FINDINGS {
            return false;
        }
        self.findings[self.length] = Some(explain(event));
        self.length += 1;
        true
    }

    pub fn get(&self, index: usize) -> Option<Finding> {
        self.findings.get(index).copied().flatten()
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Monitor, Severity, explain};
    use crate::security::{AuditAction, AuditEvent};

    fn event(action: AuditAction, allowed: bool) -> AuditEvent {
        AuditEvent {
            sequence: 7,
            tick: 1,
            cpu: 0,
            action,
            actor: 1,
            target: 2,
            detail: 3,
            allowed,
        }
    }

    #[test]
    fn monitor_explains_denials_without_enforcing_them() {
        let finding = explain(&event(AuditAction::WovenGuardDeny, false));
        assert_eq!(finding.severity, Severity::Warning);
        assert!(!finding.allowed);
        assert_eq!(
            &finding.explanation[..finding.explanation_len as usize],
            b"WovenGuard denied an authority request"
        );
    }

    #[test]
    fn monitor_capacity_is_bounded() {
        let mut monitor = Monitor::new();
        let event = event(AuditAction::IpcSend, true);
        for _ in 0..super::MAX_FINDINGS {
            assert!(monitor.observe(&event));
        }
        assert!(!monitor.observe(&event));
    }
}
