//! PS/2 auxiliary-port (mouse) driver.
//!
//! The i8042 controller multiplexes the keyboard and the auxiliary device
//! through one data port; status bit 5 marks a byte as auxiliary. The keyboard
//! handler already leaves auxiliary bytes alone, and this driver takes only
//! those. Three-byte packets are decoded in the IRQ handler and published
//! straight into WovenInput, so a focus holder asleep in `InputWait` is woken
//! by the hardware itself rather than by whichever task happens to poll.

use core::arch::asm;

use crate::irq_lock::IrqMutex as Mutex;
use crate::woven_input::{self, Event};

pub const IRQ: u8 = 12;

const DATA_PORT: u16 = 0x60;
const STATUS_PORT: u16 = 0x64;
const STATUS_OUTPUT_FULL: u8 = 1 << 0;
const STATUS_INPUT_FULL: u8 = 1 << 1;
const STATUS_AUX_DATA: u8 = 1 << 5;

const COMMAND_ENABLE_AUX: u8 = 0xa8;
const COMMAND_READ_CONFIG: u8 = 0x20;
const COMMAND_WRITE_CONFIG: u8 = 0x60;
const COMMAND_WRITE_AUX: u8 = 0xd4;
const CONFIG_AUX_IRQ: u8 = 1 << 1;
const CONFIG_AUX_CLOCK_DISABLED: u8 = 1 << 5;

const MOUSE_SET_DEFAULTS: u8 = 0xf6;
const MOUSE_ENABLE_REPORTING: u8 = 0xf4;
const MOUSE_ACK: u8 = 0xfa;

/// Bound on controller polling during initialisation. Initialisation runs with
/// interrupts disabled, so the timer cannot bound it; each iteration includes
/// a port-0x80 delay of about a microsecond, so this is on the order of 100 ms,
/// far longer than an i8042 takes to answer.
const POLL_ITERATIONS: usize = 100_000;

/// Three-byte packet decoder.
pub struct Decoder {
    bytes: [u8; 3],
    index: usize,
    buttons: u8,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 3],
            index: 0,
            buttons: 0,
        }
    }

    /// Feed one byte; returns the events a completed packet produces.
    ///
    /// The first byte of every packet has bit 3 set; a byte without it in the
    /// first position is discarded, which resynchronises after a lost byte.
    /// Packets flagging X or Y overflow are dropped whole, since their deltas
    /// are meaningless. PS/2 Y grows upwards; events use screen coordinates,
    /// where Y grows downwards.
    pub fn push(&mut self, byte: u8) -> [Option<Event>; 4] {
        let mut events = [None; 4];
        if self.index == 0 && byte & 0x08 == 0 {
            return events;
        }
        self.bytes[self.index] = byte;
        self.index += 1;
        if self.index < 3 {
            return events;
        }
        self.index = 0;
        let flags = self.bytes[0];
        if flags & 0xc0 != 0 {
            return events;
        }
        let dx = i16::from(self.bytes[1]) - if flags & 0x10 != 0 { 256 } else { 0 };
        let dy = i16::from(self.bytes[2]) - if flags & 0x20 != 0 { 256 } else { 0 };
        let mut count = 0;
        if dx != 0 || dy != 0 {
            events[count] = Some(Event::PointerMove { dx, dy: -dy });
            count += 1;
        }
        let buttons = flags & 0x07;
        for button in 0..3u8 {
            let mask = 1 << button;
            if (buttons ^ self.buttons) & mask != 0 {
                events[count] = Some(Event::PointerButton {
                    button,
                    pressed: buttons & mask != 0,
                });
                count += 1;
            }
        }
        self.buttons = buttons;
        events
    }
}

static DECODER: Mutex<Decoder> = Mutex::with_rank(Decoder::new(), 10);

/// Bring up the auxiliary port and enable mouse reporting. Returns whether a
/// mouse acknowledged; without one, the auxiliary IRQ stays disabled.
///
/// Must run with interrupts disabled: the controller's command responses
/// arrive through the shared data port, and an IRQ handler must not consume
/// them first.
pub fn init() -> bool {
    debug_assert!(!x86_64::instructions::interrupts::are_enabled());
    // SAFETY: standard i8042 command and data ports, polled with bounds.
    unsafe {
        if !command(COMMAND_ENABLE_AUX) {
            return false;
        }
        for request in [MOUSE_SET_DEFAULTS, MOUSE_ENABLE_REPORTING] {
            if !command(COMMAND_WRITE_AUX) || !write_data(request) || read_aux() != Some(MOUSE_ACK)
            {
                return false;
            }
        }
        if !command(COMMAND_READ_CONFIG) {
            return false;
        }
        let Some(config) = read_any() else {
            return false;
        };
        let config = (config | CONFIG_AUX_IRQ) & !CONFIG_AUX_CLOCK_DISABLED;
        command(COMMAND_WRITE_CONFIG) && write_data(config)
    }
}

/// IRQ12: take one auxiliary byte, decode, publish.
pub fn handle_interrupt() {
    // SAFETY: reading the i8042 status and data ports.
    let status = unsafe { inb(STATUS_PORT) };
    if status & STATUS_OUTPUT_FULL == 0 || status & STATUS_AUX_DATA == 0 {
        return;
    }
    let byte = unsafe { inb(DATA_PORT) };
    let events = DECODER.lock().push(byte);
    // Published with DECODER released.
    for event in events.into_iter().flatten() {
        let _ = woven_input::publish(event);
    }
}

