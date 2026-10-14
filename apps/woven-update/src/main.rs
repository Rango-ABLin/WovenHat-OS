#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_update::{parse, Command};

libwoven::entry!(main);

const MAX_ENTRIES: usize = 64;

fn main(args: Args) -> ! {
    let command = parse(args.get(1).unwrap_or(b"--check"));
    let status = match command {
        Command::Check => check_updates(),
        Command::Help => {
            let _ = libwoven::print("WovenUpdate: --check (read-only preflight)\n");
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenUpdate: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn check_updates() -> u64 {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir("/updates", index, &mut name) {
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
    let _ = libwoven::print("updates: candidates=");
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(count as u64, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
    let _ = libwoven::print(" (bounded at 64; no installation performed)\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
