// Capability-Gated Dynamic Kernel Extension and Plug-in Module Loader for WovenHat-OS
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleError {
    PermissionDenied,
    ModuleNotFound,
    AlreadyLoaded,
    VerificationFailed,
}

#[derive(Clone, Debug)]
pub struct KernelModule {
    pub module_id: u64,
    pub name: String,
    pub base_addr: usize,
    pub size: usize,
}

pub struct ModuleManager {
    modules: Vec<KernelModule>,
}

impl ModuleManager {
    pub const fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn load_module(
        &mut self,
        module_id: u64,
        name: &str,
        base_addr: usize,
        size: usize,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), ModuleError> {
        if !cap_ctx.has_capability(CapabilityType::ProcessCreate) {
            let mut audit = audit_ring.lock();
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::ProcessCreate, false, tick);
            return Err(ModuleError::PermissionDenied);
        }

        if self.modules.iter().any(|m| m.module_id == module_id) {
            return Err(ModuleError::AlreadyLoaded);
        }

        self.modules.push(KernelModule {
            module_id,
            name: String::from(name),
            base_addr,
            size,
        });

        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::ProcessCreate, true, tick);
        drop(audit);

        Ok(())
    }

    pub fn unload_module(
        &mut self,
        module_id: u64,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), ModuleError> {
        if !cap_ctx.has_capability(CapabilityType::ProcessCreate) {
            let mut audit = audit_ring.lock();
            audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::ProcessCreate, false, tick);
            return Err(ModuleError::PermissionDenied);
        }

        let pos = self.modules.iter()
            .position(|m| m.module_id == module_id)
            .ok_or(ModuleError::ModuleNotFound)?;

        self.modules.remove(pos);

        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::ProcessCreate, true, tick);
        drop(audit);

        Ok(())
    }
}
