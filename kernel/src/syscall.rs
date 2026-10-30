// Capability-gated System Call Dispatcher for WovenHat-OS
use crate::capabilities::{TaskCapabilityContext, CapabilityType};
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

pub const SYS_IPC_SEND: u64 = 1;
pub const SYS_IPC_RECEIVE: u64 = 2;
pub const SYS_PROCESS_SPAWN: u64 = 3;
pub const SYS_MEMORY_INSPECT: u64 = 4;

#[derive(Debug, PartialEq, Eq)]
pub enum SyscallError {
    InvalidSyscall,
    PermissionDenied,
    InvalidArgument,
    InternalError,
}

pub fn dispatch_syscall(
    syscall_number: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    cap_ctx: &TaskCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<u64, SyscallError> {
    let (required_cap, audit_action) = match syscall_number {
        SYS_IPC_SEND => (CapabilityType::Ipc, AuditAction::IpcSend),
        SYS_IPC_RECEIVE => (CapabilityType::Ipc, AuditAction::IpcReceive),
        SYS_PROCESS_SPAWN => (CapabilityType::ProcessCreate, AuditAction::ProcessSpawn),
        SYS_MEMORY_INSPECT => (CapabilityType::MemoryInspect, AuditAction::MemoryFault),
        _ => return Err(SyscallError::InvalidSyscall),
    };

    let has_cap = cap_ctx.has_capability(required_cap);
    {
        let mut audit = audit_ring.lock();
        if !has_cap {
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, required_cap, false, tick);
            return Err(SyscallError::PermissionDenied);
        }
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, required_cap, true, tick);
        audit.log_event(cap_ctx.pid, audit_action, required_cap, true, tick);
    }

    match syscall_number {
        SYS_IPC_SEND => Ok(0),
        SYS_IPC_RECEIVE => Ok(0),
        SYS_PROCESS_SPAWN => Ok(arg1),
        SYS_MEMORY_INSPECT => Ok(0),
        _ => Err(SyscallError::InvalidSyscall),
    }
}
