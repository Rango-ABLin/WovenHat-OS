// Keyboard input is production; other event producers belong to the Stage 13.7 probe.
use crate::irq_lock::IrqMutex as Mutex;

const INPUT_QUEUE_CAPACITY: usize = 128;

#[cfg(feature = "stage13-7-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Touchpad,
    Touchscreen,
    Pen,
    GameController,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    F1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    #[cfg(feature = "stage13-7-test")]
    PointerMove {
        dx: i16,
        dy: i16,
    },
    #[cfg(feature = "stage13-7-test")]
    PointerButton {
        button: u8,
        pressed: bool,
    },
    #[cfg(feature = "stage13-7-test")]
    Scroll {
        vertical: i16,
    },
    #[cfg(feature = "stage13-7-test")]
    Touch {
        contact: u8,
        x: u16,
        y: u16,
        active: bool,
    },
    #[cfg(feature = "stage13-7-test")]
    Pen {
        x: u16,
        y: u16,
        pressure: u16,
        touching: bool,
    },
    #[cfg(feature = "stage13-7-test")]
    GameController {
        control: u16,
        value: i16,
    },
}

#[derive(Clone, Copy)]
struct Queue {
    events: [Option<Event>; INPUT_QUEUE_CAPACITY],
    read: usize,
    write: usize,
    count: usize,
    dropped: u64,
}

impl Queue {
    const fn new() -> Self {
        Self {
            events: [None; INPUT_QUEUE_CAPACITY],
            read: 0,
            write: 0,
            count: 0,
            dropped: 0,
        }
    }

