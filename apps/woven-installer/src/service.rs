//! Privileged installer authority boundary.
//!
//! This daemon is deliberately separate from the graphical process. It can
//! inventory the selected target and validate a bounded plan; confirmed writes
//! are delegated to the capability-gated kernel transaction.

use libwoven::installer_ipc::{self, Operation, Request, Response};
use libwoven::process;

const CLIENT_SEND: u8 = 1;
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
        return Response { request_id: 0, status: installer_ipc::STATUS_BAD_REQUEST, progress: 0, value: 0 };
    };
    let _plan = request.plan;
    let (status, progress, value) = match request.operation {
        Operation::Inventory => match libwoven::installer_inventory() {
            Ok(sectors) => (installer_ipc::STATUS_READY, 5, sectors),
            Err(_) => (installer_ipc::STATUS_TARGET_UNAVAILABLE, 0, 0),
        },
        Operation::Validate | Operation::Install => match libwoven::installer_inventory() {
            Ok(sectors) => match installer_ipc::validate_plan(request.plan, sectors, request.operation == Operation::Install) {
                Ok(()) if request.operation == Operation::Validate => (installer_ipc::STATUS_READY, 10, 0),
                Ok(()) => match libwoven::installer_install(request.plan) {
                    Ok(progress) => (installer_ipc::STATUS_READY, progress as u8, 0),
                    Err(_) => (installer_ipc::STATUS_INSTALL_FAILED, 10, 0),
                },
                Err(_) => (installer_ipc::STATUS_INVALID_PLAN, 0, 0),
            },
            Err(_) => (installer_ipc::STATUS_TARGET_UNAVAILABLE, 0, 0),
        },
        Operation::Cancel => (installer_ipc::STATUS_NOT_READY, 0, 0),
    };
    Response { request_id: request.request_id, status, progress, value }
}
