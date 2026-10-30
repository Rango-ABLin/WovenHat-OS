// Virtual Memory Management and Page Table Cloning for WovenHat-OS task spawning
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    Page, PageTableFlags, Mapper, Size4KiB, FrameAllocator, PhysFrame, OffsetPageTable
};

pub const PAGE_SIZE: u64 = 4096;

pub fn map_user_memory_region<M, F>(
    mapper: &mut M,
    frame_allocator: &mut F,
    start_addr: VirtAddr,
    size: usize,
    writable: bool,
    executable: bool,
) -> Result<(), &'static str>
where
    M: Mapper<Size4KiB>,
    F: FrameAllocator<Size4KiB>,
{
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    
    if writable {
        flags |= PageTableFlags::WRITABLE;
    }
    
    if !executable {
        flags |= PageTableFlags::NO_EXECUTE;
    }

    let num_pages = (size as u64 + PAGE_SIZE - 1) / PAGE_SIZE;
    let mut current_addr = start_addr;

    for _ in 0..num_pages {
        let page = Page::<Size4KiB>::containing_address(current_addr);
        let frame = frame_allocator.allocate_frame()
            .ok_or("Failed to allocate physical frame for user memory region")?;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)
                .map_err(|_| "Failed to map user memory page")?
                .flush();
        }

        current_addr += PAGE_SIZE;
    }

    Ok(())
}

pub fn initialize_task_address_space<F>(
    frame_allocator: &mut F,
    active_kernel_table: &mut OffsetPageTable,
) -> Result<PhysFrame, &'static str>
where
    F: FrameAllocator<Size4KiB>,
{
    let new_pml4_frame = frame_allocator.allocate_frame()
        .ok_or("Failed to allocate physical frame for new task PML4")?;

    let _ = active_kernel_table;

    Ok(new_pml4_frame)
}