    fn push(&mut self, event: Event) -> bool {
        if self.count == INPUT_QUEUE_CAPACITY {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.events[self.write] = Some(event);
        self.write = (self.write + 1) % INPUT_QUEUE_CAPACITY;
        self.count += 1;
        true
    }

    fn pop(&mut self) -> Option<Event> {
        if self.count == 0 {
            return None;
        }
        let event = self.events[self.read].take();
        self.read = (self.read + 1) % INPUT_QUEUE_CAPACITY;
        self.count -= 1;
        event
    }
}

static QUEUE: Mutex<Queue> = Mutex::with_rank(Queue::new(), 10);

/// Input focus: while a process holds it, every event goes to its private
/// queue instead of the terminal's. The compositor grants focus to its lease
/// holder when that holder takes the display, and withdraws it on release.
///
/// An IRQ mutex because the keyboard interrupt path publishes through it.
/// Lock order: FOCUS before QUEUE; callers holding the compositor lock or the
/// keyboard decoder lock (both rank 10) may take it.
struct Focus {
    owner: Option<u64>,
    queue: Queue,
}

static FOCUS: Mutex<Focus> = Mutex::with_rank(
    Focus {
        owner: None,
        queue: Queue::new(),
    },
    10,
);

/// The caller does not hold input focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotFocused;

pub fn publish(event: Event) -> bool {
    let mut focus = FOCUS.lock();
    if focus.owner.is_some() {
        return focus.queue.push(event);
    }
    // Held across the terminal push so focus cannot be granted between the
    // check and the push, which would send one event to the wrong reader.
    // The push is bound and FOCUS dropped explicitly: as a tail expression the
    // QUEUE guard would be dropped after `focus`, out of nesting order.
    let pushed = QUEUE.lock().push(event);
    drop(focus);
    pushed
}

/// Grant input focus to `owner`, or return it to the terminal with `None`.
///
/// The focus queue is discarded on every change: events typed for one owner
/// are never delivered to the next, nor leaked to the terminal.
pub fn set_focus(owner: Option<u64>) {
    let mut focus = FOCUS.lock();
    focus.owner = owner;
    focus.queue = Queue::new();
}

/// Withdraw focus only if `owner` holds it.
pub fn release_focus(owner: u64) {
    let mut focus = FOCUS.lock();
    if focus.owner == Some(owner) {
        focus.owner = None;
        focus.queue = Queue::new();
    }
}

/// Current focus owner.
#[cfg_attr(not(feature = "stage19-5-test"), allow(dead_code))]
pub fn focus() -> Option<u64> {
    FOCUS.lock().owner
}

/// Take the next event for `owner`, which must hold focus.
pub fn poll_for(owner: u64) -> Result<Option<Event>, NotFocused> {
    let mut focus = FOCUS.lock();
    if focus.owner != Some(owner) {
        return Err(NotFocused);
    }
    Ok(focus.queue.pop())
}

pub fn publish_key(key: Key) -> bool {
    publish(Event::Key(key))
}

pub fn poll() -> Option<Event> {
    QUEUE.lock().pop()
}

#[cfg(feature = "stage13-7-test")]
pub fn poll_key() -> Option<Key> {
    #[cfg(not(feature = "stage13-7-test"))]
    {
        let Event::Key(key) = poll()?;
        Some(key)
    }
    #[cfg(feature = "stage13-7-test")]
    loop {
        if let Event::Key(key) = poll()? {
            return Some(key);
        }
    }
}

#[cfg(not(feature = "stage13-7-test"))]
pub fn poll_key() -> Option<Key> {
    let Event::Key(key) = poll()?;
    Some(key)
}

pub fn read_bytes(buffer: &mut [u8]) -> usize {
    let mut count = 0;
    while count < buffer.len() {
        let Some(key) = poll_key() else { break };
        let byte = match key {
            Key::Char(character) if character.is_ascii() => character as u8,
            Key::Enter => b'\n',
            Key::Backspace => 8,
            Key::Tab => b'\t',
            Key::F1 | Key::Char(_) => continue,
        };
        buffer[count] = byte;
        count += 1;
    }
    count
}

#[cfg(feature = "stage13-7-test")]
pub fn dropped_events() -> u64 {
    QUEUE.lock().dropped
}

#[cfg(feature = "stage13-7-test")]
pub fn self_test() -> bool {
    {
        let mut q = QUEUE.lock();
        *q = Queue::new();
    }
    let ok = publish_key(Key::Char('a'))
        && publish(Event::PointerMove { dx: 3, dy: -2 })
        && poll() == Some(Event::Key(Key::Char('a')))
        && poll() == Some(Event::PointerMove { dx: 3, dy: -2 })
        && poll().is_none();
    {
        let mut q = QUEUE.lock();
        *q = Queue::new();
    }
    ok
}

/// Focus routing checks for the Stage 19.5 gate.
#[cfg(feature = "stage19-5-test")]
pub fn focus_self_test() -> bool {
    *QUEUE.lock() = Queue::new();
    set_focus(None);

    // Without focus, events reach the terminal's queue.
    if !publish_key(Key::Char('a')) || poll() != Some(Event::Key(Key::Char('a'))) {
        return false;
    }
    // With focus, they reach only the owner.
    set_focus(Some(55));
    if !publish_key(Key::Char('b'))
        || poll().is_some()
        || poll_for(56) != Err(NotFocused)
        || poll_for(55) != Ok(Some(Event::Key(Key::Char('b'))))
        || poll_for(55) != Ok(None)
    {
        return false;
    }
    // The focus queue is bounded; overflow is refused, not wrapped.
    for _ in 0..INPUT_QUEUE_CAPACITY {
        if !publish_key(Key::Enter) {
            return false;
        }
    }
    if publish_key(Key::Tab) {
        return false;
    }
    // A non-owner releasing has no effect; changing focus discards pending
    // events instead of handing them to the next reader or the terminal.
    release_focus(56);
    if focus() != Some(55) {
        return false;
    }
    release_focus(55);
    if focus().is_some() || poll().is_some() || poll_for(55) != Err(NotFocused) {
        return false;
    }
    *QUEUE.lock() = Queue::new();
    true
}
