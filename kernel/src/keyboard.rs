pub use crate::woven_input::Key;
use crate::{irq_lock::IrqMutex as Mutex, woven_input};

use core::{
    arch::asm,
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub const IRQ: u8 = 1;

const DATA_PORT: u16 = 0x60;
const STATUS_PORT: u16 = 0x64;
const SCANCODE_QUEUE_CAPACITY: usize = 64;

static SCANCODES: ScancodeQueue = ScancodeQueue::new();
static DECODER: Mutex<Keyboard> = Mutex::with_rank(Keyboard::new(), 10);

struct ScancodeQueue {
    buffer: UnsafeCell<[u8; SCANCODE_QUEUE_CAPACITY]>,
    read_index: AtomicUsize,
    write_index: AtomicUsize,
}

// SAFETY: single-producer (keyboard IRQ) / single-consumer (service) ring. The
// producer writes only the `write_index` slot and publishes it with a Release
// store; the consumer reads only slots below `write_index` (Acquire). The
// atomic indices guarantee producer and consumer never touch the same
// `UnsafeCell` slot concurrently, so sharing it across threads is sound.
unsafe impl Sync for ScancodeQueue {}

impl ScancodeQueue {
    const fn new() -> Self {
        Self {
            buffer: UnsafeCell::new([0; SCANCODE_QUEUE_CAPACITY]),
            read_index: AtomicUsize::new(0),
            write_index: AtomicUsize::new(0),
        }
    }

    fn push(&self, scancode: u8) {
        let write_index = self.write_index.load(Ordering::Relaxed);
        let next_index = (write_index + 1) % SCANCODE_QUEUE_CAPACITY;
        if next_index == self.read_index.load(Ordering::Acquire) {
            return;
        }
        // SAFETY: SPSC ring — the sole producer owns `write_index`, which is not
        // yet published, so the consumer cannot be reading this slot; the Release
        // store below publishes it.
        unsafe { (*self.buffer.get())[write_index] = scancode };
        self.write_index.store(next_index, Ordering::Release);
    }

    fn pop(&self) -> Option<u8> {
        let read_index = self.read_index.load(Ordering::Relaxed);
        if read_index == self.write_index.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: SPSC ring — `read_index != write_index` (checked), so the
        // producer has fully published this slot and the sole consumer owns it.
        let scancode = unsafe { (*self.buffer.get())[read_index] };
        self.read_index.store(
            (read_index + 1) % SCANCODE_QUEUE_CAPACITY,
            Ordering::Release,
        );
        Some(scancode)
    }
}

pub struct Keyboard {
    shift: bool,
    control: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift: false,
            control: false,
        }
    }

    fn poll(&mut self, allow_legacy: bool) -> Option<Key> {
        if let Some(scancode) = SCANCODES.pop() {
            return self.decode(scancode);
        }
        if allow_legacy {
            return self.poll_legacy();
        }
        None
    }

    pub fn poll_legacy(&mut self) -> Option<Key> {
        // SAFETY: read of the fixed i8042 status port.
        let status = unsafe { inb(STATUS_PORT) };
        // An auxiliary (mouse) byte is left for the mouse IRQ.
        if status & 1 == 0 || status & 0x20 != 0 {
            return None;
        }
        // SAFETY: read of the fixed i8042 data port; output buffer is full and
        // is a keyboard (not auxiliary) byte per the status check above.
        let scancode = unsafe { inb(DATA_PORT) };
        self.decode(scancode)
    }

    fn decode(&mut self, scancode: u8) -> Option<Key> {
        if scancode & 0x80 != 0 {
            return match scancode {
                0xAA | 0xB6 => {
                    self.shift = false;
                    None
                }
                0x9D => {
                    self.control = false;
                    None
                }
                _ => None,
            };
        }
        match scancode {
            0x2A | 0x36 => {
                self.shift = true;
                None
            }
            0x1D => {
                self.control = true;
                None
            }
            0x1C => Some(Key::Enter),
            0x0E => Some(Key::Backspace),
            0x0F => Some(Key::Tab),
            0x3B => Some(Key::F1),
            0x3C => Some(Key::F2),
            0x3D => Some(Key::F3),
            0x3E => Some(Key::F4),
            0x58 => Some(Key::F12),
            code => decode_scancode(code, self.shift).map(|character| {
                if self.control && character.is_ascii_alphabetic() {
                    Key::Char((character.to_ascii_lowercase() as u8 - b'a' + 1) as char)
                } else {
                    Key::Char(character)
                }
            }),
        }
    }
}

