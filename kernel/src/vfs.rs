// Capability-gated Virtual File System (VFS) and File Descriptor Manager for WovenHat-OS
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsError {
    PermissionDenied,
    FileNotFound,
    InvalidDescriptor,
    TableFull,
    AlreadyOpen,
}

#[derive(Clone, Debug)]
pub struct FileDescriptorEntry {
    pub fd: u64,
    pub path: String,
    pub readable: bool,
    pub writable: bool,
    pub offset: usize,
}

pub struct ProcessFileTable {
    pub pid: u64,
    descriptors: Vec<FileDescriptorEntry>,
}

impl ProcessFileTable {
    pub fn new(pid: u64) -> Self {
        Self {
            pid,
            descriptors: Vec::new(),
        }
    }

    pub fn open(
        &mut self,
        path: &str,
        readable: bool,
        writable: bool,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<u64, VfsError> {
        if readable && !cap_ctx.has_capability(CapabilityType::FileRead) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileRead, false, tick);
            return Err(VfsError::PermissionDenied);
        }
        if writable && !cap_ctx.has_capability(CapabilityType::FileWrite) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileWrite, false, tick);
            return Err(VfsError::PermissionDenied);
        }

        let fd = (self.descriptors.len() as u64) + 1;
        let entry = FileDescriptorEntry {
            fd,
            path: String::from(path),
            readable,
            writable,
            offset: 0,
        };

        self.descriptors.push(entry);

        let mut audit = audit_ring.lock();
        if readable {
            audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileRead, true, tick);
        }
        if writable {
            audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileWrite, true, tick);
        }

        Ok(fd)
    }

    pub fn read(
        &mut self,
        fd: u64,
        buf: &mut [u8],
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<usize, VfsError> {
        if !cap_ctx.has_capability(CapabilityType::FileRead) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileRead, false, tick);
            return Err(VfsError::PermissionDenied);
        }

        let entry = self.descriptors.iter_mut()
            .find(|d| d.fd == fd)
            .ok_or(VfsError::InvalidDescriptor)?;

        if !entry.readable {
            return Err(VfsError::PermissionDenied);
        }

        let mut audit = audit_ring.lock();
        audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileRead, true, tick);
        drop(audit);

        let bytes_read = buf.len().min(32);
        entry.offset += bytes_read;
        Ok(bytes_read)
    }

    pub fn write(
        &mut self,
        fd: u64,
        buf: &[u8],
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<usize, VfsError> {
        if !cap_ctx.has_capability(CapabilityType::FileWrite) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::FileWrite, false, tick);
            return Err(VfsError::PermissionDenied);
        }

        let entry = self.descriptors.iter_mut()
            .find(|d| d.fd == fd)
            .ok_or(VfsError::InvalidDescriptor)?;

        if !entry.writable {
            return Err(VfsError::PermissionDenied);
        }

        let mut audit = audit_ring.lock();
        audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::FileWrite, true, tick);
        drop(audit);

        let bytes_written = buf.len();
        entry.offset += bytes_written;
        Ok(bytes_written)
    }
}
