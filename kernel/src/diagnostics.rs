//! On-screen boot diagnostics.
//!
//! On a machine with no serial line, where USB may not come up and the log
//! cannot reach the stick, the console shows what the boot found: every
//! driver's outcome and error, the USB controller and its devices, the
//! display, the live-test log and the desktop session, and anything
//! WovenShell reported. A photo of the screen is then enough to diagnose
//! the boot.

/// Kernel log lines worth a tester's eye.
const PREFIXES: [&str; 9] =
    ["[DRIVERS]", "[USB]", "[DISPLAY]", "[FS]", "[LIVELOG]", "[SESSION]", "[POWER]", "woven-shell", "EXCEPTION"];

/// Call `each` with every diagnostic line logged since boot, in order.
pub fn for_each_line(mut each: impl FnMut(&str)) {
    let mut log = alloc::vec![0u8; crate::serial::LOG_BYTES];
    let (_, length) = crate::serial::read_log(0, &mut log);
    for line in log[..length].split(|&byte| byte == b'\n') {
        let Ok(text) = core::str::from_utf8(line) else { continue };
        let text = text.trim();
        if PREFIXES.iter().any(|prefix| text.contains(prefix)) {
            each(text);
        }
    }
}