fn drain_scancodes(allow_legacy: bool) {
    let mut decoder = DECODER.lock();
    while let Some(key) = decoder.poll(allow_legacy) {
        let _ = woven_input::publish_key(key);
    }
}

pub fn service() {
    let allow_legacy = !x86_64::instructions::interrupts::are_enabled();
    drain_scancodes(allow_legacy);
}

pub fn poll() -> Option<Key> {
    service();
    woven_input::poll_key()
}

pub fn read_bytes(buffer: &mut [u8]) -> usize {
    service();
    woven_input::read_bytes(buffer)
}

pub fn inject_validation_input(count: usize) {
    for _ in 0..count {
        SCANCODES.push(0x1e);
    }
}

pub fn self_test() -> bool {
    let mut keyboard = Keyboard::new();
    keyboard
        .decode(0x1e)
        .is_some_and(|key| matches!(key, Key::Char('a')))
        && keyboard.decode(0x2a).is_none()
        && keyboard
            .decode(0x1e)
            .is_some_and(|key| matches!(key, Key::Char('A')))
        && keyboard.decode(0xaa).is_none()
        && keyboard
            .decode(0x1c)
            .is_some_and(|key| matches!(key, Key::Enter))
        && keyboard.decode(0x1d).is_none()
        && keyboard
            .decode(0x2e)
            .is_some_and(|key| matches!(key, Key::Char('\u{3}')))
        && keyboard.decode(0x9d).is_none()
}

pub fn handle_interrupt() {
    // SAFETY: read of the fixed i8042 status port.
    let status = unsafe { inb(STATUS_PORT) };
    if status & 0x01 == 0 || status & 0x20 != 0 {
        return;
    }
    // SAFETY: read of the fixed i8042 data port; a keyboard byte is pending per
    // the status check above.
    let scancode = unsafe { inb(DATA_PORT) };
    SCANCODES.push(scancode);
    // Decode and publish now. Waiting for a consumer to call `service` left
    // keystrokes undelivered to a focus holder asleep in `InputWait`, which
    // polls nothing. Every lock on this path is an IRQ mutex at rank 10, so
    // the interrupted code cannot hold one on this CPU.
    drain_scancodes(false);
}

/// # Safety
/// `port` must be a valid x86 I/O port (callers pass the fixed i8042
/// STATUS_PORT/DATA_PORT).
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `in al, dx` reads one byte from the valid I/O port in `dx`.
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

fn decode_scancode(code: u8, shift: bool) -> Option<char> {
    let normal = match code {
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0A => '9',
        0x0B => '0',
        0x10 => 'q',
        0x11 => 'w',
        0x12 => 'e',
        0x13 => 'r',
        0x14 => 't',
        0x15 => 'y',
        0x16 => 'u',
        0x17 => 'i',
        0x18 => 'o',
        0x19 => 'p',
        0x1E => 'a',
        0x1F => 's',
        0x20 => 'd',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x2C => 'z',
        0x2D => 'x',
        0x2E => 'c',
        0x2F => 'v',
        0x30 => 'b',
        0x31 => 'n',
        0x32 => 'm',
        0x39 => ' ',
        0x0C => '-',
        0x0D => '=',
        0x33 => ',',
        0x34 => '.',
        0x35 => '/',
        _ => return None,
    };
    if !shift {
        return Some(normal);
    }
    Some(match normal {
        'a'..='z' => normal.to_ascii_uppercase(),
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        other => other,
    })
}
