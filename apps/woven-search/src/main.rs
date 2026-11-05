#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::desktop_services::{
    CLIPBOARD_SERVICE, ClipboardClient, DesktopServiceBroker, FILE_DIALOG_SERVICE,
    FileDialogClient, MAX_MESSAGE_BYTES, SCREEN_READER_SERVICE, ScreenReaderClient,
    SyscallTransport,
};
use libwoven::search_ipc::{self, Operation, Request, Response};
use libwoven::semantic_search::{Entry, Index, Kind};
use libwoven::{Args, process};

libwoven::entry!(main);

const CLIENT_SEND: u8 = 1;
const STATUS_OK: u16 = 0;
const STATUS_BAD_REQUEST: u16 = 1;
const REQUEST_CAPACITY: usize = search_ipc::MAX_MESSAGE_BYTES;

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; MAX_MESSAGE_BYTES] = [0; MAX_MESSAGE_BYTES];

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
    let desktop_endpoints = [
        (CLIPBOARD_SERVICE, libwoven::ipc_endpoint_create()),
        (FILE_DIALOG_SERVICE, libwoven::ipc_endpoint_create()),
        (SCREEN_READER_SERVICE, libwoven::ipc_endpoint_create()),
    ];
    for (name, endpoint) in desktop_endpoints {
        let Ok(endpoint) = endpoint else {
            libwoven::exit(104)
        };
        if libwoven::ipc_service_publish(name, CLIENT_SEND, endpoint).is_err() {
            libwoven::exit(105);
        }
    }
    let mut desktop = DesktopServiceBroker::new();
    let _ =
        desktop.add_file_entry(libwoven::file_dialog::Entry::new(1, "Documents", true).unwrap());
    let _ =
        desktop.add_file_entry(libwoven::file_dialog::Entry::new(2, "Notes.txt", false).unwrap());
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
            core::slice::from_raw_parts_mut(&raw mut RESPONSE_BYTES as *mut u8, MAX_MESSAGE_BYTES)
        };
        let mut handled = false;
        if let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) {
            handled = true;
            let response = dispatch(&index, sender.0, &request_bytes[..length]);
            let Ok(response_length) = response.encode(response_bytes) else {
                libwoven::exit(106);
            };
            let _ =
                libwoven::message_send(process::Id(sender.0), &response_bytes[..response_length]);
        }
        for (_, endpoint) in desktop_endpoints {
            let Ok(endpoint) = endpoint else { continue };
            if let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) {
                handled = true;
                let Ok(response_length) =
                    desktop.dispatch(&request_bytes[..length], response_bytes)
                else {
                    libwoven::exit(107)
                };
                let _ = libwoven::message_send(
                    process::Id(sender.0),
                    &response_bytes[..response_length],
                );
            }
        }
        if !handled {
            libwoven::yield_now();
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
        // `terminal` is unique in the bounded public catalog. A multi-term
        // query including `woven` correctly returns lower-scoring partial
        // matches too, so it is not a one-result lifecycle oracle.
        query: b"terminal",
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
                    let mut clipboard = match ClipboardClient::connect(SyscallTransport) {
                        Ok(client) => client,
                        Err(_) => libwoven::exit(115),
                    };
                    if let Err(error) = clipboard.set("desktop service") {
                        let code = match error {
                            libwoven::desktop_services::Error::Transport => 116,
                            libwoven::desktop_services::Error::SendFailed => 128,
                            libwoven::desktop_services::Error::ReceiveFailed => 129,
                            libwoven::desktop_services::Error::WouldBlock => 124,
                            libwoven::desktop_services::Error::TooLarge => 125,
                            libwoven::desktop_services::Error::InvalidResponse => 126,
                            libwoven::desktop_services::Error::ServiceRejected => 127,
                        };
                        libwoven::exit(code);
                    }
                    let mut clipboard_bytes = [0u8; 64];
                    let Ok(reply) = clipboard.get(&mut clipboard_bytes) else {
                        libwoven::exit(117);
                    };
                    if &clipboard_bytes[..reply.text_len] != b"desktop service" {
                        libwoven::exit(118);
                    }
                    let mut dialog = match FileDialogClient::connect(SyscallTransport) {
                        Ok(client) => client,
                        Err(_) => libwoven::exit(119),
                    };
                    let mut name = [0u8; 96];
                    let Ok(entry) = dialog.next(&mut name) else {
                        libwoven::exit(120);
                    };
                    if entry.entry_id != 2 || &name[..entry.name_len] != b"Notes.txt" {
                        libwoven::exit(121);
                    }
                    let mut reader = match ScreenReaderClient::connect(SyscallTransport) {
                        Ok(client) => client,
                        Err(_) => libwoven::exit(122),
                    };
                    let mut tree = [0u8; 10];
                    tree[..4].copy_from_slice(b"WAX1");
                    if reader.publish_tree(&tree).is_err() {
                        libwoven::exit(123);
                    }
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
