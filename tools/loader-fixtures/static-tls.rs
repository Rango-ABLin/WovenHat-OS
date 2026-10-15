#![no_std]
#![feature(thread_local)]

#[thread_local]
#[unsafe(no_mangle)]
pub static mut WOVEN_TLS_VALUE: u64 = 42;

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
    // This variable belongs to the current thread. Volatile access ensures
    // optimization cannot replace the compiler-generated TLS access with 42.
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(WOVEN_TLS_VALUE)) }
}
