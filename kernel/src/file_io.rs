// Capability-gated File I/O subsystem for WovenHat-OS
use crate::capabilities::{TaskCapabilityContext, CapabilityType};
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileIoError {
    PermissionDenied,
    FileNotFound,
    InvalidDescriptor,
}

#[derive(Clone, Copy, Debug)]
pub struct FileDescriptor {
    pub fd: u64,
    pub inode: u64,
    pub readable: bool,
    pub writable: bool,
}

pub fn file_read(
    fd: &FileDescriptor,
    buf_len: usize,
    cap_ctx: &TaskCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<usize, FileIoError> {
    if !fd.readable {
        return Err(FileIoError::InvalidDescriptor);
    }

    let has_cap = cap_ctx.has_capability(CapabilityType::FileRead);
    let mut audit = audit_ring.lock();
    if !has_cap {
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileRead, false, tick);
        return Err(FileIoError::PermissionDenied);
    }
    audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileRead, true, tick);
    drop(audit);

    Ok(buf_len.min(64))
}

pub fn file_write(
    fd: &FileDescriptor,
    data_len: usize,
    cap_ctx: &TaskCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<usize, FileIoError> {
    if !fd.writable {
        return Err(FileIoError::InvalidDescriptor);
    }

    let has_cap = cap_ctx.has_capability(CapabilityType::FileWrite);
    let mut audit = audit_ring.lock();
    if !has_cap {
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileWrite, false, tick);
        return Err(FileIoError::PermissionDenied);
    }
    audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileWrite, true, tick);
    drop(audit);

    Ok(data_len)
}
