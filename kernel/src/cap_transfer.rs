// Capability Delegation and Transfer via IPC Subsystem for WovenHat-OS
extern crate alloc;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapTransferError {
    SourceDoesNotHaveCapability,
    PermissionDenied,
}

pub struct CapabilityTransferManager;

impl CapabilityTransferManager {
    pub fn transfer_capability(
        source_ctx: &mut DynamicCapabilityContext,
        recipient_ctx: &mut DynamicCapabilityContext,
        cap_type: CapabilityType,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), CapTransferError> {
        if !source_ctx.has_capability(cap_type) {
            let mut audit = audit_ring.lock();
            audit.log_event(source_ctx.pid, AuditAction::CapabilityCheckDenied, cap_type, false, tick);
            return Err(CapTransferError::SourceDoesNotHaveCapability);
        }

        source_ctx.revoke(cap_type);
        recipient_ctx.grant(cap_type).map_err(|_| CapTransferError::PermissionDenied)?;

        let mut audit = audit_ring.lock();
        audit.log_event(source_ctx.pid, AuditAction::CapabilityCheckAllowed, cap_type, true, tick);
        audit.log_event(recipient_ctx.pid, AuditAction::ProcessSpawn, cap_type, true, tick);
        drop(audit);

        Ok(())
    }
}
