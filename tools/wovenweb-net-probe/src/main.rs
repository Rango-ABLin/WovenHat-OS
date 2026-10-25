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
static PAGER_THREAD_ADDRESS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
libwoven::thread_entry!(pager_thread_entry, pager_thread);
extern "C" fn pager_thread() -> ! {
    let base = PAGER_THREAD_ADDRESS.load(core::sync::atomic::Ordering::Acquire);
    // SAFETY: the leader published a live read-only 8192-byte lazy mapping and
    // waits for this thread before exiting. Each volatile read must fault in a
    // different page, independently of optimizer load folding.
    let passed = unsafe {
        core::ptr::with_exposed_provenance::<u8>(base as usize).read_volatile() == 0x71
            && core::ptr::with_exposed_provenance::<u8>((base + 4096) as usize).read_volatile()
                == 0x71
    };
    // SAFETY: terminating this native sibling thread through its integer ABI.
    unsafe { libwoven::syscall::thread_exit(if passed { 0 } else { 94 }) }
}
fn pager_thread_probe() -> ! {
    let Ok(file) = libwoven::open("/tmp/wovenweb-pager-thread") else {
        libwoven::exit(92)
    };
    // SAFETY: integer file descriptor and bounded mapping request, no pointer.
    let base = unsafe { libwoven::syscall::invoke(60, file, 8192, 0) };
    let _ = libwoven::close(file);
    if base == 0 || base >= libwoven::syscall::WOULD_BLOCK {
        libwoven::exit(93)
    }
    PAGER_THREAD_ADDRESS.store(base, core::sync::atomic::Ordering::Release);
    // SAFETY: executable raw-entry trampoline establishes the required C frame.
    let thread =
        unsafe { libwoven::syscall::thread_create(pager_thread_entry as *const () as u64, 16384) };
    if thread >= libwoven::syscall::WOULD_BLOCK {
        libwoven::exit(95)
    }
    // SAFETY: this is the generation-tagged sibling handle just created here.
    let result = unsafe { libwoven::syscall::thread_join(libwoven::thread::Id(thread)) };
    libwoven::exit(if result == libwoven::thread::Join::Exited(0) {
        0
    } else {
        96
    })
}
libwoven::thread_entry!(queued_first_entry, queued_first);
libwoven::thread_entry!(queued_second_entry, queued_second);
libwoven::thread_entry!(queued_duplicate_entry, queued_first);
extern "C" fn queued_first() -> ! {
    queued_read(0)
}
extern "C" fn queued_second() -> ! {
    queued_read(4096)
}
fn queued_read(offset: u64) -> ! {
    let base = PAGER_THREAD_ADDRESS.load(core::sync::atomic::Ordering::Acquire);
    // SAFETY: the leader retains an 8192-byte read-only mapping until both joins.
    let passed = unsafe {
        core::ptr::with_exposed_provenance::<u8>((base + offset) as usize).read_volatile() == 0x71
    };
    // SAFETY: integer-only native thread exit ABI.
    unsafe { libwoven::syscall::thread_exit(if passed { 0 } else { 97 }) }
}
fn queued_pager_probe(duplicate: bool) -> ! {
    let Ok(file) = libwoven::open("/tmp/wovenweb-queued-pager") else {
        libwoven::exit(92)
    };
    // SAFETY: bounded integer-only native lazy mapping request.
    let base = unsafe { libwoven::syscall::invoke(60, file, 8192, 0) };
    if libwoven::close(file).is_err() || base == 0 || base >= libwoven::syscall::WOULD_BLOCK {
        libwoven::exit(93)
    }
    PAGER_THREAD_ADDRESS.store(base, core::sync::atomic::Ordering::Release);
    let second = if duplicate {
        queued_duplicate_entry as *const () as u64
    } else {
        queued_second_entry as *const () as u64
    };
    let mut threads = [0; 2];
    for (slot, entry) in threads
        .iter_mut()
        .zip([queued_first_entry as *const () as u64, second])
    {
        // SAFETY: executable raw entry establishes the native C call frame.
        *slot = unsafe { libwoven::syscall::thread_create(entry, 16384) };
        if *slot >= libwoven::syscall::WOULD_BLOCK {
            libwoven::exit(95)
        }
    }
    for id in threads {
        // SAFETY: owned generation-tagged handles returned by thread_create.
        if unsafe { libwoven::syscall::thread_join(libwoven::thread::Id(id)) }
            != libwoven::thread::Join::Exited(0)
        {
            libwoven::exit(96)
        }
    }
    libwoven::exit(0)
}
fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--vm-queued") {
        queued_pager_probe(false)
    }
    if args.get(1) == Some(b"--vm-duplicate") {
        queued_pager_probe(true)
    }
    if args.get(1) == Some(b"--vm-thread") {
        pager_thread_probe()
    }
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
