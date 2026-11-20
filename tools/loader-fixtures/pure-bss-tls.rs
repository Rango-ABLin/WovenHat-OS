#![no_std]
#![feature(thread_local)]

#[thread_local]
static mut ZERO: u64 = 0;

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

#[unsafe(no_mangle)]
pub extern "C" fn demo_main() -> u64 {
    // Require the loader to materialize zero-filled PT_TLS memory even though
    // the compiler/LLD image supplies no file-backed TLS initializer bytes.
    unsafe {
        if core::ptr::read_volatile(core::ptr::addr_of!(ZERO)) != 0 {
            return 1;
        }
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ZERO), 42);
        core::ptr::read_volatile(core::ptr::addr_of!(ZERO))
    }
}
