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
        Operation::Query => (STATUS_OK, request.path.len() as u64),
        Operation::Install | Operation::Remove | Operation::Rollback => (STATUS_STORAGE, 0),
    };
    Response { request_id: request.request_id, status, value }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(103)
}
