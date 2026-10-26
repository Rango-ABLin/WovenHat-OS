#![no_std]
#![no_main]
use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicU64, Ordering},
};
use libwoven::Args;
libwoven::entry!(main);
unsafe extern "C" {
    fn woven_cxx_run() -> u64;
    fn woven_tls_word() -> u64;
    fn woven_tls_zero() -> u64;
    fn woven_tls_store(first: u64, second: u64);
    fn woven_tls_address() -> u64;
}

#[repr(C)]
pub struct Pair {
    first: u64,
    second: u64,
}
#[unsafe(no_mangle)]
pub extern "C" fn woven_pair(pair: Pair) -> Pair {
    Pair {
        first: pair.first.wrapping_add(pair.second),
        second: pair.first.wrapping_mul(pair.second),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn woven_mix(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64, h: u64) -> u64 {
    a.wrapping_add(b.wrapping_mul(2))
        .wrapping_add(c.wrapping_mul(3))
        .wrapping_add(d.wrapping_mul(4))
        .wrapping_add(e.wrapping_mul(5))
        .wrapping_add(f.wrapping_mul(6))
        .wrapping_add(g.wrapping_mul(7))
        .wrapping_add(h.wrapping_mul(8))
}
#[unsafe(no_mangle)]
pub extern "C" fn woven_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let Ok(layout) = Layout::from_size_align(size, align) else {
        return core::ptr::null_mut();
    };
    // SAFETY: the native allocator validates/grows its own storage. Ownership is
    // transferred to the C++ Widget, whose delete supplies this exact layout.
    unsafe { libwoven::heap::runtime_allocator().alloc(layout) }
}
/// # Safety
/// Pointer must uniquely own a live allocation made by woven_alloc with this layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn woven_free(pointer: *mut u8, size: usize, align: usize) {
    let Ok(layout) = Layout::from_size_align(size, align) else {
        woven_fail(30);
    };
    // SAFETY: fixture Widget::operator delete passes the unique live allocation
    // and the exact size/alignment used by Widget::operator new.
    unsafe {
        libwoven::heap::runtime_allocator().dealloc(pointer, layout);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn woven_fail(code: u64) -> ! {
    libwoven::exit(code)
}

// Itanium guard layout: initialization is the first byte; bit 8 is fixture busy.
// These shims are private to this no-exceptions, single-thread C++ fixture. They
// do not provide cancellation, recursive initialization or fork recovery.
/// # Safety
/// Pointer must identify a static, aligned Clang guard exclusively accessed atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_acquire(pointer: *mut AtomicU64) -> i32 {
    // SAFETY: Clang supplies a naturally aligned, zero-initialized 64-bit guard
    // with static lifetime, used only by these atomic guard shims.
    let guard = unsafe { &*pointer };
    loop {
        if guard.load(Ordering::Acquire) & 1 != 0 {
            return 0;
        }
        if guard
            .compare_exchange(0, 256, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return 1;
        }
        core::hint::spin_loop();
    }
}
/// # Safety
/// Pointer must identify a static, aligned Clang guard exclusively accessed atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_release(pointer: *mut AtomicU64) {
    // SAFETY: same guard contract as acquire; publish completed initialization.
    unsafe { &*pointer }.store(1, Ordering::Release);
}
/// # Safety
/// Pointer must identify a static, aligned Clang guard exclusively accessed atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_abort(pointer: *mut AtomicU64) {
    // SAFETY: same guard contract as acquire; allow a subsequent initialization.
    unsafe { &*pointer }.store(0, Ordering::Release);
}
const TLS_INITIAL: u64 = 0x1122_3344_5566_7788;
static TLS_READY: AtomicU64 = AtomicU64::new(0);
static TLS_ADDRESSES: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];
libwoven::thread_entry!(tls_first_entry, tls_first);
libwoven::thread_entry!(tls_second_entry, tls_second);
extern "C" fn tls_first() -> ! {
    tls_worker(0)
}
extern "C" fn tls_second() -> ! {
    tls_worker(1)
}
fn tls_state() -> (u64, u64, u64) {
    // SAFETY: the kernel initializes this ELF's static PT_TLS block for every
    // executing native task, and these C++ accessors touch only its own block.
    unsafe { (woven_tls_word(), woven_tls_zero(), woven_tls_address()) }
}
fn store_tls(first: u64, second: u64) {
    // SAFETY: same thread-private static TLS contract, with integer payloads.
    unsafe {
        woven_tls_store(first, second);
    }
}
fn exit_tls_thread(status: i32) -> ! {
    // SAFETY: called only by native sibling bodies terminating themselves.
    unsafe { libwoven::syscall::thread_exit(status) }
}
fn tls_worker(index: usize) -> ! {
    let (initial, zero, address) = tls_state();
    if initial != TLS_INITIAL || zero != 0 {
        exit_tls_thread(61);
    }
    let first = 0xaabb_0000 + index as u64;
    let second = 0xccdd_0000 + index as u64;
    store_tls(first, second);
    TLS_ADDRESSES[index].store(address, Ordering::Release);
    TLS_READY.fetch_add(1, Ordering::AcqRel);
    while TLS_READY.load(Ordering::Acquire) != 2 {
        libwoven::yield_now();
    }
    for _ in 0..64 {
        libwoven::yield_now();
        if tls_state() != (first, second, address) {
            exit_tls_thread(62);
        }
    }
    exit_tls_thread(0)
}
fn check_static_tls() -> bool {
    let (initial, zero, main_address) = tls_state();
    if initial != TLS_INITIAL || zero != 0 {
        return false;
    }
    store_tls(0x1234, 0x5678);
    let mut threads = [0; 2];
    for (slot, entry) in threads.iter_mut().zip([
        tls_first_entry as *const () as u64,
        tls_second_entry as *const () as u64,
    ]) {
        // SAFETY: raw trampolines establish C frame alignment; stack is bounded.
        *slot = unsafe { libwoven::syscall::thread_create(entry, 16384) };
        if *slot == 0 || *slot >= libwoven::syscall::WOULD_BLOCK {
            return false;
        }
    }
    for id in threads {
        // SAFETY: owned generation-tagged handles returned by thread_create.
        if unsafe { libwoven::syscall::thread_join(libwoven::thread::Id(id)) }
            != libwoven::thread::Join::Exited(0)
        {
            return false;
        }
    }
    let first = TLS_ADDRESSES[0].load(Ordering::Acquire);
    let second = TLS_ADDRESSES[1].load(Ordering::Acquire);
    let main_intact = tls_state() == (0x1234, 0x5678, main_address);
    main_intact
        && first != 0
        && second != 0
        && first != second
        && first != main_address
        && second != main_address
        && (first | second | main_address) & 63 == 0
}
fn main(_: Args) -> ! {
    if !check_static_tls() {
        libwoven::exit(60);
    }
    let _ = libwoven::write(
        1,
        b"[WOVENWEB-CXX] static TLS template + zeroing + 2-thread isolation + FS restore: PASSED\n",
    );
    let before = libwoven::heap::runtime_stats();
    // SAFETY: linked header-free C++ fixture obeys the SysV ABI and owns all of
    // its objects. It returns after deleting every dynamically allocated object.
    let code = unsafe { woven_cxx_run() };
    let after = libwoven::heap::runtime_stats();
    if code != 0
        || after.live_allocations != before.live_allocations
        || after.quarantined_regions != 0
    {
        let _ = libwoven::write(1, b"[WOVENWEB-CXX] ABI fixture: FAILED\n");
        libwoven::exit(if code == 0 { 40 } else { code });
    }
    let _ = libwoven::write(
        1,
        b"[WOVENWEB-CXX] SysV calls + data + guarded static: PASSED\n",
    );
    let _ = libwoven::write(
        1,
        b"[WOVENWEB-CXX] 512 aligned virtual objects + destructors + heap cleanup: PASSED\n",
    );
    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    libwoven::exit(101)
}
