// ELF process loader integration with ASLR stack randomization for WovenHat-OS
use x86_64::VirtAddr;
use crate::aslr::randomize_user_stack_top;

pub const STANDARD_USER_STACK_TOP: VirtAddr = VirtAddr::new(0x0000_7FFF_FFFF_0000);
pub const USER_STACK_SIZE_PAGES: usize = 8;
pub const PAGE_SIZE: u64 = 4096;

pub struct PreparedUserStack {
    pub randomized_stack_top: VirtAddr,
    pub stack_bottom: VirtAddr,
}

pub fn prepare_randomized_user_stack() -> PreparedUserStack {
    let randomized_stack_top = randomize_user_stack_top(STANDARD_USER_STACK_TOP);
    let total_stack_size = (USER_STACK_SIZE_PAGES as u64) * PAGE_SIZE;
    let stack_bottom = VirtAddr::new(randomized_stack_top.as_u64().wrapping_sub(total_stack_size));

    PreparedUserStack {
        randomized_stack_top,
        stack_bottom,
    }
}
