//! Owned TCP and event-driven operations for native WovenHat platform ports.
//! Sends are copied by the kernel at submission. Receive buffers are supplied
//! only at completion. These APIs require native WovenHat userspace.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Socket(pub u64);
pub const MAX_TRANSFER: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Rejected,
    InvalidEndpoint,
    InvalidLength,
    OutputTooSmall,
    AlreadyConsumed,
    InvalidCompletion,
    Transport(i32),
    EndOfStream,
    TimedOut,
    InvalidTimeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub address: [u8; 4],
    pub port: u16,
}
impl Endpoint {
    pub fn packed(self) -> Result<u64, Error> {
        if self.port == 0 || self.address[0] == 0 || self.address[0] >= 224 {
            return Err(Error::InvalidEndpoint);
        }
        Ok(u64::from(u32::from_be_bytes(self.address)) | (u64::from(self.port) << 32))
    }
}

fn invoke(number: u64, first: u64, second: u64, third: u64) -> Result<u64, Error> {
    // SAFETY: module call sites meet the ABI's pointer and length contracts.
    // Like the rest of libwoven, the caller must be in native WovenHat userspace.
    let result = unsafe { crate::syscall::invoke(number, first, second, third) };
    if result >= crate::syscall::WOULD_BLOCK {
        Err(Error::Rejected)
    } else {
        Ok(result)
    }
}

