// Timer interrupt preemptive scheduler wiring for WovenHat-OS
use x86_64::structures::idt::InterruptStackFrame;
use crate::context_switch::switch_context;
use crate::scheduler::ProcessScheduler;
use spin::Mutex;

pub static GLOBAL_SCHEDULER: Mutex<ProcessScheduler> = Mutex::new(ProcessScheduler::new());

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    
    if let Some(_next_pcb) = scheduler.schedule_next() {
        // Task context switch hook point
    }
}
