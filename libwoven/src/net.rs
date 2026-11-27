//! Safe userspace wrappers for the existing bounded IPv4 network ABI.
//!
//! These wrappers provide TCP/DNS plumbing only. They intentionally do not
//! claim TLS: certificate validation and encrypted-channel ownership belong to
//! the privileged TLS service used by IPP.

use crate::{check, syscall};
use crate::Error as CoreError;

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
    pub fn open(kind: Kind) -> Result<Self, CoreError> {
        let raw = check(unsafe { syscall::invoke(syscall::SOCKET, kind as u64, 0, 0) })?;
        Ok(Self { raw, kind })
    }

    pub const fn kind(&self) -> Kind {
        self.kind
    }

    pub fn bind(&self, port: u16) -> Result<(), CoreError> {
        check(unsafe { syscall::invoke(syscall::BIND, self.raw, port as u64, 0) }).map(|_| ())
    }

    /// Put this TCP socket into bounded single-connection listen state.
    pub fn listen(&self, port: u16) -> Result<(), CoreError> {
        listen(self.raw, port)
    }

    /// Wait for one inbound connection and return its endpoint.
    pub fn accept(&self) -> Result<Endpoint, CoreError> {
        accept(self.raw)
    }

    pub fn connect(&self, endpoint: Endpoint) -> Result<(), CoreError> {
        check(unsafe { syscall::invoke(syscall::CONNECT, self.raw, endpoint.packed(), 0) })
            .map(|_| ())
    }

    pub fn send(&self, bytes: &[u8]) -> Result<usize, CoreError> {
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

    pub fn receive(&self, output: &mut [u8]) -> Result<usize, CoreError> {
        let result = unsafe {
            syscall::invoke(
                syscall::NET_RECV,
                self.raw,
                output.as_mut_ptr() as u64,
                output.len() as u64,
            )
        };
        if result == syscall::WOULD_BLOCK {
            Err(CoreError::WouldBlock)
        } else {
            check(result).map(|length| length as usize)
        }
    }
}

/// Put a process-owned TCP socket descriptor into bounded listen state.
pub fn listen(socket: u64, port: u16) -> Result<(), CoreError> {
    check(unsafe { syscall::invoke(syscall::NET_LISTEN, socket, port as u64, 0) }).map(|_| ())
}

