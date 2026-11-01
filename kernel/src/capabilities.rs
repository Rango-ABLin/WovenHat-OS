// Capability Access Control Manager for WovenHat-OS

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityType {
    ProcessCreate,
    TaskControl,
    FileRead,
    FileWrite,
    Ipc,
    IpcEndpoint,
    FileAccess,
    NetworkAccess,
    DeviceIo,
    InterruptControl,
    MemoryInspect,
}

#[derive(Clone, Copy, Debug)]
pub struct CapabilityToken {
    pub cap_type: CapabilityType,
    pub granted_to_pid: u64,
}

pub const MAX_CAPABILITIES_PER_TASK: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct TaskCapabilityContext {
    pub pid: u64,
    pub capabilities: [Option<CapabilityType>; MAX_CAPABILITIES_PER_TASK],
    pub count: usize,
}

impl TaskCapabilityContext {
    pub const fn new(pid: u64) -> Self {
        Self {
            pid,
            capabilities: [None; MAX_CAPABILITIES_PER_TASK],
            count: 0,
        }
    }

    pub fn grant(&mut self, cap_type: CapabilityType) -> Result<(), &'static str> {
        if self.count >= MAX_CAPABILITIES_PER_TASK {
            return Err("Task capability limit reached");
        }
        for existing in self.capabilities.iter() {
            if let Some(c) = existing {
                if *c == cap_type {
                    return Ok(());
                }
            }
        }
        for slot in self.capabilities.iter_mut() {
            if slot.is_none() {
                *slot = Some(cap_type);
                self.count += 1;
                return Ok(());
            }
        }
        Err("No available capability slots")
    }

    pub fn revoke(&mut self, cap_type: CapabilityType) -> Result<(), &'static str> {
        for slot in self.capabilities.iter_mut() {
            if let Some(c) = slot {
                if *c == cap_type {
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                    return Ok(());
                }
            }
        }
        Err("Capability not found")
    }

    pub fn has_capability(&self, cap_type: CapabilityType) -> bool {
        for slot in self.capabilities.iter() {
            if let Some(c) = slot {
                if *c == cap_type {
                    return true;
                }
            }
        }
        false
    }
}
