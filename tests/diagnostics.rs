//! Host-side coverage for the kernel structured-diagnostics primitive.
//!
//! The kernel bin sets `test = false`, so its modules' `#[cfg(test)]` tests run
//! through an explicit integration crate (see the other files in `tests/`).
//! The panic emission path uses a recording serial double so the host checks
//! the actual bytes emitted by the production formatter without port I/O.
#![allow(dead_code)]

#[path = "../kernel/src/diagnostics.rs"]
mod diagnostics;

use diagnostics::{Field, Level, TRUNCATION_MARKER, Value, format_record};

mod serial {
    use std::cell::RefCell;

    thread_local! {
        static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    pub fn write_bytes(bytes: &[u8]) {
        OUTPUT.with_borrow_mut(|output| output.extend_from_slice(bytes));
    }

    pub fn take_output() -> Vec<u8> {
        OUTPUT.with_borrow_mut(std::mem::take)
    }
}

#[test]
fn panic_emission_includes_registers_and_one_newline() {
    assert!(serial::take_output().is_empty());
    diagnostics::emit_panic_dump(0xdead_beef, 0xffff_c000_0000_0000, 0x1234);
    assert_eq!(
        serial::take_output(),
        b"ERROR panic: kernel panic | rip=0xdeadbeef | rsp=0xffffc00000000000 | cr2=0x1234\n"
    );
}

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
