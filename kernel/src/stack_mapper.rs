// User stack page mapping integration for WovenHat-OS ELF task spawning
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    Page, PageTableFlags, Mapper, Size4KiB, FrameAllocator
};
use crate::process_loader::PreparedUserStack;

pub const PAGE_SIZE: u64 = 4096;

pub struct MappedUserStack {
    pub stack_top: VirtAddr,
}

pub fn map_randomized_user_stack<M, F>(
    prepared_stack: &PreparedUserStack,
    mapper: &mut M,
    frame_allocator: &mut F,
) -> Result<MappedUserStack, &'static str>
where
    M: Mapper<Size4KiB>,
    F: FrameAllocator<Size4KiB>,
{
    let stack_flags = PageTableFlags::PRESENT 
        | PageTableFlags::WRITABLE 
        | PageTableFlags::USER_ACCESSIBLE 
        | PageTableFlags::NO_EXECUTE;

    let mut current_addr = prepared_stack.stack_bottom;
    let total_size = prepared_stack.randomized_stack_top.as_u64() - prepared_stack.stack_bottom.as_u64();
    let num_pages = total_size / PAGE_SIZE;

    for _ in 0..num_pages {
        let page = Page::<Size4KiB>::containing_address(current_addr);
        let frame = frame_allocator.allocate_frame()
            .ok_or("Failed to allocate physical frame for randomized user stack")?;

        unsafe {
            mapper.map_to(page, frame, stack_flags, frame_allocator)
                .map_err(|_| "Failed to map user stack page")?
                .flush();
        }

        current_addr += PAGE_SIZE;
    }

    Ok(MappedUserStack {
        stack_top: prepared_stack.randomized_stack_top,
    })
}
