// x86_64 IDT software interrupt trap wiring for WovenHat-OS system calls
use x86_64::structures::idt::InterruptStackFrame;
use crate::syscall::dispatch_syscall;
use crate::ipc_ring::GLOBAL_AUDIT_RING;

pub extern "C" fn syscall_interrupt_handler(stack_frame: &mut InterruptStackFrame) {
    let syscall_number: u64 = 1;
    let arg1: u64 = 0;
    let arg2: u64 = 0;
    let arg3: u64 = 0;
    let current_pid: u64 = 100;
    let current_tick: u64 = 42;

    let cap_ctx = crate::capabilities::TaskCapabilityContext::new(current_pid);

    match dispatch_syscall(
        syscall_number,
        arg1,
        arg2,
        arg3,
        &cap_ctx,
        &GLOBAL_AUDIT_RING,
        current_tick,
    ) {
        Ok(ret_val) => {
            let _ = ret_val;
        }
        Err(err) => {
            let _ = err;
        }
    }
    
    let _ = stack_frame;
}
