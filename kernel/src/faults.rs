// Page Fault and Memory Protection Exception Handler for WovenHat-OS
extern crate alloc;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::registers::control::Cr2;
use crate::audit::{BoundedAuditRing, AuditAction};
use crate::capabilities::CapabilityType;
use spin::Mutex;

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let faulty_address = Cr2::read();
    let is_write = error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE);
    let is_user_mode = error_code.contains(PageFaultErrorCode::USER_MODE);
    let protection_violation = error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION);

    let mut audit = BoundedAuditRing::new();
    audit.log_event(
        0,
        AuditAction::CapabilityCheckDenied,
        CapabilityType::MemoryInspect,
        false,
        0,
    );

    if !is_user_mode {
        panic!(
            "CRITICAL KERNEL PAGE FAULT\nFault Address: {:?}\nError Code: {:?}\nWrite: {} | Protection Violation: {}\nStack Frame: {:#?}",
            faulty_address, error_code, is_write, protection_violation, stack_frame
        );
    } else {
        #[cfg(feature = "serial_debug")]
        {
            let mut telemetry = crate::telemetry::SerialTelemetry::new();
            telemetry.log_string("[FAULT] User-space segmentation fault detected. Terminating task.\n");
        }
    }
}
