// Asynchronous Message Passing Queue Guarded by Capability Tokens for WovenHat-OS
extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Debug)]
pub struct IpcMessage {
    pub sender_pid: u64,
    pub recipient_pid: u64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpcError {
    PermissionDenied,
    QueueFull,
    QueueEmpty,
}

pub struct AsyncIpcQueue {
    queue: VecDeque<IpcMessage>,
    capacity: usize,
}

impl AsyncIpcQueue {
    pub const fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            capacity,
        }
    }

    pub fn send(
        &mut self,
        msg: IpcMessage,
        sender_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<(), IpcError> {
        if !sender_ctx.has_capability(CapabilityType::Ipc) {
            let mut audit = audit_ring.lock();
            audit.log_event(sender_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::Ipc, false, tick);
            return Err(IpcError::PermissionDenied);
        }

        if self.queue.len() >= self.capacity {
            return Err(IpcError::QueueFull);
        }

        self.queue.push_back(msg);

        let mut audit = audit_ring.lock();
        audit.log_event(sender_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::Ipc, true, tick);
        audit.log_event(sender_ctx.pid, AuditAction::IpcSend, CapabilityType::Ipc, true, tick);
        drop(audit);

        Ok(())
    }

    pub fn receive(
        &mut self,
        recipient_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<IpcMessage, IpcError> {
        if !recipient_ctx.has_capability(CapabilityType::Ipc) {
            let mut audit = audit_ring.lock();
            audit.log_event(recipient_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::Ipc, false, tick);
            return Err(IpcError::PermissionDenied);
        }

        let msg = self.queue.pop_front().ok_or(IpcError::QueueEmpty)?;

        let mut audit = audit_ring.lock();
        audit.log_event(recipient_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::Ipc, true, tick);
        audit.log_event(recipient_ctx.pid, AuditAction::IpcReceive, CapabilityType::Ipc, true, tick);
        drop(audit);

        Ok(msg)
    }
}