/// Have the controller present `byte` as if the mouse had sent it (i8042
/// command 0xD3). The byte travels the real hardware path: auxiliary output
/// buffer, IRQ12, this driver. Waits, bounded, until it has been consumed.
#[cfg(any(
    feature = "stage19-12-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test"
))]
pub fn loopback(byte: u8) -> bool {
    loopback_through(0xd3, byte)
}

/// Same, through the keyboard side (i8042 command 0xD2): IRQ1, the keyboard
/// driver.
#[cfg(any(
    feature = "stage19-12-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test"
))]
pub fn keyboard_loopback(byte: u8) -> bool {
    loopback_through(0xd2, byte)
}

#[cfg(any(
    feature = "stage19-12-test",
    feature = "stage20-2-test",
    feature = "stage20-3-test",
    feature = "stage20-4-test",
    feature = "stage20-5-test",
    feature = "stage20-6-test",
    feature = "stage20-7-test",
    feature = "stage20-8-test",
    feature = "stage20-9-test"
))]
fn loopback_through(command_byte: u8, byte: u8) -> bool {
    // The output buffer holds one byte: wait for the previous one to be taken
    // by its IRQ handler, write, then wait for this one to be taken too.
    // Interrupts are on here, so the timer bounds the waits.
    let drained = || {
        let start = crate::timer::ticks();
        loop {
            // SAFETY: reading the i8042 status port.
            if unsafe { inb(STATUS_PORT) } & STATUS_OUTPUT_FULL == 0 {
                return true;
            }
            if crate::timer::ticks().wrapping_sub(start) > 100 {
                return false;
            }
            core::hint::spin_loop();
        }
    };
    // SAFETY: standard i8042 command and data ports.
    drained() && unsafe { command(command_byte) && write_data(byte) } && drained()
}

/// Decoder checks for the gate.
#[cfg(feature = "stage19-12-test")]
pub fn decoder_self_test() -> bool {
    let mut decoder = Decoder::new();
    let feed = |decoder: &mut Decoder, bytes: [u8; 3]| {
        let mut last = [None; 4];
        for byte in bytes {
            last = decoder.push(byte);
        }
        last
    };
    // A first byte without bit 3 is discarded (resynchronisation): only the
    // next three bytes form a packet.
    let resync = decoder.push(0x00).iter().all(Option::is_none);
    // +5 right, PS/2 +3 up = screen -3, left button pressed.
    let moved = feed(&mut decoder, [0x09, 5, 3]);
    let first = moved[0] == Some(Event::PointerMove { dx: 5, dy: -3 })
        && moved[1]
            == Some(Event::PointerButton {
                button: 0,
                pressed: true,
            })
        && moved[2].is_none();
    // Negative deltas through the sign bits: -2 left, PS/2 -4 = screen +4;
    // left released.
    let signed = feed(&mut decoder, [0x08 | 0x10 | 0x20, 0xfe, 0xfc]);
    let second = signed[0] == Some(Event::PointerMove { dx: -2, dy: 4 })
        && signed[1]
            == Some(Event::PointerButton {
                button: 0,
                pressed: false,
            });
    // Overflow drops the packet whole, including its button change.
    let overflow = feed(&mut decoder, [0x08 | 0x40 | 0x02, 1, 1])
        .iter()
        .all(Option::is_none);
    // Right button alone, no motion: one event.
    let right = feed(&mut decoder, [0x08 | 0x02, 0, 0]);
    let third = right[0]
        == Some(Event::PointerButton {
            button: 1,
            pressed: true,
        })
        && right[1].is_none();
    resync && first && second && overflow && third
}

unsafe fn command(byte: u8) -> bool {
    unsafe {
        wait_input_empty() && {
            outb(STATUS_PORT, byte);
            true
        }
    }
}

unsafe fn write_data(byte: u8) -> bool {
    unsafe {
        wait_input_empty() && {
            outb(DATA_PORT, byte);
            true
        }
    }
}

unsafe fn wait_input_empty() -> bool {
    for _ in 0..POLL_ITERATIONS {
        if unsafe { inb(STATUS_PORT) } & STATUS_INPUT_FULL == 0 {
            return true;
        }
        unsafe { io_delay() };
    }
    false
}

/// Next auxiliary byte. A keyboard byte arriving meanwhile is discarded:
/// initialisation runs before input is meaningful.
unsafe fn read_aux() -> Option<u8> {
    for _ in 0..POLL_ITERATIONS {
        let status = unsafe { inb(STATUS_PORT) };
        if status & STATUS_OUTPUT_FULL != 0 {
            let byte = unsafe { inb(DATA_PORT) };
            if status & STATUS_AUX_DATA != 0 {
                return Some(byte);
            }
        }
        unsafe { io_delay() };
    }
    None
}

/// Next controller byte, auxiliary or not (command responses such as the
/// configuration byte arrive on the keyboard side).
unsafe fn read_any() -> Option<u8> {
    for _ in 0..POLL_ITERATIONS {
        if unsafe { inb(STATUS_PORT) } & STATUS_OUTPUT_FULL != 0 {
            return Some(unsafe { inb(DATA_PORT) });
        }
        unsafe { io_delay() };
    }
    None
}

unsafe fn io_delay() {
    unsafe { outb(0x80, 0) };
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}
