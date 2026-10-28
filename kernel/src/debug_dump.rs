//! Kernel debug dump: panic context combined with a ring-buffer snapshot
//! (Stage 32 / WovenDev).
//!
//! [`emit_dump`] is the single entry point: it writes a structured crash
//! context header (RIP / RSP / CR2) followed by the most recent
//! [`DRAIN_MAX`] entries from the kernel log ring to the serial port.
//! Crash handlers and post-mortem analysis tools can therefore see both
//! *where* the kernel faulted and *what it was doing* in the moments before
//! the fault — without replaying the full serial log.
//!
//! **Integration with the panic handler.** When the `debug-dump` feature is
//! enabled, the call site in [`crate::panic::kernel_panic`] can replace or
//! supplement the existing [`crate::diagnostics::emit_panic_dump`] call with
//! [`emit_dump`]. The two coexist safely; this module does not modify the
//! accepted panic output on its own.
//!
//! **Zero-cost when disabled.** [`emit_dump`] is an `#[inline(always)]` no-op
//! when the `debug-dump` feature is not enabled.
//!
//! **Design constraints:**
//! * All stack allocation is bounded: one 256-byte format buffer plus
//!   [`DRAIN_MAX`] × [`crate::log_ring::ENTRY_MAX`] bytes for the ring drain.
//! * [`emit_dump`] never acquires the ring lock more than once and releases
//!   it before writing to serial, so it is safe to call from IRQ context and
//!   from inside the panic handler.
//! * Serial writes use [`format_record`] + `serial::write_bytes` directly
//!   (bypassing `diagnostics::emit`) so that the dump lines do not recurse
//!   into the ring-push path.

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of ring entries included in one dump (tail of the ring, oldest
/// first). Capped below [`crate::log_ring::RING_CAPACITY`] to bound the stack
/// allocation in [`emit_dump`].
pub const DRAIN_MAX: usize = 8;

// ── Active (debug-dump feature enabled) production path ──────────────────────

#[cfg(all(feature = "debug-dump", not(test)))]
pub fn emit_dump(rip: u64, rsp: u64, cr2: u64) {
    use crate::diagnostics::{format_record, Field, Level, Value};
    use crate::log_ring;
    use crate::serial;

    let mut fmt_buf = [0u8; 256];

    // 1. Drain the ring into a local stack buffer BEFORE writing anything to
    //    serial.  This releases the ring lock before any further serial I/O
    //    and eliminates any risk of lock-order inversion.
    let mut out = [[0u8; log_ring::ENTRY_MAX]; DRAIN_MAX];
    let mut lens = [0usize; DRAIN_MAX];
    let n = log_ring::drain(&mut out, &mut lens);

    // 2. Emit the crash context header.
    let f = format_record(
        &mut fmt_buf,
        Level::Error,
        "debugger",
        "panic_context",
        &[
            Field::new("rip", Value::Hex(rip)),
            Field::new("rsp", Value::Hex(rsp)),
            Field::new("cr2", Value::Hex(cr2)),
        ],
    );
    serial::write_bytes(&fmt_buf[..f.written]);
    serial::write_bytes(b"\n");

    // 3. Emit the ring snapshot header.
    let f = format_record(
        &mut fmt_buf,
        Level::Debug,
        "debugger",
        "ring_snapshot",
        &[Field::new("count", Value::U64(n as u64))],
    );
    serial::write_bytes(&fmt_buf[..f.written]);
    serial::write_bytes(b"\n");

    // 4. Re-emit each drained entry.
    for i in 0..n {
        let record = core::str::from_utf8(&out[i][..lens[i]]).unwrap_or("(non-utf8)");
        let f = format_record(
            &mut fmt_buf,
            Level::Debug,
            "debugger",
            "ring_entry",
            &[
                Field::new("idx", Value::U64(i as u64)),
                Field::new("record", Value::Str(record)),
            ],
        );
        serial::write_bytes(&fmt_buf[..f.written]);
        serial::write_bytes(b"\n");
    }
}

// ── Zero-cost stub ────────────────────────────────────────────────────────────

#[cfg(not(all(feature = "debug-dump", not(test))))]
#[inline(always)]
pub fn emit_dump(_rip: u64, _rsp: u64, _cr2: u64) {}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test. Verifies that the crash-context record is formatted
/// correctly (correct level tag, subsystem, and hex fields). Uses
/// [`crate::diagnostics::format_record`] directly — no serial, no ring lock —
/// so the test is safe in both host and QEMU environments. Returns `true` on
/// pass.
#[cfg(any(test, feature = "stage32-6-test"))]
pub fn self_test() -> bool {
    use crate::diagnostics::{format_record, Field, Level, Value};

    const TEST_RIP: u64 = 0x0000_ffff_dead_c0de;
    const TEST_RSP: u64 = 0xffff_8000_0010_0000;
    const TEST_CR2: u64 = 0x0000_0000_bad0_0000;

    let mut buf = [0u8; 256];
    let f = format_record(
        &mut buf,
        Level::Error,
        "debugger",
        "panic_context",
        &[
            Field::new("rip", Value::Hex(TEST_RIP)),
            Field::new("rsp", Value::Hex(TEST_RSP)),
            Field::new("cr2", Value::Hex(TEST_CR2)),
        ],
    );

    if f.truncated {
        return false;
    }

    let out = match core::str::from_utf8(&buf[..f.written]) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Verify level tag and subsystem.
    if !out.starts_with("ERROR debugger: panic_context") {
        return false;
    }
    // Verify each hex field is present.  Value::Hex renders as 0x{v:x} (no
    // leading zeros beyond what the value requires), so:
    //   0x0000_ffff_dead_c0de → "0xffffdeadc0de"
    //   0xffff_8000_0010_0000 → "0xffff800000100000"
    //   0x0000_0000_bad0_0000 → "0xbad00000"
    if !out.contains("rip=0xffffdeadc0de") {
        return false;
    }
    if !out.contains("rsp=0xffff800000100000") {
        return false;
    }
    if !out.contains("cr2=0xbad00000") {
        return false;
    }

    // Verify ring_snapshot formatting with a known count.
    let f2 = format_record(
        &mut buf,
        Level::Debug,
        "debugger",
        "ring_snapshot",
        &[Field::new("count", Value::U64(2))],
    );
    if f2.truncated {
        return false;
    }
    let out2 = match core::str::from_utf8(&buf[..f2.written]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if !out2.starts_with("DEBUG debugger: ring_snapshot") {
        return false;
    }
    if !out2.contains("count=2") {
        return false;
    }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
