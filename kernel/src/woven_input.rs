// Keyboard and pointer input are production; other event producers belong to
// the Stage 13.7 probe.
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
    F2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    /// Relative pointer motion in screen orientation (Y grows downwards).
    PointerMove {
        dx: i16,
        dy: i16,
    },
    /// Button 0 is left, 1 right, 2 middle.
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
///
/// One task of the owner may sleep in [`wait_for`]; `waiter` names it so
/// [`publish`] can wake it. Waking happens after FOCUS is released, never
/// under it: the task-exit path reaches FOCUS while holding the scheduler
/// lock, so FOCUS must never be held while taking the scheduler lock.
struct Focus {
    owner: Option<u64>,
    queue: Queue,
    waiter: Option<crate::task::TaskId>,
}

static FOCUS: Mutex<Focus> = Mutex::with_rank(
    Focus {
        owner: None,
        queue: Queue::new(),
        waiter: None,
    },
    10,
);

/// The caller does not hold input focus, or another of the owner's tasks is
/// already waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotFocused;

pub fn publish(event: Event) -> bool {
    let mut focus = FOCUS.lock();
    if focus.owner.is_some() {
        let pushed = focus.queue.push(event);
        let waiter = focus.waiter.take();
        drop(focus);
        if let Some(waiter) = waiter {
            crate::task::signal_event(waiter);
        }
        return pushed;
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
///
/// A registered waiter is forgotten, not woken. Focus is withdrawn only when
/// its owner exits, which tears down every task that could be waiting, and
/// this runs under the scheduler lock on that path, where waking is illegal.
pub fn set_focus(owner: Option<u64>) {
    let mut focus = FOCUS.lock();
    focus.owner = owner;
    focus.queue = Queue::new();
    focus.waiter = None;
}

/// Withdraw focus only if `owner` holds it.
pub fn release_focus(owner: u64) {
    let mut focus = FOCUS.lock();
    if focus.owner == Some(owner) {
        focus.owner = None;
        focus.queue = Queue::new();
        focus.waiter = None;
    }
}

/// Register `task` as `owner`'s focus waiter without sleeping, for a combined
/// wait over several sources. Returns the other task already registered, if
/// any, without replacing it.
pub fn register_waiter(
    owner: u64,
    task: crate::task::TaskId,
) -> Result<Option<crate::task::TaskId>, NotFocused> {
    let mut focus = FOCUS.lock();
    if focus.owner != Some(owner) {
        return Err(NotFocused);
    }
    match focus.waiter {
        Some(other) if other != task => Ok(Some(other)),
        _ => {
            focus.waiter = Some(task);
            Ok(None)
        }
    }
}

/// Drop `task`'s waiter registration if it is the one registered.
pub fn clear_waiter(task: crate::task::TaskId) {
    let mut focus = FOCUS.lock();
    if focus.waiter == Some(task) {
        focus.waiter = None;
    }
}

/// Whether `owner` holds focus and has an event queued.
pub fn pending(owner: u64) -> bool {
    let focus = FOCUS.lock();
    focus.owner == Some(owner) && focus.queue.count != 0
}

/// Block task `task` of process `owner` until an event arrives or the timer
/// reaches `deadline` (`u64::MAX` waits indefinitely). Returns `Ok(None)` on
/// timeout.
///
/// The task registers as the waiter under FOCUS, then sleeps with no lock
/// held. A publish between the two is not lost: `signal_event` latches a
/// pending permit for a task that is not yet blocked, and the sleep consumes
/// it. Wakeups for other reasons simply re-check.
pub fn wait_for(
    owner: u64,
    task: crate::task::TaskId,
    deadline: u64,
) -> Result<Option<Event>, NotFocused> {
    loop {
        let other_waiter = {
            let mut focus = FOCUS.lock();
            if focus.owner != Some(owner) {
                return Err(NotFocused);
            }
            if let Some(event) = focus.queue.pop() {
                if focus.waiter == Some(task) {
                    focus.waiter = None;
                }
                return Ok(Some(event));
            }
            match focus.waiter {
                Some(waiter) if waiter != task => Some(waiter),
                _ => {
                    focus.waiter = Some(task);
                    None
                }
            }
        };
        if let Some(waiter) = other_waiter {
            // Another task of the owner is registered. If it still exists the
            // slot is taken; if it died without unregistering, take over. The
            // liveness check runs with FOCUS released (lock order above).
            if crate::task::task_live(waiter) {
                return Err(NotFocused);
            }
            let mut focus = FOCUS.lock();
            if focus.waiter == Some(waiter) {
                focus.waiter = None;
            }
            continue;
        }
        if deadline != u64::MAX && crate::timer::ticks() >= deadline {
            let mut focus = FOCUS.lock();
            if focus.waiter == Some(task) {
                focus.waiter = None;
            }
            // An event may have landed after the check above.
            return Ok(focus.queue.pop());
        }
        crate::task::wait_for_event_until(deadline);
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

/// Next key for the terminal, skipping pointer and other events, which only a
/// focus holder consumes.
pub fn poll_key() -> Option<Key> {
    loop {
        if let Event::Key(key) = poll()? {
            return Some(key);
        }
    }
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
            Key::F1 | Key::F2 | Key::Char(_) => continue,
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
