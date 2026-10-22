//! Privileged installer authority boundary.
//!
//! This daemon is deliberately separate from the graphical process. Until the
//! kernel-backed physical target transaction is integrated, every operation
//! returns a deterministic not-ready response and performs no device I/O.

use libwoven::installer_ipc::{self, Request, Response};
use libwoven::process;

const CLIENT_SEND: u8 = 1;
const STATUS_BAD_REQUEST: u16 = 1;
const STATUS_NOT_READY: u16 = 2;
const REQUEST_CAPACITY: usize = installer_ipc::MAX_MESSAGE_BYTES;

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; Response::ENCODED_LEN] = [0; Response::ENCODED_LEN];

pub fn run() -> ! {
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(101),
    };
    if libwoven::ipc_service_publish(installer_ipc::SERVICE_NAME, CLIENT_SEND, endpoint).is_err() {
        libwoven::exit(102);
    }
    loop {
        // SAFETY: the service is single-threaded and these buffers never
        // escape the individual receive/send syscalls.
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(&raw mut REQUEST_BYTES as *mut u8, REQUEST_CAPACITY)
        };
        let response_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut RESPONSE_BYTES as *mut u8,
                Response::ENCODED_LEN,
            )
        };
        let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) else {
            libwoven::yield_now();
            continue;
        };
        let response = dispatch(&request_bytes[..length]);
        let Ok(response_length) = response.encode(response_bytes) else {
            libwoven::exit(103);
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..response_length]).is_err() {
            libwoven::exit(104);
        }
    }
}

fn dispatch(bytes: &[u8]) -> Response {
    let Ok(request) = Request::decode(bytes) else {
        return Response { request_id: 0, status: STATUS_BAD_REQUEST, progress: 0, value: 0 };
    };
    // The service boundary is live, but no physical operation is reachable
    // yet. Keeping this response explicit prevents the UI from treating
    // publication as proof that a disk was inspected or modified.
    let _operation = request.operation;
    let _plan = request.plan;
    Response {
        request_id: request.request_id,
        status: STATUS_NOT_READY,
        progress: 0,
        value: 0,
    }
}
