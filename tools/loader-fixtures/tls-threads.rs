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

fn runtime_tls_lifecycle() -> u64 {
    const ERROR: u64 = u64::MAX;
    let initializer = [0x31_u8, 0x32, 0x33];
    if invoke(146, 0, 0, 0) != 1 {
        return 1;
    }
    let first = invoke(
        149,
        2,
        initializer.as_ptr() as u64,
        ((initializer.len() as u64) << 32) | 8,
    );
    let first_generation = first >> 32;
    let first_slot = first & 0xffff_ffff;
    let first_address = invoke(141, first, 0, 0);
    let initialized = if first == ERROR { 2 }
        else if first_generation != 1 { 3 }
        else if first_slot != 2 { 4 }
        else if first_address == ERROR { 5 }
        else if unsafe { core::ptr::read_volatile(first_address as *const u8) } != 0x31 { 6 }
        else if invoke(144, first_slot, first_generation, 0) != 1 { 7 }
        else if invoke(145, first_slot, first_generation, 0) != 0 { 8 }
        else if invoke(145, first_slot, first_generation, 0) != 1 { 9 }
        else if invoke(141, first, 0, 0) != ERROR { 10 }
        else { 0 };
    if initialized != 0 {
        let _ = invoke(147, 0, 0, 0);
        return initialized;
    }
    let second = invoke(149, 2, initializer.as_ptr() as u64, 8);
    let second_generation = second >> 32;
    let second_address = invoke(141, second, 7, 0);
    if second == ERROR { 11 }
    else if second_generation != 2 { 12 }
    else if (second & 0xffff_ffff) != 2 { 13 }
    else if second_address == ERROR { 14 }
    else if unsafe { core::ptr::read_volatile(second_address as *const u8) } != 0 { 15 }
    else if invoke(145, 2, second_generation, 0) != 1 { 16 }
    else if invoke(141, second, 0, 0) != ERROR { 17 }
    else if invoke(147, 0, 0, 0) != 1 { 18 }
    else { 0 }
}

fn runtime_image_lifecycle() -> u64 {
    const ERROR: u64 = u64::MAX;
    const IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_RUNTIME_MODULE_ELF"));
    const SYMBOL: &[u8] = b"woven_runtime_answer";
    const STATE: &[u8] = b"WOVEN_RUNTIME_LIFECYCLE_STATE";
    if invoke(146, 0, 0, 0) != 1 {
        return 1;
    }
    let handle = invoke(156, IMAGE.as_ptr() as u64, IMAGE.len() as u64, 3);
    let generation = handle >> 32;
    let slot = handle & 0xffff_ffff;
    let address = invoke(157, SYMBOL.as_ptr() as u64, SYMBOL.len() as u64, 0);
    let state = invoke(157, STATE.as_ptr() as u64, STATE.len() as u64, 0);
    let result = if handle == ERROR { 2 }
        else if generation != 1 || slot != 3 { 3 }
        else if address == ERROR || state == ERROR { 4 }
        else {
            let init = invoke(158, handle, 0, 0);
            if init == 0 || init == ERROR { return 5; }
            if invoke(145, slot, generation, 0) != ERROR { return 6; }
            let init_function: extern "C" fn() = unsafe { core::mem::transmute(init) };
            init_function();
            if invoke(158, handle, 0, 0) != 0
                || unsafe { core::ptr::read_volatile(state as *const u64) } != 0xa5
            {
                return 7;
            }
            let function: extern "C" fn() -> u64 = unsafe { core::mem::transmute(address) };
            let fini = invoke(158, handle, 1, 0);
            if function() != 77 { 8 }
            else if fini == 0 || fini == ERROR { 9 }
            else {
                let fini_function: extern "C" fn() = unsafe { core::mem::transmute(fini) };
                fini_function();
                if unsafe { core::ptr::read_volatile(state as *const u64) } != 0x5a { 10 }
                else if invoke(158, handle, 1, 0) != 0 { 11 }
                else if invoke(145, slot, generation, 0) != 1 { 12 }
                else if invoke(157, SYMBOL.as_ptr() as u64, SYMBOL.len() as u64, 0) != ERROR { 13 }
                else if invoke(158, handle, 1, 0) != ERROR { 14 }
                else if invoke(147, 0, 0, 0) != 1 { 15 }
                else { 0 }
            }
        };
    if result != 0 {
        let _ = invoke(147, 0, 0, 0);
    }
    result
}

fn runtime_image_tls_coupling() -> u64 {
    const ERROR: u64 = u64::MAX;
    const IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_RUNTIME_TLS_MODULE_ELF"));
    if invoke(146, 0, 0, 0) != 1 {
        return 1;
    }
    let handle = invoke(156, IMAGE.as_ptr() as u64, IMAGE.len() as u64, 4);
    let address = invoke(141, handle, 0, 0);
    let generation = handle >> 32;
    let result = if handle == ERROR { 2 }
        else if generation != 1 || (handle & 0xffff_ffff) != 4 { 3 }
        else if address == ERROR { 4 }
        else if unsafe { core::ptr::read_volatile(address as *const u64) } != 55 { 5 }
        else if invoke(145, 4, generation, 0) != 1 { 6 }
        else if invoke(141, handle, 0, 0) != ERROR { 7 }
        else if invoke(147, 0, 0, 0) != 1 { 8 }
        else { 0 };
    if result != 0 {
        let _ = invoke(147, 0, 0, 0);
    }
    result
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
    let runtime_tls = runtime_tls_lifecycle();
    if runtime_tls != 0
        || !report(b"[S11.TLS] runtime generation handle and unload: PASSED\n")
        || !report(b"[S11.TLS] runtime pure-BSS reload: PASSED\n")
    {
        return 30 + runtime_tls;
    }
    let runtime_image = runtime_image_lifecycle();
    if runtime_image != 0
        || !report(b"[S11.RUNTIME] live ELF mapping and execution: PASSED\n")
        || !report(b"[S11.RUNTIME] symbol publication and unload: PASSED\n")
        || !report(b"[S11.RUNTIME] constructor/destructor generation ownership: PASSED\n")
    {
        return 60 + runtime_image;
    }
    let runtime_tls = runtime_image_tls_coupling();
    if runtime_tls != 0
        || !report(b"[S11.RUNTIME] ELF and PT_TLS atomic coupling: PASSED\n")
    {
        return 80 + runtime_tls;
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
