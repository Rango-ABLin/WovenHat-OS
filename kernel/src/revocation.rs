// Capability Token Revocation Broadcast Protocol for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

pub struct RevocationManager;

impl RevocationManager {
    pub fn broadcast_revocation(
        contexts: &mut [DynamicCapabilityContext],
        target_cap: CapabilityType,
        revoker_pid: u64,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> usize {
        let mut revoked_count = 0;

        for ctx in contexts.iter_mut() {
            if ctx.has_capability(target_cap) {
                ctx.revoke(target_cap);
                revoked_count += 1;

                let mut audit = audit_ring.lock();
                audit.log_event(
                    ctx.pid,
                    AuditAction::CapabilityCheckDenied,
                    target_cap,
                    false,
                    tick,
                );
                drop(audit);
            }
        }

        let mut audit = audit_ring.lock();
        audit.log_event(
            revoker_pid,
            AuditAction::CapabilityCheckAllowed,
            target_cap,
            true,
            tick,
        );
        drop(audit);

        revoked_count
    }
}