/// Wait event-driven for one inbound connection on a listening descriptor and
/// return the accepted peer. No Ring-3 pointer is retained and no spin loop is
/// used. The descriptor stays a listener: once the accepted peer closes, the
/// kernel re-arms it so this call can accept the next client. There is no
/// simultaneous backlog; a second `accept` waits until a client disconnects.
pub fn accept(socket: u64) -> Result<Endpoint, CoreError> {
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
        return Err(CoreError::Failed);
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

pub fn dns_start(host: &str) -> Result<u64, CoreError> {
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
    pub fn start(host: &str) -> Result<Self, CoreError> {
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

pub fn dns_cancel(query: u64) -> Result<(), CoreError> {
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


// ---------------------------------------------------------------------------
// Owned native TCP (`net::TcpStream`) built on the existing bounded syscalls.
// ---------------------------------------------------------------------------

/// Bounded network error. Distinguishes authority rejection, invalid endpoint
/// and timeout inputs, deadlines, would-block backpressure and output sizing
/// from a generic kernel rejection. Fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The kernel rejected the call for a reason not separately classified.
    Failed,
    /// Operation would block; no result is available yet.
    WouldBlock,
    /// Network authority is absent or revoked for this process.
    Rejected,
    /// Endpoint address or port is not a valid reachable target.
    InvalidEndpoint,
    /// Timeout argument is zero or above the bounded maximum.
    InvalidTimeout,
    /// The operation did not complete within its deadline.
    TimedOut,
    /// The poll output buffer is smaller than the available data.
    OutputTooSmall,
    /// The peer closed the connection.
    Closed,
}

impl From<CoreError> for Error {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::WouldBlock => Error::WouldBlock,
            CoreError::Failed => Error::Failed,
        }
    }
}

/// Largest accepted deadline in 100 Hz ticks; `u64::MAX` and zero are refused.
pub const MAX_TIMEOUT_TICKS: u64 = u64::MAX - 1;

/// Reject a zero or above-bound deadline before any socket work.
pub fn validate_timeout(timeout: u64) -> Result<(), Error> {
    if timeout == 0 || timeout > MAX_TIMEOUT_TICKS {
        Err(Error::InvalidTimeout)
    } else {
        Ok(())
    }
}

const ASYNC_NET_RECV: u64 = 78;
const ASYNC_CANCEL: u64 = 67;
const ASYNC_TCP_CONNECT: u64 = 81;
const MAX_IO: usize = 4096;

fn validate_endpoint(endpoint: Endpoint) -> Result<(), Error> {
    if endpoint.port == 0
        || endpoint.address == [0, 0, 0, 0]
        || endpoint.address == [255, 255, 255, 255]
    {
        Err(Error::InvalidEndpoint)
    } else {
        Ok(())
    }
}

/// An owned TCP connection. Construction requires Network authority; failure
/// to obtain a socket is `Rejected`. Dropping closes the socket.
pub struct TcpStream {
    raw: u64,
}

impl TcpStream {
    /// Create a bounded TCP socket. `Rejected` if network authority is absent.
    pub fn new() -> Result<Self, Error> {
        let raw = unsafe { syscall::invoke(syscall::SOCKET, Kind::Tcp as u64, 0, 0) };
        if raw >= syscall::WOULD_BLOCK {
            Err(Error::Rejected)
        } else {
            Ok(Self { raw })
        }
    }

    /// Raw kernel socket id.
    pub const fn raw(&self) -> u64 {
        self.raw
    }

    /// Connect to `endpoint`, awaiting the TCP handshake to complete. An
    /// in-progress connect is not a caller error; it is awaited. `InvalidEndpoint`
    /// for an unreachable/invalid target, `Rejected` without network authority.
    pub fn connect(&self, endpoint: Endpoint) -> Result<&Self, Error> {
        validate_endpoint(endpoint)?;
        let handle =
            unsafe { syscall::invoke(ASYNC_TCP_CONNECT, self.raw, endpoint.packed(), 0) };
        if handle >= syscall::WOULD_BLOCK {
            return Err(Error::Rejected);
        }
        let mut completion = [0u8; 16];
        let _ = unsafe {
            syscall::invoke(
                syscall::ASYNC_NET_WAIT,
                handle,
                completion.as_mut_ptr() as u64,
                0,
            )
        };
        let status = i32::from_ne_bytes([completion[0], completion[1], completion[2], completion[3]]);
        if status != 0 {
            Err(Error::InvalidEndpoint)
        } else {
            Ok(self)
        }
    }

    /// `connect` already awaited the handshake; validate `ticks` and return.
    pub fn wait_timeout(&self, _output: &mut [u8], ticks: u64) -> Result<&Self, Error> {
        validate_timeout(ticks)?;
        Ok(self)
    }

    /// Send all bytes, retrying on backpressure. No timeout bound.
    pub fn write_all(&self, data: &[u8]) -> Result<(), Error> {
        let mut rest = data;
        while !rest.is_empty() {
            let sent = self.send_once(rest)?;
            rest = &rest[sent..];
        }
        Ok(())
    }

    /// Send all bytes within `ticks` 100 Hz ticks.
    pub fn write_all_timeout(&self, data: &[u8], ticks: u64) -> Result<(), Error> {
        validate_timeout(ticks)?;
        let start = crate::ticks();
        let mut rest = data;
        while !rest.is_empty() {
            match self.send_once(rest) {
                Ok(sent) => rest = &rest[sent..],
                Err(Error::WouldBlock) => {
                    if crate::ticks().wrapping_sub(start) >= ticks {
                        return Err(Error::TimedOut);
                    }
                    crate::yield_now();
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn send_once(&self, data: &[u8]) -> Result<usize, Error> {
        let chunk = data.len().min(MAX_IO);
        let result =
            unsafe { syscall::invoke(syscall::NET_SEND, self.raw, data.as_ptr() as u64, chunk as u64) };
        if result == syscall::WOULD_BLOCK {
            Err(Error::WouldBlock)
        } else if result == syscall::ERROR {
            Err(Error::Failed)
        } else {
            Ok(result as usize)
        }
    }

    /// Receive exactly `buf.len()` bytes, retrying on backpressure.
    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), Error> {
        self.fill(buf, None)
    }

    /// Receive exactly `buf.len()` bytes within `ticks` 100 Hz ticks.
    pub fn read_exact_timeout(&self, buf: &mut [u8], ticks: u64) -> Result<(), Error> {
        validate_timeout(ticks)?;
        self.fill(buf, Some(ticks))
    }

    fn fill(&self, buf: &mut [u8], timeout: Option<u64>) -> Result<(), Error> {
        let start = crate::ticks();
        let mut offset = 0;
        while offset < buf.len() {
            let result = unsafe {
                syscall::invoke(
                    syscall::NET_RECV,
                    self.raw,
                    buf[offset..].as_mut_ptr() as u64,
                    (buf.len() - offset) as u64,
                )
            };
            if result == syscall::WOULD_BLOCK {
                if let Some(limit) = timeout {
                    if crate::ticks().wrapping_sub(start) >= limit {
                        return Err(Error::TimedOut);
                    }
                }
                crate::yield_now();
                continue;
            }
            if result == syscall::ERROR {
                return Err(Error::Failed);
            }
            let n = result as usize;
            if n == 0 {
                return Err(Error::Closed);
            }
            offset += n;
        }
        Ok(())
    }

    /// Submit an asynchronous receive of up to `len` bytes as a `Pending`.
    pub fn receive(&self, len: usize) -> Pending {
        Pending::submit_recv(self.raw, len)
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = unsafe { syscall::invoke(syscall::NET_CLOSE, self.raw, 0, 0) };
    }
}

/// A submitted async receive awaiting completion.
pub struct Pending {
    handle: u64,
    len: usize,
    done: bool,
}

impl Pending {
    fn submit_recv(raw: u64, len: usize) -> Pending {
        let capacity = len.min(MAX_IO);
        let handle = unsafe { syscall::invoke(ASYNC_NET_RECV, raw, capacity as u64, 0) };
        Pending { handle, len, done: handle >= syscall::WOULD_BLOCK }
    }

    /// Poll once. `OutputTooSmall` if `output` is shorter than `len`. `WouldBlock`
    /// while pending. On completion, `Ok(())` and received bytes are in `output`.
    pub fn poll(&mut self, output: &mut [u8]) -> Result<(), Error> {
        if output.len() < self.len {
            return Err(Error::OutputTooSmall);
        }
        if self.done {
            return Err(Error::Failed);
        }
        if self.handle >= syscall::WOULD_BLOCK {
            return Err(Error::Failed);
        }
        let mut completion = [0u8; 16];
        let user_data = if output.is_empty() { 0 } else { output.as_mut_ptr() as u64 };
        let ready = unsafe {
            syscall::invoke(
                syscall::ASYNC_NET_POLL,
                self.handle,
                completion.as_mut_ptr() as u64,
                user_data,
            )
        };
        if ready == 0 {
            return Err(Error::WouldBlock);
        }
        self.done = true;
        let status = i32::from_ne_bytes([completion[0], completion[1], completion[2], completion[3]]);
        if status != 0 {
            return Err(Error::Failed);
        }
        Ok(())
    }

    /// Cancel a still-pending operation and release its kernel reference.
    pub fn cancel(mut self) -> Result<(), Error> {
        let handle = self.handle;
        self.done = true;
        self.handle = syscall::WOULD_BLOCK;
        if handle >= syscall::WOULD_BLOCK {
            return Err(Error::Failed);
        }
        let result = unsafe { syscall::invoke(ASYNC_CANCEL, handle, 0, 0) };
        if result == syscall::ERROR { Err(Error::Failed) } else { Ok(()) }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.done && self.handle < syscall::WOULD_BLOCK {
            let _ = unsafe { syscall::invoke(ASYNC_CANCEL, self.handle, 0, 0) };
        }
    }
}
