//! Bounded desktop-service IPC clients.
//!
//! These clients keep policy in the service and expose only fixed-size,
//! versioned records to applications.  A caller must already possess the
//! normal IPC capability; service discovery and the kernel handle rights are
//! still the authority boundary.

pub const MAX_MESSAGE_BYTES: usize = 256;
pub const CLIPBOARD_SERVICE: &[u8] = b"woven.clipboard";
pub const FILE_DIALOG_SERVICE: &[u8] = b"woven.file-dialog";
pub const SCREEN_READER_SERVICE: &[u8] = b"woven.screen-reader";
pub const STATUS_OK: u16 = 0;
pub const STATUS_BAD_REQUEST: u16 = 1;
pub const STATUS_TOO_LARGE: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Transport,
    TooLarge,
    InvalidResponse,
    ServiceRejected,
}

/// The minimum transport an application needs.  Implementations must not
/// retain the supplied buffer after `send` returns.
pub trait Transport {
    fn discover(&mut self, name: &[u8]) -> Result<u64, Error>;
    fn send(&mut self, endpoint: u64, message: &[u8]) -> Result<(), Error>;
    fn receive(&mut self, endpoint: u64, output: &mut [u8]) -> Result<usize, Error>;
}

/// The production userspace transport.  The syscall layer validates every
/// pointer and applies the discovered handle's rights; this adapter owns no
/// endpoint state beyond the process-local handle returned by discovery.
#[derive(Clone, Copy, Debug, Default)]
pub struct SyscallTransport;

impl Transport for SyscallTransport {
    fn discover(&mut self, name: &[u8]) -> Result<u64, Error> {
        let handle = unsafe { crate::syscall::ipc_service_discover(name.as_ptr(), name.len()) };
        if handle == crate::syscall::ERROR {
            Err(Error::Transport)
        } else {
            Ok(handle)
        }
    }

    fn send(&mut self, endpoint: u64, message: &[u8]) -> Result<(), Error> {
        let result =
            unsafe { crate::syscall::ipc_message_send(endpoint, message.as_ptr(), message.len()) };
        if result == crate::syscall::ERROR {
            Err(Error::Transport)
        } else {
            Ok(())
        }
    }

