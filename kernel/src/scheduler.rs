// Round-Robin Process Scheduler and PCB Manager for WovenHat-OS
use crate::capabilities::TaskCapabilityContext;
use x86_64::VirtAddr;

pub const MAX_PROCESSES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessControlBlock {
    pub pid: u64,
    pub state: ProcessState,
    pub stack_top: VirtAddr,
    pub capability_ctx: TaskCapabilityContext,
    pub time_slices_allocated: u64,
}

pub struct ProcessScheduler {
    processes: [Option<ProcessControlBlock>; MAX_PROCESSES],
    current_index: usize,
    active_count: usize,
}

impl ProcessScheduler {
    pub const fn new() -> Self {
        Self {
            processes: [None; MAX_PROCESSES],
            current_index: 0,
            active_count: 0,
        }
    }

    pub fn spawn_process(
        &mut self,
        pid: u64,
        stack_top: VirtAddr,
        capability_ctx: TaskCapabilityContext,
    ) -> Result<(), &'static str> {
        if self.active_count >= MAX_PROCESSES {
            return Err("Process table capacity reached");
        }

        for slot in self.processes.iter_mut() {
            if slot.is_none() {
                *slot = Some(ProcessControlBlock {
                    pid,
                    state: ProcessState::Ready,
                    stack_top,
                    capability_ctx,
                    time_slices_allocated: 0,
                });
                self.active_count += 1;
                return Ok(());
            }
        }
        Err("No available process slots")
    }

    pub fn schedule_next(&mut self) -> Option<&mut ProcessControlBlock> {
        if self.active_count == 0 {
            return None;
        }

        let start_index = self.current_index;
        let mut found_index = None;
        
        loop {
            self.current_index = (self.current_index + 1) % MAX_PROCESSES;
            
            if let Some(pcb) = &self.processes[self.current_index] {
                if pcb.state == ProcessState::Ready {
                    found_index = Some(self.current_index);
                    break;
                }
            }

            if self.current_index == start_index {
                break;
            }
        }
        
        found_index.and_then(|idx| {
            self.processes[idx].as_mut().map(|pcb| {
                pcb.state = ProcessState::Running;
                pcb.time_slices_allocated += 1;
                pcb
            })
        })
    }
}
