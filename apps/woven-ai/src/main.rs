#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::ai_ipc::{self, Operation, Request, Response, BROKER_ABI};
use libwoven::ai_model;
use libwoven::ai_provider;
use libwoven::{process, Args};

libwoven::entry!(main);

const SERVICE_NAME: &[u8] = b"woven.ai";
const STATUS_OK: u16 = 0;
const STATUS_NO_PROVIDER: u16 = 2;
const REQUEST_CAPACITY: usize = ai_ipc::MAX_MESSAGE_BYTES;
const CLOUD_TEST_CREDENTIAL: &[u8] = b"woven-cloud-credential-fixture";

const fn local_provider_descriptor() -> u64 {
    BROKER_ABI | ai_model::LOCAL_PROVIDER_FLAG | (ai_model::MODEL_ABI << 8)
}

static mut REQUEST_BYTES: [u8; REQUEST_CAPACITY] = [0u8; REQUEST_CAPACITY];
static mut RESPONSE_BYTES: [u8; Response::ENCODED_LEN] = [0u8; Response::ENCODED_LEN];

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--probe") {
        probe(false)
    } else if args.get(1) == Some(b"--probe-local") {
        probe(true)
    } else if args.get(1) == Some(b"--probe-cloud") {
        probe_cloud()
    } else if args.get(1) == Some(b"--cloud-plugin") {
        cloud_plugin()
    } else if args.get(1) == Some(b"--local") {
        broker(true, false)
    } else if args.get(1) == Some(b"--cloud") {
        broker(false, true)
    } else {
        broker(false, false)
    }
}

fn broker(local_provider: bool, cloud_provider: bool) -> ! {
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(101),
    };
    if libwoven::ipc_service_publish(SERVICE_NAME, 1, endpoint).is_err() {
        libwoven::exit(102);
    }
    loop {
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut REQUEST_BYTES as *mut u8,
                REQUEST_CAPACITY,
            )
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
        let response = match Request::decode(&request_bytes[..length]) {
            Ok(request) => match request.operation {
                Operation::Describe if cloud_provider => {
                    forward_cloud(request, &request_bytes[..length], response_bytes)
                }
                Operation::Describe => Response {
                    request_id: request.request_id,
                    status: STATUS_OK,
                    value: if local_provider {
                        local_provider_descriptor()
                    } else {
                        BROKER_ABI
                    },
                },
                Operation::Complete if cloud_provider => {
                    forward_cloud(request, &request_bytes[..length], response_bytes)
                }
                Operation::Complete if local_provider => Response {
                    request_id: request.request_id,
                    status: STATUS_OK,
                    value: ai_model::encode_result(ai_model::infer(request.input)),
                },
                Operation::Complete => Response {
                    request_id: request.request_id,
                    status: STATUS_NO_PROVIDER,
                    value: 0,
                },
            },
            Err(_) => Response {
                request_id: 0,
                status: 1,
                value: 0,
            },
        };
        let Ok(length) = response.encode(response_bytes) else {
            continue;
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..length]).is_err() {
            libwoven::exit(105);
        }
    }
}

fn forward_cloud(
    request: Request<'_>,
    request_bytes: &[u8],
    response_bytes: &mut [u8],
) -> Response {
    let Ok(endpoint) = libwoven::ipc_service_discover(ai_provider::SERVICE_NAME) else {
        return Response {
            request_id: request.request_id,
            status: ai_provider::STATUS_UNAVAILABLE,
            value: 0,
        };
    };
    if libwoven::ipc_message_send(endpoint, request_bytes).is_err() {
        return Response {
            request_id: request.request_id,
            status: ai_provider::STATUS_UNAVAILABLE,
            value: 0,
        };
    }
    let deadline = libwoven::ticks().wrapping_add(500);
    loop {
        match libwoven::message_receive(response_bytes) {
            Ok((length, _sender)) => {
                if let Ok(response) = Response::decode(&response_bytes[..length]) {
                    if response.request_id == request.request_id {
                        return response;
                    }
                }
            }
            Err(_) => libwoven::yield_now(),
        }
        if libwoven::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            return Response {
                request_id: request.request_id,
                status: ai_provider::STATUS_UNAVAILABLE,
                value: 0,
            };
        }
    }
}

fn cloud_plugin() -> ! {
    let Ok(credential) = libwoven::secret_store(CLOUD_TEST_CREDENTIAL) else {
        libwoven::exit(130);
    };
    let endpoint = match libwoven::ipc_endpoint_create() {
        Ok(handle) => handle,
        Err(_) => libwoven::exit(131),
    };
    if libwoven::ipc_service_publish(ai_provider::SERVICE_NAME, 1, endpoint).is_err() {
        libwoven::exit(132);
    }
    loop {
        let request_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut REQUEST_BYTES as *mut u8,
                REQUEST_CAPACITY,
            )
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
        let response = match Request::decode(&request_bytes[..length]) {
            Ok(request) => match request.operation {
                Operation::Describe => Response {
                    request_id: request.request_id,
                    status: ai_provider::STATUS_OK,
                    value: ai_provider::descriptor(),
                },
                Operation::Complete => {
                    let mut credential_bytes = [0u8; 64];
                    let length = match libwoven::secret_read(credential, &mut credential_bytes) {
                        Ok(length) => length,
                        Err(_) => libwoven::exit(133),
                    };
                    if credential_bytes[..length] != CLOUD_TEST_CREDENTIAL[..] {
                        Response {
                            request_id: request.request_id,
                            status: ai_provider::STATUS_NO_CREDENTIAL,
                            value: 0,
                        }
                    } else {
                        Response {
                            request_id: request.request_id,
                            status: ai_provider::STATUS_OK,
                            value: ai_provider::result(ai_model::encode_result(
                                ai_model::infer(request.input),
                            )),
                        }
                    }
                }
            },
            Err(_) => Response {
                request_id: 0,
                status: 1,
                value: 0,
            },
        };
        let Ok(length) = response.encode(response_bytes) else {
            continue;
        };
        if libwoven::message_send(process::Id(sender.0), &response_bytes[..length]).is_err() {
            libwoven::exit(134);
        }
    }
}

