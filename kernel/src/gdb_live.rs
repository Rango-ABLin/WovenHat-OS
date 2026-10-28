//! Interrupt-safe, scheduler-neutral debugger trap handoff.
//!
//! The handler records one stop and returns immediately. A scheduler-owned
//! safe point consumes it later; no IRQ handler takes a lock, sleeps, or
//! switches tasks.

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use crate::gdb_trap::{self, StopEvent};

static PENDING: AtomicU64 = AtomicU64::new(0);

pub fn record(vector: u8, error_code: Option<u64>) -> bool {
    let packed = ((error_code.unwrap_or(0) & 0x00ff_ffff_ffff_ffff) << 8) | u64::from(vector);
    PENDING
        .compare_exchange(0, packed, Ordering::Release, Ordering::Relaxed)
        .is_ok()
}

pub fn take() -> Option<StopEvent> {
    let packed = PENDING.swap(0, Ordering::Acquire);
    let vector = (packed & 0xff) as u8;
    if vector == 0 {
        return None;
    }
    Some(gdb_trap::classify(vector, Some(packed >> 8)))
}

#[cfg(any(test, feature = "stage32-integrated-test"))]
pub fn self_test() -> bool {
    let _ = take();
    if !record(3, None) || record(1, Some(1)) {
        return false;
    }
    let Some(event) = take() else {
        return false;
    };
    event.vector == 3 && event.signal == 5 && take().is_none()
}
