#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_store::{parse, Command};

mod gui;
mod service;

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    let raw: [&[u8]; 2] = [
        args.get(1).unwrap_or(b""),
        args.get(2).unwrap_or(b""),
    ];
    let command = parse(&raw[..if args.get(2).is_some() { 2 } else { 1 }]);
    let status = match command {
        Command::List => list_catalog(),
        Command::Query => transaction(libwoven::package_ipc::Operation::Query, raw[1], "queried"),
        Command::Install => transaction(
            libwoven::package_ipc::Operation::Install,
            raw[1],
            "installed",
        ),
        Command::Remove => transaction(
            libwoven::package_ipc::Operation::Remove,
            raw[1],
            "removed",
        ),
        Command::Rollback => transaction(
            libwoven::package_ipc::Operation::Rollback,
            raw[1],
            "rolled back",
        ),
        Command::Help => {
            let _ = libwoven::print(
                "WovenStore: --list | --query PATH | --install PATH | --remove PATH | --rollback PATH\n",
            );
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenStore: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn transaction(operation: libwoven::package_ipc::Operation, raw_path: &[u8], action: &str) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        let _ = libwoven::print("WovenStore: invalid path\n");
        return 2;
    };
    match service::call(operation, path, 1) {
        Ok(value) => {
            let _ = libwoven::print("WovenStore: package ");
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
            let _ = libwoven::print("WovenStore: package transaction failed: ");
            let _ = libwoven::print(service::status_text(status));
            let _ = libwoven::print("\n");
            3
        }
    }
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
    let _ = libwoven::print(" (bounded at 128; use --query/--install/--remove/--rollback for transactions)\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
