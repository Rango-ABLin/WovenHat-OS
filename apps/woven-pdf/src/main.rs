#![no_std]
#![no_main]

use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use libwoven::Args;
use wovenhat_pdf::{
    extract_plain_text, inspect, parse_command, Command, MAX_DOCUMENT_BYTES, MAX_TEXT_BYTES,
};

mod gui;

libwoven::entry!(main);

// Single-threaded program: this buffer is exclusively accessed from main.
struct Buffer(UnsafeCell<[u8; MAX_DOCUMENT_BYTES]>);
unsafe impl Sync for Buffer {}
static FILE: Buffer = Buffer(UnsafeCell::new([0; MAX_DOCUMENT_BYTES]));

struct TextBuffer(UnsafeCell<[u8; MAX_TEXT_BYTES]>);
unsafe impl Sync for TextBuffer {}
static TEXT: TextBuffer = TextBuffer(UnsafeCell::new([0; MAX_TEXT_BYTES]));

fn main(args: Args) -> ! {
    // A process-start diagnostic, not proof of rendered content.
    let _ = libwoven::print("[WOVENPDF] process: STARTED\n");
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2), args.get(3));
    }
    // Stage 20.2's experimental desktop client slot uses this argument.
    // Unlike normal --gui, its fourth argument is a fixture identifier,
    // not a PDF path. Never attempt to open it as a document.
    if args.get(1) == Some(b"--desktop-client") {
        gui::run(args.get(2), None);
    }
    let raw = [args.get(1), args.get(2)];
    let status = match (raw[0], raw[1]) {
        (None, None) => run(Command::Help),
        (Some(first), None) => run(parse_command(&[first])),
        (Some(first), Some(second)) => run(parse_command(&[first, second])),
        _ => 2,
    };
    libwoven::exit(status)
}

fn run(command: Command<'_>) -> u64 {
    match command {
        Command::Help => {
            let _ = libwoven::print("WovenPDF 0.1: --info PATH | --text PATH\n");
            let _ =
                libwoven::print("Only basic uncompressed PDF literal-text preview is supported.\n");
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenPDF: invalid argument\n");
            2
        }
        Command::Info(path) | Command::Text(path) => {
            let Ok(name) = core::str::from_utf8(path) else {
                return 2;
            };
            let Ok(fd) = libwoven::open(name) else {
                let _ = libwoven::print("WovenPDF: cannot open document\n");
                return 2;
            };
            let buffer = unsafe { &mut *FILE.0.get() };
            let mut total = 0;
            let mut overflow = false;
            loop {
                if total == buffer.len() {
                    let mut probe = [0u8; 1];
                    overflow = !matches!(libwoven::read(fd, &mut probe), Ok(0));
                    break;
                }
                match libwoven::read(fd, &mut buffer[total..]) {
                    Ok(0) => break,
                    Ok(n) if n <= buffer.len() - total => total += n,
                    _ => {
                        overflow = true;
                        break;
                    }
                }
            }
            let _ = libwoven::close(fd);
            if overflow {
                let _ = libwoven::print("WovenPDF: document too large or read failed\n");
                return 3;
            }
            let data = &buffer[..total];
            let Ok(info) = inspect(data) else {
                let _ = libwoven::print("WovenPDF: invalid or unsupported PDF header/EOF\n");
                return 3;
            };
            if matches!(command, Command::Info(_)) {
                let _ = libwoven::print("PDF version ");
                let digits = [b'0' + info.major, b'.', b'0' + info.minor, b'\n'];
                let _ = libwoven::write(libwoven::syscall::STDOUT, &digits);
                return 0;
            }
            let out = unsafe { &mut *TEXT.0.get() };
            match extract_plain_text(data, out) {
                Ok(n) => {
                    let _ = libwoven::write(libwoven::syscall::STDOUT, &out[..n]);
                    0
                }
                Err(_) => {
                    let _ = libwoven::print(
                        "WovenPDF: text not available in supported uncompressed Tj subset\n",
                    );
                    4
                }
            }
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
