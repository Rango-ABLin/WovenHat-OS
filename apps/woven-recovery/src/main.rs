#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_recovery::{parse, Command};

libwoven::entry!(main);

const EXIT_OK: u64 = 0;
const EXIT_USAGE: u64 = 2;
const EXIT_DENIED: u64 = 3;
const EXIT_UNAVAILABLE: u64 = 4;

fn main(args: Args) -> ! {
    let command = parse(args.get(1).unwrap_or(b"status"));
    let status = match command {
        Command::Status => status(),
        Command::FilesystemCheck => filesystem_check(),
        Command::Logs => logs(),
        Command::Reset => admission(libwoven::recovery::OP_RESET_ADMISSION, "reset"),
        Command::Reinstall => admission(libwoven::recovery::OP_REINSTALL_ADMISSION, "reinstall"),
        Command::Help => {
            let _ = libwoven::print(
                "WovenRecovery: status | check | logs | reset | reinstall\n\
                 reset/reinstall are admission-only and never mutate the system\n",
            );
            EXIT_OK
        }
        Command::Invalid => {
            let _ = libwoven::eprint("WovenRecovery: invalid command\n");
            EXIT_USAGE
        }
    };
    libwoven::exit(status)
}

fn write_u64(value: u64) {
    let mut digits = [0u8; 20];
    let text = libwoven::format_u64(value, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
}

fn status() -> u64 {
    let Ok(value) = libwoven::recovery::status() else {
        let _ = libwoven::eprint("WovenRecovery: status denied or unavailable\n");
        return EXIT_DENIED;
    };
    let _ = libwoven::print("recovery abi=");
    write_u64(u64::from(value.abi_version));
    let _ = libwoven::print(" boot_mode=");
    write_u64(u64::from(value.boot_mode_authorized));
    let _ = libwoven::print(" vfs=");
    write_u64(u64::from(value.vfs_mounted));
    let _ = libwoven::print(" mount=");
    write_u64(u64::from(value.mount_status));
    let _ = libwoven::print(" dirty=");
    write_u64(u64::from(value.dirty));
    let _ = libwoven::print(" entries=");
    write_u64(value.imported_entries);
    let _ = libwoven::print(" syncs=");
    write_u64(value.sync_count);
    let _ = libwoven::print(" log_position=");
    write_u64(value.log_position);
    let _ = libwoven::print("\n");
    EXIT_OK
}

fn filesystem_check() -> u64 {
    match libwoven::recovery::filesystem_check() {
        Ok(report) => {
            let _ = libwoven::print("filesystem check: passed files=");
            write_u64(report.files);
            let _ = libwoven::print(" directories=");
            write_u64(report.directories);
            let _ = libwoven::print(" free_clusters=");
            write_u64(u64::from(report.free_clusters));
            let _ = libwoven::print(" fsinfo=");
            write_u64(u64::from(report.fs_info_matches));
            let _ = libwoven::print("\n");
            EXIT_OK
        }
        Err(_) => {
            let _ = libwoven::eprint(
                "filesystem check: unavailable; no successful check result\n",
            );
            EXIT_UNAVAILABLE
        }
    }
}

fn logs() -> u64 {
    let mut bytes = [0u8; libwoven::recovery::MAX_LOG_READ];
    match libwoven::recovery::read_log(0, &mut bytes) {
        Ok(length) => {
            let _ = libwoven::write(libwoven::syscall::STDOUT, &bytes[..length]);
            EXIT_OK
        }
        Err(_) => {
            let _ = libwoven::eprint("WovenRecovery: log retrieval denied\n");
            EXIT_DENIED
        }
    }
}

fn admission(operation: u64, name: &str) -> u64 {
    match libwoven::recovery::admit(operation) {
        Ok(libwoven::recovery::Admission::NotImplemented) => {
            let _ = libwoven::print(name);
            let _ = libwoven::print(
                " admission authorized, but no destructive operation is implemented\n",
            );
            EXIT_UNAVAILABLE
        }
        Err(_) => {
            let _ = libwoven::eprint(name);
            let _ = libwoven::eprint(" admission denied\n");
            EXIT_DENIED
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
