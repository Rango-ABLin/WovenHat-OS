#![no_std]
#![feature(thread_local)]

#[repr(align(64))]
pub struct Aligned(u64);

#[thread_local]
static mut VALUE: u64 = 42;
#[thread_local]
static mut ZERO: u64 = 0;
#[thread_local]
static mut ALIGNED: Aligned = Aligned(17);

core::arch::global_asm!(
    ".section .text.entry,\"ax\"",
    ".global _start",
    "_start:",
    "call demo_main",
    "mov rdi, rax",
    "mov eax, 3",
    "int 0x80",
    "ud2",
    ".global tls_thread_entry",
    "tls_thread_entry:",
    "call thread_main",
    "mov rdi, rax",
    "mov eax, 131",
    "int 0x80",
    "ud2",
);

unsafe extern "C" {
    fn tls_thread_entry();
}

fn syscall(number: u64, arg0: u64, arg1: u64) -> u64 {
    let result;
    // WovenHat userspace ABI: scalar arguments only, no borrowed buffers.
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") 0_u64,
            options(preserves_flags),
        );
    }
    result
}

fn matches(value: u64, zero: u64, aligned: u64) -> bool {
    // Each reference is to this thread's own TLS, never to another task's data.
    unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(VALUE)) == value
            && core::ptr::read_volatile(core::ptr::addr_of!(ZERO)) == zero
            && core::ptr::read_volatile(core::ptr::addr_of!(ALIGNED.0)) == aligned
            && (core::ptr::addr_of!(ALIGNED) as usize).is_multiple_of(64)
    }
}

fn set(value: u64, zero: u64, aligned: u64) {
    // The variables are private to this thread; volatile writes prevent folding.
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(VALUE), value);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ZERO), zero);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ALIGNED.0), aligned);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn thread_main() -> u64 {
    if !matches(42, 0, 17) {
        return 2;
    }
    set(7, 5, 3);
    if matches(7, 5, 3) { 42 } else { 3 }
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_main() -> u64 {
    if !matches(42, 0, 17) {
        return 1;
    }
    set(99, 23, 19);
    for _ in 0..2 {
        let id = syscall(130, tls_thread_entry as *const () as u64, 16 * 1024);
        if id == u64::MAX {
            return 4;
        }
        // ThreadJoin blocks on the kernel's latched event, not a polling loop.
        if syscall(132, id, 0) != 42 {
            return 5;
        }
        if !matches(99, 23, 19) {
            return 6;
        }
    }
    42
}
