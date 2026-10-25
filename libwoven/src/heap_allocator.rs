//! Runtime adapter. All mutable metadata and mapping operations are serialized.
//! The process-local lock never disables interrupts and is not an IRQ lock.
//! VM methods must not allocate/reenter this allocator or invoke user callbacks.
//! This opt-in adapter qualifies coordinated process mappings only. Parallel raw
//! mmap/munmap, fork/exec and thread-lifecycle VM transactions require the future
//! kernel lease layer; this lock serializes heap calls, not those other syscalls.
use crate::heap::{Heap, MAX_REGIONS, REGION_BYTES, Stats};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

/// Backing storage contract for the runtime allocator.
/// # Safety
/// Each successful map must return exclusive, writable, valid REGION_BYTES
/// storage aligned to a page and disjoint from other live regions. A successful
/// unmap invalidates exactly that region. Failure may be partial: it is quarantined.
/// Neither method may call back into this allocator. Calls are serialized.
pub(crate) unsafe trait Vm: Sync {
    fn map(&self) -> Option<usize>;
    fn unmap(&self, base: usize) -> bool;
}

pub(crate) struct RuntimeHeap<V> {
    held: AtomicBool,
    state: UnsafeCell<Heap>,
    vm: V,
}
// SAFETY: all accesses to state and to the VM backend are serialized by held.
// Returned allocations are disjoint spans; clients own their payload access.
unsafe impl<V: Vm> Sync for RuntimeHeap<V> {}
struct Guard<'a, V> {
    allocator: &'a RuntimeHeap<V>,
}
impl<V> Drop for Guard<'_, V> {
    fn drop(&mut self) {
        self.allocator.held.store(false, Ordering::Release);
    }
}
impl<V> RuntimeHeap<V> {
    pub(crate) const fn new(vm: V) -> Self {
        Self {
            held: AtomicBool::new(false),
            state: UnsafeCell::new(Heap::new()),
            vm,
        }
    }
    fn lock(&self) -> Guard<'_, V> {
        while self
            .held
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Guard { allocator: self }
    }
}
impl<V: Vm> Guard<'_, V> {
    fn state(&mut self) -> &mut Heap {
        // SAFETY: only this guard can access state until its Release on Drop.
        unsafe { &mut *self.allocator.state.get() }
    }
    fn release(&mut self, base: usize) {
        if self.allocator.vm.unmap(base) {
            self.state().remove_region(base);
        } else {
            self.state().quarantine_region(base);
        }
    }
}
impl<V: Vm> RuntimeHeap<V> {
    pub(crate) fn stats(&self) -> Stats {
        self.lock().state().stats()
    }
}
// SAFETY: backend mapping guarantees and the metadata lock give each successful
// allocation an exclusive writable span with requested alignment. Deallocation
// accepts the original layout by GlobalAlloc's unsafe caller contract.
unsafe impl<V: Vm> GlobalAlloc for RuntimeHeap<V> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        if !Heap::can_fit(layout) {
            return core::ptr::null_mut();
        }
        let mut guard = self.lock();
        if let Some(address) = guard.state().allocate(layout) {
            return address as *mut u8;
        }
        let mut grown = [0usize; MAX_REGIONS];
        let mut count = 0;
        while guard.state().stats().regions < MAX_REGIONS {
            let Some(base) = self.vm.map() else {
                break;
            };
            if !guard.state().add_region(base) {
                // An overlapping/invalid mapping violates Vm's unsafe contract.
                // Do not unmap an address that might belong to a prior live span.
                break;
            }
            grown[count] = base;
            count += 1;
            if let Some(address) = guard.state().allocate(layout) {
                return address as *mut u8;
            }
        }
        // No client could use a new arena while the lock was held. Roll back
        // failed growth, quarantining uncertain release outcomes rather than
        // exposing possibly unmapped bytes to future allocations.
        for base in grown[..count].iter().rev().copied() {
            guard.release(base);
        }
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        let mut guard = self.lock();
        if !guard.state().deallocate(pointer as usize, layout) {
            return;
        }
        // Cache one idle region; return additional idle backing to the kernel.
        // Live cross-region allocations keep each region nonempty until freed.
        for index in (0..guard.state().stats().regions).rev() {
            if guard.state().stats().regions <= 1 {
                break;
            }
            if let Some(base) = guard.state().region_base(index)
                && guard.state().region_empty(base)
            {
                guard.release(base);
            }
        }
    }
}

