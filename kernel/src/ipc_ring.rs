// Lock-free atomic IPC ring buffer with integrated Capability Context checks for WovenHat-OS
use core::sync::atomic::{AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use crate::capabilities::{TaskCapabilityContext, CapabilityType};

pub const RING_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct IpcMessage {
    pub sender_pid: u64,
    pub target_pid: u64,
    pub command: u32,
    pub payload: [u8; 32],
}

#[derive(Debug, PartialEq, Eq)]
pub enum IpcError {
    PermissionDenied,
    BufferFull,
    BufferEmpty,
}

pub struct AtomicIpcRingBuffer {
    buffer: [UnsafeCell<IpcMessage>; RING_CAPACITY],
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl Sync for AtomicIpcRingBuffer {}

impl AtomicIpcRingBuffer {
    pub const fn new() -> Self {
        const EMPTY_MSG: IpcMessage = IpcMessage {
            sender_pid: 0,
            target_pid: 0,
            command: 0,
            payload: [0; 32],
        };
        
        Self {
            buffer: [const { UnsafeCell::new(EMPTY_MSG) }; RING_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn send(&self, msg: IpcMessage, cap_ctx: &TaskCapabilityContext) -> Result<(), IpcError> {
        if !cap_ctx.has_capability(CapabilityType::Ipc) {
            return Err(IpcError::PermissionDenied);
        }

        let mask = RING_CAPACITY - 1;
        let mut current_tail = self.tail.load(Ordering::Relaxed);

        loop {
            let current_head = self.head.load(Ordering::Acquire);

            if (current_tail.wrapping_sub(current_head)) >= RING_CAPACITY {
                return Err(IpcError::BufferFull);
            }

            match self.tail.compare_exchange_weak(
                current_tail,
                current_tail.wrapping_add(1),
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    unsafe {
                        let slot = self.buffer[current_tail & mask].get();
                        *slot = msg;
                    }
                    return Ok(());
                }
                Err(actual) => {
                    current_tail = actual;
                }
            }
        }
    }

    pub fn receive(&self, cap_ctx: &TaskCapabilityContext) -> Result<IpcMessage, IpcError> {
        if !cap_ctx.has_capability(CapabilityType::Ipc) {
            return Err(IpcError::PermissionDenied);
        }

        let mask = RING_CAPACITY - 1;
        let mut current_head = self.head.load(Ordering::Relaxed);

        loop {
            let current_tail = self.tail.load(Ordering::Acquire);

            if current_head == current_tail {
                return Err(IpcError::BufferEmpty);
            }

            match self.head.compare_exchange_weak(
                current_head,
                current_head.wrapping_add(1),
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let msg = unsafe {
                        let slot = self.buffer[current_head & mask].get();
                        *slot
                    };
                    return Ok(msg);
                }
                Err(actual) => {
                    current_head = actual;
                }
            }
        }
    }
}
