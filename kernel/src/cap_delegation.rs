// Inter-Process Capability Token Delegation for WovenHat-OS
use crate::capabilities::{TaskCapabilityContext, CapabilityType};
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationError {
    SenderDoesNotPossessCapability,
    ReceiverContextFull,
    PermissionDenied,
}

pub fn delegate_capability(
    sender_ctx: &TaskCapabilityContext,
    receiver_ctx: &mut TaskCapabilityContext,
    cap_type: CapabilityType,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<(), DelegationError> {
    if !sender_ctx.has_capability(cap_type) {
        let mut audit = audit_ring.lock();
        audit.log_event(
            sender_ctx.pid,
            AuditAction::CapabilityCheckDenied,
            cap_type,
            false,
            tick,
        );
        drop(audit);
        return Err(DelegationError::SenderDoesNotPossessCapability);
    }

    if receiver_ctx.grant(cap_type).is_err() {
        return Err(DelegationError::ReceiverContextFull);
    }

    let mut audit = audit_ring.lock();
    audit.log_event(
        sender_ctx.pid,
        AuditAction::CapabilityCheckAllowed,
        cap_type,
        true,
        tick,
    );
    audit.log_event(
        receiver_ctx.pid,
        AuditAction::CapabilityCheckAllowed,
        cap_type,
        true,
        tick,
    );
    drop(audit);

    Ok(())
}
