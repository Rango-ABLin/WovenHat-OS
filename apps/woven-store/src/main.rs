#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_store::{parse, Command};

mod gui;

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    let command = parse(args.get(1).unwrap_or(b""));
    let status = match command {
        Command::List => list_catalog(),
        Command::Help => {
            let _ = libwoven::print("WovenStore: --list (read-only installed catalog)\n");
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenStore: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn list_catalog() -> u64 {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir("/share/applications", index, &mut name) {
            Ok((length, directory)) => {
                if !directory {
                    let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                    let _ = libwoven::print("\n");
                    count += 1;
                }
            }
            Err(_) => break,
        }
    }
    let _ = libwoven::print("store: installed-descriptors=");
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(count as u64, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
    let _ = libwoven::print(" (bounded at 128; no package mutation)\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