fn probe(local_provider: bool) -> ! {
    let Ok(endpoint) = libwoven::ipc_service_discover(SERVICE_NAME) else {
        libwoven::exit(110);
    };
    let request = Request {
        operation: Operation::Describe,
        request_id: 0x23_01_01,
        input: b"",
    };
    let mut bytes = [0u8; REQUEST_CAPACITY];
    let Ok(length) = request.encode(&mut bytes) else {
        libwoven::exit(111);
    };
    if libwoven::ai_message_send(endpoint, &bytes[..length]).is_err() {
        libwoven::exit(112);
    }
    let mut response_bytes = [0u8; Response::ENCODED_LEN];
    loop {
        match libwoven::message_receive(&mut response_bytes) {
            Ok((length, _sender)) => {
                let Ok(response) = Response::decode(&response_bytes[..length]) else {
                    libwoven::exit(113);
                };
                if response.request_id == request.request_id
                    && response.status == STATUS_OK
                    && response.value
                        == (BROKER_ABI
                            | if local_provider {
                                ai_model::LOCAL_PROVIDER_FLAG
                                    | (ai_model::MODEL_ABI << 8)
                            } else {
                                0
                            })
                {
                    break;
                }
                libwoven::exit(114);
            }
            Err(_) => libwoven::yield_now(),
        }
    }
    if !local_provider {
        let request = Request {
            operation: Operation::Complete,
            request_id: 0x23_01_02,
            input: b"bounded prompt",
        };
        let Ok(length) = request.encode(&mut bytes) else {
            libwoven::exit(115);
        };
        if libwoven::ai_message_send(endpoint, &bytes[..length]).is_err() {
            libwoven::exit(116);
        }
        loop {
            match libwoven::message_receive(&mut response_bytes) {
                Ok((length, _sender)) => {
                    let Ok(response) = Response::decode(&response_bytes[..length]) else {
                        libwoven::exit(117);
                    };
                    if response.request_id == request.request_id
                        && response.status == STATUS_NO_PROVIDER
                    {
                        libwoven::exit(0);
                    }
                    libwoven::exit(118);
                }
                Err(_) => libwoven::yield_now(),
            }
        }
    }
    let request = Request {
        operation: Operation::Complete,
        request_id: 0x23_02_02,
        input: b"Hello WovenHat",
    };
    let Ok(length) = request.encode(&mut bytes) else {
        libwoven::exit(119);
    };
    if libwoven::ai_message_send(endpoint, &bytes[..length]).is_err() {
        libwoven::exit(120);
    }
    loop {
        match libwoven::message_receive(&mut response_bytes) {
            Ok((length, _sender)) => {
                let Ok(response) = Response::decode(&response_bytes[..length]) else {
                    libwoven::exit(121);
                };
                if response.request_id == request.request_id
                    && response.status == STATUS_OK
                    && response.value == (1 | (6u64 << 32))
                {
                    libwoven::exit(0);
                }
                libwoven::exit(122);
            }
            Err(_) => libwoven::yield_now(),
        }
    }
}

fn probe_cloud() -> ! {
    let Ok(endpoint) = libwoven::ipc_service_discover(SERVICE_NAME) else {
        libwoven::exit(140);
    };
    let mut bytes = [0u8; REQUEST_CAPACITY];
    let mut response_bytes = [0u8; Response::ENCODED_LEN];
    let describe = Request {
        operation: Operation::Describe,
        request_id: 0x23_03_01,
        input: b"",
    };
    let Ok(length) = describe.encode(&mut bytes) else {
        libwoven::exit(141);
    };
    if libwoven::ai_message_send(endpoint, &bytes[..length]).is_err() {
        libwoven::exit(142);
    }
    let response = receive_response(&mut response_bytes, describe.request_id);
    if response.status != ai_provider::STATUS_OK
        || response.value != (BROKER_ABI | ai_provider::CLOUD_PROVIDER_FLAG)
    {
        libwoven::exit(143);
    }
    let request = Request {
        operation: Operation::Complete,
        request_id: 0x23_03_02,
        input: b"Hello WovenHat",
    };
    let Ok(length) = request.encode(&mut bytes) else {
        libwoven::exit(144);
    };
    if libwoven::ai_message_send(endpoint, &bytes[..length]).is_err() {
        libwoven::exit(145);
    }
    let response = receive_response(&mut response_bytes, request.request_id);
    if response.status != ai_provider::STATUS_OK
        || response.value != ai_provider::result(1 | (6u64 << 32))
    {
        libwoven::exit(146);
    }
    libwoven::exit(0)
}

fn receive_response(buffer: &mut [u8], request_id: u64) -> Response {
    loop {
        match libwoven::message_receive(buffer) {
            Ok((length, _sender)) => {
                if let Ok(response) = Response::decode(&buffer[..length]) {
                    if response.request_id == request_id {
                        return response;
                    }
                }
            }
            Err(_) => libwoven::yield_now(),
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(103);
}
