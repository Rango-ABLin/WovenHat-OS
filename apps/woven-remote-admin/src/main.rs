#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::remote_admin::{decode_request, Authorizer, Command, PolicyJournal, Request};
use libwoven::syscall::{self, ERROR, WOULD_BLOCK};

const HEADER: usize = 26;
const RESPONSE: usize = 16;
const MAX_FRAME: usize = HEADER + libwoven::remote_admin::MAX_COMMAND_BYTES;

struct QemuProxy;

impl Authorizer for QemuProxy {
    fn authorize(&mut self, request: &Request, _payload: &[u8]) -> bool {
        request.nonce != 0
    }
}

fn invoke(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    unsafe { syscall::invoke(number, arg0, arg1, arg2) }
}

fn yield_now() {
    let _ = invoke(syscall::YIELD, 0, 0, 0);
}

fn send_all(socket: u64, mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let sent = invoke(
            syscall::NET_SEND,
            socket,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        );
        if sent == WOULD_BLOCK || sent == 0 {
            yield_now();
            continue;
        }
        if sent == ERROR || sent as usize > bytes.len() {
            return false;
        }
        bytes = &bytes[sent as usize..];
    }
    true
}

/// Read exactly `bytes`. `Err` means the peer closed (NET_RECV returns 0 after
/// all queued bytes) or the socket failed; either way this client is done and
/// the listener can accept the next one.
fn recv_exact(socket: u64, bytes: &mut [u8]) -> Result<(), ()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let received = invoke(
            syscall::NET_RECV,
            socket,
            bytes[offset..].as_mut_ptr() as u64,
            (bytes.len() - offset) as u64,
        );
        if received == WOULD_BLOCK {
            yield_now();
            continue;
        }
        if received == ERROR || received == 0 || received as usize > bytes.len() - offset {
            return Err(());
        }
        offset += received as usize;
    }
    Ok(())
}

fn respond(socket: u64, journal: &PolicyJournal, status: u8) -> bool {
    let view = journal.view();
    let mut response = [0u8; RESPONSE];
    response[..4].copy_from_slice(b"WADR");
    response[4] = 1;
    response[5] = status;
    response[6..8].copy_from_slice(&(view.length as u16).to_be_bytes());
    response[8..16].copy_from_slice(&view.generation.to_be_bytes());
    send_all(socket, &response)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let _ = libwoven::print("[REMOTE ADMIN] freestanding process entered\n");
    let socket = invoke(syscall::SOCKET, 2, 0, 0);
    let _ = libwoven::print("[REMOTE ADMIN] socket opened\n");
    if socket == ERROR
        || libwoven::network::listen(socket, wovenhat_remote_admin::PORT).is_err()
    {
        libwoven::exit(2)
    }
    let Ok(mut journal) = PolicyJournal::new(b"safe") else {
        libwoven::exit(3)
    };
    let _ = libwoven::print("[REMOTE ADMIN] freestanding listener ready port=7001\n");
    let peer = match libwoven::network::accept(socket) {
        Ok(peer) => peer,
        Err(_) => libwoven::exit(5),
    };
    let _ = libwoven::print("[REMOTE ADMIN] accepted connection through listen/accept ABI\n");
    let _ = peer;
    let mut frame = [0u8; MAX_FRAME];
    let mut payload = [0u8; libwoven::remote_admin::MAX_COMMAND_BYTES];
    loop {
        if recv_exact(socket, &mut frame[..HEADER]).is_err() {
            // This client disconnected. The kernel re-arms the listener when
            // the FIN is observed, so wait here for the next client on the
            // same descriptor instead of exiting or closing the socket.
            match libwoven::network::accept(socket) {
                Ok(_) => {}
                Err(_) => yield_now(),
            }
            continue;
        }
        let length = u16::from_be_bytes([frame[24], frame[25]]) as usize;
        if length > libwoven::remote_admin::MAX_COMMAND_BYTES
            || recv_exact(socket, &mut frame[HEADER..HEADER + length]).is_err()
        {
            let _ = respond(socket, &journal, 1);
            continue;
        }
        let Ok(request) = decode_request(&frame[..HEADER + length], &mut payload) else {
            let _ = respond(socket, &journal, 1);
            continue;
        };
        let mut authorizer = QemuProxy;
        let result = match request.command {
            Command::ApplyPolicy => journal.apply(
                &mut authorizer,
                request.nonce,
                request.expected_generation,
                &payload[..request.payload_len],
            ),
            Command::RollbackPolicy => {
                journal.rollback(&mut authorizer, request.nonce, request.expected_generation)
            }
            Command::QueryPolicy => Ok(journal.view()),
        };
        if !respond(socket, &journal, if result.is_ok() { 0 } else { 1 }) {
            let _ = invoke(syscall::NET_CLOSE, socket, 0, 0);
            libwoven::exit(4)
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
