#![no_std]
#![no_main]
extern crate alloc;
mod heap;
use libwoven::{
    Args,
    net::{self, Endpoint, TcpStream},
    socks5,
};
libwoven::entry!(main);
const PROXY: Endpoint = Endpoint {
    address: [10, 0, 2, 2],
    port: 18085,
};
const BODY: &[u8] = b"<html><body><h1>WovenWeb native transport</h1></body></html>";
fn check() -> Result<(), u64> {
    let stream = socks5::connect(PROXY, "fixture.invalid", 80, 200).map_err(|_| 10u64)?;
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: fixture.invalid\r\nConnection: close\r\n\r\n")
        .map_err(|_| 11u64)?;
    // Read the head byte-by-byte so no body bytes are lost to coalescing.
    let mut head = [0u8; 512];
    let mut used = 0;
    loop {
        if used == head.len() {
            return Err(12);
        }
        stream
            .read_exact(&mut head[used..used + 1])
            .map_err(|_| 13u64)?;
        used += 1;
        if used >= 4 && &head[used - 4..used] == b"\r\n\r\n" {
            break;
        }
    }
    let parsed = libwoven::web::parse_response_head(&head[..used]).ok_or(14u64)?;
    if parsed.status != 200 || parsed.content_length != Some(BODY.len() as u64) {
        return Err(15);
    }
    let mut body = [0u8; BODY.len()];
    stream.read_exact(&mut body).map_err(|_| 16u64)?;
    if body != BODY {
        return Err(17);
    }
    let document = libwoven::browser::parse_html::<16>(&body).map_err(|_| 18u64)?;
    if document.len() != 3 {
        return Err(19);
    }
    let _ = libwoven::print("[WOVENWEB] remote-name SOCKS + HTTP + native DOM: PASSED\n");
    // More than the bounded kernel queue capacity: every drop must reclaim its
    // operation without retaining this stack output or exhausting table slots.
    for _ in 0..256 {
        let mut pending = stream.receive(1).map_err(|_| 20u64)?;
        if pending.poll(&mut []) != Err(net::Error::OutputTooSmall) {
            return Err(21);
        }
        pending.cancel().map_err(|_| 22u64)?;
    }
    drop(stream);
    match socks5::connect(PROXY, "fixture.invalid", 80, 200) {
        Err(socks5::Error::AuthenticationRequired) => {}
        _ => return Err(23),
    }
    // Establish transport before testing a deliberately stalled method reply;
    // the deadline assertion must not depend on TCP handshake scheduling.
    let stalled = TcpStream::new().map_err(|_| 24u64)?;
    stalled
        .connect(PROXY)
        .map_err(|_| 25u64)?
        .wait_timeout(&mut [], 200)
        .map_err(|_| 26u64)?;
    stalled
        .write_all_timeout(&socks5::GREETING, 200)
        .map_err(|_| 27u64)?;
    let mut method = [0u8; 2];
    match stalled
        .receive(2)
        .map_err(|_| 28u64)?
        .wait_timeout(&mut method, 3)
    {
        Err(net::Error::TimedOut) => {}
        _ => return Err(29),
    }
    drop(stalled);
    let _ = libwoven::print("[WOVENWEB] deadline + cancellation + proxy refusal: PASSED\n");
    Ok(())
}
fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--vm-reap") {
        // SAFETY: integer-only native mmap ABI; no user pointer is retained.
        let base = unsafe { libwoven::syscall::invoke(8, 4096, 1, 0) };
        if base == 0 || base >= libwoven::syscall::WOULD_BLOCK {
            libwoven::exit(91);
        }
        let pointer = core::ptr::with_exposed_provenance_mut::<u8>(base as usize);
        // SAFETY: mmap returned a fresh writable page owned by this child.
        unsafe { pointer.write(0x71) };
        libwoven::exit(0);
    }
    if args.get(1) == Some(b"--heap") {
        libwoven::exit(heap::run().err().unwrap_or(0));
    }
    if args.get(1) == Some(b"--denied") {
        let status = if matches!(TcpStream::new(), Err(net::Error::Rejected)) {
            0
        } else {
            30
        };
        let _ = libwoven::print("[WOVENWEB] network authority denial checked\n");
        libwoven::exit(status)
    }
    libwoven::exit(check().err().unwrap_or(0))
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    libwoven::exit(101)
}
