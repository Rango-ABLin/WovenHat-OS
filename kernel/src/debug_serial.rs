//! Bounded second-UART transport for the WovenDev debugger path.
//!
//! This slice owns COM2 only. It deliberately does not parse GDB packets or
//! touch trap state; [`crate::gdb_stub`] remains the pure framing boundary.

#[cfg(feature = "debug-serial")]
use core::arch::asm;

#[cfg(feature = "debug-serial")]
const COM2: u16 = 0x2f8;
#[cfg(feature = "debug-serial")]
const LINE_STATUS: u16 = 5;
#[cfg(feature = "debug-serial")]
const DATA_READY: u8 = 1 << 0;
#[cfg(feature = "debug-serial")]
const TRANSMITTER_EMPTY: u8 = 1 << 5;
#[cfg(feature = "debug-serial")]
const TX_POLL_LIMIT: u16 = 4096;
#[cfg(feature = "debug-serial")]
const LOOPBACK: u8 = 1 << 4;
#[cfg(feature = "debug-serial")]
const MODEM_CONTROL: u16 = 4;

/// Initialize the second 16550-compatible UART at 115200 8N1.
#[cfg(feature = "debug-serial")]
pub fn init() {
    // SAFETY: WovenHat owns the legacy COM2 port during kernel startup; these
    // are the standard 16550 divisor, line-control, FIFO, and modem registers.
    unsafe {
        outb(COM2 + 1, 0x00);
        outb(COM2 + 3, 0x80);
        outb(COM2, 0x03);
        outb(COM2 + 1, 0x00);
        outb(COM2 + 3, 0x03);
        outb(COM2 + 2, 0xc7);
        outb(COM2 + MODEM_CONTROL, 0x0b);
    }
}

#[cfg(not(feature = "debug-serial"))]
#[inline(always)]
pub fn init() {}

/// Write bytes to COM2, dropping a byte only after a bounded transmitter wait.
#[cfg(feature = "debug-serial")]
pub fn write_bytes(bytes: &[u8]) {
    for &byte in bytes {
        let mut polls = 0u16;
        // SAFETY: COM2 was initialized by [`init`] and is a fixed legacy UART.
        while unsafe { inb(COM2 + LINE_STATUS) } & TRANSMITTER_EMPTY == 0 {
            polls += 1;
            if polls == TX_POLL_LIMIT {
                break;
            }
            core::hint::spin_loop();
        }
        // SAFETY: The bounded poll above protects the transmit register access.
        unsafe { outb(COM2, byte) };
    }
}

#[cfg(not(feature = "debug-serial"))]
#[inline(always)]
pub fn write_bytes(_bytes: &[u8]) {}

/// Read one pending COM2 byte without blocking.
#[cfg(feature = "debug-serial")]
pub fn try_read() -> Option<u8> {
    // SAFETY: COM2 was initialized by [`init`] and is a fixed legacy UART.
    if unsafe { inb(COM2 + LINE_STATUS) } & DATA_READY == 0 {
        return None;
    }
    // SAFETY: DATA_READY guarantees a byte is available in the receiver FIFO.
    Some(unsafe { inb(COM2) })
}

#[cfg(not(feature = "debug-serial"))]
#[inline(always)]
pub fn try_read() -> Option<u8> {
    None
}

/// Verify COM2 loopback without depending on a host-side serial consumer.
#[cfg(feature = "stage32-9-test")]
pub fn self_test() -> bool {
    init();
    // SAFETY: The modem-control loopback bit is a standard 16550 diagnostic
    // mode. DTR/RTS/OUT2 remain asserted so QEMU exposes normal UART status.
    unsafe { outb(COM2 + MODEM_CONTROL, 0x0b | LOOPBACK) };
    write_bytes(&[0x55, 0xa6]);
    let first = try_read();
    let second = try_read();
    let empty = try_read().is_none();
    // SAFETY: Return COM2 to normal transmit mode before emitting the marker.
    unsafe { outb(COM2 + MODEM_CONTROL, 0x0b) };
    if first != Some(0x55) || second != Some(0xa6) || !empty {
        return false;
    }
    write_bytes(b"WOVENHAT-DEBUG-SERIAL\n");
    true
}

#[cfg(feature = "debug-serial")]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: Callers validate that `port` is a COM2 UART register.
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

#[cfg(feature = "debug-serial")]
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: Callers validate that `port` is a COM2 UART register.
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn disabled_host_boundary_is_nonblocking() {
        assert_eq!(super::try_read(), None);
    }
}
