//! GDB debug serial port — Stage 32.9.
//!
//! COM2 (base I/O 0x2F8): the dedicated transport channel for the GDB Remote
//! Serial Protocol stub.  Kept strictly separate from the console COM1 so that
//! debugger packet traffic never contaminates the kernel log and log output
//! never appears in the GDB stream.
//!
//! The module is compiled unconditionally; the `gdb-serial` feature gates its
//! call sites in higher-level code.  The `stage32-9-test` boot block enables a
//! loopback self-test on COM2 to verify initialization and byte-level I/O.

use crate::serial::{inb, outb};

// ── COM2 I/O-port map ─────────────────────────────────────────────────────────

/// COM2 base I/O address.
const COM2: u16 = 0x2f8;
/// Line Status Register offset: bit 0 = Data Ready, bit 5 = THR Empty.
const LSR: u16 = COM2 + 5;
/// Modem Control Register offset: bit 4 = Loopback Enable.
const MCR: u16 = COM2 + 4;

const LSR_DATA_READY: u8 = 1 << 0;
const LSR_THR_EMPTY: u8 = 1 << 5;
#[cfg(any(test, feature = "stage32-9-test"))]
const MCR_LOOPBACK: u8 = 1 << 4;
/// Standard UART operating mode: DTR + RTS + OUT2 asserted.
const MCR_NORMAL: u8 = 0x0b;

/// Bounded poll limit for transmit-ready and receive-ready checks.
/// Far longer than any one byte transfer on a real UART, short enough that a
/// wedged or absent port cannot stall the kernel.
const POLL_LIMIT: u16 = 4096;

// ── Public API ────────────────────────────────────────────────────────────────

/// Initialize COM2 at 115 200 baud, 8N1, with the FIFO enabled.
///
/// Safe to call multiple times; reinitializes the port each time.
pub fn init() {
    // SAFETY: WovenHat owns COM2.  These are standard 16550-compatible registers.
    unsafe {
        outb(COM2 + 1, 0x00); // disable all interrupts
        outb(COM2 + 3, 0x80); // set DLAB to program divisor
        outb(COM2, 0x01);     // divisor low  = 1  → 115 200 baud
        outb(COM2 + 1, 0x00); // divisor high = 0
        outb(COM2 + 3, 0x03); // 8N1, clear DLAB
        outb(COM2 + 2, 0xc7); // enable FIFO, clear TX/RX, 14-byte threshold
        outb(MCR, MCR_NORMAL); // assert DTR, RTS, OUT2
    }
}

/// Write `bytes` to the GDB serial port.
///
/// Polls until the transmit holding register is empty before each byte, giving
/// up after [`POLL_LIMIT`] polls.  Never blocks the caller indefinitely.
pub fn write_bytes(bytes: &[u8]) {
    for &byte in bytes {
        let mut polls = 0u16;
        // SAFETY: reads COM2 line-status register.
        while unsafe { inb(LSR) } & LSR_THR_EMPTY == 0 {
            polls += 1;
            if polls == POLL_LIMIT {
                break;
            }
            core::hint::spin_loop();
        }
        // SAFETY: writes one byte to COM2 transmit-holding register.
        unsafe { outb(COM2, byte) };
    }
}

/// Read one byte from the GDB serial port, non-blocking.
///
/// Polls the line-status register up to [`POLL_LIMIT`] times.  Returns the
/// received byte on success, or `None` when no data arrives within the limit.
pub fn read_byte() -> Option<u8> {
    let mut polls = 0u16;
    loop {
        // SAFETY: reads COM2 line-status register.
        if unsafe { inb(LSR) } & LSR_DATA_READY != 0 {
            // SAFETY: reads COM2 receive-buffer register.
            return Some(unsafe { inb(COM2) });
        }
        polls += 1;
        if polls == POLL_LIMIT {
            return None;
        }
        core::hint::spin_loop();
    }
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test.
///
/// Enables UART loopback mode so bytes written to COM2 immediately appear in
/// the receive FIFO.  Verifies:
/// 1. Single-byte write/read round-trip.
/// 2. Full GDB RSP packet encode → write → read → decode round-trip.
///
/// Restores normal MCR state before returning.  Returns `true` on pass.
#[cfg(any(test, feature = "stage32-9-test"))]
pub fn self_test() -> bool {
    init();

    // ── Enable UART loopback ──────────────────────────────────────────────────
    // SAFETY: sets COM2 MCR loopback bit so TX data feeds RX without external
    // hardware.  Standard 16550 loopback mode.
    unsafe { outb(MCR, MCR_LOOPBACK) };

    // ── Single-byte round-trip ────────────────────────────────────────────────
    write_bytes(b"W");
    let Some(b) = read_byte() else {
        unsafe { outb(MCR, MCR_NORMAL) };
        return false;
    };
    if b != b'W' {
        unsafe { outb(MCR, MCR_NORMAL) };
        return false;
    }

    // ── GDB RSP round-trip ────────────────────────────────────────────────────
    // Encode "$?#3f" and read it back through the loopback FIFO.
    let mut enc = [0u8; 8];
    let Some(n) = crate::gdb_stub::encode(b"?", &mut enc) else {
        unsafe { outb(MCR, MCR_NORMAL) };
        return false;
    };
    write_bytes(&enc[..n]);

    let mut received = [0u8; 8];
    let mut i = 0usize;
    while i < n {
        let Some(b) = read_byte() else {
            unsafe { outb(MCR, MCR_NORMAL) };
            return false;
        };
        received[i] = b;
        i += 1;
    }

    let Some(pkt) = crate::gdb_stub::decode(&received[..n]) else {
        unsafe { outb(MCR, MCR_NORMAL) };
        return false;
    };
    if pkt.data() != b"?" {
        unsafe { outb(MCR, MCR_NORMAL) };
        return false;
    }

    // ── Restore normal MCR ────────────────────────────────────────────────────
    // SAFETY: restores COM2 MCR to operating mode after loopback test.
    unsafe { outb(MCR, MCR_NORMAL) };
    true
}
