#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;
const MAX_PREVIEW: usize = 2048;

fn main(args: Args) -> ! {
    let status = match args.get(1) {
        Some(b"--view") => match args.get(2) {
            Some(path) => view(path),
            None => 2,
        },
        Some(b"--help") => {
            let _ = libwoven::print("WovenFiles: --path PATH or --view PATH\n");
            0
        }
        Some(b"--path") => match args.get(2) {
            Some(path) => list(path),
            None => 2,
        },
        None => list(b"/mnt"),
        Some(_) => 2,
    };
    libwoven::exit(status)
}

fn list(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let _ = libwoven::print("WovenFiles: ");
    let _ = libwoven::print(path);
    let _ = libwoven::print("\n");
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print(if directory { "/\n" } else { "\n" });
            }
            Err(_) => return 0,
        }
    }
    0
}

fn view(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenFiles: cannot open file\n");
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

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
