// Scheduler Interrupt Hook for Capability Revocation in WovenHat-OS
extern crate alloc;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::revocation::RevocationManager;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

pub struct SchedulerRevocationHook;

impl SchedulerRevocationHook {
    pub fn handle_tick(
        contexts: &mut [DynamicCapabilityContext],
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) {
        if tick % 10000 == 0 {
            let revoked = RevocationManager::broadcast_revocation(
                contexts,
                CapabilityType::ProcessCreate,
                0,
                audit_ring,
                tick,
            );

            if revoked > 0 {
                #[cfg(feature = "serial_debug")]
                {
                    let mut telemetry = crate::telemetry::SerialTelemetry::new();
                    telemetry.log_string("[SCHEDULER] Periodic security sweep revoked unauthorized ProcessCreate tokens.\n");
                }
            }
        }
    }
}
