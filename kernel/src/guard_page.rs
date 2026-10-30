// Kernel stack guard pages and page fault overflow detection for WovenHat-OS
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    Page, PageTableFlags, Mapper, Size4KiB, FrameAllocator
};

pub const PAGE_SIZE: u64 = 4096;
pub const KERNEL_STACK_SIZE_PAGES: usize = 4; // 16 KB stack

pub struct KernelStack {
    pub top_of_stack: VirtAddr,
}

pub fn map_kernel_stack_with_guard<M, F>(
    stack_bottom_virt: VirtAddr,
    mapper: &mut M,
    frame_allocator: &mut F,
) -> Result<KernelStack, &'static str>
where
    M: Mapper<Size4KiB>,
    F: FrameAllocator<Size4KiB>,
{
    // 1. Guard page sits at the lowest address (bottom of allocation range)
    let guard_page_addr = stack_bottom_virt;
    let guard_page = Page::<Size4KiB>::containing_address(guard_page_addr);

    // Map guard page with Present = 0 (no flags) so any access triggers a Page Fault
    unsafe {
        let frame = frame_allocator.allocate_frame()
            .ok_or("Failed to allocate frame for guard page setup")?;
        
        mapper.map_to(guard_page, frame, PageTableFlags::empty(), frame_allocator)
            .map_err(|_| "Failed to map guard page")?
            .flush();
    }

    // 2. Map actual stack pages above the guard page (Present + Writable + NoExecute)
    let stack_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
    let mut current_addr = stack_bottom_virt + PAGE_SIZE;

    for _ in 0..KERNEL_STACK_SIZE_PAGES {
        let page = Page::<Size4KiB>::containing_address(current_addr);
        let frame = frame_allocator.allocate_frame()
            .ok_or("Failed to allocate frame for kernel stack")?;

        unsafe {
            mapper.map_to(page, frame, stack_flags, frame_allocator)
                .map_err(|_| "Failed to map kernel stack page")?
                .flush();
        }

        current_addr += PAGE_SIZE;
    }

    Ok(KernelStack {
        top_of_stack: current_addr,
    })
}
