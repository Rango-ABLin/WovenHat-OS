//! Bounded byte pump between the debug UART and verified GDB packets.

use crate::gdb_stub::{self, Packet, MAX_PACKET_DATA};

const MAX_FRAME: usize = MAX_PACKET_DATA + 4;

#[derive(Clone, Copy)]
pub enum Event {
    PacketReady,
    Ack,
    Nack,
    Ignore,
}

pub struct Transport {
    frame: [u8; MAX_FRAME],
    len: usize,
    active: bool,
    ready: Option<Packet>,
}

impl Transport {
    pub const fn new() -> Self {
        Self {
            frame: [0; MAX_FRAME],
            len: 0,
            active: false,
            ready: None,
        }
    }

    pub fn feed(&mut self, byte: u8) -> Event {
        if !self.active {
            return match byte {
                b'$' => {
                    self.active = true;
                    self.len = 1;
                    self.ready = None;
                    self.frame[0] = byte;
                    Event::Ignore
                }
                b'+' => Event::Ack,
                b'-' => Event::Nack,
                _ => Event::Ignore,
            };
        }
        if self.len >= MAX_FRAME {
            self.active = false;
            self.len = 0;
            return Event::Ignore;
        }
        self.frame[self.len] = byte;
        self.len += 1;
        if byte != b'#' {
            return Event::Ignore;
        }
        if self.len + 2 > MAX_FRAME {
            self.active = false;
            self.len = 0;
            return Event::Ignore;
        }
        Event::Ignore
    }

    /// Feed the two checksum digits after `#`; returns a verified packet or
    /// resets the pump on malformed input.
    pub fn finish_checksum(&mut self, high: u8, low: u8) -> Event {
        if !self.active || self.len == 0 || self.frame[self.len - 1] != b'#' {
            return Event::Ignore;
        }
        if self.len + 2 > MAX_FRAME {
            self.active = false;
            self.len = 0;
            return Event::Ignore;
        }
        self.frame[self.len] = high;
        self.frame[self.len + 1] = low;
        self.len += 2;
        let Some(packet) = gdb_stub::decode(&self.frame[..self.len]) else {
            self.active = false;
            self.len = 0;
            return Event::Nack;
        };
        self.ready = Some(packet);
        self.active = false;
        self.len = 0;
        Event::PacketReady
    }

    pub fn take_packet(&mut self) -> Option<Packet> {
        self.ready.take()
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "stage32-16-test"))]
pub fn self_test() -> bool {
    let mut transport = Transport::new();
    if !matches!(transport.feed(b'+'), Event::Ack) || !matches!(transport.feed(b'-'), Event::Nack) {
        return false;
    }
    for byte in b"$?#" {
        if !matches!(transport.feed(*byte), Event::Ignore) {
            return false;
        }
    }
    if !matches!(transport.finish_checksum(b'3', b'f'), Event::PacketReady) {
        return false;
    }
    let Some(packet) = transport.take_packet() else {
        return false;
    };
    if packet.data() != b"?" {
        return false;
    }
    for byte in b"$bad#" {
        let _ = transport.feed(*byte);
    }
    if !matches!(transport.finish_checksum(b'0', b'0'), Event::Nack) {
        return false;
    }
    transport.feed(b'$');
    for _ in 0..MAX_FRAME {
        let _ = transport.feed(b'x');
    }
    matches!(transport.finish_checksum(b'0', b'0'), Event::Ignore)
}

#[cfg(test)]
mod tests {
    use super::{Event, Transport};

    #[test]
    fn acknowledgements_are_nonblocking() {
        let mut transport = Transport::new();
        assert!(matches!(transport.feed(b'+'), Event::Ack));
        assert!(matches!(transport.feed(b'-'), Event::Nack));
    }
}
