// Secure System Call Dispatch Gateway for WovenHat-OS
#![no_std]

extern crate alloc;

use core::result::Result::{self, Ok, Err};
use core::option::Option::{self, Some, None};
use core::mem::drop;

use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::privilege::{PrivilegeEnforcer, PrivilegeLevel, PrivilegeError};
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallError {
    UnauthorizedPrivilege,
    CapabilityMissing,
    InvalidSyscallNumber,
    ExecutionFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallNumber {
    ProcessSpawn = 1,
    MemoryAllocate = 2,
    IpcSend = 3,
    FileRead = 4,
}

pub struct SyscallDispatcher;

impl SyscallDispatcher {
    pub fn dispatch(
        syscall_num: u64,
        arg1: u64,
        arg2: u64,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<u64, SyscallError> {
        let (required_cap, max_ring) = match syscall_num {
            1 => (CapabilityType::ProcessCreate, PrivilegeLevel::Ring2Service),
            2 => (CapabilityType::MemoryInspect, PrivilegeLevel::Ring3User),
            3 => (CapabilityType::IpcEndpoint, PrivilegeLevel::Ring3User),
            4 => (CapabilityType::FileAccess, PrivilegeLevel::Ring3User),
            _ => return Err(SyscallError::InvalidSyscallNumber),
        };

        match PrivilegeEnforcer::validate_capability_with_privilege(
            cap_ctx,
            required_cap,
            max_ring,
            audit_ring,
            tick,
        ) {
            Ok(()) => {
                match syscall_num {
                    1 => Ok(arg1 + 1000),
                    2 => Ok(arg1),
                    3 => Ok(0),
                    4 => Ok(arg2),
                    _ => Err(SyscallError::InvalidSyscallNumber),
                }
            }
            Err(PrivilegeError::InsufficientPrivilege) => {
                let mut audit = audit_ring.lock();
                audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, required_cap, false, tick);
                drop(audit);
                Err(SyscallError::UnauthorizedPrivilege)
            }
            Err(PrivilegeError::CapabilityDenied) => {
                let mut audit = audit_ring.lock();
                audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, required_cap, false, tick);
                drop(audit);
                Err(SyscallError::CapabilityMissing)
            }
        }
    }
}
