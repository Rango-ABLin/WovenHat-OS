//! Allocation-free kernel log ring buffer (Stage 32 / WovenDev).
//!
//! Stores the most recent [`RING_CAPACITY`] formatted diagnostic records in a
//! fixed-size circular buffer so crash handlers, debuggers, and boot-time
//! analysis can read recent kernel activity without replaying the serial log.
//!
//! **Integration with diagnostics.** When the `log-ring` feature is enabled,
//! [`crate::diagnostics::emit`] automatically calls [`push`] after writing to
//! the serial port; no call-site changes are needed.
//!
//! **Zero-cost when disabled.** [`push`] and [`len`] are `#[inline(always)]`
//! no-ops when the `log-ring` feature is not enabled. [`drain`] returns 0.
//!
//! **Design constraints:**
//! * No allocation; all state lives in one static IRQ-safe mutex.
//! * Exactly [`RING_CAPACITY`] slots; the oldest entry is overwritten silently
//!   when the ring is full.
//! * Each entry stores at most [`ENTRY_MAX`] bytes; longer records are silently
//!   truncated on push (already-formatted records from `emit` are bounded by
//!   `EMIT_BUFFER_BYTES` in diagnostics, so this is rarely reached).
//! * Keys: none — entries are pure byte slices with no metadata beyond length.

#[cfg(all(feature = "log-ring", not(test)))]
use crate::irq_lock::IrqMutex;

/// Number of entries the ring can hold before the oldest is overwritten.
pub const RING_CAPACITY: usize = 64;

/// Maximum bytes stored per entry; longer pushes are truncated.
pub const ENTRY_MAX: usize = 256;

// ── Internal data ─────────────────────────────────────────────────────────────

#[cfg(any(test, feature = "log-ring"))]
#[derive(Clone, Copy)]
struct Entry {
    len: usize,
    data: [u8; ENTRY_MAX],
}

#[cfg(any(test, feature = "log-ring"))]
impl Entry {
    const EMPTY: Self = Self {
        len: 0,
        data: [0u8; ENTRY_MAX],
    };
}

#[cfg(any(test, feature = "log-ring"))]
struct Ring {
    entries: [Entry; RING_CAPACITY],
    /// Monotone write counter (never wraps to keep overflow logic simple).
    total: u64,
}

#[cfg(any(test, feature = "log-ring"))]
impl Ring {
    const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; RING_CAPACITY],
            total: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        let idx = (self.total as usize) % RING_CAPACITY;
        let copy_len = bytes.len().min(ENTRY_MAX);
        let e = &mut self.entries[idx];
        e.data[..copy_len].copy_from_slice(&bytes[..copy_len]);
        e.len = copy_len;
        self.total = self.total.saturating_add(1);
    }

    fn count(&self) -> usize {
        (self.total as usize).min(RING_CAPACITY)
    }

    /// Index of the oldest valid entry.
    fn start(&self) -> usize {
        if self.total < RING_CAPACITY as u64 {
            0
        } else {
            (self.total as usize) % RING_CAPACITY
        }
    }

    /// Copy entries oldest-first into caller buffers; returns count copied.
    fn drain(&self, out: &mut [[u8; ENTRY_MAX]], lens: &mut [usize]) -> usize {
        let n = self.count().min(out.len()).min(lens.len());
        let start = self.start();
        for i in 0..n {
            let idx = (start + i) % RING_CAPACITY;
            let e = &self.entries[idx];
            out[i][..e.len].copy_from_slice(&e.data[..e.len]);
            lens[i] = e.len;
        }
        n
    }
}

// Rank 2: the ring holds no other locks; keep below profiler (3) and
// diagnostics callers so they can push from inside an emit() call.
#[cfg(all(feature = "log-ring", not(test)))]
static RING: IrqMutex<Ring> = IrqMutex::with_rank(Ring::new(), 2);

// ── Active (log-ring feature enabled) implementation ─────────────────────────

/// Push a formatted record into the ring. Called automatically by
/// [`crate::diagnostics::emit`] when the `log-ring` feature is enabled.
#[cfg(all(feature = "log-ring", not(test)))]
pub fn push(bytes: &[u8]) {
    RING.lock().push(bytes);
}

/// Number of valid entries currently held in the ring (0 … [`RING_CAPACITY`]).
#[cfg(all(feature = "log-ring", not(test)))]
pub fn len() -> usize {
    RING.lock().count()
}

/// Copy entries oldest-first into caller-provided buffers. Returns the number
/// of entries written; stops when `out` or `lens` is exhausted.
#[cfg(all(feature = "log-ring", not(test)))]
pub fn drain(out: &mut [[u8; ENTRY_MAX]], lens: &mut [usize]) -> usize {
    RING.lock().drain(out, lens)
}

// ── Zero-cost stubs ───────────────────────────────────────────────────────────

