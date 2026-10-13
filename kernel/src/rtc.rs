//! CMOS real-time clock: wall-clock date and time.
//!
//! The PC's battery-backed clock sits behind index port 0x70 and data port
//! 0x71. Its registers can be mid-update when read, so a reading is taken only
//! while the update-in-progress flag is clear, and accepted only when two
//! consecutive readings agree. Register B says whether values are BCD or
//! binary and whether hours are 12- or 24-hour; both encodings are decoded.
//!
//! The clock carries no time zone. QEMU and most firmware keep it in UTC, and
//! that is what this module reports.

use core::arch::asm;

use crate::irq_lock::PreemptMutex as Mutex;

const INDEX_PORT: u16 = 0x70;
const DATA_PORT: u16 = 0x71;

const REGISTER_SECONDS: u8 = 0x00;
const REGISTER_MINUTES: u8 = 0x02;
const REGISTER_HOURS: u8 = 0x04;
const REGISTER_DAY: u8 = 0x07;
const REGISTER_MONTH: u8 = 0x08;
const REGISTER_YEAR: u8 = 0x09;
const REGISTER_A: u8 = 0x0a;
const REGISTER_B: u8 = 0x0b;

const UPDATE_IN_PROGRESS: u8 = 1 << 7;
const STATUS_B_24_HOUR: u8 = 1 << 1;
const STATUS_B_BINARY: u8 = 1 << 2;
const HOUR_PM: u8 = 1 << 7;

/// Bound on polling for the update flag to clear. An update cycle lasts
/// about 2 ms; this is far longer, and keeps a dead clock from hanging the
/// caller.
const POLL_ITERATIONS: usize = 100_000;
/// Readings attempted before giving up on two that agree.
const ATTEMPTS: usize = 8;

/// A calendar date and time of day, UTC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    /// `second | minute << 8 | hour << 16 | day << 24 | month << 32 |
    /// year << 40`, the `ClockRead` syscall's return value.
    pub const fn pack(self) -> u64 {
        self.second as u64
            | (self.minute as u64) << 8
            | (self.hour as u64) << 16
            | (self.day as u64) << 24
            | (self.month as u64) << 32
            | (self.year as u64) << 40
    }
}

/// Raw register values, in the order seconds, minutes, hours, day, month,
/// year.
pub type Raw = [u8; 6];

/// Serialises use of the index/data port pair: selecting a register and
/// reading it must not interleave with another reader. Nothing takes it from
/// interrupt context.
static PORTS: Mutex<()> = Mutex::with_rank((), 10);

/// Read the current date and time, or `None` if the clock never settles or
/// reports values out of range.
pub fn read() -> Option<DateTime> {
    let _ports = PORTS.lock();
    // SAFETY: standard CMOS index and data ports, serialised by PORTS.
    unsafe {
        let status_b = register(REGISTER_B);
        for _ in 0..ATTEMPTS {
            if !wait_until_settled() {
                return None;
            }
            let first = raw();
            if !wait_until_settled() {
                return None;
            }
            if raw() == first {
                return decode(first, status_b);
            }
        }
    }
    None
}

/// Decode raw registers under the format register B describes. Values that
/// are not valid BCD in BCD mode, or out of calendar range, are refused.
/// The two-digit year is taken as 20xx.
pub fn decode(raw: Raw, status_b: u8) -> Option<DateTime> {
    let binary = status_b & STATUS_B_BINARY != 0;
    let value = |byte: u8| -> Option<u8> {
        if binary {
            return Some(byte);
        }
        let (tens, units) = (byte >> 4, byte & 0x0f);
        (tens <= 9 && units <= 9).then_some(tens * 10 + units)
    };
    let [second, minute, hours, day, month, year] = raw;
    let mut hour = value(hours & !HOUR_PM)?;
    if status_b & STATUS_B_24_HOUR == 0 {
        // 12-hour clock: 12 AM is midnight, 12 PM is noon.
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = hour % 12 + if hours & HOUR_PM != 0 { 12 } else { 0 };
    }
    let time = DateTime {
        year: 2000 + u16::from(value(year)?),
        month: value(month)?,
        day: value(day)?,
        hour,
        minute: value(minute)?,
        second: value(second)?,
    };
    let valid = time.year < 2100
        && (1..=12).contains(&time.month)
        && (1..=31).contains(&time.day)
        && time.hour < 24
        && time.minute < 60
        && time.second < 60;
    valid.then_some(time)
}

unsafe fn wait_until_settled() -> bool {
    for _ in 0..POLL_ITERATIONS {
        if unsafe { register(REGISTER_A) } & UPDATE_IN_PROGRESS == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

unsafe fn raw() -> Raw {
    unsafe {
        [
            register(REGISTER_SECONDS),
            register(REGISTER_MINUTES),
            register(REGISTER_HOURS),
            register(REGISTER_DAY),
            register(REGISTER_MONTH),
            register(REGISTER_YEAR),
        ]
    }
}

/// Select `index` and read it. Bit 7 of the index port stays clear, leaving
/// NMI enabled.
unsafe fn register(index: u8) -> u8 {
    let value: u8;
    unsafe {
        asm!("out dx, al", in("dx") INDEX_PORT, in("al") index, options(nomem, nostack, preserves_flags));
        asm!("in al, dx", out("al") value, in("dx") DATA_PORT, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Decoder checks for the Stage 20.1 gate.
#[cfg(feature = "stage20-1-test")]
pub fn decode_self_test() -> bool {
    let at = |year, month, day, hour, minute, second| DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    };
    // BCD, 24-hour: 2026-10-13 21:07:59.
    let bcd = decode([0x59, 0x07, 0x21, 0x13, 0x10, 0x26], STATUS_B_24_HOUR)
        == Some(at(2026, 10, 13, 21, 7, 59));
    // Binary, 12-hour, PM: 9 PM is 21; 12 AM is midnight; 12 PM is noon.
    let binary = STATUS_B_BINARY;
    let pm = decode([0, 30, HOUR_PM | 9, 1, 2, 30], binary) == Some(at(2030, 2, 1, 21, 30, 0));
    let midnight = decode([0, 0, 12, 1, 1, 0], binary) == Some(at(2000, 1, 1, 0, 0, 0));
    let noon = decode([0, 0, HOUR_PM | 12, 1, 1, 0], binary) == Some(at(2000, 1, 1, 12, 0, 0));
    // Invalid BCD digits and out-of-range fields are refused.
    let bad_digit = decode([0x5a, 0, 0, 1, 1, 0], STATUS_B_24_HOUR).is_none();
    let bad_month = decode([0, 0, 0, 1, 13, 0], binary | STATUS_B_24_HOUR).is_none();
    let bad_hour = decode([0, 0, 0, 1, 1, 0], binary).is_none();
    let packed = at(2026, 10, 13, 21, 7, 59).pack()
        == (2026 << 40) | (10 << 32) | (13 << 24) | (21 << 16) | (7 << 8) | 59;
    bcd && pm && midnight && noon && bad_digit && bad_month && bad_hour && packed
}
