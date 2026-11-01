// Local APIC Timer Initialization and IDT Registration for WovenHat-OS
use x86_64::structures::idt::InterruptDescriptorTable;
use crate::timer_sched::timer_interrupt_handler;

pub const APIC_BASE_MMIO: usize = 0xFEE0_0000;
pub const APIC_REG_SPURIOUS: usize = 0x00F0;
pub const APIC_REG_TIMER_VECTOR: usize = 0x0320;
pub const APIC_REG_TIMER_INITIAL: usize = 0x0380;
pub const APIC_REG_TIMER_DIVIDE: usize = 0x03E0;
pub const APIC_TIMER_VECTOR: u8 = 0x20;

pub fn register_apic_idt(idt: &mut InterruptDescriptorTable) {
    idt[usize::from(APIC_TIMER_VECTOR)].set_handler_fn(timer_interrupt_handler);
}

pub unsafe fn init_apic_timer() {
    let base = APIC_BASE_MMIO as *mut u32;

    let spurious_reg = base.add(APIC_REG_SPURIOUS / 4);
    let spurious_val = ptr_read_volatile(spurious_reg) | 0x100;
    ptr_write_volatile(spurious_reg, spurious_val);

    let divide_reg = base.add(APIC_REG_TIMER_DIVIDE / 4);
    ptr_write_volatile(divide_reg, 0x3);

    let timer_vector_reg = base.add(APIC_REG_TIMER_VECTOR / 4);
    let timer_config = (1 << 17) | (APIC_TIMER_VECTOR as u32);
    ptr_write_volatile(timer_vector_reg, timer_config);

    let initial_count_reg = base.add(APIC_REG_TIMER_INITIAL / 4);
    ptr_write_volatile(initial_count_reg, 100_000);
}

#[inline(always)]
unsafe fn ptr_read_volatile(addr: *const u32) -> u32 {
    core::ptr::read_volatile(addr)
}

#[inline(always)]
unsafe fn ptr_write_volatile(addr: *mut u32, val: u32) {
    core::ptr::write_volatile(addr, val)
}
