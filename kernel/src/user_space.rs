// User-Space Task Spawning and Ring Transition Subsystem for WovenHat-OS
extern crate alloc;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;
use x86_64::VirtAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserSpawnError {
    PermissionDenied,
    InvalidEntryPoint,
    AllocationFailed,
}

#[derive(Clone, Debug)]
pub struct UserTaskContext {
    pub pid: u64,
    pub entry_point: VirtAddr,
    pub stack_pointer: VirtAddr,
}

impl UserTaskContext {
    pub fn spawn_and_transition(
        pid: u64,
        entry_point: VirtAddr,
        stack_pointer: VirtAddr,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<Self, UserSpawnError> {
        if !cap_ctx.has_capability(CapabilityType::ProcessCreate) {
            let mut audit = audit_ring.lock();
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::ProcessCreate, false, tick);
            return Err(UserSpawnError::PermissionDenied);
        }

        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::ProcessCreate, true, tick);
        audit.log_event(cap_ctx.pid, AuditAction::ProcessSpawn, CapabilityType::ProcessCreate, true, tick);
        drop(audit);

        Ok(Self {
            pid,
            entry_point,
            stack_pointer,
        })
    }
}
