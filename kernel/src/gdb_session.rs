//! Bounded GDB session state machine above packet framing and dispatch.
//!
//! The session owns protocol state only. Continue/step/kill are returned as
//! actions to a future trap owner; this module never changes CPU state or
//! dereferences guest memory.

use crate::gdb_debugger::{self, Action, RegisterContext};
use crate::gdb_stub;

/// Result of a session packet after any reply has been encoded.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    Reply(usize),
    Continue,
    Step,
    Kill,
    /// Detach acknowledged. The payload is `frame[..n]`, the encoded `OK`
    /// reply, written the same way as [`SessionAction::Reply`]. The caller
    /// must send it before releasing any attachment and resuming.
    Detach(usize),
    Ignore,
}

/// Protocol lifecycle for one debugger attachment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Session {
    active: bool,
    stopped: bool,
    signal: u8,
}

impl Session {
    pub const STOPPED: Self = Self {
        active: true,
        stopped: true,
        signal: 5,
    };

    pub const fn is_active(self) -> bool {
        self.active
    }

    pub const fn is_stopped(self) -> bool {
        self.stopped
    }

    /// Publish a trap stop and its POSIX-compatible signal number.
    pub fn stop(&mut self, signal: u8) {
        if self.active {
            self.stopped = true;
            self.signal = signal;
        }
    }

    /// Handle one complete decoded RSP packet and encode any reply packet.
    ///
    /// A `gdb_stub::Packet` is copied into a bounded local view by the caller;
    /// no packet or guest memory is retained after this call.
    pub fn handle(
        &mut self,
        packet: &gdb_stub::Packet,
        context: &mut RegisterContext,
        memory_base: u64,
        memory: &[u8],
        frame: &mut [u8],
    ) -> SessionAction {
        if !self.active {
            return SessionAction::Ignore;
        }
        let command = packet.data();
        if command == b"?" {
            // Signal values used by this boundary are one byte; retain the
            // conventional two-hex-digit stop reply.
            let mut stop_reply = [0u8; 3];
            stop_reply[0] = b'S';
            stop_reply[1] = hex(self.signal >> 4);
            stop_reply[2] = hex(self.signal & 0x0f);
            return encode_reply(&stop_reply, frame);
        }
        if !self.stopped {
            return SessionAction::Ignore;
        }
        let mut payload = [0u8; 512];
        if command.starts_with(b"G") {
            return match gdb_debugger::write_registers(command, context, &mut payload) {
                Some(written) => encode_reply(&payload[..written], frame),
                None => encode_reply(b"E01", frame),
            };
        }
        let Some((action, written)) =
            gdb_debugger::dispatch(command, context, memory_base, memory, &mut payload)
        else {
            return SessionAction::Ignore;
        };
        match action {
            Action::Reply => encode_reply(&payload[..written], frame),
            Action::Continue => {
                self.stopped = false;
                SessionAction::Continue
            }
            Action::Step => {
                self.stopped = false;
                SessionAction::Step
            }
            Action::Kill => {
                self.active = false;
                self.stopped = false;
                SessionAction::Kill
            }
            Action::Detach => {
                self.active = false;
                self.stopped = false;
                match gdb_stub::encode(&payload[..written], frame) {
                    Some(encoded) => SessionAction::Detach(encoded),
                    None => SessionAction::Ignore,
                }
            }
            Action::Empty => SessionAction::Ignore,
        }
    }
}

fn encode_reply(payload: &[u8], frame: &mut [u8]) -> SessionAction {
    match gdb_stub::encode(payload, frame) {
        Some(written) => SessionAction::Reply(written),
        None => SessionAction::Ignore,
    }
}

fn hex(value: u8) -> u8 {
    match value {
        0..=9 => b'0' + value,
        _ => b'a' + value - 10,
    }
}

#[cfg(any(test, feature = "stage32-11-test"))]
pub fn self_test() -> bool {
    let mut context = RegisterContext::ZERO;
    let mut session = Session::STOPPED;
    let mut frame = [0u8; 520];
    let Some(packet) = gdb_stub::decode(b"$?#3f") else {
        return false;
    };
    let SessionAction::Reply(written) = session.handle(&packet, &mut context, 0, b"", &mut frame)
    else {
        return false;
    };
    let Some(reply) = gdb_stub::decode(&frame[..written]) else {
        return false;
    };
    if reply.data() != b"S05" {
        return false;
    }
    let c = gdb_stub::decode(b"$c#63");
    let Some(c) = c else {
        return false;
    };
    if session.handle(&c, &mut context, 0, b"", &mut frame) != SessionAction::Continue
        || session.is_stopped()
    {
        return false;
    }
    session.stop(11);
    if !session.is_stopped() {
        return false;
    }
    let s = gdb_stub::decode(b"$s#73");
    let Some(s) = s else {
        return false;
    };
    if session.handle(&s, &mut context, 0, b"", &mut frame) != SessionAction::Step {
        return false;
    }
    session.stop(5);
    let k = gdb_stub::decode(b"$k#6b");
    let Some(k) = k else {
        return false;
    };
    session.handle(&k, &mut context, 0, b"", &mut frame) == SessionAction::Kill
        && !session.is_active()
}

#[cfg(test)]
mod tests {
    use super::{Session, SessionAction};
    use crate::{gdb_debugger::RegisterContext, gdb_stub};

    #[test]
    fn fresh_session_is_stopped_and_active() {
        assert_eq!(Session::STOPPED, Session::STOPPED);
        assert!(Session::STOPPED.is_active());
        assert!(Session::STOPPED.is_stopped());
        assert_eq!(SessionAction::Ignore, SessionAction::Ignore);
    }

    #[test]
    fn detach_acknowledges_and_deactivates() {
        let mut context = RegisterContext::ZERO;
        let mut session = Session::STOPPED;
        let mut frame = [0u8; 520];
        let packet = gdb_stub::decode(b"$D#44").expect("valid D packet");
        let SessionAction::Detach(written) =
            session.handle(&packet, &mut context, 0, b"", &mut frame)
        else {
            panic!("expected Detach action");
        };
        let reply = gdb_stub::decode(&frame[..written]).expect("encoded OK reply");
        assert_eq!(reply.data(), b"OK");
        assert!(!session.is_active());
        assert!(!session.is_stopped());
    }
}
