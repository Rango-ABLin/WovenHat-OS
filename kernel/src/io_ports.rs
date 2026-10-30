// Capability-gated I/O Port Access Control Subsystem for WovenHat-OS
use x86_64::instructions::port::Port;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::capabilities::CapabilityType;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoError {
    PermissionDenied,
}

pub fn port_read_u8(
    port_num: u16,
    cap_ctx: &DynamicCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<u8, IoError> {
    if !cap_ctx.has_capability(CapabilityType::DeviceIo) {
        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::DeviceIo, false, tick);
        return Err(IoError::PermissionDenied);
    }

    let mut audit = audit_ring.lock();
    audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::DeviceIo, true, tick);
    drop(audit);

    let mut port: Port<u8> = Port::new(port_num);
    Ok(unsafe { port.read() })
}

pub fn port_write_u8(
    port_num: u16,
    value: u8,
    cap_ctx: &DynamicCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<(), IoError> {
    if !cap_ctx.has_capability(CapabilityType::DeviceIo) {
        let mut audit = audit_ring.lock();
        audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, CapabilityType::DeviceIo, false, tick);
        return Err(IoError::PermissionDenied);
    }

    let mut audit = audit_ring.lock();
    audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckAllowed, CapabilityType::DeviceIo, true, tick);
    drop(audit);

    let mut port: Port<u8> = Port::new(port_num);
    unsafe { port.write(value); }
    Ok(())
}
