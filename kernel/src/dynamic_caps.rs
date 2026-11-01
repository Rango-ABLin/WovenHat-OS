// Dynamic Capability Registry supporting runtime grant, revoke, and delegation for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Debug)]
pub struct DynamicCapabilityContext {
    pub pid: u64,
    capabilities: Vec<CapabilityType>,
}

impl DynamicCapabilityContext {
    pub fn new(pid: u64) -> Self {
        Self {
            pid,
            capabilities: Vec::new(),
        }
    }

    pub fn grant(&mut self, cap: CapabilityType) -> Result<(), &'static str> {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
            Ok(())
        } else {
            Err("Capability already exists in context")
        }
    }

    pub fn revoke(&mut self, cap: CapabilityType) -> Result<(), &'static str> {
        if let Some(pos) = self.capabilities.iter().position(|&c| c == cap) {
            self.capabilities.remove(pos);
            Ok(())
        } else {
            Err("Capability not found in context")
        }
    }

    pub fn has_capability(&self, cap: CapabilityType) -> bool {
        self.capabilities.contains(&cap)
    }

    pub fn delegate_to(
        &self,
        recipient: &mut DynamicCapabilityContext,
        cap: CapabilityType,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), &'static str> {
        if !self.has_capability(cap) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, cap, false, tick);
            return Err("Sender does not possess capability to delegate");
        }

        recipient.grant(cap)?;

        let mut audit = audit_ring.lock();
        audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, cap, true, tick);
        audit.log_event(recipient.pid, AuditAction::CapabilityCheckAllowed, cap, true, tick);
        
        Ok(())
    }
}
