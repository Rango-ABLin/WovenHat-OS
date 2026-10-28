#![no_std]
#![feature(thread_local)]

use core::sync::atomic::{AtomicUsize, Ordering};

static READY: AtomicUsize = AtomicUsize::new(0);
static GLOBAL: u64 = 17;

#[repr(align(64))]
pub struct Aligned(u64);

#[thread_local]
static mut VALUE: u64 = 42;
#[thread_local]
static mut ZERO: u64 = 0;
#[thread_local]
static mut ALIGNED: Aligned = Aligned(17);
#[thread_local]
static mut POINTER: *const u64 = core::ptr::addr_of!(GLOBAL);

core::arch::global_asm!(
    ".section .text.entry,\"ax\"",
    ".global _start",
    "_start:",
    "call demo_main",
    "mov rdi, rax",
    "mov eax, 3",
    "int 0x80",
    "ud2",
    ".global tls_sleep_entry",
    "tls_sleep_entry:",
    "call sleeping_main",
    "mov rdi, rax",
    "mov eax, 131",
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
    fn tls_sleep_entry();
}

fn syscall(number: u64, arg0: u64, arg1: u64) -> u64 {
    invoke(number, arg0, arg1, 0)
}

fn invoke(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result;
    // CPU syscall boundary. Report buffers are static user bytes; Write copies
    // them synchronously through the kernel's validated user-buffer boundary.
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            options(preserves_flags),
        );
    }
    result
}

fn report(message: &'static [u8]) -> bool {
    invoke(1, 1, message.as_ptr() as u64, message.len() as u64) == message.len() as u64
}

fn matches(value: u64, zero: u64, aligned: u64) -> bool {
    // Each reference is to this thread's own TLS, never to another task's data.
    unsafe {
        let pointer = core::ptr::read_volatile(core::ptr::addr_of!(POINTER));
        pointer == core::ptr::addr_of!(GLOBAL)
            && core::ptr::read_volatile(pointer) == 17
            && core::ptr::read_volatile(core::ptr::addr_of!(VALUE)) == value
            && core::ptr::read_volatile(core::ptr::addr_of!(ZERO)) == zero
            && core::ptr::read_volatile(core::ptr::addr_of!(ALIGNED.0)) == aligned
            && (core::ptr::addr_of!(ALIGNED) as usize).is_multiple_of(64)
    }
}

fn create(entry: unsafe extern "C" fn()) -> u64 {
    syscall(130, entry as *const () as u64, 16 * 1024)
}

fn wait_ready(expected: usize) -> bool {
    for _ in 0..100 {
        if READY.load(Ordering::Acquire) == expected {
            return true;
        }
        syscall(29, 1, 0);
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn sleeping_main() -> u64 {
    if !matches(42, 0, 17) {
        return 20;
    }
    set(7, 5, 3);
    if !matches(7, 5, 3) {
        return 21;
    }
    READY.fetch_add(1, Ordering::Release);
    loop {
        syscall(29, 1000, 0);
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
    if !report(b"[S11.TLS] relocated pointer initializer: PASSED\n") {
        return 7;
    }
    set(99, 23, 19);
    let pid = syscall(16, 0, 0);
    if pid == u64::MAX {
        return 8;
    }
    if pid == 0 {
        if !matches(99, 23, 19) {
            return 9;
        }
        set(31, 37, 41);
        let id = create(tls_thread_entry);
        if id == u64::MAX || syscall(132, id, 0) != 42 || !matches(31, 37, 41) {
            return 10;
        }
        return 42;
    }
    loop {
        match syscall(5, pid, 0) {
            42 => break,
            value if value == u64::MAX - 1 => {
                syscall(29, 1, 0);
            }
            _ => return 11,
        }
    }
    if !matches(99, 23, 19)
        || !report(b"[S11.TLS] compiled fork state and fresh descendant: PASSED\n")
    {
        return 12;
    }
    for _ in 0..2 {
        let id = create(tls_thread_entry);
        if id == u64::MAX {
            return 4;
        }
        // ThreadJoin blocks on the kernel's latched event, not a polling loop.
        if syscall(132, id, 0) != 42 {
            return 5;
        }
        if syscall(132, id, 0) != u64::MAX {
            return 13;
        }
        if !matches(99, 23, 19) {
            return 6;
        }
    }
    READY.store(0, Ordering::Release);
    // The established process arena has sixteen slots; each TLS thread owns
    // one stack and one TLS slot. Assert capacity without enlarging the kernel.
    let mut ids = [0_u64; 8];
    for id in &mut ids {
        *id = create(tls_sleep_entry);
        if *id == u64::MAX {
            return 14;
        }
    }
    if !wait_ready(ids.len()) || create(tls_sleep_entry) != u64::MAX {
        return 15;
    }
    for id in ids {
        if syscall(136, id, (-2_i32) as u32 as u64) != 0
            || syscall(132, id, 0) != (-2_i32) as u32 as u64
            || syscall(132, id, 0) != u64::MAX
        {
            return 16;
        }
    }
    if !matches(99, 23, 19)
        || !report(b"[S11.TLS] compiled capacity cancellation and stale joins: PASSED\n")
    {
        return 17;
    }
    READY.store(0, Ordering::Release);
    if create(tls_sleep_entry) == u64::MAX || !wait_ready(1) || !matches(99, 23, 19) {
        return 18;
    }
    // Deliberately leave an initialized sleeping sibling. The kernel-side
    // wait/resource gate proves its mappings and record are reclaimed at exit.
    if !report(b"[S11.TLS] compiled owner-exit sibling armed: PASSED\n") {
        return 19;
    }
    42
}