    fn receive(&mut self, endpoint: u64, output: &mut [u8]) -> Result<usize, Error> {
        let result = unsafe {
            crate::syscall::ipc_message_receive(endpoint, output.as_mut_ptr(), output.len())
        };
        if result == crate::syscall::ERROR || result == crate::syscall::WOULD_BLOCK {
            Err(Error::Transport)
        } else {
            usize::try_from(result).map_err(|_| Error::InvalidResponse)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipboardReply {
    pub status: u16,
    pub generation: u64,
    pub text_len: usize,
}

pub struct ClipboardClient<T> {
    transport: T,
    endpoint: u64,
    request_id: u64,
}

impl<T: Transport> ClipboardClient<T> {
    pub fn connect(mut transport: T) -> Result<Self, Error> {
        let endpoint = transport.discover(CLIPBOARD_SERVICE)?;
        Ok(Self {
            transport,
            endpoint,
            request_id: 1,
        })
    }

    pub fn set(&mut self, text: &str) -> Result<ClipboardReply, Error> {
        if text.is_empty() || text.len() > crate::clipboard::MAX_TEXT_BYTES {
            return Err(Error::TooLarge);
        }
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let length = encode_clipboard_request(1, self.next_id(), text.as_bytes(), &mut request)?;
        self.transact(&request[..length])
    }

    pub fn get(&mut self, output: &mut [u8]) -> Result<ClipboardReply, Error> {
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let length = encode_clipboard_request(2, self.next_id(), &[], &mut request)?;
        self.transact_into(&request[..length], output)
    }

    pub fn clear(&mut self) -> Result<ClipboardReply, Error> {
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let length = encode_clipboard_request(3, self.next_id(), &[], &mut request)?;
        self.transact(&request[..length])
    }

    fn next_id(&mut self) -> u64 {
        let id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1).max(1);
        id
    }

    fn transact(&mut self, request: &[u8]) -> Result<ClipboardReply, Error> {
        let mut output = [0u8; MAX_MESSAGE_BYTES];
        self.transact_into(request, &mut output)
    }

    fn transact_into(
        &mut self,
        request: &[u8],
        output: &mut [u8],
    ) -> Result<ClipboardReply, Error> {
        self.transport.send(self.endpoint, request)?;
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        let length = self.transport.receive(self.endpoint, &mut response)?;
        let reply = decode_clipboard_response(&response[..length], None)?;
        if output.len() < reply.text_len {
            return Err(Error::TooLarge);
        }
        output[..reply.text_len].copy_from_slice(&response[28..28 + reply.text_len]);
        Ok(reply)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileDialogReply {
    pub status: u16,
    pub entry_id: u64,
    pub directory: bool,
    pub name_len: usize,
}

pub struct FileDialogClient<T> {
    transport: T,
    endpoint: u64,
    request_id: u64,
}

impl<T: Transport> FileDialogClient<T> {
    pub fn connect(mut transport: T) -> Result<Self, Error> {
        let endpoint = transport.discover(FILE_DIALOG_SERVICE)?;
        Ok(Self {
            transport,
            endpoint,
            request_id: 1,
        })
    }

    pub fn next(&mut self, name: &mut [u8]) -> Result<FileDialogReply, Error> {
        self.command(1, name)
    }

    pub fn previous(&mut self, name: &mut [u8]) -> Result<FileDialogReply, Error> {
        self.command(2, name)
    }

    pub fn cancel(&mut self) -> Result<FileDialogReply, Error> {
        self.command(3, &mut [])
    }

    fn command(&mut self, operation: u8, name: &mut [u8]) -> Result<FileDialogReply, Error> {
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let length = encode_simple_request(*b"WFD1", operation, self.next_id(), &mut request)?;
        self.transport.send(self.endpoint, &request[..length])?;
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        let used = self.transport.receive(self.endpoint, &mut response)?;
        decode_file_dialog_response(&response[..used], name)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1).max(1);
        id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenReaderReply {
    pub status: u16,
    pub generation: u64,
}

pub struct ScreenReaderClient<T> {
    transport: T,
    endpoint: u64,
    request_id: u64,
}

/// Service-owned state and dispatch for the three desktop brokers.  The
/// process hosting this value owns the buffers and calls `dispatch` after an
/// IPC receive; no request slice is retained after the call returns.
pub struct DesktopServiceBroker {
    clipboard: crate::clipboard::TextClipboard,
    file_dialog: crate::file_dialog::FileDialog,
    snapshot: [u8; crate::accessibility::MAX_SNAPSHOT_BYTES],
    snapshot_len: usize,
    generation: u64,
}

impl DesktopServiceBroker {
    pub const fn new() -> Self {
        Self {
            clipboard: crate::clipboard::TextClipboard::new(),
            file_dialog: crate::file_dialog::FileDialog::new(),
            snapshot: [0; crate::accessibility::MAX_SNAPSHOT_BYTES],
            snapshot_len: 0,
            generation: 0,
        }
    }

    pub fn add_file_entry(
        &mut self,
        entry: crate::file_dialog::Entry,
    ) -> Result<(), crate::file_dialog::Error> {
        self.file_dialog.push(entry)
    }

    pub fn dispatch(&mut self, request: &[u8], response: &mut [u8]) -> Result<usize, Error> {
        if request.len() < 16 {
            return Err(Error::InvalidResponse);
        }
        match &request[..4] {
            b"WCB1" => self.dispatch_clipboard(request, response),
            b"WFD1" => self.dispatch_file_dialog(request, response),
            b"WSR1" => self.dispatch_screen_reader(request, response),
            _ => Err(Error::InvalidResponse),
        }
    }

    fn dispatch_clipboard(&mut self, request: &[u8], response: &mut [u8]) -> Result<usize, Error> {
        let (operation, id, payload) = decode_request(request, *b"WCB1")?;
        let status = match operation {
            1 => match core::str::from_utf8(payload)
                .ok()
                .and_then(|text| self.clipboard.set(text).ok())
            {
                Some(()) => STATUS_OK,
                None => STATUS_BAD_REQUEST,
            },
            2 => STATUS_OK,
            3 => {
                self.clipboard.clear();
                STATUS_OK
            }
            _ => STATUS_BAD_REQUEST,
        };
        let body = if operation == 2 {
            self.clipboard.text().as_bytes()
        } else {
            &[]
        };
        encode_response(
            *b"WCB1",
            id,
            status,
            self.clipboard.generation(),
            body,
            response,
        )
    }

    fn dispatch_file_dialog(
        &mut self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Error> {
        let (operation, id, payload) = decode_request(request, *b"WFD1")?;
        if !payload.is_empty() {
            return encode_file_response(id, STATUS_BAD_REQUEST, None, response);
        }
        let selected = match operation {
            1 => self.file_dialog.select_next(),
            2 => self.file_dialog.select_previous(),
            3 => {
                self.file_dialog.cancel();
                None
            }
            _ => return encode_file_response(id, STATUS_BAD_REQUEST, None, response),
        };
        encode_file_response(id, STATUS_OK, selected, response)
    }

    fn dispatch_screen_reader(
        &mut self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Error> {
        let (operation, id, payload) = decode_request(request, *b"WSR1")?;
        let valid = match operation {
            1 => {
                payload.len() >= 4
                    && payload[..4] == *b"WAX1"
                    && payload.len() <= self.snapshot.len()
            }
            2 => {
                payload.len() == crate::accessibility::ANNOUNCEMENT_BYTES
                    && payload[..4] == *b"WAA1"
            }
            _ => false,
        };
        if !valid {
            return encode_response(
                *b"WSR1",
                id,
                STATUS_BAD_REQUEST,
                self.generation,
                &[],
                response,
            );
        }
        if operation == 1 {
            self.snapshot[..payload.len()].copy_from_slice(payload);
            self.snapshot_len = payload.len();
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        encode_response(*b"WSR1", id, STATUS_OK, self.generation, &[], response)
    }
}

impl Default for DesktopServiceBroker {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_request(request: &[u8], magic: [u8; 4]) -> Result<(u8, u64, &[u8]), Error> {
    if request.len() < 16 || request[..4] != magic || request[4] != 1 {
        return Err(Error::InvalidResponse);
    }
    let length = u16::from_le_bytes([request[6], request[7]]) as usize;
    if 16usize.checked_add(length) != Some(request.len()) || request.len() > MAX_MESSAGE_BYTES {
        return Err(Error::InvalidResponse);
    }
    Ok((
        request[5],
        u64::from_le_bytes(
            request[8..16]
                .try_into()
                .map_err(|_| Error::InvalidResponse)?,
        ),
        &request[16..],
    ))
}

fn encode_response(
    magic: [u8; 4],
    id: u64,
    status: u16,
    generation: u64,
    body: &[u8],
    output: &mut [u8],
) -> Result<usize, Error> {
    let total = 28usize.checked_add(body.len()).ok_or(Error::TooLarge)?;
    if total > output.len() || total > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge);
    }
    output[..4].copy_from_slice(&magic);
    output[4] = 1;
    output[5] = 0;
    output[6..8].copy_from_slice(&(body.len() as u16).to_le_bytes());
    output[8..16].copy_from_slice(&id.to_le_bytes());
    output[16..18].copy_from_slice(&status.to_le_bytes());
    output[18..26].copy_from_slice(&generation.to_le_bytes());
    output[26..28].fill(0);
    output[28..total].copy_from_slice(body);
    Ok(total)
}

fn encode_file_response(
    id: u64,
    status: u16,
    entry: Option<crate::file_dialog::Entry>,
    output: &mut [u8],
) -> Result<usize, Error> {
    let mut name_storage = [0u8; crate::file_dialog::MAX_NAME_BYTES];
    let (entry_id, directory, name_len) = match entry {
        Some(entry) => {
            let bytes = entry.name().as_bytes();
            name_storage[..bytes.len()].copy_from_slice(bytes);
            (entry.id, entry.directory, bytes.len())
        }
        None => (0, false, 0),
    };
    let name_bytes = &name_storage[..name_len];
    if name_bytes.len() > 96 {
        return Err(Error::TooLarge);
    }
    let total = 30 + name_bytes.len();
    if output.len() < total || total > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge);
    }
    output[..4].copy_from_slice(b"WFD1");
    output[4] = 1;
    output[5] = 0;
    output[6..8].copy_from_slice(&((2 + name_bytes.len()) as u16).to_le_bytes());
    output[8..16].copy_from_slice(&id.to_le_bytes());
    output[16..18].copy_from_slice(&status.to_le_bytes());
    output[18..26].copy_from_slice(&entry_id.to_le_bytes());
    output[26..28].fill(0);
    output[28] = name_bytes.len() as u8;
    output[29] = u8::from(directory);
    output[30..total].copy_from_slice(name_bytes);
    Ok(total)
}

impl<T: Transport> ScreenReaderClient<T> {
    pub fn connect(mut transport: T) -> Result<Self, Error> {
        let endpoint = transport.discover(SCREEN_READER_SERVICE)?;
        Ok(Self {
            transport,
            endpoint,
            request_id: 1,
        })
    }

    pub fn publish_tree(&mut self, snapshot: &[u8]) -> Result<ScreenReaderReply, Error> {
        self.transact(1, snapshot)
    }

    pub fn announce_focus(&mut self, announcement: &[u8]) -> Result<ScreenReaderReply, Error> {
        self.transact(2, announcement)
    }

    fn transact(&mut self, operation: u8, payload: &[u8]) -> Result<ScreenReaderReply, Error> {
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let length =
            encode_payload_request(*b"WSR1", operation, self.next_id(), payload, &mut request)?;
        self.transport.send(self.endpoint, &request[..length])?;
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        let used = self.transport.receive(self.endpoint, &mut response)?;
        decode_screen_reader_response(&response[..used])
    }

    fn next_id(&mut self) -> u64 {
        let id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1).max(1);
        id
    }
}

fn encode_simple_request(
    magic: [u8; 4],
    operation: u8,
    id: u64,
    output: &mut [u8],
) -> Result<usize, Error> {
    encode_payload_request(magic, operation, id, &[], output)
}

fn encode_payload_request(
    magic: [u8; 4],
    operation: u8,
    id: u64,
    payload: &[u8],
    output: &mut [u8],
) -> Result<usize, Error> {
    let total = 16usize.checked_add(payload.len()).ok_or(Error::TooLarge)?;
    if total > output.len() || total > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge);
    }
    output[..4].copy_from_slice(&magic);
    output[4] = 1;
    output[5] = operation;
    output[6..8].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    output[8..16].copy_from_slice(&id.to_le_bytes());
    output[16..total].copy_from_slice(payload);
    Ok(total)
}

fn encode_clipboard_request(
    operation: u8,
    id: u64,
    payload: &[u8],
    output: &mut [u8],
) -> Result<usize, Error> {
    if !payload.is_empty() && core::str::from_utf8(payload).is_err() {
        return Err(Error::InvalidResponse);
    }
    encode_payload_request(*b"WCB1", operation, id, payload, output)
}

fn decode_clipboard_response(
    bytes: &[u8],
    mut text: Option<&mut [u8]>,
) -> Result<ClipboardReply, Error> {
    if bytes.len() < 28 || bytes[..4] != *b"WCB1" || bytes[4] != 1 {
        return Err(Error::InvalidResponse);
    }
    let payload_len = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
    if 28usize.checked_add(payload_len) != Some(bytes.len()) {
        return Err(Error::InvalidResponse);
    }
    let status = u16::from_le_bytes([bytes[16], bytes[17]]);
    let generation = u64::from_le_bytes(
        bytes[18..26]
            .try_into()
            .map_err(|_| Error::InvalidResponse)?,
    );
    if let Some(output) = text.as_mut() {
        if output.len() < payload_len {
            return Err(Error::TooLarge);
        }
        output[..payload_len].copy_from_slice(&bytes[28..]);
    }
    Ok(ClipboardReply {
        status,
        generation,
        text_len: payload_len,
    })
}

fn decode_file_dialog_response(bytes: &[u8], name: &mut [u8]) -> Result<FileDialogReply, Error> {
    if bytes.len() < 28 || bytes[..4] != *b"WFD1" || bytes[4] != 1 {
        return Err(Error::InvalidResponse);
    }
    let payload_len = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
    if 28usize.checked_add(payload_len) != Some(bytes.len()) || payload_len < 10 {
        return Err(Error::InvalidResponse);
    }
    let status = u16::from_le_bytes([bytes[16], bytes[17]]);
    let id = u64::from_le_bytes(
        bytes[18..26]
            .try_into()
            .map_err(|_| Error::InvalidResponse)?,
    );
    let name_len = bytes[28] as usize;
    if payload_len != 2 + name_len || name_len > name.len() || bytes[29] > 1 {
        return Err(Error::InvalidResponse);
    }
    name[..name_len].copy_from_slice(&bytes[30..30 + name_len]);
    Ok(FileDialogReply {
        status,
        entry_id: id,
        directory: bytes[29] != 0,
        name_len,
    })
}

fn decode_screen_reader_response(bytes: &[u8]) -> Result<ScreenReaderReply, Error> {
    if bytes.len() != 28
        || bytes[..4] != *b"WSR1"
        || bytes[4] != 1
        || bytes[6] != 0
        || bytes[7] != 0
        || bytes[26] != 0
        || bytes[27] != 0
    {
        return Err(Error::InvalidResponse);
    }
    Ok(ScreenReaderReply {
        status: u16::from_le_bytes([bytes[16], bytes[17]]),
        generation: u64::from_le_bytes(
            bytes[18..26]
                .try_into()
                .map_err(|_| Error::InvalidResponse)?,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Loopback {
        sent: [u8; MAX_MESSAGE_BYTES],
        len: usize,
        response: [u8; MAX_MESSAGE_BYTES],
        response_len: usize,
    }
    impl Transport for Loopback {
        fn discover(&mut self, _: &[u8]) -> Result<u64, Error> {
            Ok(7)
        }
        fn send(&mut self, _: u64, message: &[u8]) -> Result<(), Error> {
            self.sent[..message.len()].copy_from_slice(message);
            self.len = message.len();
            Ok(())
        }
        fn receive(&mut self, _: u64, output: &mut [u8]) -> Result<usize, Error> {
            output[..self.response_len].copy_from_slice(&self.response[..self.response_len]);
            Ok(self.response_len)
        }
    }

    #[test]
    fn clipboard_client_emits_versioned_utf8_request() {
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        response[..4].copy_from_slice(b"WCB1");
        response[4] = 1;
        response[6..8].copy_from_slice(&3u16.to_le_bytes());
        response[16..18].copy_from_slice(&0u16.to_le_bytes());
        response[18..26].copy_from_slice(&9u64.to_le_bytes());
        response[28..31].copy_from_slice(b"hey");
        let mut client = ClipboardClient::connect(Loopback {
            sent: [0; MAX_MESSAGE_BYTES],
            len: 0,
            response,
            response_len: 31,
        })
        .unwrap();
        let reply = client.set("hé").unwrap();
        assert_eq!(reply.generation, 9);
        assert_eq!(client.transport.sent[0..4], *b"WCB1");
        assert_eq!(
            core::str::from_utf8(&client.transport.sent[16..19]).unwrap(),
            "hé"
        );
    }

    #[test]
    fn clients_reject_oversized_payloads_before_transport() {
        let mut client = ClipboardClient::connect(Loopback {
            sent: [0; MAX_MESSAGE_BYTES],
            len: 0,
            response: [0; MAX_MESSAGE_BYTES],
            response_len: 0,
        })
        .unwrap();
        let too_long = "x".repeat(crate::clipboard::MAX_TEXT_BYTES + 1);
        assert_eq!(client.set(&too_long), Err(Error::TooLarge));
    }

    #[test]
    fn broker_owns_clipboard_and_returns_generation() {
        let mut broker = DesktopServiceBroker::new();
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let request_len = encode_clipboard_request(1, 41, b"hello", &mut request).unwrap();
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        let response_len = broker
            .dispatch(&request[..request_len], &mut response)
            .unwrap();
        let reply = decode_clipboard_response(&response[..response_len], None).unwrap();
        assert_eq!(reply.status, STATUS_OK);
        assert_eq!(reply.generation, 1);
        assert_eq!(reply.text_len, 0);
    }

    #[test]
    fn broker_rejects_untrusted_screen_reader_payloads() {
        let mut broker = DesktopServiceBroker::new();
        let mut request = [0u8; MAX_MESSAGE_BYTES];
        let request_len =
            encode_payload_request(*b"WSR1", 1, 2, b"not-a-tree", &mut request).unwrap();
        let mut response = [0u8; MAX_MESSAGE_BYTES];
        let response_len = broker
            .dispatch(&request[..request_len], &mut response)
            .unwrap();
        let reply = decode_screen_reader_response(&response[..response_len]).unwrap();
        assert_eq!(reply.status, STATUS_BAD_REQUEST);
        assert_eq!(reply.generation, 0);
    }
}
