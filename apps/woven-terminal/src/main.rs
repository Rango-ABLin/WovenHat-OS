#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_calculator::evaluate;
use wovenhat_terminal::{parse, Command};

libwoven::entry!(main);

const MAX_LIST_ENTRIES: usize = 128;
const MAX_CAT_CHUNKS: usize = 16;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--interactive") {
        interactive()
    }
    let command = match args.get(1) {
        Some(b"--help") | None => Command::Help,
        Some(argument) => parse(argument),
    };
    let status = run(command);
    libwoven::exit(status)
}

fn interactive() -> ! {
    let _ = libwoven::print("WovenTerminal\n");
    let _ = libwoven::print("> ");
    let mut line = [0u8; 128];
    let mut length = 0usize;
    loop {
        let mut byte = [0u8; 1];
        match libwoven::read(libwoven::syscall::STDIN, &mut byte) {
            Ok(0) => libwoven::exit(0),
            Ok(_) if byte[0] == b'\n' || byte[0] == b'\r' => {
                let status = run(parse(&line[..length]));
                if status != 0 {
                    let _ = libwoven::print("status=");
                    let mut digits = [0u8; 20];
                    let text = libwoven::format_u64(status, &mut digits);
                    let _ = libwoven::write(libwoven::syscall::STDOUT, text);
                    let _ = libwoven::print("\n");
                }
                if parse(&line[..length]) == Command::Quit {
                    libwoven::exit(0);
                }
                length = 0;
                let _ = libwoven::print("> ");
            }
            Ok(_) if length < line.len() => {
                line[length] = byte[0];
                length += 1;
            }
            Ok(_) => {
                let _ = libwoven::print("WovenTerminal: command too long\n> ");
                length = 0;
            }
            Err(_) => libwoven::exit(2),
        }
    }
}

fn run(command: Command<'_>) -> u64 {
    match command {
        Command::Help => {
            let _ = libwoven::print("WovenTerminal commands: help, ls PATH, cat PATH, calc EXPR, exit\n");
            0
        }
        Command::Quit => 0,
        Command::Calc(expression) => match evaluate(expression) {
            Ok(value) => {
                let mut digits = [0u8; 24];
                let text = format_signed(value, &mut digits);
                let _ = libwoven::write(libwoven::syscall::STDOUT, text.as_bytes());
                let _ = libwoven::print("\n");
                0
            }
            Err(_) => {
                let _ = libwoven::print("WovenTerminal: invalid calculation\n");
                2
            }
        },
        Command::List(path) => list(path),
        Command::Cat(path) => cat(path),
        Command::Invalid => {
            let _ = libwoven::print("WovenTerminal: invalid command\n");
            2
        }
    }
}

fn list(path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(path) else {
        return 2;
    };
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut index = 0;
    while index < MAX_LIST_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print(if directory { "/\n" } else { "\n" });
                index += 1;
            }
            // The current ABI uses one failure sentinel for both an absent
            // directory and end-of-directory; a bounded listing treats it as
            // completion after the entries already returned.
            Err(_) => return 0,
        }
    }
    0
}

fn cat(path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenTerminal: cannot open file\n");
        return 2;
    };
    let mut buffer = [0u8; 512];
    let mut result = 0;
    for _ in 0..MAX_CAT_CHUNKS {
        match libwoven::read(descriptor, &mut buffer) {
            Ok(0) => break,
            Ok(length) => {
                let _ = libwoven::write(libwoven::syscall::STDOUT, &buffer[..length]);
                if length < buffer.len() {
                    break;
                }
            }
            Err(_) => {
                result = 2;
                break;
            }
        }
    }
    let _ = libwoven::close(descriptor);
    result
}

fn format_signed(value: i64, output: &mut [u8; 24]) -> &str {
    let negative = value < 0;
    let mut number = value.unsigned_abs();
    let mut position = output.len();
    if number == 0 {
        position -= 1;
        output[position] = b'0';
    } else {
        while number != 0 {
            position -= 1;
            output[position] = b'0' + (number % 10) as u8;
            number /= 10;
        }
    }
    if negative {
        position -= 1;
        output[position] = b'-';
    }
    core::str::from_utf8(&output[position..]).unwrap_or("?")
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
