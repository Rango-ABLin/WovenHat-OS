// Address Space Layout Randomization (ASLR) Subsystem for WovenHat-OS
#![no_std]

extern crate alloc;

use core::result::Result::{self, Ok, Err};
use core::option::Option::{self, Some, None};
use core::mem::drop;

use x86_64::VirtAddr;
use x86_64::instructions::registers::read_tsc;
use crate::audit::{BoundedAuditRing, AuditAction};
use crate::capabilities::CapabilityType;
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AslrError {
    InsufficientEntropy,
    InvalidRange,
}

pub struct AslrManager {
    entropy_seed: u64,
}

impl AslrManager {
    pub fn new() -> Self {
        let tsc_val = unsafe { read_tsc() };
        Self {
            entropy_seed: tsc_val ^ 0x9999_5555_AAAA_3333,
        }
    }

    pub fn randomize_base(
        &mut self,
        base_address: u64,
        max_offset_pages: u64,
        pid: u64,
        audit_ring: &Mutex<BoundedAuditRing>,
        tick: u64,
    ) -> Result<VirtAddr, AslrError> {
        if max_offset_pages == 0 {
            return Err(AslrError::InvalidRange);
        }

        self.entropy_seed = self.entropy_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let page_size = 4096;
        let random_pages = self.entropy_seed % max_offset_pages;
        let randomized_addr = base_address.wrapping_add(random_pages * page_size);

        let mut audit = audit_ring.lock();
        audit.log_event(
            pid,
            AuditAction::CapabilityCheckAllowed,
            CapabilityType::MemoryInspect,
            true,
            tick,
        );
        drop(audit);

        Ok(VirtAddr::new(randomized_addr))
    }
}
