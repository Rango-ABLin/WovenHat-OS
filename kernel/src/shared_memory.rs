// Secure Zero-Copy Shared Memory Regions Guarded by Capability Tokens for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedMemoryError {
    PermissionDenied,
    RegionNotFound,
    AlreadyExists,
    OutOfBounds,
}

pub struct SharedMemoryRegion {
    pub id: u64,
    pub owner_pid: u64,
    pub data: Vec<u8>,
}

pub struct SharedMemoryManager {
    regions: Vec<SharedMemoryRegion>,
}

impl SharedMemoryManager {
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn create_region(
        &mut self,
        id: u64,
        size: usize,
        caller_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), SharedMemoryError> {
        if !caller_ctx.has_capability(CapabilityType::MemoryInspect) {
            let mut audit = audit_ring.lock();
            audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::MemoryInspect, false, tick);
            return Err(SharedMemoryError::PermissionDenied);
        }

        if self.regions.iter().any(|r| r.id == id) {
            return Err(SharedMemoryError::AlreadyExists);
        }

        let mut data = Vec::with_capacity(size);
        data.resize(size, 0);

        self.regions.push(SharedMemoryRegion {
            id,
            owner_pid: caller_ctx.pid,
            data,
        });

        let mut audit = audit_ring.lock();
        audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::MemoryInspect, true, tick);
        drop(audit);

        Ok(())
    }

    pub fn map_region<'a>(
        &'a mut self,
        id: u64,
        caller_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<&'a mut [u8], SharedMemoryError> {
        if !caller_ctx.has_capability(CapabilityType::MemoryInspect) {
            let mut audit = audit_ring.lock();
            audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::MemoryInspect, false, tick);
            return Err(SharedMemoryError::PermissionDenied);
        }

        let region = self.regions.iter_mut()
            .find(|r| r.id == id)
            .ok_or(SharedMemoryError::RegionNotFound)?;

        let mut audit = audit_ring.lock();
        audit.log_event(caller_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::MemoryInspect, true, tick);
        drop(audit);

        Ok(&mut region.data[..])
    }
}
