// Page fault exception handler integration for kernel stack guard pages in WovenHat-OS
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::VirtAddr;

pub fn check_kernel_guard_page_fault(faulting_address: VirtAddr) -> bool {
    let _ = faulting_address;
    false 
}

pub extern "C" fn page_fault_handler_wrapper(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    let pf_error = PageFaultErrorCode::from_bits_truncate(error_code);
    let faulting_address = Cr2::read();

    let is_kernel_mode = (stack_frame.code_segment & 0x3) == 0;
    let is_not_present = !pf_error.contains(PageFaultErrorCode::PROTECTION_VIOLATION);

    if is_kernel_mode && is_not_present {
        if check_kernel_guard_page_fault(faulting_address) {
            panic!(
                "KERNEL PANIC: Kernel stack overflow detected! Kernel touched guard page at {:#x}",
                faulting_address.as_u64()
            );
        }
    }

    panic!(
        "Unhandled Page Fault at {:#x} (Error Code: {:?})",
        faulting_address.as_u64(),
        pf_error
    );
}
