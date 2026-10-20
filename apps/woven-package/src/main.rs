#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::package_ipc::{Operation, Request, Response};
use libwoven::{process, Args};

libwoven::entry!(main);

const SERVICE_NAME: &[u8] = b"woven.package";
const CLIENT_SEND: u8 = 1;
const STATUS_OK: u16 = 0;
const STATUS_BAD_REQUEST: u16 = 1;
const STATUS_NOT_FOUND: u16 = 2;
const STATUS_STORAGE: u16 = 3;
const REQUEST_CAPACITY: usize = 256;

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0u8; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; Response::ENCODED_LEN] = [0u8; Response::ENCODED_LEN];

fn main(_: Args) -> ! {
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(101),
    };
    if libwoven::ipc_service_publish(SERVICE_NAME, CLIENT_SEND, endpoint).is_err() {
        libwoven::exit(102);
    }
    loop {
        // SAFETY: this is the single-threaded service-owned request/response
        // storage; no pointer escapes either syscall call.
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut REQUEST_BYTES as *mut u8,
                REQUEST_CAPACITY,
            )
        };
        let response_bytes = unsafe {
            core::slice::from_raw_parts_mut(&raw mut RESPONSE_BYTES as *mut u8, Response::ENCODED_LEN)
        };
        let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) else {
            libwoven::yield_now();
            continue;
        };
        let response = dispatch(&request_bytes[..length]);
        let Ok(response_length) = response.encode(response_bytes) else {
            libwoven::yield_now();
            continue;
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..response_length]).is_err() {
            libwoven::exit(105);
        }
    }
}

fn dispatch(bytes: &[u8]) -> Response {
    let Ok(request) = Request::decode(bytes) else {
        return Response { request_id: 0, status: STATUS_BAD_REQUEST, value: 0 };
    };
    let (status, value) = match request.operation {
        Operation::Query => query_storage(request.path),
        operation @ (Operation::Install | Operation::Remove | Operation::Rollback) => {
            match libwoven::package_transaction(operation, core::str::from_utf8(request.path).unwrap_or("")) {
                Ok(value) => (STATUS_OK, value),
                Err(_) => (STATUS_STORAGE, 0),
            }
        }
    };
    Response { request_id: request.request_id, status, value }
}

fn query_storage(path: &[u8]) -> (u16, u64) {
    let Some(separator) = path.iter().rposition(|byte| *byte == b'/') else {
        return (STATUS_BAD_REQUEST, 0);
    };
    let parent = if separator == 0 { b"/" } else { &path[..separator] };
    let Ok(parent) = core::str::from_utf8(parent) else {
        return (STATUS_NOT_FOUND, 0);
    };
    let leaf = &path[separator + 1..];
    if leaf.is_empty() {
        return (STATUS_BAD_REQUEST, 0);
    }
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..128 {
        let Ok((length, directory)) = libwoven::read_dir(parent, index, &mut name) else {
            break;
        };
        if !directory && &name[..length] == leaf {
            return (STATUS_OK, 1);
        }
    }
    (STATUS_NOT_FOUND, 0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(103)
}
