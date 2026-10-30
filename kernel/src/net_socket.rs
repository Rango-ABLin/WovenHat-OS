// Capability-gated Network Socket and Packet Transmission Subsystem for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketError {
    PermissionDenied,
    InvalidSocket,
    ConnectionFailed,
    TransmissionFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketProtocol {
    Tcp,
    Udp,
}

#[derive(Clone, Debug)]
pub struct SocketEntry {
    pub socket_id: u64,
    pub protocol: SocketProtocol,
    pub local_port: u16,
    pub remote_port: u16,
    pub connected: bool,
}

pub struct ProcessSocketTable {
    pub pid: u64,
    sockets: Vec<SocketEntry>,
}

impl ProcessSocketTable {
    pub fn new(pid: u64) -> Self {
        Self {
            pid,
            sockets: Vec::new(),
        }
    }

    pub fn create_socket(
        &mut self,
        protocol: SocketProtocol,
        local_port: u16,
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<u64, SocketError> {
        if !cap_ctx.has_capability(CapabilityType::NetworkAccess) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::NetworkAccess, false, tick);
            return Err(SocketError::PermissionDenied);
        }

        let socket_id = (self.sockets.len() as u64) + 1;
        let entry = SocketEntry {
            socket_id,
            protocol,
            local_port,
            remote_port: 0,
            connected: false,
        };

        self.sockets.push(entry);

        let mut audit = audit_ring.lock();
        audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::NetworkAccess, true, tick);
        drop(audit);

        Ok(socket_id)
    }

    pub fn send_packet(
        &mut self,
        socket_id: u64,
        payload: &[u8],
        cap_ctx: &DynamicCapabilityContext,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<usize, SocketError> {
        if !cap_ctx.has_capability(CapabilityType::NetworkAccess) {
            let mut audit = audit_ring.lock();
            audit.log_event(self.pid, AuditAction::CapabilityCheckDenied, CapabilityType::NetworkAccess, false, tick);
            return Err(SocketError::PermissionDenied);
        }

        let _socket = self.sockets.iter()
            .find(|s| s.socket_id == socket_id)
            .ok_or(SocketError::InvalidSocket)?;

        let mut audit = audit_ring.lock();
        audit.log_event(self.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::NetworkAccess, true, tick);
        drop(audit);

        Ok(payload.len())
    }
}
