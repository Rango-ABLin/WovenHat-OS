//! Deterministic physical-memory pressure classification.
//!
//! This module does not reclaim memory itself. It is the allocation boundary's
//! bounded policy signal: reclaimers can observe a stable level, while an
//! allocation failure is recorded as an explicit OOM decision.

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Level {
    Normal = 0,
    Moderate = 1,
    Critical = 2,
    Exhausted = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub level: Level,
    pub remaining_frames: u64,
    pub total_frames: u64,
    pub transitions: u64,
    pub allocation_failures: u64,
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Normal as u8);
static REMAINING: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static TRANSITIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_FAILURES: AtomicU64 = AtomicU64::new(0);

const fn classify(remaining: u64, total: u64) -> Level {
    if total == 0 || remaining == 0 {
        Level::Exhausted
    } else if remaining.saturating_mul(100) <= total.saturating_mul(5) {
        Level::Critical
    } else if remaining.saturating_mul(100) <= total.saturating_mul(20) {
        Level::Moderate
    } else {
        Level::Normal
    }
}

pub fn observe(remaining: u64, total: u64) -> Level {
    let next = classify(remaining, total);
    REMAINING.store(remaining, Ordering::Release);
    TOTAL.store(total, Ordering::Release);
    let previous = LEVEL.swap(next as u8, Ordering::AcqRel);
    if previous != next as u8 {
        TRANSITIONS.fetch_add(1, Ordering::Relaxed);
    }
    next
}

pub fn allocation_failed(remaining: u64, total: u64) {
    observe(remaining, total);
    ALLOCATION_FAILURES.fetch_add(1, Ordering::Relaxed);
}

pub fn snapshot() -> Snapshot {
    let level = match LEVEL.load(Ordering::Acquire) {
        0 => Level::Normal,
        1 => Level::Moderate,
        2 => Level::Critical,
        _ => Level::Exhausted,
    };
    Snapshot {
        level,
        remaining_frames: REMAINING.load(Ordering::Acquire),
        total_frames: TOTAL.load(Ordering::Acquire),
        transitions: TRANSITIONS.load(Ordering::Acquire),
        allocation_failures: ALLOCATION_FAILURES.load(Ordering::Acquire),
    }
}

#[cfg(feature = "s05-memory-pressure-test")]
pub fn self_test() -> bool {
    classify(80, 100) == Level::Normal
        && classify(20, 100) == Level::Moderate
        && classify(5, 100) == Level::Critical
        && classify(0, 100) == Level::Exhausted
        && classify(0, 0) == Level::Exhausted
        && observe(80, 100) == Level::Normal
        && observe(5, 100) == Level::Critical
        && snapshot().level == Level::Critical
}
