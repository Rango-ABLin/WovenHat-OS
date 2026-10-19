#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_security::{parse, Command};

mod gui;

libwoven::entry!(main);

const MAX_EVENTS: u64 = 16;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
    let command = parse(args.get(1).unwrap_or(b"--all"));
    let status = match command {
        Command::All => {
            print_help();
            report_identity();
            report_audit();
            0
        }
        Command::Identity => report_identity(),
        Command::Audit => report_audit(),
        Command::Help => {
            print_help();
            0
        }
        Command::Invalid => {
            let _ = libwoven::print("WovenSecurity: invalid mode\n");
            2
        }
    };
    libwoven::exit(status)
}

fn print_help() {
    let _ = libwoven::print("WovenSecurity: --all, --identity, --audit\n");
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

fn report_audit() -> u64 {
    let _ = libwoven::print("audit: ");
    let mut count = 0u64;
    for offset in 0..MAX_EVENTS {
        match libwoven::audit_read(offset) {
            Ok(Some(event)) => {
                if count != 0 {
                    let _ = libwoven::print(",");
                }
                let _ = libwoven::print(" seq=");
                print_number(event.sequence);
                let _ = libwoven::print(if event.allowed { " allow" } else { " deny" });
                count += 1;
            }
            Ok(None) => break,
            Err(_) => {
                let _ = libwoven::print("unavailable (TaskInspect required)\n");
                return 0;
            }
        }
    }
    let _ = libwoven::print(" entries=");
    print_number(count);
    let _ = libwoven::print(" (bounded at 16)\n");
    0
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
