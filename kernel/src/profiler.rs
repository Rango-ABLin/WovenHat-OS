//! Allocation-free kernel performance profiler (Stage 32 / WovenDev).
//!
//! Accumulates per-`(subsystem, name)` timing statistics from tracing span
//! events. Each entry holds count, sum, min and max TSC-cycle deltas so a
//! developer can see both the hottest and the slowest path at a glance.
//!
//! **Integration with tracing.** When both the `tracing` and `profiler`
//! features are enabled, [`crate::tracing::span_end`] automatically calls
//! [`record`] after emitting the structured span record; no manual
//! instrumentation is needed at the call site.
//!
//! **Zero-cost when disabled.** Every public function is an
//! `#[inline(always)]` no-op when the `profiler` feature is not enabled.
//!
//! **Design constraints** (same as [`crate::diagnostics`]):
//! * No allocation; all state lives in one static IRQ-safe mutex.
//! * At most [`MAX_SLOTS`] distinct `(subsystem, name)` pairs. Additional
//!   pairs are silently dropped; existing pairs continue accumulating.
//! * Keys are identified by pointer equality on the underlying `&'static str`
//!   literals; two distinct literals with identical text are treated as
//!   different keys. This is intentional — it prevents linear string scans on
//!   every `span_end` call.

use crate::irq_lock::IrqMutex;

/// Maximum number of distinct `(subsystem, name)` pairs the profiler tracks.
pub const MAX_SLOTS: usize = 64;

// ── Internal data ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct SlotData {
    subsystem: &'static str,
    name: &'static str,
    count: u64,
    sum_tsc: u64,
    min_tsc: u64,
    max_tsc: u64,
}

impl SlotData {
    const EMPTY: Self = Self {
        subsystem: "",
        name: "",
        count: 0,
        sum_tsc: 0,
        min_tsc: u64::MAX,
        max_tsc: 0,
    };
}

struct Table {
    slots: [SlotData; MAX_SLOTS],
    used: usize,
}

impl Table {
    const fn new() -> Self {
        Self {
            slots: [SlotData::EMPTY; MAX_SLOTS],
            used: 0,
        }
    }

    fn record(&mut self, subsystem: &'static str, name: &'static str, tsc: u64) {
        // Key lookup: pointer equality on the static string literals. This is
        // O(used) but avoids byte-by-byte content comparison in the fast path.
        for i in 0..self.used {
            if self.slots[i].subsystem.as_ptr() == subsystem.as_ptr()
                && self.slots[i].name.as_ptr() == name.as_ptr()
            {
                let s = &mut self.slots[i];
                s.count = s.count.saturating_add(1);
                s.sum_tsc = s.sum_tsc.saturating_add(tsc);
                if tsc < s.min_tsc {
                    s.min_tsc = tsc;
                }
                if tsc > s.max_tsc {
                    s.max_tsc = tsc;
                }
                return;
            }
        }
        if self.used < MAX_SLOTS {
            let s = &mut self.slots[self.used];
            s.subsystem = subsystem;
            s.name = name;
            s.count = 1;
            s.sum_tsc = tsc;
            s.min_tsc = tsc;
            s.max_tsc = tsc;
            self.used += 1;
        }
    }

    fn reset(&mut self) {
        for slot in &mut self.slots[..self.used] {
            *slot = SlotData::EMPTY;
        }
        self.used = 0;
    }
}

// Rank 3: the profiler holds no other locks, so any rank above 0 is safe.
// Benchmark holds rank 10; we stay lower so the profiler can be called from
// code that later acquires the benchmark lock without inverting the order.
static PROFILER: IrqMutex<Table> = IrqMutex::with_rank(Table::new(), 3);

// ── Active (profiler feature enabled) implementation ──────────────────────────

/// Accumulate one timing sample for `(subsystem, name)`.
///
/// Called automatically by [`crate::tracing::span_end`] when both `tracing`
/// and `profiler` features are enabled; may also be called directly.
#[cfg(all(feature = "profiler", not(test)))]
pub fn record(subsystem: &'static str, name: &'static str, tsc_delta: u64) {
    PROFILER.lock().record(subsystem, name, tsc_delta);
}

/// Clear all accumulated samples.
#[cfg(all(feature = "profiler", not(test)))]
pub fn reset() {
    PROFILER.lock().reset();
}

