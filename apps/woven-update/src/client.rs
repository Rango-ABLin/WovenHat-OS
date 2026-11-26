//! Client for the authenticated `woven.update` broker.
//!
//! This app holds no `StorageIo`: it sends bounded control metadata to the
//! privileged service, which owns signature verification and the durable A/B
//! transaction.

use libwoven::update_ipc::{self, Operation, Request, Response};

const SERVICE_NAME: &[u8] = b"woven.update";
const REQUEST_CAPACITY: usize = 256;
const WAIT_ATTEMPTS: usize = 64;

pub const STATUS_OK: u16 = 0;
pub const STATUS_BAD_REQUEST: u16 = 1;
pub const STATUS_TRANSACTION_FAILED: u16 = 2;
pub const STATUS_UNAVAILABLE: u16 = 3;

pub fn call(operation: Operation, source: &str, slot: u8, request_id: u64) -> Result<u64, u16> {
    let generation = if operation == Operation::Stage { 1 } else { 0 };
    let request = Request {
        operation,
        request_id,
        generation,
        slot,
        source: source.as_bytes(),
    };
    if request.encode(&mut [0u8; REQUEST_CAPACITY]).is_err() {
        return Err(STATUS_BAD_REQUEST);
    }
    let service =
        libwoven::ipc_service_discover(SERVICE_NAME).map_err(|_| STATUS_UNAVAILABLE)?;
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
        STATUS_TRANSACTION_FAILED => "transaction failed",
        _ => "service unavailable",
    }
}

pub const fn no_slot() -> u8 {
    update_ipc::NO_SLOT
}
