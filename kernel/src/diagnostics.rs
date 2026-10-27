//! Structured diagnostics (Stage 32 / WovenDev foundation).
//!
//! WovenHat's existing kernel log ([`crate::serial`]) and panic report
//! ([`crate::panic`]) emit free-form text. Structured diagnostics add a
//! bounded, allocation-free record format that carries a severity level, an
//! originating subsystem, a human message, and up to [`MAX_FIELDS`]
//! `key=value` fields, serialised into a caller-owned buffer as one
//! deterministic, machine-parseable line:
//!
//! ```text
//! <LEVEL> <subsystem>: <message>[ | key=value key=value ...]
//! ```
//!
//! Design goals that match the rest of the kernel:
//!
//! * **No allocation.** Formatting writes into a caller `&mut [u8]`, so the
//!   same path is usable from interrupt context and, later, from the panic
//!   handler.
//! * **Fail-closed and bounded.** A record that would exceed the caller's
//!   buffer is truncated and the output ends with [`TRUNCATION_MARKER`] so a
//!   reader can tell the line was cut. Nothing is ever written out of bounds.
//! * **Deterministic.** Field order is the caller's order; the grammar above is
//!   stable, so a log reader or crash-dump parser can rely on it.
//!
//! This module is the primitive; wiring it into the boot log, the panic
//! handler and a tracing span API are separate increments that each carry
//! their own QEMU acceptance (the accepted `serial`/`panic` output is left
//! unchanged here).


use core::fmt::{self, Write};

/// Maximum number of `key=value` fields serialised per record. Extra fields are
/// dropped before the truncation marker so the grammar stays well-formed.
pub const MAX_FIELDS: usize = 8;

/// ASCII marker appended when a record did not fit the caller's buffer.
pub const TRUNCATION_MARKER: &str = "...";

/// Severity of a diagnostic record, ordered from most to least severe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    /// Stable upper-case tag used as the first token of a record.
    pub const fn tag(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

/// A single `key=value` field value. Integers are formatted without allocation
/// through `core::fmt`.
#[derive(Clone, Copy, Debug)]
pub enum Value<'a> {
    Str(&'a str),
    U64(u64),
    I64(i64),
    /// Rendered as `0x…` lower-case hex (for addresses, flags, handles).
    Hex(u64),
    Bool(bool),
}

/// One structured field. `key` should be a short, stable identifier
/// (`pid`, `addr`, `errno`); it is written verbatim.
#[derive(Clone, Copy, Debug)]
pub struct Field<'a> {
    pub key: &'a str,
    pub value: Value<'a>,
}

impl<'a> Field<'a> {
    pub const fn new(key: &'a str, value: Value<'a>) -> Self {
        Self { key, value }
    }
}

/// Outcome of formatting a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formatted {
    /// Number of bytes written to the caller's buffer (always `<= buf.len()`).
    pub written: usize,
    /// Whether the record was cut to fit; when true the output ends with
    /// [`TRUNCATION_MARKER`].
    pub truncated: bool,
}

/// A bounded writer over a byte buffer. Once the buffer fills it records the
/// overflow and silently drops further bytes, so no write is ever out of
/// bounds. It never writes a partial UTF-8 sequence: a `write_str` that would
/// not fit whole is dropped whole.
struct BoundedWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
    overflow: bool,
}

impl<'a> BoundedWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            len: 0,
            overflow: false,
        }
    }
}

