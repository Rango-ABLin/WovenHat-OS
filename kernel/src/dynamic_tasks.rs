// Dynamic Task and Capability Manager using kernel heap for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use alloc::boxed::Box;
use x86_64::VirtAddr;
use crate::capabilities::TaskCapabilityContext;
use crate::scheduler::ProcessState;

#[derive(Clone, Debug)]
pub struct DynamicProcessControlBlock {
    pub pid: u64,
    pub state: ProcessState,
    pub stack_top: VirtAddr,
    pub capability_ctx: Box<TaskCapabilityContext>,
    pub time_slices_allocated: u64,
}

pub struct DynamicTaskManager {
    processes: Vec<DynamicProcessControlBlock>,
    current_index: usize,
}

impl DynamicTaskManager {
    pub const fn new() -> Self {
        Self {
            processes: Vec::new(),
            current_index: 0,
        }
    }

    pub fn spawn(
        &mut self,
        pid: u64,
        stack_top: VirtAddr,
        capability_ctx: TaskCapabilityContext,
    ) {
        let pcb = DynamicProcessControlBlock {
            pid,
            state: ProcessState::Ready,
            stack_top,
            capability_ctx: Box::new(capability_ctx),
            time_slices_allocated: 0,
        };
        self.processes.push(pcb);
    }

    pub fn schedule_next(&mut self) -> Option<&mut DynamicProcessControlBlock> {
        if self.processes.is_empty() {
            return None;
        }

        let start_index = self.current_index;
        let len = self.processes.len();

        loop {
            self.current_index = (self.current_index + 1) % len;
            let pcb = &mut self.processes[self.current_index];

            if pcb.state == ProcessState::Ready {
                pcb.state = ProcessState::Running;
                pcb.time_slices_allocated += 1;
                return Some(pcb);
            }

            if self.current_index == start_index {
                break;
            }
        }
        None
    }

    pub fn terminate(&mut self, pid: u64) -> bool {
        if let Some(pos) = self.processes.iter().position(|p| p.pid == pid) {
            self.processes.remove(pos);
            if !self.processes.is_empty() {
                self.current_index %= self.processes.len();
            } else {
                self.current_index = 0;
            }
            true
        } else {
            false
        }
    }
}
