// Task Registry and Guard Page Tracker for WovenHat-OS
use x86_64::VirtAddr;

pub const MAX_TASKS: usize = 32;
pub const PAGE_SIZE: u64 = 4096;

#[derive(Clone, Copy, Debug)]
pub struct KernelTaskInfo {
    pub pid: u64,
    pub guard_page_addr: VirtAddr,
    pub stack_top: VirtAddr,
    pub active: bool,
}

pub struct TaskRegistry {
    tasks: [KernelTaskInfo; MAX_TASKS],
    count: usize,
}

impl TaskRegistry {
    pub const fn new() -> Self {
        const EMPTY_TASK: KernelTaskInfo = KernelTaskInfo {
            pid: 0,
            guard_page_addr: VirtAddr::zero(),
            stack_top: VirtAddr::zero(),
            active: false,
        };
        Self {
            tasks: [EMPTY_TASK; MAX_TASKS],
            count: 0,
        }
    }

    pub fn register_task(&mut self, pid: u64, guard_page_addr: VirtAddr, stack_top: VirtAddr) -> Result<(), &'static str> {
        if self.count >= MAX_TASKS {
            return Err("Task registry capacity reached");
        }
        for task in self.tasks.iter_mut() {
            if !task.active {
                *task = KernelTaskInfo {
                    pid,
                    guard_page_addr,
                    stack_top,
                    active: true,
                };
                self.count += 1;
                return Ok(());
            }
        }
        Err("No available task slots")
    }

    pub fn is_guard_page(&self, addr: VirtAddr) -> bool {
        for task in self.tasks.iter() {
            if task.active {
                let g_addr = task.guard_page_addr.as_u64();
                let f_addr = addr.as_u64();
                if f_addr >= g_addr && f_addr < g_addr + PAGE_SIZE {
                    return true;
                }
            }
        }
        false
    }
}
