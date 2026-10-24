#![no_std]

// WovenHat's entry ABI supplies a 16-byte-aligned stack. The CPU-specific
// entry/exit boundary calls ordinary Rust code and returns its value to exit.
core::arch::global_asm!(
    ".section .text.entry,\"ax\"",
    ".global _start",
    "_start:",
    "call demo_main",
    "mov rdi, rax",
    "mov eax, 3",
    "int 0x80",
    "ud2",
);

unsafe extern "C" {
    fn woven_answer() -> u64;
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_main() -> u64 {
    // The test library has this exact ABI, takes no pointers, and owns no state.
    unsafe { woven_answer() }
}
