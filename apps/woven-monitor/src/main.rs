#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_monitor::{parse, Command};

mod gui;

libwoven::entry!(main);

const MAX_STORAGE_ENTRIES: usize = 128;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    let command = parse(args.get(1).unwrap_or(b"--all"));
    let status = match command {
        Command::All => {
            print_help();
            report_clock();
            report_display();
            report_storage();
            0
        }
        Command::Clock => report_clock(),
        Command::Display => report_display(),
        Command::Storage => report_storage(),
        Command::Help => {
            print_help();
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenMonitor: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn print_help() {
    let _ = libwoven::print("WovenMonitor: --all, --clock, --display, --storage\n");
}

fn report_clock() -> u64 {
    let _ = libwoven::print("clock: ");
    let Some(time) = libwoven::clock().ok() else {
        let _ = libwoven::print("unavailable\n");
        return 0;
    };
    let mut digits = [0u8; 20];
    let fields = [
        ("", u64::from(time.year), 4usize),
        ("-", u64::from(time.month), 2),
        ("-", u64::from(time.day), 2),
        ("T", u64::from(time.hour), 2),
        (":", u64::from(time.minute), 2),
        (":", u64::from(time.second), 2),
    ];
    for (separator, value, width) in fields {
        let _ = libwoven::print(separator);
        let text = libwoven::format_u64(value, &mut digits);
        for _ in 0..width.saturating_sub(text.len()) {
            let _ = libwoven::print("0");
        }
        let _ = libwoven::write(libwoven::syscall::STDOUT, text);
    }
    let _ = libwoven::print("Z\n");
    0
}

fn report_display() -> u64 {
    let _ = libwoven::print("display: ");
    match libwoven::compositor_info() {
        Ok(info) => {
            let mut digits = [0u8; 20];
            for (label, value) in [("width=", info.width), (" height=", info.height), (" scale=", info.scale)] {
                let _ = libwoven::print(label);
                let text = libwoven::format_u64(u64::from(value), &mut digits);
                let _ = libwoven::write(libwoven::syscall::STDOUT, text);
            }
            let _ = libwoven::print("\n");
            0
        }
        Err(_) => {
            let _ = libwoven::print("unavailable (DisplayIo required)\n");
            0
        }
    }
}

fn report_storage() -> u64 {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_STORAGE_ENTRIES {
        if libwoven::read_dir("/mnt", index, &mut name).is_err() {
            break;
        }
        count += 1;
    }
    let _ = libwoven::print("storage: /mnt entries=");
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(count as u64, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
    let _ = libwoven::print(" (bounded at 128)\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
