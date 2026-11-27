//! Accepted legacy arena, retained for native applications during runtime migration.
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};

const ARENA_BYTES: u64 = 64 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
static END: AtomicU64 = AtomicU64::new(0);

struct Arena;

unsafe impl GlobalAlloc for Arena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        let mut next = NEXT.load(Ordering::Acquire);
        loop {
            if next == 0 {
                // SAFETY: mmap returns a private writable mapping for this
                // process and takes no userspace pointers.
                let base = unsafe { crate::syscall::invoke(8, ARENA_BYTES, 1, 0) };
                if base == crate::syscall::ERROR {
                    return core::ptr::null_mut();
                }
                if NEXT
                    .compare_exchange(0, base, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    next = NEXT.load(Ordering::Acquire);
                    continue;
                }
                END.store(base.saturating_add(ARENA_BYTES), Ordering::Release);
                next = base;
            }
            let aligned = match next.checked_add(layout.align() as u64 - 1) {
                Some(value) => value & !(layout.align() as u64 - 1),
                None => return core::ptr::null_mut(),
            };
            let end = match aligned.checked_add(layout.size() as u64) {
                Some(value) => value,
                None => return core::ptr::null_mut(),
            };
            if end > END.load(Ordering::Acquire) {
                return core::ptr::null_mut();
            }
            if NEXT
                .compare_exchange(next, end, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return aligned as *mut u8;
            }
            next = NEXT.load(Ordering::Acquire);
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Arena = Arena;