pub struct TcpStream {
    handle: u64,
}
impl TcpStream {
    pub fn new() -> Result<Self, Error> {
        invoke(37, 2, 0, 0).map(|handle| Self { handle })
    }
    pub fn connect(&self, endpoint: Endpoint) -> Result<Pending<'_>, Error> {
        let handle = invoke(81, self.handle, endpoint.packed()?, 0)?;
        Ok(Pending::new(self, handle, Kind::Connect, 0))
    }
    /// The kernel copies input before returning; completion may be partial.
    pub fn send(&self, bytes: &[u8]) -> Result<Pending<'_>, Error> {
        validate_length(bytes.len())?;
        let handle = invoke(77, self.handle, bytes.as_ptr() as u64, bytes.len() as u64)?;
        Ok(Pending::new(self, handle, Kind::Send, bytes.len()))
    }
    pub fn receive(&self, capacity: usize) -> Result<Pending<'_>, Error> {
        validate_length(capacity)?;
        let handle = invoke(78, self.handle, capacity as u64, 0)?;
        Ok(Pending::new(self, handle, Kind::Receive, capacity))
    }
    /// Event-driven blocking. Use a network worker; no deadline is provided.
    pub fn write_all(&self, mut bytes: &[u8]) -> Result<(), Error> {
        while !bytes.is_empty() {
            let count = self
                .send(&bytes[..bytes.len().min(MAX_TRANSFER)])?
                .wait(&mut [])?;
            if count == 0 {
                return Err(Error::EndOfStream);
            }
            bytes = &bytes[count..];
        }
        Ok(())
    }
    /// Bound each submitted operation using the kernel completion-port deadline.
    pub fn write_all_timeout(&self, mut bytes: &[u8], timeout: u64) -> Result<(), Error> {
        validate_timeout(timeout)?;
        while !bytes.is_empty() {
            let count = self
                .send(&bytes[..bytes.len().min(MAX_TRANSFER)])?
                .wait_timeout(&mut [], timeout)?;
            if count == 0 {
                return Err(Error::EndOfStream);
            }
            bytes = &bytes[count..];
        }
        Ok(())
    }
    pub fn read_exact_timeout(&self, mut output: &mut [u8], timeout: u64) -> Result<(), Error> {
        validate_timeout(timeout)?;
        while !output.is_empty() {
            let capacity = output.len().min(MAX_TRANSFER);
            let count = self
                .receive(capacity)?
                .wait_timeout(&mut output[..capacity], timeout)?;
            if count == 0 {
                return Err(Error::EndOfStream);
            }
            output = &mut output[count..];
        }
        Ok(())
    }
    pub fn read_exact(&self, mut output: &mut [u8]) -> Result<(), Error> {
        while !output.is_empty() {
            let capacity = output.len().min(MAX_TRANSFER);
            let count = self.receive(capacity)?.wait(&mut output[..capacity])?;
            if count == 0 {
                return Err(Error::EndOfStream);
            }
            output = &mut output[count..];
        }
        Ok(())
    }
}
impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = invoke(42, self.handle, 0, 0);
    }
}
fn validate_length(length: usize) -> Result<(), Error> {
    if length == 0 || length > MAX_TRANSFER {
        Err(Error::InvalidLength)
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Connect,
    Send,
    Receive,
}

/// One generation-tagged operation, borrowing its socket and cancelling on drop.
/// Receive buffers must meet the submitted capacity before any copyout happens.
///
/// ```compile_fail
/// use libwoven::net::TcpStream;
/// let pending;
/// {
///     let stream = TcpStream::new().unwrap();
///     pending = stream.receive(1).unwrap();
/// }
/// drop(pending); // The socket cannot be destroyed while borrowed by work.
/// ```
///
/// Completions are owned by the submitting kernel task, so pending work cannot
/// be moved to another thread or shared across threads.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<libwoven::net::Pending<'static>>();
/// ```
///
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<libwoven::net::Pending<'static>>();
/// ```
pub struct Pending<'socket> {
    _socket: &'socket TcpStream,
    _submitting_task: core::marker::PhantomData<*mut ()>,
    handle: Option<u64>,
    kind: Kind,
    capacity: usize,
}
impl<'socket> Pending<'socket> {
    fn new(socket: &'socket TcpStream, handle: u64, kind: Kind, capacity: usize) -> Self {
        Self {
            _socket: socket,
            _submitting_task: core::marker::PhantomData,
            handle: Some(handle),
            kind,
            capacity,
        }
    }
    pub fn poll(&mut self, output: &mut [u8]) -> Result<Option<usize>, Error> {
        self.finish(output, false)
    }
    pub fn wait(mut self, output: &mut [u8]) -> Result<usize, Error> {
        self.finish(output, true)?.ok_or(Error::InvalidCompletion)
    }
    /// Event-driven deadline in 100 Hz kernel ticks, relative to this wait.
    /// A timeout consumes this operation and cancels it during Drop.
    pub fn wait_timeout(mut self, output: &mut [u8], timeout: u64) -> Result<usize, Error> {
        validate_timeout(timeout)?;
        let handle = self.handle.ok_or(Error::AlreadyConsumed)?;
        if self.kind == Kind::Receive && output.len() < self.capacity {
            return Err(Error::OutputTooSmall);
        }
        let port = CompletionPort(invoke(82, 0, 0, 0)?);
        invoke(84, handle, port.0, 0)?;
        let options = [1u64, timeout]; // capacity, relative timeout
        let mut event = [0u8; 40];
        let count = invoke(
            86,
            port.0,
            event.as_mut_ptr() as u64,
            options.as_ptr() as u64,
        )?;
        if count == 0 {
            return Err(Error::TimedOut);
        }
        if count != 1 || !valid_event(&event, handle) {
            return Err(Error::InvalidCompletion);
        }
        self.finish(output, false)?.ok_or(Error::InvalidCompletion)
    }
    pub fn cancel(mut self) -> Result<(), Error> {
        invoke(67, self.handle.ok_or(Error::AlreadyConsumed)?, 0, 0)?;
        self.handle = None;
        Ok(())
    }
    fn finish(&mut self, output: &mut [u8], wait: bool) -> Result<Option<usize>, Error> {
        let handle = self.handle.ok_or(Error::AlreadyConsumed)?;
        if self.kind == Kind::Receive && output.len() < self.capacity {
            return Err(Error::OutputTooSmall);
        }
        let mut completion = [0u8; 16];
        // Receive output has its full submitted capacity. Send/connect do not
        // copy payload. The kernel retains no pointer after this call returns.
        let result = invoke(
            if wait { 80 } else { 79 },
            handle,
            completion.as_mut_ptr() as u64,
            output.as_mut_ptr() as u64,
        )?;
        if result == 0 {
            return Ok(None);
        }
        self.handle = None; // Consumed: never cancel or poll this generation again.
        if result != 1 {
            return Err(Error::InvalidCompletion);
        }
        decode_completion(completion, self.kind, self.capacity).map(Some)
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = invoke(67, handle, 0, 0);
        }
    }
}
struct CompletionPort(u64);
impl Drop for CompletionPort {
    fn drop(&mut self) {
        let _ = invoke(83, self.0, 0, 0);
    }
}
pub(crate) fn validate_timeout(timeout: u64) -> Result<(), Error> {
    if timeout == u64::MAX {
        Err(Error::InvalidTimeout)
    } else {
        Ok(())
    }
}
fn valid_event(event: &[u8; 40], handle: u64) -> bool {
    u64::from_le_bytes(event[..8].try_into().unwrap()) == handle
        && event[8..16] == [0; 8] // cookie
        && u32::from_le_bytes(event[16..20].try_into().unwrap()) == 1 // Network
        && event[20..24] == [0; 4] // no cancellation flag
        && event[28..32] == [0; 4] // reserved
}
fn decode_completion(bytes: [u8; 16], kind: Kind, capacity: usize) -> Result<usize, Error> {
    let status = i32::from_le_bytes(bytes[..4].try_into().unwrap());
    let reserved = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let value = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    if reserved != 0 {
        return Err(Error::InvalidCompletion);
    }
    if status != 0 {
        return Err(Error::Transport(status));
    }
    if value > capacity as u64 || (kind == Kind::Connect && value != 0) {
        return Err(Error::InvalidCompletion);
    }
    Ok(value as usize)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_matches_kernel_packing_and_bounds() {
        assert_eq!(
            Endpoint {
                address: [10, 0, 2, 2],
                port: 9050
            }
            .packed(),
            Ok((9050u64 << 32) | 0x0a000202)
        );
        for endpoint in [
            Endpoint {
                address: [0; 4],
                port: 80,
            },
            Endpoint {
                address: [224, 0, 0, 1],
                port: 80,
            },
            Endpoint {
                address: [127, 0, 0, 1],
                port: 0,
            },
        ] {
            assert_eq!(endpoint.packed(), Err(Error::InvalidEndpoint));
        }
    }
    #[test]
    fn completion_rejects_oversize_reserved_and_transport_failure() {
        let mut bytes = [0; 16];
        bytes[8..].copy_from_slice(&1025u64.to_le_bytes());
        assert_eq!(
            decode_completion(bytes, Kind::Receive, 1024),
            Err(Error::InvalidCompletion)
        );
        bytes[8..].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(decode_completion(bytes, Kind::Send, 8), Ok(7));
        assert_eq!(
            decode_completion(bytes, Kind::Connect, 0),
            Err(Error::InvalidCompletion)
        );
        bytes[..4].copy_from_slice(&(-3i32).to_le_bytes());
        assert_eq!(
            decode_completion(bytes, Kind::Receive, 8),
            Err(Error::Transport(-3))
        );
        bytes[4] = 1;
        assert_eq!(
            decode_completion(bytes, Kind::Receive, 8),
            Err(Error::InvalidCompletion)
        );
    }
    #[test]
    fn deadline_and_event_identity_are_checked() {
        assert_eq!(validate_timeout(u64::MAX), Err(Error::InvalidTimeout));
        assert_eq!(validate_timeout(0), Ok(()));
        let mut event = [0; 40];
        event[..8].copy_from_slice(&55u64.to_le_bytes());
        event[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(valid_event(&event, 55));
        assert!(!valid_event(&event, 54));
        event[20] = 1;
        assert!(!valid_event(&event, 55));
    }
    #[test]
    fn transfer_bounds() {
        assert_eq!(validate_length(0), Err(Error::InvalidLength));
        assert_eq!(validate_length(1025), Err(Error::InvalidLength));
        assert_eq!(validate_length(1024), Ok(()));
    }
}
