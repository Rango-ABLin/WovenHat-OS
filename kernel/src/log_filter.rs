//! Minimum-severity filter for [`crate::diagnostics::emit`] (Stage 32 / WovenDev).
//!
//! A single global `AtomicU8` holds the minimum [`Level`] threshold.  Records
//! whose level is *less severe* than (strictly greater than, in the `Level`
//! ordering) the threshold are dropped before any formatting, serial write, or
//! ring push.  No lock is needed: the store and load use `Relaxed` ordering,
//! which is sufficient for a best-effort advisory gate — a record that races
//! a threshold change may or may not be emitted, and that is intentional.
//!
//! **Default level**: [`Level::Debug`] — passes `Error`, `Warn`, `Info`, and
//! `Debug`; drops only `Trace`.  The default is conservative so no diagnostic
//! activity is invisible unless explicitly silenced.
//!
//! **Zero-cost when disabled.** When the `log-filter` feature is not enabled,
//! [`diagnostics::emit`] does not check the threshold at all; the filter
//! functions are still callable but only the `set_min_level`/`min_level`
//! stubs below exist — they are `#[inline(always)]` no-ops.
//!
//! **Thread safety.** `AtomicU8` is safe to read/write from any CPU and from
//! interrupt context.  Concurrent calls are linearised by the hardware
//! atomicity of the byte load/store.

use crate::diagnostics::Level;

// ── Active (log-filter feature enabled) implementation ────────────────────────

#[cfg(feature = "log-filter")]
mod active {
    use core::sync::atomic::{AtomicU8, Ordering};
    use crate::diagnostics::Level;

    // Level is #[repr(u8)] so the discriminants are 0..4 in declaration order.
    pub const fn level_to_u8(l: Level) -> u8 {
        l as u8
    }

    pub fn u8_to_level(v: u8) -> Level {
        match v {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            // Any value ≥ 4 maps to Trace — a safe fallback that passes all
            // records rather than silently dropping them.
            _ => Level::Trace,
        }
    }

    // Default: Level::Debug (= 3). Drops only Trace.
    static MIN_LEVEL: AtomicU8 = AtomicU8::new(level_to_u8(Level::Debug));

    pub fn set(level: Level) {
        MIN_LEVEL.store(level_to_u8(level), Ordering::Relaxed);
    }

    pub fn get() -> Level {
        u8_to_level(MIN_LEVEL.load(Ordering::Relaxed))
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Set the minimum severity level.  Records with a lower severity (higher
/// enum value) than `level` will be dropped by [`crate::diagnostics::emit`]
/// when the `log-filter` feature is active.
#[cfg(feature = "log-filter")]
pub fn set_min_level(level: Level) {
    active::set(level);
}

/// Read the current minimum severity level.
#[cfg(feature = "log-filter")]
pub fn min_level() -> Level {
    active::get()
}

// ── Zero-cost stubs ───────────────────────────────────────────────────────────

#[cfg(not(feature = "log-filter"))]
#[inline(always)]
pub fn set_min_level(_level: Level) {}

#[cfg(not(feature = "log-filter"))]
#[inline(always)]
pub fn min_level() -> Level {
    Level::Trace // pass-through when filter is disabled
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test.  Verifies the `Level` ordering assumptions the filter
/// relies on, and confirms that `set_min_level` / `min_level` round-trip
/// correctly.  Returns `true` on pass.
#[cfg(any(test, feature = "stage32-7-test"))]
pub fn self_test() -> bool {
    // Level ordering: Error is most severe (lowest discriminant).
    if !(Level::Error < Level::Warn) { return false; }
    if !(Level::Warn  < Level::Info) { return false; }
    if !(Level::Info  < Level::Debug) { return false; }
    if !(Level::Debug < Level::Trace) { return false; }

    // Filter logic: with min = Info, Error/Warn/Info pass and Debug/Trace drop.
    let min = Level::Info;
    // Pass (level ≤ min means "at least as severe as threshold"):
    if Level::Error > min { return false; }
    if Level::Warn  > min { return false; }
    if Level::Info  > min { return false; }
    // Drop (level > min means "less severe than threshold"):
    if !(Level::Debug > min) { return false; }
    if !(Level::Trace > min) { return false; }

    // get/set round-trip — only testable when the atomics are live.
    #[cfg(feature = "log-filter")]
    {
        let saved = min_level();
        set_min_level(Level::Warn);
        if min_level() != Level::Warn { return false; }
        set_min_level(Level::Trace);
        if min_level() != Level::Trace { return false; }
        set_min_level(saved);
    }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ordering_matches_severity() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn  < Level::Info);
        assert!(Level::Info  < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }

    #[test]
    #[cfg(feature = "log-filter")]
    fn set_get_round_trip() {
        let orig = min_level();
        set_min_level(Level::Error);
        assert_eq!(min_level(), Level::Error);
        set_min_level(Level::Trace);
        assert_eq!(min_level(), Level::Trace);
        set_min_level(orig);
    }

    #[test]
    fn filter_logic_info_threshold() {
        let min = Level::Info;
        assert!(!(Level::Error > min)); // passes
        assert!(!(Level::Warn  > min)); // passes
        assert!(!(Level::Info  > min)); // passes
        assert!(  Level::Debug > min);  // dropped
        assert!(  Level::Trace > min);  // dropped
    }

    #[test]
    fn u8_round_trip_all_levels() {
        for l in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
            assert_eq!(u8_to_level(level_to_u8(l)), l);
        }
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
