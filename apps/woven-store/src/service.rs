//! Client for the authenticated `woven.package` broker.
//!
//! This app holds no `StorageIo`: it sends control metadata and a bounded
//! path to the privileged service, which owns the durable transaction.

use libwoven::package_ipc::{Operation, Request, Response};

const SERVICE_NAME: &[u8] = b"woven.package";
const REQUEST_CAPACITY: usize = 256;
const REPLY_TIMEOUT_TICKS: u64 = 3_000; // 30 seconds at 100 Hz.

pub const STATUS_OK: u16 = 0;
pub const STATUS_BAD_REQUEST: u16 = 1;
pub const STATUS_NOT_FOUND: u16 = 2;
pub const STATUS_STORAGE: u16 = 3;
pub const STATUS_UNAVAILABLE: u16 = 4;
const STATUS_REPLY_MISSING: u16 = 5;

/// Call the broker and return its value, or its bounded status code.
pub fn call(operation: Operation, path: &str, request_id: u64) -> Result<u64, u16> {
    if !wovenhat_store::valid_package_path(path) {
        return Err(STATUS_BAD_REQUEST);
    }
    let service =
        libwoven::ipc_service_discover(SERVICE_NAME).map_err(|_| STATUS_UNAVAILABLE)?;
    let request = Request {
        operation,
        request_id,
        path: path.as_bytes(),
    };
    let mut bytes = [0u8; REQUEST_CAPACITY];
    let length = request.encode(&mut bytes).map_err(|_| STATUS_BAD_REQUEST)?;
    libwoven::ipc_message_send(service, &bytes[..length]).map_err(|_| STATUS_UNAVAILABLE)?;

    // The GUI mailbox can also contain input messages. Receive at the full
    // transport capacity so an unrelated message cannot block the reply.
    let mut response = [0u8; REQUEST_CAPACITY];
    let deadline = libwoven::ticks().saturating_add(REPLY_TIMEOUT_TICKS);
    while libwoven::ticks() < deadline {
        match libwoven::wait_any(libwoven::wait::MESSAGE, Some(deadline)) {
            Ok(ready) if ready & libwoven::wait::MESSAGE != 0 => {}
            _ => break,
        }
        let Ok((length, _sender)) = libwoven::message_receive(&mut response) else {
            break;
        };
        if let Ok(reply) = Response::decode(&response[..length]) {
            if reply.request_id == request_id {
                return if reply.status == STATUS_OK {
                    Ok(reply.value)
                } else {
                    Err(reply.status)
                };
            }
        }
    }
    Err(STATUS_REPLY_MISSING)
}

pub fn status_text(status: u16) -> &'static str {
    match status {
        STATUS_OK => "ok",
        STATUS_BAD_REQUEST => "bad request",
        STATUS_NOT_FOUND => "not found",
        STATUS_STORAGE => "storage failed",
        STATUS_REPLY_MISSING => "reply missing; transaction outcome unknown",
        _ => "service unavailable",
    }
}
