// Process Lifecycle Monitoring, Suspension, and Termination Guarded by Capability Tokens for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use crate::scheduler::ProcessState;
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessControlError {
    PermissionDenied,
    ProcessNotFound,
    InvalidStateTransition,
}

#[derive(Clone, Debug)]
pub struct MonitoredProcessInfo {
    pub pid: u64,
    pub state: ProcessState,
    pub cpu_time_slices: u64,
}

pub struct ProcessMonitorManager {
    processes: Vec<MonitoredProcessInfo>,
}

impl ProcessMonitorManager {
    pub const fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    pub fn register_process(&mut self, pid: u64) {
        if !self.processes.iter().any(|p| p.pid == pid) {
            self.processes.push(MonitoredProcessInfo {
                pid,
                state: ProcessState::Ready,
                cpu_time_slices: 0,
            });
        }
    }

    pub fn suspend_process(
        &mut self,
        pid: u64,
        caller_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), ProcessControlError> {
        if !caller_ctx.has_capability(CapabilityType::ProcessCreate) {
            let mut audit = audit_ring.lock();
            audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::ProcessCreate, false, tick);
            return Err(ProcessControlError::PermissionDenied);
        }

        let proc = self.processes.iter_mut()
            .find(|p| p.pid == pid)
            .ok_or(ProcessControlError::ProcessNotFound)?;

        if proc.state == ProcessState::Terminated {
            return Err(ProcessControlError::InvalidStateTransition);
        }

        proc.state = ProcessState::Blocked;

        let mut audit = audit_ring.lock();
        audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::ProcessCreate, true, tick);
        audit.log_event(caller_ctx.pid, AuditAction::ProcessSpawn, CapabilityType::ProcessCreate, true, tick);
        drop(audit);

        Ok(())
    }

    pub fn kill_process(
        &mut self,
        pid: u64,
        caller_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), ProcessControlError> {
        if !caller_ctx.has_capability(CapabilityType::ProcessCreate) {
            let mut audit = audit_ring.lock();
            audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::ProcessCreate, false, tick);
            return Err(ProcessControlError::PermissionDenied);
        }

        let pos = self.processes.iter()
            .position(|p| p.pid == pid)
            .ok_or(ProcessControlError::ProcessNotFound)?;

        self.processes.remove(pos);

        let mut audit = audit_ring.lock();
        audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::ProcessCreate, true, tick);
        drop(audit);

        Ok(())
    }
}
