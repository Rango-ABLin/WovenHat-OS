// Locked Kernel Heap Allocator for WovenHat-OS
use linked_list_allocator::LockedHeap;

pub const HEAP_SIZE: usize = 1024 * 1024;

#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();

static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

pub unsafe fn init_heap() {
    let heap_start = HEAP_MEM.as_ptr() as usize;
    ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
}
