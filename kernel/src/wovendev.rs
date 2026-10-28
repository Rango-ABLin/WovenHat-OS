//! Integrated bounded WovenDev debugger coordinator.

use crate::{
    gdb_binding::TaskBinding,
    gdb_debugger::RegisterContext,
    gdb_frame::CapturedFrame,
    gdb_owner::Owner,
    gdb_session::{Session, SessionAction},
    gdb_transport::{Event, Transport},
};

pub struct WovenDev {
    owner: Owner,
    binding: Option<TaskBinding>,
    session: Session,
    transport: Transport,
    checksum: [u8; 2],
    checksum_len: usize,
    reply: [u8; 520],
}

impl WovenDev {
    pub const fn new() -> Self {
        Self {
            owner: Owner::new(),
            binding: None,
            session: Session::STOPPED,
            transport: Transport::new(),
            checksum: [0; 2],
            checksum_len: 0,
            reply: [0; 520],
        }
    }

    pub fn attach(&mut self, task: u64, frame: CapturedFrame) -> bool {
        let Some(token) = self.owner.attach(task) else {
            return false;
        };
        let Some(binding) = TaskBinding::capture(&self.owner, token, frame) else {
            return false;
        };
        self.binding = Some(binding);
        self.session = Session::STOPPED;
        true
    }

    pub fn detach(&mut self) -> bool {
        let Some(binding) = self.binding else {
            return false;
        };
        if !binding.release(&mut self.owner) {
            return false;
        }
        self.binding = None;
        true
    }

    pub fn context(&self) -> Option<RegisterContext> {
        self.binding?.context(&self.owner)
    }

    #[cfg(feature = "debug-serial")]
    pub fn poll_uart(&mut self) -> SessionAction {
        for _ in 0..64 {
            let Some(byte) = crate::debug_serial::try_read() else {
                break;
            };
            let action = self.feed(byte);
            if let SessionAction::Reply(written) = action {
                crate::debug_serial::write_bytes(&self.reply[..written]);
                return action;
            }
        }
        SessionAction::Ignore
    }

    /// Feed one wire byte. Checksum bytes are held until the transport can
    /// validate the complete packet; no unverified packet reaches the session.
    pub fn feed(&mut self, byte: u8) -> SessionAction {
        if self.checksum_len != 0 {
            self.checksum[self.checksum_len] = byte;
            self.checksum_len += 1;
            if self.checksum_len == 2 {
                self.checksum_len = 0;
                return self.finish_packet();
            }
            return SessionAction::Ignore;
        }
        let event = self.transport.feed(byte);
        if byte == b'#' {
            self.checksum_len = 0;
        }
        self.handle_event(event)
    }

    fn finish_packet(&mut self) -> SessionAction {
        let event = self
            .transport
            .finish_checksum(self.checksum[0], self.checksum[1]);
        self.handle_event(event)
    }

    fn handle_event(&mut self, event: Event) -> SessionAction {
        let Event::PacketReady = event else {
            return SessionAction::Ignore;
        };
        let Some(packet) = self.transport.take_packet() else {
            return SessionAction::Ignore;
        };
        let Some(binding) = self.binding.as_mut() else {
            return SessionAction::Ignore;
        };
        let Some(context) = binding.context_mut(&self.owner) else {
            return SessionAction::Ignore;
        };
        self.session
            .handle(&packet, context, 0, &[], &mut self.reply)
    }

    #[cfg(any(test, feature = "stage32-integrated-test"))]
    pub fn self_test() -> bool {
        let mut debugger = Self::new();
        if !debugger.attach(3, CapturedFrame::ZERO) {
            return false;
        }
        for byte in b"$?#3f" {
            let _ = debugger.feed(*byte);
        }
        debugger.session.is_stopped() && debugger.context().is_some()
    }
}

impl Default for WovenDev {
    fn default() -> Self {
        Self::new()
    }
}
