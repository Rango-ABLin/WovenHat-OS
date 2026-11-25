//! Safe userspace wrappers for the existing bounded IPv4 network ABI.
//!
//! These wrappers provide TCP/DNS plumbing only. They intentionally do not
//! claim TLS: certificate validation and encrypted-channel ownership belong to
//! the privileged TLS service used by IPP.

use crate::{Error, check, syscall};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Udp = 1,
    Tcp = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub address: [u8; 4],
    pub port: u16,
}

impl Endpoint {
    pub const fn packed(self) -> u64 {
        (u32::from_be_bytes(self.address) as u64) | (self.port as u64) << 32
    }
}

#[derive(Debug)]
pub struct Socket {
    raw: u64,
    kind: Kind,
}

impl Socket {
    pub fn open(kind: Kind) -> Result<Self, Error> {
        let raw = check(unsafe { syscall::invoke(syscall::SOCKET, kind as u64, 0, 0) })?;
        Ok(Self { raw, kind })
    }

    pub const fn kind(&self) -> Kind {
        self.kind
    }

    pub fn bind(&self, port: u16) -> Result<(), Error> {
        check(unsafe { syscall::invoke(syscall::BIND, self.raw, port as u64, 0) }).map(|_| ())
    }

    /// Put this TCP socket into bounded single-connection listen state.
    pub fn listen(&self, port: u16) -> Result<(), Error> {
        listen(self.raw, port)
    }

    /// Wait for one inbound connection and return its endpoint.
    pub fn accept(&self) -> Result<Endpoint, Error> {
        accept(self.raw)
    }

    pub fn connect(&self, endpoint: Endpoint) -> Result<(), Error> {
        check(unsafe { syscall::invoke(syscall::CONNECT, self.raw, endpoint.packed(), 0) })
            .map(|_| ())
    }

    pub fn send(&self, bytes: &[u8]) -> Result<usize, Error> {
        check(unsafe {
            syscall::invoke(
                syscall::NET_SEND,
                self.raw,
                bytes.as_ptr() as u64,
                bytes.len() as u64,
            )
        })
        .map(|length| length as usize)
    }

    pub fn receive(&self, output: &mut [u8]) -> Result<usize, Error> {
        let result = unsafe {
            syscall::invoke(
                syscall::NET_RECV,
                self.raw,
                output.as_mut_ptr() as u64,
                output.len() as u64,
            )
        };
        if result == syscall::WOULD_BLOCK {
            Err(Error::WouldBlock)
        } else {
            check(result).map(|length| length as usize)
        }
    }
}

/// Put a process-owned TCP socket descriptor into bounded listen state.
pub fn listen(socket: u64, port: u16) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::NET_LISTEN, socket, port as u64, 0) }).map(|_| ())
}

/// Wait event-driven for one inbound connection on a listening descriptor and
/// return the accepted peer. No Ring-3 pointer is retained and no spin loop is
/// used. After a successful accept the descriptor is connected; another client
/// requires a new socket in listen state.
pub fn accept(socket: u64) -> Result<Endpoint, Error> {
    let handle = check(unsafe { syscall::invoke(syscall::ASYNC_NET_ACCEPT, socket, 0, 0) })?;
    let mut completion = [0u8; 16];
    check(unsafe {
        syscall::invoke(
            syscall::ASYNC_NET_WAIT,
            handle,
            completion.as_mut_ptr() as u64,
            0,
        )
    })?;
    let status = i32::from_ne_bytes([completion[0], completion[1], completion[2], completion[3]]);
    if status != 0 {
        return Err(Error::Failed);
    }
    let value = u64::from_ne_bytes([
        completion[8],
        completion[9],
        completion[10],
        completion[11],
        completion[12],
        completion[13],
        completion[14],
        completion[15],
    ]);
    Ok(Endpoint {
        address: (value as u32).to_be_bytes(),
        port: (value >> 32) as u16,
    })
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = unsafe { syscall::invoke(syscall::NET_CLOSE, self.raw, 0, 0) };
    }
}

pub fn dns_start(host: &str) -> Result<u64, Error> {
    check(unsafe {
        syscall::invoke(
            syscall::DNS_START,
            host.as_ptr() as u64,
            host.len() as u64,
            0,
        )
    })
}

/// An asynchronous DNS lookup. Dropping an unfinished lookup cancels the
/// kernel-owned query, so a caller cannot strand one of the bounded resolver
/// slots by abandoning a request.
#[derive(Debug)]
pub struct DnsQuery {
    raw: u64,
    finished: bool,
}

impl DnsQuery {
    pub fn start(host: &str) -> Result<Self, Error> {
        Ok(Self {
            raw: dns_start(host)?,
            finished: false,
        })
    }

    pub fn poll(&mut self) -> Result<Option<[u8; 4]>, Error> {
        let result = dns_poll(self.raw)?;
        if result.is_some() {
            self.finished = true;
        }
        Ok(result)
    }
}

impl Drop for DnsQuery {
    fn drop(&mut self) {
        if !self.finished {
            let _ = dns_cancel(self.raw);
        }
    }
}

pub fn dns_poll(query: u64) -> Result<Option<[u8; 4]>, Error> {
    let mut address = [0u8; 4];
    let result =
        unsafe { syscall::invoke(syscall::DNS_POLL, query, address.as_mut_ptr() as u64, 0) };
    if result == 0 {
        Ok(None)
    } else {
        check(result).map(|_| Some(address))
    }
}

pub fn dns_cancel(query: u64) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::DNS_CANCEL, query, 0, 0) }).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_packing_is_network_order_and_stable() {
        assert_eq!(
            Endpoint {
                address: [192, 0, 2, 7],
                port: 631,
            }
            .packed(),
            631u64 << 32 | 0xc0000207
        );
    }

    #[test]
    fn socket_kinds_are_explicit() {
        assert_ne!(Kind::Tcp, Kind::Udp);
        assert_eq!(Kind::Tcp as u64, 2);
    }
}
