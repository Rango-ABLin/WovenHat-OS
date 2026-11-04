#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::Args;
use wovenhat_calculator::evaluate;
use wovenhat_terminal::{
    parse, parse_shell, Command, ShellCommand, ShellLine, MAX_ARG, MAX_PIPE_COMMANDS,
};

mod gui;

libwoven::entry!(main);

const MAX_LIST_ENTRIES: usize = 128;
const MAX_CAT_CHUNKS: usize = 16;

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        gui::run(args.get(2));
    }
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
    let _ = libwoven::create_session();
    let _ = libwoven::set_process_group(None, None);
    if let Ok(group) = libwoven::process_group() {
        let _ = libwoven::set_foreground_process_group(group);
    }
    let mut jobs = Jobs::new();
    let _ = libwoven::print("WovenTerminal\n");
    let _ = libwoven::print("> ");
    let mut line = [0u8; 128];
    let mut length = 0usize;
    loop {
        let mut byte = [0u8; 1];
        match libwoven::read(libwoven::syscall::STDIN, &mut byte) {
            Ok(0) => libwoven::exit(0),
            Ok(_) if byte[0] == b'\n' || byte[0] == b'\r' => {
                let status = run_with_jobs(parse(&line[..length]), &mut jobs);
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

pub(crate) fn run(command: Command<'_>) -> u64 {
    let mut jobs = Jobs::new();
    run_with_jobs(command, &mut jobs)
}

#[derive(Clone, Copy)]
struct Job {
    count: usize,
    pids: [libwoven::process::Id; MAX_PIPE_COMMANDS],
    group: u64,
}

struct Jobs {
    entries: [Option<Job>; 4],
}

impl Jobs {
    const fn new() -> Self {
        Self { entries: [None; 4] }
    }

    fn add(&mut self, job: Job) -> bool {
        let Some(slot) = self.entries.iter_mut().find(|entry| entry.is_none()) else {
            return false;
        };
        *slot = Some(job);
        true
    }

    fn reap(&mut self) {
        for slot in &mut self.entries {
            let Some(job) = *slot else { continue };
            let mut live = false;
            for pid in job.pids.iter().take(job.count) {
                if matches!(libwoven::wait(*pid), Err(libwoven::Error::WouldBlock)) {
                    live = true;
                }
            }
            if !live {
                *slot = None;
            }
        }
    }

    fn print(&mut self) {
        self.reap();
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.is_some() {
                let mut digits = [0u8; 20];
                let text = libwoven::format_u64((index + 1) as u64, &mut digits);
                let _ = libwoven::print("[");
                let _ = libwoven::write(libwoven::syscall::STDOUT, text);
                let _ = libwoven::print("] running\n");
            }
        }
    }

    fn foreground(&mut self, number: usize) -> u64 {
        self.reap();
        let Some(slot) = self.entries.get_mut(number.saturating_sub(1)) else {
            return 2;
        };
        let Some(job) = *slot else { return 2 };
        let _ = libwoven::set_foreground_process_group(job.group);
        let mut status = 0;
        for pid in job.pids.iter().take(job.count) {
            loop {
                match libwoven::wait(*pid) {
                    Ok(value) => {
                        status = value;
                        break;
                    }
                    Err(libwoven::Error::WouldBlock) => libwoven::yield_now(),
                    Err(_) => {
                        status = 126;
                        break;
                    }
                }
            }
        }
        *slot = None;
        if let Ok(group) = libwoven::process_group() {
            let _ = libwoven::set_foreground_process_group(group);
        }
        status
    }
}

fn run_with_jobs(command: Command<'_>, jobs: &mut Jobs) -> u64 {
    match command {
        Command::Help => {
            let _ = libwoven::print("WovenTerminal: quoting, $ENV, |, <, >, >>, 2>, and trailing & are supported; jobs are bounded.\n");
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
        Command::External(command) => {
            if command == b"jobs" {
                jobs.print();
                0
            } else if command.starts_with(b"fg %") {
                let number = command[4..].iter().fold(0usize, |value, digit| {
                    value * 10 + usize::from(digit.saturating_sub(b'0'))
                });
                jobs.foreground(number)
            } else {
                execute_shell(command, jobs)
            }
        }
        Command::List(path) => list(path),
        Command::Cat(path) => cat(path),
        Command::Invalid => {
            let _ = libwoven::print("WovenTerminal: invalid command\n");
            2
        }
    }
}

const ACCESS_READ: u64 = 1;
const ACCESS_WRITE_CREATE: u64 = 2 | 4;

fn expand_word(raw: &[u8], output: &mut [u8; MAX_ARG]) -> Result<usize, u64> {
    let mut length = 0usize;
    let mut quote = 0u8;
    let mut index = 0usize;
    while index < raw.len() {
        let byte = raw[index];
        if byte == b'\\' && quote != b'\'' {
            index += 1;
            if index >= raw.len() || length >= MAX_ARG {
                return Err(2);
            }
            output[length] = raw[index];
            length += 1;
        } else if (byte == b'\'' || byte == b'"') && (quote == 0 || quote == byte) {
            quote = if quote == 0 { byte } else { 0 };
        } else if byte == b'$' && quote != b'\'' {
            let start = index + 1;
            let mut end = start;
            if raw.get(start) == Some(&b'{') {
                end += 1;
                while end < raw.len() && raw[end] != b'}' {
                    end += 1;
                }
                if end >= raw.len() {
                    return Err(2);
                }
                let key = &raw[start + 1..end];
                let close = end;
                let mut value = [0u8; 96];
                let value_len = libwoven::env_get(key, &mut value).map_err(|_| 1_u64)?;
                if length + value_len > MAX_ARG {
                    return Err(2);
                }
                output[length..length + value_len].copy_from_slice(&value[..value_len]);
                length += value_len;
                index = close;
            } else {
                while end < raw.len() && (raw[end].is_ascii_alphanumeric() || raw[end] == b'_') {
                    end += 1;
                }
                if end == start {
                    if length >= MAX_ARG {
                        return Err(2);
                    }
                    output[length] = b'$';
                    length += 1;
                } else {
                    let mut value = [0u8; 96];
                    let value_len = libwoven::env_get(&raw[start..end], &mut value).unwrap_or(0);
                    if length + value_len > MAX_ARG {
                        return Err(2);
                    }
                    output[length..length + value_len].copy_from_slice(&value[..value_len]);
                    length += value_len;
                    index = end - 1;
                }
            }
        } else {
            if length >= MAX_ARG {
                return Err(2);
            }
            output[length] = byte;
            length += 1;
        }
        index += 1;
    }
    if quote != 0 {
        return Err(2);
    }
    Ok(length)
}

fn expand_line(mut line: ShellLine) -> Result<ShellLine, u64> {
    for command in &mut line.commands[..line.count] {
        for index in 0..command.argc {
            let raw = command.args[index];
            let raw = &raw[..command.lengths[index]];
            let mut expanded = [0u8; MAX_ARG];
            let length = expand_word(raw, &mut expanded)?;
            command.args[index] = expanded;
            command.lengths[index] = length;
        }
    }
    Ok(line)
}

fn command_text(command: &ShellCommand, output: &mut [u8; 256]) -> Option<usize> {
    let mut length = 0usize;
    for index in 0..command.argc {
        if Some(index) == command.input
            || Some(index) == command.output
            || Some(index) == command.error
        {
            continue;
        }
        if length != 0 {
            if length >= output.len() {
                return None;
            }
            output[length] = b' ';
            length += 1;
        }
        let word = &command.args[index][..command.lengths[index]];
        if length + word.len() > output.len() {
            return None;
        }
        output[length..length + word.len()].copy_from_slice(word);
        length += word.len();
    }
    (length != 0).then_some(length)
}

fn redirect(
    command: &ShellCommand,
    index: Option<usize>,
    fd: u64,
    append: bool,
) -> Result<(), u64> {
    let Some(index) = index else {
        return Ok(());
    };
    let path =
        core::str::from_utf8(&command.args[index][..command.lengths[index]]).map_err(|_| 2_u64)?;
    let access = if fd == libwoven::syscall::STDIN {
        ACCESS_READ
    } else if append {
        ACCESS_READ | ACCESS_WRITE_CREATE
    } else {
        ACCESS_WRITE_CREATE
    };
    let descriptor = libwoven::open_access(path, access).map_err(|_| 1_u64)?;
    if append {
        let mut discard = [0u8; 256];
        while matches!(libwoven::read(descriptor, &mut discard), Ok(length) if length != 0) {}
    }
    libwoven::dup2(descriptor, fd).map_err(|_| 1_u64)?;
    let _ = libwoven::close(descriptor);
    Ok(())
}

fn execute_shell(input: &[u8], jobs: &mut Jobs) -> u64 {
    let Ok(parsed) = parse_shell(input) else {
        let _ = libwoven::print("WovenTerminal: invalid shell syntax or expansion\n");
        return 2;
    };
    let Ok(line) = expand_line(parsed) else {
        let _ = libwoven::print("WovenTerminal: invalid shell syntax or expansion\n");
        return 2;
    };
    let mut pipes = [[0u64; 2]; MAX_PIPE_COMMANDS - 1];
    for pipe in pipes.iter_mut().take(line.count.saturating_sub(1)) {
        *pipe = match libwoven::pipe() {
            Ok(value) => [value.0, value.1],
            Err(_) => return 1,
        };
    }
    let mut children = [libwoven::process::Id(0); MAX_PIPE_COMMANDS];
    let mut child_count = 0usize;
    let mut group = 0u64;
    for stage in 0..line.count {
        let forked = match libwoven::fork() {
            Ok(pid) => pid,
            Err(_) => return 1,
        };
        if forked.0 == 0 {
            let _ = if group == 0 {
                libwoven::set_process_group(None, None)
            } else {
                libwoven::set_process_group(None, Some(group))
            };
            if stage > 0 {
                let _ = libwoven::dup2(pipes[stage - 1][0], libwoven::syscall::STDIN);
            }
            if stage + 1 < line.count {
                let _ = libwoven::dup2(pipes[stage][1], libwoven::syscall::STDOUT);
            }
            for pipe in pipes.iter().take(line.count.saturating_sub(1)) {
                let _ = libwoven::close(pipe[0]);
                let _ = libwoven::close(pipe[1]);
            }
            let command = &line.commands[stage];
            if redirect(command, command.input, libwoven::syscall::STDIN, false).is_err()
                || redirect(
                    command,
                    command.output,
                    libwoven::syscall::STDOUT,
                    command.append,
                )
                .is_err()
                || redirect(
                    command,
                    command.error,
                    libwoven::syscall::STDERR,
                    command.append,
                )
                .is_err()
            {
                libwoven::exit(1);
            }
            let mut text = [0u8; 256];
            let Some(length) = command_text(command, &mut text) else {
                libwoven::exit(2);
            };
            let command = core::str::from_utf8(&text[..length]).unwrap_or("");
            if libwoven::exec(command).is_err() {
                libwoven::exit(127);
            }
            libwoven::exit(127);
        }
        if group == 0 {
            group = forked.0;
        }
        let _ = libwoven::set_process_group(Some(forked), Some(group));
        children[child_count] = forked;
        child_count += 1;
    }
    for pipe in pipes.iter().take(line.count.saturating_sub(1)) {
        let _ = libwoven::close(pipe[0]);
        let _ = libwoven::close(pipe[1]);
    }
    if line.background {
        if !jobs.add(Job {
            count: child_count,
            pids: children,
            group,
        }) {
            let _ = libwoven::print("WovenTerminal: job table full\n");
            return 1;
        }
        let _ = libwoven::print("[background] started\n");
        return 0;
    }
    let _ = libwoven::set_foreground_process_group(group);
    let mut status = 0;
    for child in children.iter().take(child_count) {
        loop {
            match libwoven::wait(*child) {
                Ok(value) => {
                    status = value;
                    break;
                }
                Err(libwoven::Error::WouldBlock) => libwoven::yield_now(),
                Err(_) => {
                    status = 126;
                    break;
                }
            }
        }
    }
    if let Ok(shell_group) = libwoven::process_group() {
        let _ = libwoven::set_foreground_process_group(shell_group);
    }
    status
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
