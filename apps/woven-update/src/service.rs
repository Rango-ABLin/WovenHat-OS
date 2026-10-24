//! Privileged WovenUpdate IPC daemon.
//!
//! The daemon owns no update policy outside the kernel transaction boundary:
//! it validates the WUDI envelope, converts the bounded path to UTF-8, and
//! forwards the operation to `libwoven::update_transaction`. Package bytes
//! never cross IPC. A malformed request receives a deterministic response and
//! cannot reach the privileged syscall.

use libwoven::process;
use libwoven::update_ipc::{self, Operation, Request, Response};

const SERVICE_NAME: &[u8] = b"woven.update";
const CLIENT_SEND: u8 = 1;
const STATUS_OK: u16 = 0;
const STATUS_BAD_REQUEST: u16 = 1;
const STATUS_TRANSACTION_FAILED: u16 = 2;
// The WUDI codec is independently bounded at 4 KiB, but the kernel IPC
// transport currently caps one message at 256 bytes. Keep the receive buffer
// within the syscall ABI so the daemon can actually receive requests; longer
// candidates remain rejected by the bounded request encoder before transport.
const REQUEST_CAPACITY: usize = 256;

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0u8; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; Response::ENCODED_LEN] = [0u8; Response::ENCODED_LEN];

pub fn run() -> u64 {
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(101),
    };
    if libwoven::ipc_service_publish(SERVICE_NAME, CLIENT_SEND, endpoint).is_err() {
        libwoven::exit(102);
    }

    loop {
        // SAFETY: this is single-threaded daemon-owned storage. The slices
        // are borrowed only for the duration of each syscall/dispatch.
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(&raw mut REQUEST_BYTES as *mut u8, REQUEST_CAPACITY)
        };
        let response_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut RESPONSE_BYTES as *mut u8,
                Response::ENCODED_LEN,
            )
        };

        // The public IPC receive wrapper is nonblocking. Yielding on an empty
        // queue hands the CPU back to the scheduler and avoids timer polling
        // or a tight busy loop while preserving the established ABI.
        let Ok((length, sender)) = libwoven::ipc_message_receive(endpoint, request_bytes) else {
            libwoven::yield_now();
            continue;
        };
        let response = dispatch(&request_bytes[..length]);
        let Ok(response_length) = response.encode(response_bytes) else {
            libwoven::exit(103);
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..response_length])
            .is_err()
        {
            // A dead client is not a reason to keep accepting privileged
            // work in an unobservable state; fail closed and let supervision
            // restart the service.
            libwoven::exit(104);
        }
    }
}

fn dispatch(bytes: &[u8]) -> Response {
    let Ok(request) = Request::decode(bytes) else {
        return Response {
            request_id: 0,
            status: STATUS_BAD_REQUEST,
            value: 0,
        };
    };

    let result = match request.operation {
        Operation::Stage => {
            let Ok(source) = core::str::from_utf8(request.source) else {
                return Response {
                    request_id: request.request_id,
                    status: STATUS_BAD_REQUEST,
                    value: 0,
                };
            };
            libwoven::update_transaction(Operation::Stage, update_ipc::NO_SLOT, source)
        }
        Operation::Activate | Operation::BootSucceeded | Operation::BootFailed => {
            libwoven::update_transaction(request.operation, request.slot, "")
        }
        Operation::Query => libwoven::update_transaction(Operation::Query, update_ipc::NO_SLOT, ""),
    };

    match result {
        Ok(value) => Response {
            request_id: request.request_id,
            status: STATUS_OK,
            value,
        },
        Err(_) => Response {
            request_id: request.request_id,
            status: STATUS_TRANSACTION_FAILED,
            value: 0,
        },
    }
}
