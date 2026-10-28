//! Zero-cost kernel tracing span API (Stage 32 / WovenDev).
//!
//! When the `tracing` feature is enabled, every [`SpanGuard::enter`] /
//! [`SpanGuard`] drop pair emits two structured records to the serial log
//! via [`crate::diagnostics`]:
//!
//! ```text
//! TRACE <subsystem>: span_begin <name> | id=<n>
//! TRACE <subsystem>: span_end <name>   | id=<n> | tsc=<cycles>
//! ```
//!
//! When `tracing` is disabled (the default production build), all public
//! functions are `#[inline(always)]` no-ops; the compiler eliminates them
//! entirely and no span counter or TSC read ever executes.
//!
//! Design constraints (same as [`crate::diagnostics`]):
//! * **No allocation.** Stack-only, safe from interrupt context.
//! * **Fail-closed.** A span whose record exceeds the diagnostics buffer is
//!   truncated with `...` rather than silently dropped.
//! * **Zero-cost when disabled.** Every call site compiles to nothing without
//!   the feature.

#[cfg(any(test, feature = "tracing"))]
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(feature = "tracing", not(test)))]
use crate::diagnostics::{Field, Level, Value};

/// Monotone span-ID counter. Starts at 1 so that id=0 is always invalid.
#[cfg(any(test, feature = "tracing"))]
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

// ── TSC read (available on x86_64 regardless of privilege level) ─────────────

#[cfg(any(test, feature = "tracing"))]
fn rdtsc() -> u64 {
    // SAFETY: RDTSC is unprivileged on x86_64 and does not access memory or
    // modify any register except the outputs.
    unsafe { core::arch::x86_64::_rdtsc() }
}

// ── Active (tracing feature enabled) implementation ──────────────────────────

/// Emit a span-begin record and return `(span_id, tsc_start)` for the paired
/// [`span_end`] call.
///
/// `span_id` is a session-unique, monotone counter value.
#[cfg(all(feature = "tracing", not(test)))]
pub fn span_begin(subsystem: &'static str, name: &'static str) -> (u64, u64) {
    let id = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tsc = rdtsc();
    crate::diagnostics::emit(
        Level::Trace,
        subsystem,
        "span_begin",
        &[
            Field::new("name", Value::Str(name)),
            Field::new("id", Value::U64(id)),
        ],
    );
    (id, tsc)
}

/// Emit a span-end record with the TSC cycle count elapsed since the paired
/// [`span_begin`] call. When the `profiler` feature is also enabled, the
/// delta is forwarded to [`crate::profiler::record`] for statistical
/// accumulation.
#[cfg(all(feature = "tracing", not(test)))]
pub fn span_end(subsystem: &'static str, name: &'static str, id: u64, tsc_start: u64) {
    let tsc_delta = rdtsc().wrapping_sub(tsc_start);
    crate::diagnostics::emit(
        Level::Trace,
        subsystem,
        "span_end",
        &[
            Field::new("name", Value::Str(name)),
            Field::new("id", Value::U64(id)),
            Field::new("tsc", Value::U64(tsc_delta)),
        ],
    );
    #[cfg(feature = "profiler")]
    crate::profiler::record(subsystem, name, tsc_delta);
}

// ── Zero-cost stubs (tracing feature disabled or host-test context) ───────────

/// No-op stub — returns `(0, 0)` and emits nothing.
#[cfg(not(all(feature = "tracing", not(test))))]
#[inline(always)]
pub fn span_begin(_subsystem: &'static str, _name: &'static str) -> (u64, u64) {
    (0, 0)
}

/// No-op stub.
#[cfg(not(all(feature = "tracing", not(test))))]
#[inline(always)]
pub fn span_end(
    _subsystem: &'static str,
    _name: &'static str,
    _id: u64,
    _tsc_start: u64,
) {
}

// ── RAII guard ────────────────────────────────────────────────────────────────

/// An RAII span guard. Emits `span_begin` on construction and `span_end` on
/// drop. When tracing is disabled the type is a zero-sized no-op and the
/// compiler eliminates every use of it.
///
/// # Example
///
/// ```rust,ignore
/// let _s = tracing::SpanGuard::enter("sched", "schedule");
/// // ... code to instrument ...
/// // span_end emitted when _s drops at end of scope
/// ```
pub struct SpanGuard {
    #[cfg(all(feature = "tracing", not(test)))]
    subsystem: &'static str,
    #[cfg(all(feature = "tracing", not(test)))]
    name: &'static str,
    #[cfg(all(feature = "tracing", not(test)))]
    id: u64,
    #[cfg(all(feature = "tracing", not(test)))]
    tsc_start: u64,
}

impl SpanGuard {
    /// Enter a span. Returns a guard that emits `span_end` when dropped.
    #[inline]
    pub fn enter(subsystem: &'static str, name: &'static str) -> Self {
        #[cfg(all(feature = "tracing", not(test)))]
        {
            let (id, tsc_start) = span_begin(subsystem, name);
            SpanGuard { subsystem, name, id, tsc_start }
        }
        #[cfg(not(all(feature = "tracing", not(test))))]
        {
            let _ = (subsystem, name);
            SpanGuard {}
        }
    }
}

impl Drop for SpanGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(all(feature = "tracing", not(test)))]
        span_end(self.subsystem, self.name, self.id, self.tsc_start);
    }
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test. Verifies the span-ID counter increments and that the
/// TSC reads are non-zero and non-decreasing. When the `tracing` feature is
/// disabled this always returns `true` immediately. Intended to be wired
/// behind `stage32-3-test`.
#[cfg(any(test, feature = "stage32-3-test"))]
pub fn self_test() -> bool {
    // Span counter must increment monotonically.
    let before = SPAN_COUNTER.load(Ordering::Relaxed);
    let id1 = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let id2 = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    if id2 != id1 + 1 {
        return false;
    }
    if SPAN_COUNTER.load(Ordering::Relaxed) != before + 2 {
        return false;
    }

    // TSC reads must be non-zero and non-decreasing.
    let t1 = rdtsc();
    let t2 = rdtsc();
    if t1 == 0 {
        return false;
    }
    if t2 < t1 {
        return false;
    }

    // Exercise the full emit path in kernel mode.
    #[cfg(not(test))]
    {
        let (id3, tsc3) = span_begin("tracing", "self_test");
        span_end("tracing", "self_test", id3, tsc3);
        let _guard = SpanGuard::enter("tracing", "self_test_guard");
    }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_increments_monotonically() {
        let a = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let b = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
        assert_eq!(b, a + 1);
    }

    #[test]
    fn rdtsc_is_nonzero_and_nondecreasing() {
        let t1 = rdtsc();
        let t2 = rdtsc();
        assert!(t1 != 0, "TSC should be non-zero");
        assert!(t2 >= t1, "TSC must be non-decreasing");
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }

    #[test]
    fn span_guard_is_zero_sized_without_tracing() {
        // Without tracing, SpanGuard must be a ZST so the compiler can
        // eliminate it entirely.
        #[cfg(not(feature = "tracing"))]
        assert_eq!(core::mem::size_of::<SpanGuard>(), 0);
        // With tracing enabled we still want the guard to fit on the stack —
        // just verify it compiles and can be constructed and dropped.
        let _g = SpanGuard::enter("test", "guard");
    }
}
