#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_update::{parse, Command};

mod client;
mod gui;
mod service;

libwoven::entry!(main);

const MAX_ENTRIES: usize = 64;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    let first = args.get(1).unwrap_or(b"--check");
    let raw: [&[u8]; 2] = [first, args.get(2).unwrap_or(b"")];
    let command = parse(&raw[..if args.get(2).is_some() { 2 } else { 1 }]);
    let status = match command {
        Command::Check => check_updates(),
        Command::Service => service::run(),
        Command::Query => transaction(libwoven::update_ipc::Operation::Query, "", client::no_slot(), "queried"),
        Command::Stage => {
            if !wovenhat_update::valid_update_path(core::str::from_utf8(raw[1]).unwrap_or("")) {
                let _ = libwoven::print("WovenUpdate: invalid candidate path\n");
                2
            } else {
                transaction(
                    libwoven::update_ipc::Operation::Stage,
                    core::str::from_utf8(raw[1]).unwrap_or(""),
                    client::no_slot(),
                    "staged",
                )
            }
        }
        Command::Activate => {
            match core::str::from_utf8(raw[1]).unwrap_or("").parse::<u8>() {
                Ok(slot) if slot < 2 => transaction(
                    libwoven::update_ipc::Operation::Activate,
                    "",
                    slot,
                    "activated",
                ),
                _ => {
                    let _ = libwoven::print("WovenUpdate: invalid slot\n");
                    2
                }
            }
        }
        Command::Help => {
            let _ = libwoven::print(
                "WovenUpdate: --check | --query | --stage PATH | --activate SLOT | --service\n",
            );
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenUpdate: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn transaction(operation: libwoven::update_ipc::Operation, source: &str, slot: u8, action: &str) -> u64 {
    match client::call(operation, source, slot, 1) {
        Ok(value) => {
            let _ = libwoven::print("WovenUpdate: ");
            let _ = libwoven::print(action);
            let _ = libwoven::print(" value=");
            let mut digits = [0u8; 20];
            if let Ok(text) = core::str::from_utf8(libwoven::format_u64(value, &mut digits)) {
                let _ = libwoven::print(text);
            }
            let _ = libwoven::print("\n");
            0
        }
        Err(status) => {
            let _ = libwoven::print("WovenUpdate: transaction failed: ");
            let _ = libwoven::print(client::status_text(status));
            let _ = libwoven::print("\n");
            3
        }
    }
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
