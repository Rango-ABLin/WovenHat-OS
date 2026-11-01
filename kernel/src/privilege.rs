// Ring-Based Privilege Level Enforcement for Capability Validations in WovenHat-OS
extern crate alloc;
use x86_64::registers::segmentation::{CS, Segment};
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivilegeLevel {
    Ring0Kernel = 0,
    Ring1Driver = 1,
    Ring2Service = 2,
    Ring3User = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivilegeError {
    InsufficientPrivilege,
    CapabilityDenied,
}

pub struct PrivilegeEnforcer;

impl PrivilegeEnforcer {
    pub fn current_privilege_level() -> PrivilegeLevel {
        let cs_selector = CS::get_reg();
        let rpl = cs_selector.0 & 0b11;
        match rpl {
            0 => PrivilegeLevel::Ring0Kernel,
            1 => PrivilegeLevel::Ring1Driver,
            2 => PrivilegeLevel::Ring2Service,
            _ => PrivilegeLevel::Ring3User,
        }
    }

    pub fn validate_capability_with_privilege(
        cap_ctx: &DynamicCapabilityContext,
        required_cap: CapabilityType,
        required_max_ring: PrivilegeLevel,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), PrivilegeError> {
        let current_ring = Self::current_privilege_level();

        if (current_ring as u8) > (required_max_ring as u8) {
            let mut audit = audit_ring.lock();
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, required_cap, false, tick);
            return Err(PrivilegeError::InsufficientPrivilege);
        }

        if !cap_ctx.has_capability(required_cap) {
            let mut audit = audit_ring.lock();
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, required_cap, false, tick);
            return Err(PrivilegeError::CapabilityDenied);
        }

        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, required_cap, true, tick);
        drop(audit);

        Ok(())
    }
}
