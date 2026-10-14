#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_images::{kind, parse, Command};

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;
const MAX_HEADER: usize = 32;

fn main(args: Args) -> ! {
    let mut raw = [&[][..]; 2];
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
            let _ = libwoven::print("WovenImages: --list PATH | --info PATH\n");
            0
        }
        Command::List(path) => list(path),
        Command::Info(path) => info(path),
        Command::Invalid => {
            let _ = libwoven::print("WovenImages: invalid command\n");
            2
        }
    };
    libwoven::exit(status)
}

fn list(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                if directory {
                    continue;
                }
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print("\n");
            }
            Err(_) => break,
        }
    }
    0
}

fn info(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenImages: cannot open file\n");
        return 2;
    };
    let mut header = [0u8; MAX_HEADER];
    let result = match libwoven::read(descriptor, &mut header) {
        Ok(length) => {
            let _ = libwoven::print("image: ");
            let _ = libwoven::print(kind(&header[..length]));
            let _ = libwoven::print(" header-bytes=");
            let mut digits = [0u8; 20];
            let text = libwoven::format_u64(length as u64, &mut digits);
            let _ = libwoven::write(libwoven::syscall::STDOUT, text);
            let _ = libwoven::print(" (bounded at 32)\n");
            0
        }
        Err(_) => 2,
    };
    let _ = libwoven::close(descriptor);
    result
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