impl Write for BoundedWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let bytes = text.as_bytes();
        if bytes.len() > self.buf.len() - self.len {
            // Drop the whole fragment so we never split a UTF-8 code point or a
            // field in half; mark the record as truncated.
            self.overflow = true;
            return Err(fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

/// Serialise one structured record into `buf`.
///
/// Returns the number of bytes written and whether the record was truncated to
/// fit. At most [`MAX_FIELDS`] fields are emitted; any beyond that are dropped
/// exactly as an over-long buffer drops trailing fields, and both cases set
/// [`Formatted::truncated`]. A buffer too small even for the marker yields a
/// zero-length, truncated result.
pub fn format_record(
    buf: &mut [u8],
    level: Level,
    subsystem: &str,
    message: &str,
    fields: &[Field<'_>],
) -> Formatted {
    // Reserve room for the truncation marker so it can always be appended.
    let marker = TRUNCATION_MARKER.as_bytes();
    let reserve = marker.len();
    let capacity = buf.len();
    let body_cap = capacity.saturating_sub(reserve);

    let mut extra_fields_dropped = false;
    let written;
    {
        // The body writer may use at most `body_cap` bytes, leaving space for
        // the marker regardless of how full it gets.
        let (body, _) = buf.split_at_mut(body_cap);
        let mut writer = BoundedWriter::new(body);

        // Header: "<LEVEL> <subsystem>: <message>".
        let _ = write!(writer, "{} {subsystem}: {message}", level.tag());

        let emit = fields.len().min(MAX_FIELDS);
        if fields.len() > emit {
            extra_fields_dropped = true;
        }
        for field in &fields[..emit] {
            // Checkpoint so a field that does not fit whole can be rolled back:
            // `write!` renders a field as several fragments ("| ", key, "=",
            // value), and the buffer may fill mid-field. Discarding the partial
            // field keeps the grammar well-formed (no dangling `key=`).
            let checkpoint = writer.len;
            let result = match field.value {
                Value::Str(v) => write!(writer, " | {}={v}", field.key),
                Value::U64(v) => write!(writer, " | {}={v}", field.key),
                Value::I64(v) => write!(writer, " | {}={v}", field.key),
                Value::Hex(v) => write!(writer, " | {}=0x{v:x}", field.key),
                Value::Bool(v) => write!(writer, " | {}={v}", field.key),
            };
            if result.is_err() {
                // Buffer full: drop this partial field and stop at the boundary.
                writer.len = checkpoint;
                writer.overflow = true;
                break;
            }
        }

        let truncated = writer.overflow || extra_fields_dropped;
        written = writer.len;

        if !truncated {
            return Formatted {
                written,
                truncated: false,
            };
        }
    }

    // Append the marker after the body. `body_cap` guaranteed `reserve` free
    // bytes beyond `written`, so this stays in bounds.
    let end = written + marker.len();
    buf[written..end].copy_from_slice(marker);
    Formatted {
        written: end,
        truncated: true,
    }
}

/// Emit a structured record to the kernel serial log.
///
/// Formats into a fixed stack buffer (so it never allocates and is safe from
/// interrupt context) and writes one line. Records longer than
/// [`EMIT_BUFFER_BYTES`] are truncated with [`TRUNCATION_MARKER`].
#[cfg(not(test))]
pub fn emit(level: Level, subsystem: &str, message: &str, fields: &[Field<'_>]) {
    /// Fixed line budget for a serial diagnostic record.
    const EMIT_BUFFER_BYTES: usize = 512;
    let mut buf = [0u8; EMIT_BUFFER_BYTES];
    let formatted = format_record(&mut buf, level, subsystem, message, fields);
    crate::serial::write_bytes(&buf[..formatted.written]);
    crate::serial::write_bytes(b"\n");
}

/// Boot-time self-test for the structured-record contract. Returns `true` when
/// header formatting, field serialisation, hex rendering, field-count capping
/// and buffer-overflow truncation all behave as documented. Intended to be
/// wired behind a `stage32-*` feature once a QEMU gate is available; it is also
/// exercised directly by the host unit tests below.
#[cfg(any(test, feature = "stage32-1-test"))]
pub fn self_test() -> bool {
    let mut buf = [0u8; 128];

    // Plain header, no fields.
    let f = format_record(&mut buf, Level::Info, "boot", "ready", &[]);
    if f.truncated || &buf[..f.written] != b"INFO boot: ready" {
        return false;
    }

    // Mixed fields including hex, decimal and bool.
    let f = format_record(
        &mut buf,
        Level::Error,
        "paging",
        "fault",
        &[
            Field::new("addr", Value::Hex(0xdead_beef)),
            Field::new("pid", Value::U64(7)),
            Field::new("user", Value::Bool(true)),
        ],
    );
    if f.truncated
        || &buf[..f.written] != b"ERROR paging: fault | addr=0xdeadbeef | pid=7 | user=true"
    {
        return false;
    }

    // Too many fields are dropped and the record is marked truncated.
    let many = [Field::new("k", Value::U64(1)); MAX_FIELDS + 2];
    let f = format_record(&mut buf, Level::Trace, "s", "m", &many);
    if !f.truncated || !ends_with_marker(&buf[..f.written]) {
        return false;
    }

    // An over-long record is truncated with the marker and never overflows.
    let mut small = [0u8; 20];
    let f = format_record(&mut small, Level::Warn, "net", "a very long message here", &[]);
    if !f.truncated || f.written > small.len() || !ends_with_marker(&small[..f.written]) {
        return false;
    }

    true
}

#[cfg(any(test, feature = "stage32-1-test"))]
fn ends_with_marker(bytes: &[u8]) -> bool {
    let marker = TRUNCATION_MARKER.as_bytes();
    bytes.len() >= marker.len() && &bytes[bytes.len() - marker.len()..] == marker
}

/// Emit a structured crash-dump record to serial with the key fault registers.
///
/// Called from the panic handler alongside the human-readable register dump so
/// a machine-readable crash parser can extract the fault context without
/// parsing the freeform text block.
#[cfg(not(test))]
pub fn emit_panic_dump(rip: u64, rsp: u64, cr2: u64) {
    const PANIC_DUMP_BYTES: usize = 256;
    let mut buf = [0u8; PANIC_DUMP_BYTES];
    let f = format_record(
        &mut buf,
        Level::Error,
        "panic",
        "kernel panic",
        &[
            Field::new("rip", Value::Hex(rip)),
            Field::new("rsp", Value::Hex(rsp)),
            Field::new("cr2", Value::Hex(cr2)),
        ],
    );
    crate::serial::write_bytes(&buf[..f.written]);
    crate::serial::write_bytes(b"\n");
}

/// Boot-time self-test for the structured-panic-dump path. Returns `true` when
/// the exact record format that [`emit_panic_dump`] would write is produced
/// correctly. Exercised by the stage32-2 QEMU gate.
#[cfg(any(test, feature = "stage32-2-test"))]
pub fn self_test_panic_dump() -> bool {
    let mut buf = [0u8; 256];
    let f = format_record(
        &mut buf,
        Level::Error,
        "panic",
        "kernel panic",
        &[
            Field::new("rip", Value::Hex(0x0000_ffff_dead_c0de)),
            Field::new("rsp", Value::Hex(0xffff_c000_0000_0000)),
            Field::new("cr2", Value::Hex(0x0000_0000_0000_0000)),
        ],
    );
    if f.truncated {
        return false;
    }
    let expected =
        b"ERROR panic: kernel panic | rip=0xffffdeadc0de | rsp=0xffffc00000000000 | cr2=0x0";
    &buf[..f.written] == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(buf: &[u8], f: Formatted) -> &str {
        core::str::from_utf8(&buf[..f.written]).expect("records are valid UTF-8")
    }

    #[test]
    fn header_only_record() {
        let mut buf = [0u8; 64];
        let f = format_record(&mut buf, Level::Info, "boot", "ready", &[]);
        assert!(!f.truncated);
        assert_eq!(rendered(&buf, f), "INFO boot: ready");
    }

    #[test]
    fn fields_render_in_order_with_types() {
        let mut buf = [0u8; 128];
        let f = format_record(
            &mut buf,
            Level::Error,
            "paging",
            "fault",
            &[
                Field::new("addr", Value::Hex(0xdead_beef)),
                Field::new("pid", Value::U64(7)),
                Field::new("delta", Value::I64(-3)),
                Field::new("user", Value::Bool(true)),
                Field::new("name", Value::Str("init")),
            ],
        );
        assert!(!f.truncated);
        assert_eq!(
            rendered(&buf, f),
            "ERROR paging: fault | addr=0xdeadbeef | pid=7 | delta=-3 | user=true | name=init"
        );
    }

    #[test]
    fn level_tags_are_stable() {
        for (level, tag) in [
            (Level::Error, "ERROR"),
            (Level::Warn, "WARN"),
            (Level::Info, "INFO"),
            (Level::Debug, "DEBUG"),
            (Level::Trace, "TRACE"),
        ] {
            assert_eq!(level.tag(), tag);
        }
        // Ordering: Error is the most severe (smallest).
        assert!(Level::Error < Level::Trace);
    }

    #[test]
    fn excess_fields_are_dropped_and_marked() {
        let mut buf = [0u8; 256];
        let many = [Field::new("k", Value::U64(1)); MAX_FIELDS + 3];
        let f = format_record(&mut buf, Level::Trace, "s", "m", &many);
        assert!(f.truncated);
        let text = rendered(&buf, f);
        assert!(text.ends_with(TRUNCATION_MARKER));
        // Exactly MAX_FIELDS "| k=1" groups survive before the marker.
        assert_eq!(text.matches("| k=1").count(), MAX_FIELDS);
    }

    #[test]
    fn oversized_record_truncates_in_bounds() {
        let mut buf = [0u8; 24];
        let f = format_record(
            &mut buf,
            Level::Warn,
            "net",
            "a message far larger than the buffer",
            &[],
        );
        assert!(f.truncated);
        assert!(f.written <= buf.len());
        assert!(rendered(&buf, f).ends_with(TRUNCATION_MARKER));
    }

    #[test]
    fn field_boundary_truncation_keeps_valid_utf8() {
        // Buffer large enough for the header and one field, not two.
        let mut buf = [0u8; 40];
        let f = format_record(
            &mut buf,
            Level::Info,
            "io",
            "x",
            &[
                Field::new("first", Value::Str("aaaa")),
                Field::new("second", Value::Str("bbbbbbbbbbbbbbbbbbbb")),
            ],
        );
        assert!(f.truncated);
        // Still valid UTF-8 and marker-terminated; the second field was cut at
        // its boundary rather than split.
        let text = rendered(&buf, f);
        assert!(text.ends_with(TRUNCATION_MARKER));
        assert!(!text.contains("second"));
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
