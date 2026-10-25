//! Native GlobalAlloc checks, with no mock VM backend.
use core::alloc::GlobalAlloc;
use core::{
    alloc::Layout,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
const REGION: usize = libwoven::heap::REGION_BYTES;
static START: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
static COMPLETED: AtomicUsize = AtomicUsize::new(0);
// SAFETY: these helpers have exactly GlobalAlloc's ownership/layout contracts.
// Direct trait calls keep the native failure/counter tests observable instead of
// relying on compiler-recognized Rust allocation intrinsics.
unsafe fn alloc(layout: Layout) -> *mut u8 {
    unsafe { libwoven::heap::runtime_allocator().alloc(layout) }
}
unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    unsafe { libwoven::heap::runtime_allocator().alloc_zeroed(layout) }
}
unsafe fn dealloc(pointer: *mut u8, layout: Layout) {
    unsafe { libwoven::heap::runtime_allocator().dealloc(pointer, layout) }
}
unsafe fn realloc(pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
    unsafe { libwoven::heap::runtime_allocator().realloc(pointer, layout, size) }
}
fn spec(size: usize, align: usize) -> Result<Layout, u64> {
    Layout::from_size_align(size, align).map_err(|_| 40)
}
libwoven::thread_entry!(worker_entry, worker);
extern "C" fn worker() -> ! {
    let object = libwoven::WaitObject {
        handle: START.load(Ordering::Acquire),
        timer: false,
        mask: 1 << 3,
    };
    if libwoven::wait_any_objects(&[object], None).is_err() {
        unsafe { libwoven::syscall::thread_exit(41) }
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let Ok(layout) = spec(192 + id * 32, 64) else {
        unsafe { libwoven::syscall::thread_exit(42) }
    };
    // SAFETY: each checked allocation is accessed within its requested extent
    // and freed exactly once with its original layout by its owning thread.
    unsafe {
        for _ in 0..200 {
            let pointer = alloc(layout);
            if pointer.is_null() {
                libwoven::syscall::thread_exit(43);
            }
            pointer.write_bytes(id as u8 + 1, layout.size());
            libwoven::yield_now();
            if !core::slice::from_raw_parts(pointer, layout.size())
                .iter()
                .all(|b| *b == id as u8 + 1)
            {
                libwoven::syscall::thread_exit(44);
            }
            dealloc(pointer, layout);
        }
        COMPLETED.fetch_add(1, Ordering::Release);
        libwoven::syscall::thread_exit(0)
    }
}
fn concurrent_initialization() -> Result<(), u64> {
    let event = libwoven::event_create(false).map_err(|_| 45u64)?;
    START.store(event, Ordering::Release);
    let mut handles = [0u64; 4];
    for handle in &mut handles {
        // SAFETY: native code entry, kernel-owned bounded stack, no stack borrows.
        *handle =
            unsafe { libwoven::syscall::thread_create(worker_entry as *const () as u64, 16384) };
        if *handle >= libwoven::syscall::WOULD_BLOCK {
            return Err(46);
        }
    }
    libwoven::event_set(event, true).map_err(|_| 47u64)?;
    for handle in handles {
        // SAFETY: each handle was created in and belongs to this process.
        let joined = unsafe { libwoven::syscall::thread_join(libwoven::thread::Id(handle)) };
        if joined != libwoven::thread::Join::Exited(0) {
            let _ = libwoven::print("[WOVENWEB-HEAP] worker exit (u32): ");
            let code = match joined {
                libwoven::thread::Join::Exited(code) => code as u32 as u64,
                _ => u64::MAX,
            };
            let mut digits = [0u8; 20];
            let _ = libwoven::write(
                libwoven::syscall::STDOUT,
                libwoven::format_u64(code, &mut digits),
            );
            let _ = libwoven::print("\n");
            return Err(48);
        }
    }
    libwoven::event_close(event).map_err(|_| 49u64)?;
    let stats = libwoven::heap::runtime_stats();
    if COMPLETED.load(Ordering::Acquire) != 4
        || stats.live_allocations != 0
        || stats.regions != 1
        || stats.quarantined_regions != 0
    {
        return Err(50);
    }
    let _ = libwoven::print(
        "[WOVENWEB-HEAP] concurrent initialization + 800 thread allocations: PASSED\n",
    );
    Ok(())
}
fn payloads() -> Result<(), u64> {
    // SAFETY: validated layouts, checked pointers, in-bounds payload access and
    // exactly one deallocation using each live allocation's original layout.
    unsafe {
        for align in [1, 8, 16, 64, 256, 4096, 65536] {
            let layout = spec(73, align)?;
            let pointer = alloc(layout);
            if pointer.is_null() || pointer as usize % align != 0 {
                return Err(51);
            }
            pointer.write_bytes(0xa7, layout.size());
            dealloc(pointer, layout);
        }
        let small = spec(4096, 256)?;
        let pointer = alloc(small);
        if pointer.is_null() {
            return Err(52);
        }
        pointer.write_bytes(0x5a, small.size());
        let large = realloc(pointer, small, 180000);
        if large.is_null() || large as usize % 256 != 0 {
            return Err(53);
        }
        if !core::slice::from_raw_parts(large, small.size())
            .iter()
            .all(|b| *b == 0x5a)
        {
            return Err(54);
        }
        large.add(179999).write(0xc3);
        if large.add(179999).read() != 0xc3 {
            return Err(55);
        }
        dealloc(large, spec(180000, 256)?);
        let zeroed = alloc_zeroed(small);
        if zeroed.is_null() {
            return Err(56);
        }
        if !core::slice::from_raw_parts(zeroed, small.size())
            .iter()
            .all(|b| *b == 0)
        {
            return Err(57);
        }
        dealloc(zeroed, small);
        for _ in 0..512 {
            let layout = spec(24000, 64)?;
            let pointer = alloc(layout);
            if pointer.is_null() {
                return Err(58);
            }
            pointer.write_bytes(0x6d, layout.size());
            if pointer.add(23999).read() != 0x6d {
                return Err(59);
            }
            dealloc(pointer, layout);
        }
    }
    let stats = libwoven::heap::runtime_stats();
    if stats.live_allocations != 0 || stats.live_bytes != 0 || stats.regions != 1 {
        return Err(60);
    }
    let _ = libwoven::print(
        "[WOVENWEB-HEAP] reuse + alignment + cross-region realloc + zeroing: PASSED\n",
    );
    Ok(())
}
fn exhaustion() -> Result<(), u64> {
    let mut mappings = [0u64; 16];
    let mut count = 0;
    // Threads have joined. Occupy the remaining legacy mapping slots to test a
    // real kernel refusal without racing any other user mapping operation.
    for mapping in &mut mappings {
        // SAFETY: integer-only ABI, private writable anonymous memory.
        let address = unsafe { libwoven::syscall::invoke(8, REGION as u64, 1, 0) };
        if address >= libwoven::syscall::WOULD_BLOCK {
            break;
        }
        *mapping = address;
        count += 1;
    }
    let before = libwoven::heap::runtime_stats();
    // SAFETY: a failed allocation supplies no ownership and is never dereferenced.
    let failed = unsafe { alloc(spec(2 * REGION, 16)?) };
    if !failed.is_null() {
        unsafe {
            dealloc(failed, spec(2 * REGION, 16)?);
        }
        return Err(61);
    }
    if libwoven::heap::runtime_stats() != before {
        return Err(62);
    }
    for address in &mappings[..count] {
        // SAFETY: exact fixture mapping base and its original size, released once.
        if unsafe { libwoven::syscall::invoke(9, *address, REGION as u64, 0) } != 0 {
            return Err(63);
        }
    }
    // SAFETY: the checked large span is live for all payload reads and writes.
    unsafe {
        let layout = spec(4 * REGION, 16)?;
        let pointer = alloc(layout);
        if pointer.is_null() {
            return Err(64);
        }
        pointer.write(0x41);
        pointer.add(layout.size() - 1).write(0x42);
        if pointer.read() != 0x41 || pointer.add(layout.size() - 1).read() != 0x42 {
            return Err(65);
        }
        dealloc(pointer, layout);
    }
    if libwoven::heap::runtime_stats().regions != 1 {
        return Err(66);
    }
    let _ = libwoven::print(
        "[WOVENWEB-HEAP] kernel slot exhaustion + recovery + idle-region release: PASSED\n",
    );
    Ok(())
}
fn standard_rust_allocations() -> Result<(), u64> {
    let mut data = alloc::vec::Vec::<u32>::new();
    data.try_reserve_exact(20000).map_err(|_| 68u64)?;
    for value in 0..20000 {
        data.push(value ^ 0xa5a5);
    }
    let data = core::hint::black_box(data);
    if data[0] != 0xa5a5 || data[19999] != (19999 ^ 0xa5a5) {
        return Err(69);
    }
    drop(data);
    if libwoven::heap::runtime_stats().live_allocations != 0 {
        return Err(70);
    }
    let _ = libwoven::print("[WOVENWEB-HEAP] standard Rust Vec integration: PASSED\n");
    Ok(())
}
pub fn run() -> Result<(), u64> {
    concurrent_initialization()?;
    payloads()?;
    exhaustion()?;
    standard_rust_allocations()?;
    let stats = libwoven::heap::runtime_stats();
    if stats.live_allocations != 0
        || stats.quarantined_regions != 0
        || stats.peak_bytes < 4 * REGION
        || stats.allocations < 800
    {
        return Err(67);
    }
    let _ = libwoven::print("[WOVENWEB-HEAP] native reclaiming runtime: PASSED\n");
    Ok(())
}
