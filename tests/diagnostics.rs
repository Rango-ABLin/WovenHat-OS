//! Host-side coverage for the kernel structured-diagnostics primitive.
//!
//! The kernel bin sets `test = false`, so its modules' `#[cfg(test)]` tests run
//! through an explicit integration crate (see the other files in `tests/`).
//! `diagnostics` depends only on `core::fmt` under `cfg(test)` — its `emit`
//! helper is `cfg(not(test))` and compiled out here — so no kernel doubles are
//! needed; including the source is enough to run its own test module.
#![allow(dead_code)]

#[path = "../kernel/src/diagnostics.rs"]
mod diagnostics;

use diagnostics::{format_record, Field, Level, Value, TRUNCATION_MARKER};

/// A second, crate-level check that the public API composes as documented,
/// independent of the module's in-file tests.
#[test]
fn integration_record_round_trips() {
    let mut buf = [0u8; 128];
    let f = format_record(
        &mut buf,
        Level::Warn,
        "sched",
        "preempt",
        &[
            Field::new("cpu", Value::U64(2)),
            Field::new("task", Value::Hex(0x41)),
        ],
    );
    assert!(!f.truncated);
    let text = core::str::from_utf8(&buf[..f.written]).unwrap();
    assert_eq!(text, "WARN sched: preempt | cpu=2 | task=0x41");
    assert!(!text.ends_with(TRUNCATION_MARKER));
}

#[test]
fn integration_self_test() {
    assert!(diagnostics::self_test());
}
