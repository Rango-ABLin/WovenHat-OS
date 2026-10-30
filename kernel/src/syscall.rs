// User-space System Call Dispatcher & Trap Handler for WovenHat-OS
use x86_64::registers::model_specific::{LStar, Star, SFMask};
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::SegmentSelector;
use crate::dynamic_caps::DynamicCapabilityContext;
use crate::audit::{BoundedAuditRing, AuditAction};
use spin::Mutex;

/// Syscall numbers
pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_OPEN: u64 = 3;
pub const SYS_DELEGATE_CAP: u64 = 4;

/// Initializes x86_64 Model Specific Registers (MSRs) for fast `syscall`/`sysret` execution
pub unsafe fn init_syscalls() {
    LStar::write(x86_64::VirtAddr::new(syscall_entry_point as usize as u64));
    Star::write(
        SegmentSelector(0x0008),
        SegmentSelector(0x0010),
        SegmentSelector(0x001B),
        SegmentSelector(0x0023),
    ).unwrap();
    SFMask::write(RFlags::INTERRUPT_FLAG);
}

/// Low-level assembly/Rust trap entry point for user syscalls
pub extern "C" fn syscall_handler(
    syscall_number: u64,
    arg1: u64,
    arg2: u64,
    _arg3: u64,
    cap_ctx: &DynamicCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> i64 {
    match syscall_number {
        SYS_WRITE => {
            if !cap_ctx.has_capability(crate::capabilities::CapabilityType::FileWrite) {
                let mut audit = audit_ring.lock();
                audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, crate::capabilities::CapabilityType::FileWrite, false, tick);
                return -1; // EPERM
            }
            arg2 as i64
        }
        SYS_READ => {
            if !cap_ctx.has_capability(crate::capabilities::CapabilityType::FileRead) {
                let mut audit = audit_ring.lock();
                audit.log_event(cap_ctx.pid, AuditAction::CapabilityCheckDenied, crate::capabilities::CapabilityType::FileRead, false, tick);
                return -1;
            }
            arg1 as i64
        }
        _ => -2, // ENOSYS
    }
}

/// Compatibility adapter for the IDT path. Never synthesizes privileges:
/// only capabilities already held by the task are granted to the dispatcher.
/// The IDT entry still needs real register/PID capture before it can be enabled.
pub fn dispatch_syscall(
    syscall_number: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    cap_ctx: &crate::capabilities::TaskCapabilityContext,
    audit_ring: &Mutex<BoundedAuditRing>,
    tick: u64,
) -> Result<i64, &'static str> {
    let mut dynamic = DynamicCapabilityContext::new(cap_ctx.pid);
    for cap in cap_ctx.capabilities.iter().flatten() {
        dynamic.grant(*cap).map_err(|_| "Failed to transfer held capability")?;
    }
    let result = syscall_handler(syscall_number, arg1, arg2, arg3, &dynamic, audit_ring, tick);
    if result < 0 { Err("Syscall denied or unsupported") } else { Ok(result) }
}

extern "C" {
    fn syscall_entry_point();
}