/// Emit all accumulated slots to the kernel serial log as structured
/// diagnostics records, one record per `(subsystem, name)` pair:
///
/// ```text
/// INFO profiler: slot | sub=<subsystem> | name=<name> | count=<n> | avg_tsc=<avg> | min_tsc=<min> | max_tsc=<max>
/// ```
#[cfg(all(feature = "profiler", not(test)))]
pub fn report() {
    use crate::diagnostics::{emit, Field, Level, Value};
    let table = PROFILER.lock();
    for slot in &table.slots[..table.used] {
        let avg = slot.sum_tsc.checked_div(slot.count).unwrap_or(0);
        emit(
            Level::Info,
            "profiler",
            "slot",
            &[
                Field::new("sub", Value::Str(slot.subsystem)),
                Field::new("name", Value::Str(slot.name)),
                Field::new("count", Value::U64(slot.count)),
                Field::new("avg_tsc", Value::U64(avg)),
                Field::new("min_tsc", Value::U64(slot.min_tsc)),
                Field::new("max_tsc", Value::U64(slot.max_tsc)),
            ],
        );
    }
}

// ── Zero-cost stubs ───────────────────────────────────────────────────────────

#[cfg(not(all(feature = "profiler", not(test))))]
#[inline(always)]
pub fn record(_subsystem: &'static str, _name: &'static str, _tsc_delta: u64) {}

#[cfg(not(all(feature = "profiler", not(test))))]
#[inline(always)]
pub fn reset() {}

#[cfg(not(all(feature = "profiler", not(test))))]
#[inline(always)]
pub fn report() {}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test. Verifies accumulation arithmetic, slot allocation,
/// and min/max/avg computation. Returns `true` on pass. When the `profiler`
/// feature is disabled, trivially returns `true`.
pub fn self_test() -> bool {
    // Use the inner Table directly so the test is cfg-independent and does
    // not rely on the IrqMutex or serial (no #[cfg(not(test))] needed).
    let mut t = Table::new();

    // Static string literals used as keys — pointer equality must hold.
    const SUB: &str = "self_test";
    const NAME_A: &str = "op_a";
    const NAME_B: &str = "op_b";

    // Record three samples for op_a: 100, 200, 300 cycles.
    t.record(SUB, NAME_A, 100);
    t.record(SUB, NAME_A, 200);
    t.record(SUB, NAME_A, 300);

    // Record one sample for op_b.
    t.record(SUB, NAME_B, 500);

    if t.used != 2 {
        return false;
    }

    let a = &t.slots[0];
    if a.count != 3 { return false; }
    if a.sum_tsc != 600 { return false; }
    if a.min_tsc != 100 { return false; }
    if a.max_tsc != 300 { return false; }
    if a.sum_tsc.checked_div(a.count) != Some(200) { return false; } // avg

    let b = &t.slots[1];
    if b.count != 1 { return false; }
    if b.min_tsc != 500 || b.max_tsc != 500 { return false; }

    // reset() must clear all slots.
    t.reset();
    if t.used != 0 { return false; }

    // Filling all MAX_SLOTS slots must not panic; a (MAX_SLOTS + 1)-th
    // distinct key is silently dropped.
    for i in 0..MAX_SLOTS {
        // Reuse the same two literals — we need distinct pointer-identified
        // keys, but since we can't create dynamic &'static strs here, just
        // record the same two keys many times (this verifies the count path,
        // not the overflow path, but avoids unsafe pointer trickery).
        let sub = if i % 2 == 0 { NAME_A } else { NAME_B };
        t.record(SUB, sub, i as u64 * 10);
    }
    // Two distinct (SUB, sub) pairs exist; used must be 2.
    if t.used != 2 { return false; }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulation_arithmetic() {
        let mut t = Table::new();
        const S: &str = "sub";
        const N: &str = "name";
        t.record(S, N, 10);
        t.record(S, N, 30);
        t.record(S, N, 20);
        let slot = &t.slots[0];
        assert_eq!(slot.count, 3);
        assert_eq!(slot.sum_tsc, 60);
        assert_eq!(slot.min_tsc, 10);
        assert_eq!(slot.max_tsc, 30);
        assert_eq!(slot.sum_tsc.checked_div(slot.count), Some(20));
    }

    #[test]
    fn separate_keys_get_separate_slots() {
        let mut t = Table::new();
        const S: &str = "sub";
        const A: &str = "a";
        const B: &str = "b";
        t.record(S, A, 1);
        t.record(S, B, 2);
        assert_eq!(t.used, 2);
        assert_eq!(t.slots[0].name.as_ptr(), A.as_ptr());
        assert_eq!(t.slots[1].name.as_ptr(), B.as_ptr());
    }

    #[test]
    fn reset_clears_all_slots() {
        let mut t = Table::new();
        const S: &str = "s";
        const N: &str = "n";
        t.record(S, N, 99);
        assert_eq!(t.used, 1);
        t.reset();
        assert_eq!(t.used, 0);
        assert_eq!(t.slots[0].count, 0);
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
