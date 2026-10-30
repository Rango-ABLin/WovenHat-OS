// Secure Shared Memory Mapping between Tasks Guarded by Capability Tokens for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use x86_64::VirtAddr;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedMemoryError {
    PermissionDenied,
    RegionNotFound,
    MappingFailed,
    AlreadyExists,
}

#[derive(Clone, Debug)]
pub struct SharedMemoryRegion {
    pub region_id: u64,
    pub owner_pid: u64,
    pub size: usize,
    pub virt_addr: VirtAddr,
    pub writable: bool,
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
        region_id: u64,
        owner_pid: u64,
        size: usize,
        virt_addr: VirtAddr,
        writable: bool,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), SharedMemoryError> {
        if !cap_ctx.has_capability(CapabilityType::MemoryInspect) {
            let mut audit = audit_ring.lock();
            audit.log_event(owner_pid, AuditAction::CapabilityCheckDenied, CapabilityType::MemoryInspect, false, tick);
            return Err(SharedMemoryError::PermissionDenied);
        }

        if self.regions.iter().any(|r| r.region_id == region_id) {
            return Err(SharedMemoryError::AlreadyExists);
        }

        self.regions.push(SharedMemoryRegion {
            region_id,
            owner_pid,
            size,
            virt_addr,
            writable,
        });

        let mut audit = audit_ring.lock();
        audit.log_event(owner_pid, AuditAction::CapabilityCheckAllowed, CapabilityType::MemoryInspect, true, tick);
        drop(audit);

        Ok(())
    }

    pub fn map_region(
        &self,
        region_id: u64,
        accessor_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<&SharedMemoryRegion, SharedMemoryError> {
        if !accessor_ctx.has_capability(CapabilityType::MemoryInspect) {
            let mut audit = audit_ring.lock();
            audit.log_event(accessor_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::MemoryInspect, false, tick);
            return Err(SharedMemoryError::PermissionDenied);
        }

        let region = self.regions.iter()
            .find(|r| r.region_id == region_id)
            .ok_or(SharedMemoryError::RegionNotFound)?;

        let mut audit = audit_ring.lock();
        audit.log_event(accessor_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::MemoryInspect, true, tick);
        drop(audit);

        Ok(region)
    }
}
