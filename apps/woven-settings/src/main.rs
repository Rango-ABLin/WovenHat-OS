#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_settings::{parse, Command};

libwoven::entry!(main);

fn main(args: Args) -> ! {
    let command = parse(args.get(1).unwrap_or(b"--all"));
    let status = match command {
        Command::All => {
            print_help();
            report_identity();
            report_clock();
            report_display();
            0
        }
        Command::Identity => report_identity(),
        Command::Clock => report_clock(),
        Command::Display => report_display(),
        Command::Help => {
            print_help();
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenSettings: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn print_help() {
    let _ = libwoven::print("WovenSettings: --all, --identity, --clock, --display\n");
}

fn print_number(value: u64) {
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(value, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
}

fn report_identity() -> u64 {
    let _ = libwoven::print("identity: uid=");
    print_number(unsafe { libwoven::syscall::getuid() });
    let _ = libwoven::print(" gid=");
    print_number(unsafe { libwoven::syscall::getgid() });
    let _ = libwoven::print(" pid=");
    print_number(libwoven::current_pid().0);
    let _ = libwoven::print("\n");
    0
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
            let _ = libwoven::print("width=");
            print_number(u64::from(info.width));
            let _ = libwoven::print(" height=");
            print_number(u64::from(info.height));
            let _ = libwoven::print(" scale=");
            print_number(u64::from(info.scale));
            let _ = libwoven::print("\n");
        }
        Err(_) => {
            let _ = libwoven::print("unavailable (DisplayIo required)\n");
        }
    }
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