#[cfg(all(feature = "reclaiming-allocator", target_os = "none"))]
pub(crate) struct NativeVm;
#[cfg(all(feature = "reclaiming-allocator", target_os = "none"))]
// SAFETY: native mmap returns private writable zeroed memory owned by this
// process. Serialized calls prevent competing heap growth from choosing a slot.
unsafe impl Vm for NativeVm {
    fn map(&self) -> Option<usize> {
        // SAFETY: integer-only native ABI, no user pointers are retained.
        let result = unsafe { crate::syscall::invoke(8, REGION_BYTES as u64, 1, 0) };
        (result < crate::syscall::WOULD_BLOCK && result != 0).then_some(result as usize)
    }
    fn unmap(&self, base: usize) -> bool {
        // SAFETY: only an idle mapped region is passed, with its original size.
        unsafe { crate::syscall::invoke(9, base as u64, REGION_BYTES as u64, 0) == 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU16, AtomicUsize};
    use std::{alloc::System, sync::Arc, thread};
    struct HostVm {
        base: usize,
        slots: AtomicU16,
        limit: usize,
        refuse_unmap: AtomicBool,
    }
    impl HostVm {
        fn new(limit: usize) -> Self {
            let spec = Layout::from_size_align(REGION_BYTES * MAX_REGIONS, REGION_BYTES).unwrap();
            // SAFETY: System owns a suitably aligned test buffer until HostVm Drop.
            let pointer = unsafe { System.alloc_zeroed(spec) };
            assert!(!pointer.is_null());
            Self {
                base: pointer as usize,
                slots: AtomicU16::new(0),
                limit,
                refuse_unmap: AtomicBool::new(false),
            }
        }
    }
    // SAFETY: distinct slot bits name disjoint chunks of an owned aligned buffer.
    // RuntimeHeap serializes calls; the buffer is freed after all test users stop.
    unsafe impl Vm for HostVm {
        fn map(&self) -> Option<usize> {
            let slots = self.slots.load(Ordering::Relaxed);
            if slots.count_ones() as usize >= self.limit {
                return None;
            }
            let slot = (0..MAX_REGIONS).find(|index| slots & (1 << index) == 0)?;
            self.slots.store(slots | (1 << slot), Ordering::Relaxed);
            Some(self.base + slot * REGION_BYTES)
        }
        fn unmap(&self, base: usize) -> bool {
            if self.refuse_unmap.load(Ordering::Relaxed) {
                return false;
            }
            let slot = (base - self.base) / REGION_BYTES;
            self.slots.fetch_and(!(1 << slot), Ordering::Relaxed);
            true
        }
    }
    impl Drop for HostVm {
        fn drop(&mut self) {
            let spec = Layout::from_size_align(REGION_BYTES * MAX_REGIONS, REGION_BYTES).unwrap();
            // SAFETY: matching System allocation, after all test payload owners stop.
            unsafe {
                System.dealloc(self.base as *mut u8, spec);
            }
        }
    }
    #[test]
    fn real_payload_realloc_and_zeroed_reuse() {
        let heap = RuntimeHeap::new(HostVm::new(MAX_REGIONS));
        let spec = Layout::from_size_align(4096, 256).unwrap();
        // SAFETY: test follows GlobalAlloc ownership/layout contracts exactly.
        unsafe {
            let old = heap.alloc(spec);
            assert!(!old.is_null());
            old.write_bytes(0xa5, 4096);
            let large = heap.realloc(old, spec, 180000);
            assert!(!large.is_null());
            assert_eq!(large as usize % 256, 0);
            assert!(
                core::slice::from_raw_parts(large, 4096)
                    .iter()
                    .all(|b| *b == 0xa5)
            );
            heap.dealloc(large, Layout::from_size_align(180000, 256).unwrap());
            let zeroed = heap.alloc_zeroed(spec);
            assert!(!zeroed.is_null());
            assert!(
                core::slice::from_raw_parts(zeroed, 4096)
                    .iter()
                    .all(|b| *b == 0)
            );
            heap.dealloc(zeroed, spec);
        }
        assert_eq!(heap.stats().live_allocations, 0);
        assert_eq!(heap.stats().regions, 1);
    }
    #[test]
    fn failed_growth_rolls_back_without_destroying_existing_allocations() {
        let heap = RuntimeHeap::new(HostVm::new(2));
        let small = Layout::from_size_align(32, 16).unwrap();
        // SAFETY: the kept allocation remains owned throughout failed growth.
        unsafe {
            let kept = heap.alloc(small);
            kept.write_bytes(0x55, 32);
            assert!(
                heap.alloc(Layout::from_size_align(3 * REGION_BYTES, 16).unwrap())
                    .is_null()
            );
            assert_eq!(heap.stats().regions, 1);
            assert_eq!(heap.vm.slots.load(Ordering::Relaxed), 1);
            assert!(
                core::slice::from_raw_parts(kept, 32)
                    .iter()
                    .all(|b| *b == 0x55)
            );
            heap.dealloc(kept, small);
        }
    }
    #[test]
    fn uncertain_unmap_is_quarantined_never_reused() {
        let heap = RuntimeHeap::new(HostVm::new(MAX_REGIONS));
        let spec = Layout::from_size_align(2 * REGION_BYTES, 16).unwrap();
        // SAFETY: the single live allocation is released with its original layout.
        unsafe {
            let pointer = heap.alloc(spec);
            assert!(!pointer.is_null());
            heap.vm.refuse_unmap.store(true, Ordering::Relaxed);
            heap.dealloc(pointer, spec);
            assert_eq!(heap.stats().quarantined_regions, 2);
            assert!(
                heap.alloc(Layout::from_size_align(1, 1).unwrap()) as usize
                    >= heap.vm.base + 2 * REGION_BYTES
            );
        }
    }
    #[test]
    fn concurrent_payloads_remain_disjoint_and_reclaimable() {
        let heap = Arc::new(RuntimeHeap::new(HostVm::new(MAX_REGIONS)));
        let completed = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();
        for id in 0..4 {
            let heap = heap.clone();
            let completed = completed.clone();
            workers.push(thread::spawn(move || {
                for _ in 0..200 {
                    let spec = Layout::from_size_align(128 + id * 32, 64).unwrap();
                    // SAFETY: each worker owns its returned live span until free.
                    unsafe {
                        let pointer = heap.alloc(spec);
                        assert!(!pointer.is_null());
                        pointer.write_bytes(id as u8 + 1, spec.size());
                        thread::yield_now();
                        assert!(
                            core::slice::from_raw_parts(pointer, spec.size())
                                .iter()
                                .all(|b| *b == id as u8 + 1)
                        );
                        heap.dealloc(pointer, spec);
                    }
                }
                completed.fetch_add(1, Ordering::Relaxed);
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(completed.load(Ordering::Relaxed), 4);
        assert_eq!(heap.stats().live_allocations, 0);
        assert_eq!(heap.stats().regions, 1);
    }
}
