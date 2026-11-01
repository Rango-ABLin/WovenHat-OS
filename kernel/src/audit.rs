// Bounded Kernel Audit Ring Buffer for WovenHat-OS
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use crate::capabilities::CapabilityType;

pub const AUDIT_RING_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug)]
pub enum AuditAction {
    CapabilityCheckAllowed,
    CapabilityCheckDenied,
    IpcSend,
    IpcReceive,
    ProcessSpawn,
    MemoryFault,
}

#[derive(Clone, Copy, Debug)]
pub struct AuditRecord {
    pub sequence_number: u64,
    pub timestamp_tick: u64,
    pub actor_pid: u64,
    pub action: AuditAction,
    pub capability: CapabilityType,
    pub allowed: bool,
}

pub struct BoundedAuditRing {
    records: [UnsafeCell<AuditRecord>; AUDIT_RING_CAPACITY],
    head: AtomicUsize,
    tail: AtomicUsize,
    next_sequence: AtomicU64,
}

unsafe impl Sync for BoundedAuditRing {}

impl BoundedAuditRing {
    pub const fn new() -> Self {
        const EMPTY_RECORD: AuditRecord = AuditRecord {
            sequence_number: 0,
            timestamp_tick: 0,
            actor_pid: 0,
            action: AuditAction::CapabilityCheckDenied,
            capability: CapabilityType::Ipc,
            allowed: false,
        };

        Self {
            records: [const { UnsafeCell::new(EMPTY_RECORD) }; AUDIT_RING_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            next_sequence: AtomicU64::new(1),
        }
    }

    pub fn log_event(&self, actor_pid: u64, action: AuditAction, capability: CapabilityType, allowed: bool, tick: u64) {
        let seq = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let mask = AUDIT_RING_CAPACITY - 1;
        
        let record = AuditRecord {
            sequence_number: seq,
            timestamp_tick: tick,
            actor_pid,
            action,
            capability,
            allowed,
        };

        let current_tail = self.tail.load(Ordering::Relaxed);
        let next_tail = current_tail.wrapping_add(1);

        unsafe {
            let slot = self.records[current_tail & mask].get();
            *slot = record;
        }

        let current_head = self.head.load(Ordering::Relaxed);
        if next_tail.wrapping_sub(current_head) > AUDIT_RING_CAPACITY {
            let _ = self.head.compare_exchange(
                current_head,
                current_head.wrapping_add(1),
                Ordering::Release,
                Ordering::Relaxed,
            );
        }

        self.tail.store(next_tail, Ordering::Release);
    }
}
