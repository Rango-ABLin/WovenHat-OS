//! Client for the authenticated `woven.package` broker.
//!
//! This app holds no `StorageIo`: it sends control metadata and a bounded
//! path to the privileged service, which owns the durable transaction.

use libwoven::package_ipc::{Operation, Request, Response};

const SERVICE_NAME: &[u8] = b"woven.package";
const REQUEST_CAPACITY: usize = 256;
const WAIT_ATTEMPTS: usize = 64;

pub const STATUS_OK: u16 = 0;
pub const STATUS_BAD_REQUEST: u16 = 1;
pub const STATUS_NOT_FOUND: u16 = 2;
pub const STATUS_STORAGE: u16 = 3;
pub const STATUS_UNAVAILABLE: u16 = 4;

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

    let mut response = [0u8; Response::ENCODED_LEN];
    for _ in 0..WAIT_ATTEMPTS {
        if let Ok((length, _sender)) = libwoven::message_receive(&mut response) {
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
        libwoven::yield_now();
    }
    Err(STATUS_UNAVAILABLE)
}

pub fn status_text(status: u16) -> &'static str {
    match status {
        STATUS_OK => "ok",
        STATUS_BAD_REQUEST => "bad request",
        STATUS_NOT_FOUND => "not found",
        STATUS_STORAGE => "storage failed",
        _ => "service unavailable",
    }
}