#[cfg(not(all(feature = "log-ring", not(test))))]
#[inline(always)]
pub fn push(_bytes: &[u8]) {}

#[cfg(not(all(feature = "log-ring", not(test))))]
#[inline(always)]
pub fn len() -> usize {
    0
}

#[cfg(not(all(feature = "log-ring", not(test))))]
#[inline(always)]
pub fn drain(_out: &mut [[u8; ENTRY_MAX]], _lens: &mut [usize]) -> usize {
    0
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test. Verifies push/drain ordering, entry truncation, and
/// overflow (oldest entry dropped when the ring is full). Uses a local [`Ring`]
/// so the test is cfg-independent and never touches the static IRQ mutex.
/// Returns `true` on pass.
#[cfg(any(test, feature = "stage32-5-test"))]
pub fn self_test() -> bool {
    let mut r = Ring::new();

    // Empty ring.
    if r.count() != 0 {
        return false;
    }

    // Push two entries and drain in order.
    r.push(b"first");
    r.push(b"second");
    if r.count() != 2 {
        return false;
    }

    let mut out = [[0u8; ENTRY_MAX]; 4];
    let mut lens = [0usize; 4];
    let n = r.drain(&mut out, &mut lens);
    if n != 2 {
        return false;
    }
    if &out[0][..lens[0]] != b"first" {
        return false;
    }
    if &out[1][..lens[1]] != b"second" {
        return false;
    }

    // Fill to exactly capacity; no overflow yet.
    let mut r2 = Ring::new();
    for i in 0..RING_CAPACITY {
        r2.push(&[i as u8]);
    }
    if r2.count() != RING_CAPACITY {
        return false;
    }
    // Oldest entry is [0]; start() must be 0.
    if r2.start() != 0 {
        return false;
    }

    // One more push overflows: oldest (0x00) is replaced by 0x40.
    r2.push(&[0x40]);
    if r2.count() != RING_CAPACITY {
        return false;
    }
    // start() now points past the overwritten slot.
    let mut out2 = [[0u8; ENTRY_MAX]; RING_CAPACITY];
    let mut lens2 = [0usize; RING_CAPACITY];
    let n2 = r2.drain(&mut out2, &mut lens2);
    if n2 != RING_CAPACITY {
        return false;
    }
    // Oldest is now [1] (the 0x00 slot was overwritten).
    if out2[0][0] != 1 {
        return false;
    }
    // Newest is [0x40].
    if out2[RING_CAPACITY - 1][0] != 0x40 {
        return false;
    }

    // Truncation: pushing ENTRY_MAX+1 bytes stores only ENTRY_MAX.
    let long = [0xab_u8; ENTRY_MAX + 10];
    let mut r3 = Ring::new();
    r3.push(&long);
    let mut out3 = [[0u8; ENTRY_MAX]; 1];
    let mut lens3 = [0usize; 1];
    r3.drain(&mut out3, &mut lens3);
    if lens3[0] != ENTRY_MAX {
        return false;
    }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ring_count_is_zero() {
        let r = Ring::new();
        assert_eq!(r.count(), 0);
    }

    #[test]
    fn push_and_drain_order() {
        let mut r = Ring::new();
        r.push(b"a");
        r.push(b"b");
        r.push(b"c");
        let mut out = [[0u8; ENTRY_MAX]; 3];
        let mut lens = [0usize; 3];
        let n = r.drain(&mut out, &mut lens);
        assert_eq!(n, 3);
        assert_eq!(&out[0][..lens[0]], b"a");
        assert_eq!(&out[1][..lens[1]], b"b");
        assert_eq!(&out[2][..lens[2]], b"c");
    }

    #[test]
    fn overflow_drops_oldest() {
        let mut r = Ring::new();
        for i in 0..=RING_CAPACITY {
            r.push(&[i as u8]);
        }
        assert_eq!(r.count(), RING_CAPACITY);
        let mut out = [[0u8; ENTRY_MAX]; RING_CAPACITY];
        let mut lens = [0usize; RING_CAPACITY];
        let n = r.drain(&mut out, &mut lens);
        assert_eq!(n, RING_CAPACITY);
        // Entry 0 was overwritten; oldest is now entry 1.
        assert_eq!(out[0][0], 1);
        assert_eq!(out[RING_CAPACITY - 1][0], RING_CAPACITY as u8);
    }

    #[test]
    fn truncation_at_entry_max() {
        let long = [0xcc_u8; ENTRY_MAX + 5];
        let mut r = Ring::new();
        r.push(&long);
        let mut out = [[0u8; ENTRY_MAX]; 1];
        let mut lens = [0usize; 1];
        r.drain(&mut out, &mut lens);
        assert_eq!(lens[0], ENTRY_MAX);
        assert!(out[0].iter().all(|&b| b == 0xcc));
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
