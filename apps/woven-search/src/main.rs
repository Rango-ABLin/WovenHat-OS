#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::search_ipc::{self, Operation, Request, Response};
use libwoven::semantic_search::{Entry, Index, Kind};
use libwoven::{process, Args};

libwoven::entry!(main);

const CLIENT_SEND: u8 = 1;
const STATUS_OK: u16 = 0;
const STATUS_BAD_REQUEST: u16 = 1;
const REQUEST_CAPACITY: usize = search_ipc::MAX_MESSAGE_BYTES;

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; search_ipc::RESPONSE_LEN] = [0; search_ipc::RESPONSE_LEN];

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--probe") {
        probe()
    }
    broker()
}

fn broker() -> ! {
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(101),
    };
    if libwoven::ipc_service_publish(search_ipc::SERVICE_NAME, CLIENT_SEND, endpoint).is_err() {
        libwoven::exit(102);
    }
    let Some(index) = catalog_index() else {
        libwoven::exit(103);
    };
    loop {
        // SAFETY: The freestanding broker is single-threaded. Each buffer is
        // borrowed only for the receive/decode/response sequence below.
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(&raw mut REQUEST_BYTES as *mut u8, REQUEST_CAPACITY)
        };
        let response_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut RESPONSE_BYTES as *mut u8,
                search_ipc::RESPONSE_LEN,
            )
        };
        let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) else {
            libwoven::yield_now();
            continue;
        };
        let response = dispatch(&index, sender.0, &request_bytes[..length]);
        let Ok(response_length) = response.encode(response_bytes) else {
            libwoven::exit(104);
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..response_length])
            .is_err()
        {
            // The sender has gone away. No search action has side effects, so
            // discard this response and keep serving other clients.
            continue;
        }
    }
}

fn dispatch(index: &Index, owner: u64, bytes: &[u8]) -> Response {
    let Ok(request) = Request::decode(bytes) else {
        return Response::empty(0, STATUS_BAD_REQUEST);
    };
    match request.operation {
        Operation::Describe => Response::empty(request.request_id, STATUS_OK),
        Operation::Query => {
            let mut response = Response::empty(request.request_id, STATUS_OK);
            response.hit_count = index.search_for(owner, request.query, &mut response.hits) as u8;
            response
        }
    }
}

fn catalog_index() -> Option<Index> {
    const APPS: &[(u64, &[u8])] = &[
        (1, b"Woven Calculator"),
        (2, b"Woven Terminal"),
        (3, b"Woven Files"),
        (4, b"Woven Monitor"),
        (5, b"Woven Settings"),
        (6, b"Woven Security"),
        (7, b"Woven Editor"),
        (8, b"Woven Images"),
        (9, b"Woven Update"),
        (10, b"Woven Store"),
        (11, b"Woven Installer"),
    ];
    let mut index = Index::new();
    for &(id, title) in APPS {
        if !index.insert(Entry::new(id, Kind::Application, title)?) {
            return None;
        }
    }
    Some(index)
}

fn probe() -> ! {
    let Ok(endpoint) = libwoven::ipc_service_discover(search_ipc::SERVICE_NAME) else {
        libwoven::exit(110);
    };
    let request = Request {
        operation: Operation::Query,
        request_id: 0x23_05_01,
        query: b"woven terminal",
    };
    let mut request_bytes = [0u8; REQUEST_CAPACITY];
    let Ok(length) = request.encode(&mut request_bytes) else {
        libwoven::exit(111);
    };
    if libwoven::ipc_message_send(endpoint, &request_bytes[..length]).is_err() {
        libwoven::exit(112);
    }
    let mut response_bytes = [0u8; search_ipc::RESPONSE_LEN];
    loop {
        match libwoven::message_receive(&mut response_bytes) {
            Ok((length, _sender)) => {
                let Ok(response) = Response::decode(&response_bytes[..length]) else {
                    libwoven::exit(113);
                };
                if response.request_id == request.request_id
                    && response.status == STATUS_OK
                    && response.hit_count == 1
                    && response.hits[0].id == 2
                    && response.hits[0].kind == Kind::Application
                {
                    libwoven::exit(0);
                }
                libwoven::exit(114);
            }
            Err(_) => libwoven::yield_now(),
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    libwoven::exit(127)
}
