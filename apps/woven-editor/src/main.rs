#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_editor::{parse, Command};

libwoven::entry!(main);

const MAX_PREVIEW: usize = 2048;

fn main(args: Args) -> ! {
    let mut raw = [&[][..]; 3];
    let mut count = 0usize;
    for index in 1..args.len() {
        if count == raw.len() {
            break;
        }
        if let Some(argument) = args.get(index) {
            raw[count] = argument;
            count += 1;
        }
    }
    let status = match parse(&raw[..count]) {
        Command::Help => {
            let _ = libwoven::print("WovenEditor: --view PATH | --write PATH TEXT\n");
            0
        }
        Command::View(path) => view(path),
        Command::Write(path, text) => write_note(path, text),
        Command::Invalid => {
            let _ = libwoven::print("WovenEditor: invalid command\n");
            2
        }
    };
    libwoven::exit(status)
}

fn as_path(raw: &[u8]) -> Option<&str> {
    core::str::from_utf8(raw).ok()
}

fn view(raw_path: &[u8]) -> u64 {
    let Some(path) = as_path(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenEditor: cannot open file\n");
        return 2;
    };
    let mut bytes = [0u8; MAX_PREVIEW];
    let result = match libwoven::read(descriptor, &mut bytes) {
        Ok(length) => {
            let _ = libwoven::write(libwoven::syscall::STDOUT, &bytes[..length]);
            if length == MAX_PREVIEW {
                let _ = libwoven::print("\n[preview truncated]\n");
            }
            0
        }
        Err(_) => 2,
    };
    let _ = libwoven::close(descriptor);
    result
}

fn write_note(raw_path: &[u8], text: &[u8]) -> u64 {
    let Some(path) = as_path(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenEditor: cannot open writable file\n");
        return 2;
    };
    let result = if libwoven::write_file(descriptor, text) == Ok(text.len()) {
        unsafe { libwoven::syscall::sync() };
        0
    } else {
        2
    };
    let _ = libwoven::close(descriptor);
    result
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
