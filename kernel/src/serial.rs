use core::arch::asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

const COM1: u16 = 0x3f8;
const LINE_STATUS_TRANSMITTER_EMPTY: u8 = 1 << 5;
/// Never let an absent or wedged legacy UART hold up kernel progress.
const TX_POLL_LIMIT: u16 = 4096;

pub fn init() {
    // SAFETY: WovenHat owns the machine at this point in kernel startup. These
    // are the standard 16550-compatible COM1 registers exposed by QEMU.
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xc7);
        outb(COM1 + 4, 0x0b);
    }
}

/// The kernel log: every byte written to the serial port, the newest
/// [`LOG_BYTES`] of them kept, so the log can be saved where there is no
/// serial line, such as the live-test log on the WovenHat USB stick
/// (Stage 13.19).
pub const LOG_BYTES: usize = 256 * 1024;
static LOG: [AtomicU8; LOG_BYTES] = [const { AtomicU8::new(0) }; LOG_BYTES];
/// Bytes logged since boot; the next byte goes to `LOG[LOG_HEAD % LOG_BYTES]`.
static LOG_HEAD: AtomicUsize = AtomicUsize::new(0);

fn record(byte: u8) {
    let at = LOG_HEAD.fetch_add(1, Ordering::Relaxed);
    LOG[at % LOG_BYTES].store(byte, Ordering::Release);
}

/// Bytes logged since boot.
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage27-5-recovery-test"
))]
pub fn log_position() -> usize {
    LOG_HEAD.load(Ordering::Acquire)
}

/// Copy logged bytes, starting at position `from`, into `out`. Returns the
/// position actually copied from (later than `from` when older bytes have
/// been overwritten) and the number of bytes copied. A byte being written
/// concurrently may read as its predecessor; readers re-read the tail.
#[cfg(any(
    not(feature = "qemu-test"),
    feature = "stage13-12-test",
    feature = "stage27-5-recovery-test"
))]
pub fn read_log(from: usize, out: &mut [u8]) -> (usize, usize) {
    let head = LOG_HEAD.load(Ordering::Acquire);
    let start = from.max(head.saturating_sub(LOG_BYTES)).min(head);
    let count = (head - start).min(out.len());
    for (offset, byte) in out[..count].iter_mut().enumerate() {
        *byte = LOG[(start + offset) % LOG_BYTES].load(Ordering::Acquire);
    }
    (start, count)
}

pub fn write_fmt(arguments: fmt::Arguments<'_>) {
    let _ = SerialPort.write_fmt(arguments);
}

pub fn write_line(arguments: fmt::Arguments<'_>) {
    let _ = SerialPort.write_fmt(arguments);
    let _ = SerialPort.write_str("\n");
}

pub fn write_bytes(bytes: &[u8]) {
    let mut port = SerialPort;
    for &byte in bytes {
        if byte == b'\n' {
            port.write_byte(b'\r');
        }
        port.write_byte(byte);
    }
}
struct SerialPort;

impl Write for SerialPort {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(byte);
        }

        Ok(())
    }
}

impl SerialPort {
    fn write_byte(&mut self, byte: u8) {
        // The line ending's carriage return is for terminals, not the log.
        if byte != b'\r' {
            record(byte);
        }
        let mut polls = 0u16;
        while unsafe { inb(COM1 + 5) } & LINE_STATUS_TRANSMITTER_EMPTY == 0 {
            polls += 1;
            if polls == TX_POLL_LIMIT {
                return;
            }
            core::hint::spin_loop();
        }

        // SAFETY: COM1 was initialized during kernel startup, and this writes
        // one byte to its transmit register after it reports readiness.
        unsafe { outb(COM1, byte) };
    }
}

/// Read one byte from an x86 I/O port.
///
/// # Safety
///
/// The caller must ensure that `port` is valid and that WovenHat owns the
/// associated device.
unsafe fn inb(port: u16) -> u8 {
    let value: u8;

    // SAFETY: The caller upholds the port-I/O requirements above.
    unsafe {
        asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }

    value
}

/// Write one byte to an x86 I/O port.
///
/// # Safety
///
/// The caller must ensure that `port` is valid and that WovenHat owns the
/// associated device.
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller upholds the port-I/O requirements above.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}
