// ELF Process Loader and Spawning Pipeline for WovenHat-OS
use x86_64::VirtAddr;
use x86_64::structures::paging::{Mapper, Size4KiB, FrameAllocator, OffsetPageTable};
use crate::task_vm::{initialize_task_address_space, map_user_memory_region};
use crate::task_registry::TaskRegistry;
use crate::scheduler::ProcessScheduler;
use crate::capabilities::TaskCapabilityContext;

/// Validated user stack bounds for mapping; the guard page is deliberately
/// not included in the mapped range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedUserStack {
    pub stack_bottom: VirtAddr,
    pub randomized_stack_top: VirtAddr,
}

impl PreparedUserStack {
    pub fn new(bottom: VirtAddr, top: VirtAddr) -> Result<Self, &'static str> {
        if bottom.as_u64() % 4096 != 0 || top.as_u64() % 4096 != 0 || top <= bottom {
            return Err("User stack bounds must be ordered and page-aligned");
        }
        Ok(Self { stack_bottom: bottom, randomized_stack_top: top })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpawnError {
    AddressSpaceInitFailed,
    MemoryMappingFailed,
    SchedulerCapacityReached,
    TaskRegistrationFailed,
}

pub fn spawn_elf_process<M, F>(
    pid: u64,
    elf_entry_addr: VirtAddr,
    stack_top_addr: VirtAddr,
    guard_page_addr: VirtAddr,
    capability_ctx: TaskCapabilityContext,
    mapper: &mut M,
    frame_allocator: &mut F,
    active_kernel_table: &mut OffsetPageTable,
    scheduler: &mut ProcessScheduler,
    task_registry: &mut TaskRegistry,
) -> Result<(), SpawnError>
where
    M: Mapper<Size4KiB>,
    F: FrameAllocator<Size4KiB>,
{
    let _pml4_frame = initialize_task_address_space(frame_allocator, active_kernel_table)
        .map_err(|_| SpawnError::AddressSpaceInitFailed)?;

    map_user_memory_region(
        mapper,
        frame_allocator,
        elf_entry_addr,
        4096 * 4,
        false,
        true,
    ).map_err(|_| SpawnError::MemoryMappingFailed)?;

    let stack_base = stack_top_addr - 16384_u64;
    map_user_memory_region(
        mapper,
        frame_allocator,
        stack_base,
        16384,
        true,
        false,
    ).map_err(|_| SpawnError::MemoryMappingFailed)?;

    task_registry.register_task(pid, guard_page_addr, stack_top_addr)
        .map_err(|_| SpawnError::TaskRegistrationFailed)?;

    scheduler.spawn_process(pid, stack_top_addr, capability_ctx)
        .map_err(|_| SpawnError::SchedulerCapacityReached)?;

    Ok(())
}
