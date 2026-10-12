#![no_std]
//! The WovenHat userspace API.
//!
//! This is the boundary an application links against instead of issuing `int
//! 0x80` by hand. The [`syscall`] module holds one thin `unsafe` wrapper per
//! kernel entry point, and the typed modules above it provide the safe surface
//! where the unsafety can actually be discharged.
//!
//! # Calling convention
//!
//! `rax` carries the syscall number, `rdi`, `rsi` and `rdx` the three
//! arguments, and the result returns in `rax`. [`ERROR`] (`u64::MAX`) signals
//! failure; a few calls additionally use `u64::MAX - 1` to mean "would block".
//! Raw wrappers return the kernel's value unchanged so a caller can tell those
//! apart.

pub mod process {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Id(pub u64);
}
pub mod thread {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Id(pub u64);
}
pub mod fs {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Handle(pub u64);
}
pub mod net {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Socket(pub u64);
}
pub mod ipc {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Handle(pub u64);
}
pub mod async_io {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Operation(pub u64);
}
pub mod security {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Capability(pub u64);
}
pub mod time {
    pub type Ticks = u64;
    /// Timer interrupts per second, matching the kernel's PIT programming.
    pub const TICKS_PER_SECOND: u64 = 100;
}
pub mod graphics {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Surface(pub u64);
}
pub mod notifications {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Record {
        pub kind: u64,
        pub source: u64,
        pub payload: u64,
    }
}

/// Raw kernel entry points.
///
/// Every function here is `unsafe` for the same two reasons: pointers must be
/// valid readable or writable userspace addresses for the stated length, and
/// the call must originate from WovenHat userspace. Individual docs note any
/// additional contract.
pub mod syscall {
    /// Returned in place of a result when a call fails.
    pub const ERROR: u64 = u64::MAX;
    /// Returned by non-blocking calls that have no result yet.
    pub const WOULD_BLOCK: u64 = u64::MAX - 1;

    pub const STDIN: u64 = 0;
    pub const STDOUT: u64 = 1;
    pub const STDERR: u64 = 2;

    pub const READ: u64 = 0;
    pub const WRITE: u64 = 1;
    pub const OPEN: u64 = 2;
    pub const EXIT: u64 = 3;
    pub const GETPID: u64 = 4;
    pub const WAITPID: u64 = 5;
    pub const CLOSE: u64 = 6;
    pub const YIELD: u64 = 7;
    pub const GETUID: u64 = 13;
    pub const GETGID: u64 = 14;
    pub const EXEC: u64 = 15;
    pub const FORK: u64 = 16;
    pub const STAT: u64 = 17;
    pub const MKDIR: u64 = 19;
    pub const CHDIR: u64 = 20;
    pub const GETCWD: u64 = 21;
    pub const GETPPID: u64 = 25;
    pub const LSEEK: u64 = 27;
    pub const UNLINK: u64 = 28;
    pub const SLEEP: u64 = 29;
    pub const RENAME: u64 = 30;
    pub const GETTICKS: u64 = 31;
    pub const SYNC: u64 = 32;
    pub const EXEC_COMMAND: u64 = 50;
    pub const ENV_GET: u64 = 51;
    pub const ENV_COUNT: u64 = 53;
    pub const SPAWN_COMMAND: u64 = 57;
    pub const NOTIFICATION_POLL: u64 = 95;
    pub const NOTIFICATION_POST: u64 = 96;

    /// Invoke the WovenHat Ring-3 ABI.
    ///
    /// # Safety
    /// Arguments and pointers must satisfy the contract of the syscall named by
    /// `number`, and the caller must be executing in WovenHat userspace.
    #[inline]
    pub unsafe fn invoke(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
        let result: u64;
        // SAFETY: The caller is executing in WovenHat userspace with the ABI
        // pointer contract satisfied; int 0x80 is the DPL3 syscall gate.
        unsafe {
            core::arch::asm!(
                "int 0x80",
                inlateout("rax") number => result,
                in("rdi") arg0,
                in("rsi") arg1,
                in("rdx") arg2,
                options(preserves_flags),
            );
        }
        result
    }

    /// Terminate the current process. Never returns.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn exit(status: u64) -> ! {
        // SAFETY: Exit takes no pointers and does not return.
        unsafe {
            invoke(EXIT, status, 0, 0);
        }
        // The kernel does not return from Exit. Spin rather than fall off the
        // end of a `-> !` function if it ever did.
        loop {
            core::hint::spin_loop();
        }
    }

    /// Write `len` bytes from `buffer` to `descriptor`.
    ///
    /// # Safety
    /// `buffer` must be readable for `len` bytes.
    pub unsafe fn write(descriptor: u64, buffer: *const u8, len: usize) -> u64 {
        unsafe { invoke(WRITE, descriptor, buffer as u64, len as u64) }
    }

    /// Read up to `len` bytes from `descriptor` into `buffer`.
    ///
    /// # Safety
    /// `buffer` must be writable for `len` bytes.
    pub unsafe fn read(descriptor: u64, buffer: *mut u8, len: usize) -> u64 {
        unsafe { invoke(READ, descriptor, buffer as u64, len as u64) }
    }

    /// Open the path described by `path`/`path_len`, returning a descriptor.
    ///
    /// # Safety
    /// `path` must be readable for `path_len` bytes.
    pub unsafe fn open(path: *const u8, path_len: usize, flags: u64) -> u64 {
        unsafe { invoke(OPEN, path as u64, path_len as u64, flags) }
    }

    /// Release a descriptor.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn close(descriptor: u64) -> u64 {
        unsafe { invoke(CLOSE, descriptor, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getpid() -> u64 {
        unsafe { invoke(GETPID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getppid() -> u64 {
        unsafe { invoke(GETPPID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getuid() -> u64 {
        unsafe { invoke(GETUID, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn getgid() -> u64 {
        unsafe { invoke(GETGID, 0, 0, 0) }
    }

    /// Yield the remainder of this time slice.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn yield_now() -> u64 {
        unsafe { invoke(YIELD, 0, 0, 0) }
    }

    /// Block for `ticks` timer interrupts.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn sleep(ticks: u64) -> u64 {
        unsafe { invoke(SLEEP, ticks, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn ticks() -> u64 {
        unsafe { invoke(GETTICKS, 0, 0, 0) }
    }

    /// Wait for a child, returning its packed status.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn waitpid(pid: u64) -> u64 {
        unsafe { invoke(WAITPID, pid, 0, 0) }
    }

    /// Spawn a child from a command line, returning its pid.
    ///
    /// # Safety
    /// `command` must be readable for `len` bytes.
    pub unsafe fn spawn(command: *const u8, len: usize) -> u64 {
        unsafe { invoke(SPAWN_COMMAND, command as u64, len as u64, 0) }
    }

    /// Replace this process image from a command line. Returns only on failure.
    ///
    /// # Safety
    /// `command` must be readable for `len` bytes.
    pub unsafe fn exec(command: *const u8, len: usize) -> u64 {
        unsafe { invoke(EXEC_COMMAND, command as u64, len as u64, 0) }
    }

    /// Flush pending filesystem state to its device.
    ///
    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn sync() -> u64 {
        unsafe { invoke(SYNC, 0, 0, 0) }
    }

    /// # Safety
    /// `record` must be a valid writable userspace pointer.
    pub unsafe fn notification_poll(record: *mut super::notifications::Record) -> u64 {
        unsafe { invoke(NOTIFICATION_POLL, record as u64, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn notification_post(kind: u64, payload: u64) -> u64 {
        unsafe { invoke(NOTIFICATION_POST, kind, payload, 0) }
    }
}

/// Failure reported by the safe API.
///
/// The kernel returns a single sentinel rather than an errno space, so this
/// distinguishes only what the ABI itself distinguishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The kernel rejected the call.
    Failed,
    /// No result is available yet.
    WouldBlock,
}

fn check(value: u64) -> Result<u64, Error> {
    match value {
        syscall::ERROR => Err(Error::Failed),
        syscall::WOULD_BLOCK => Err(Error::WouldBlock),
        other => Ok(other),
    }
}

/// Write a byte slice to a descriptor, returning the count accepted.
pub fn write(descriptor: u64, bytes: &[u8]) -> Result<usize, Error> {
    // SAFETY: a slice is readable for its own length.
    let result = unsafe { syscall::write(descriptor, bytes.as_ptr(), bytes.len()) };
    check(result).map(|count| count as usize)
}

/// Write a string to standard output.
pub fn print(text: &str) -> Result<usize, Error> {
    write(syscall::STDOUT, text.as_bytes())
}

/// Write a string to standard error.
pub fn eprint(text: &str) -> Result<usize, Error> {
    write(syscall::STDERR, text.as_bytes())
}

/// Read into a buffer, returning the count filled.
pub fn read(descriptor: u64, buffer: &mut [u8]) -> Result<usize, Error> {
    // SAFETY: a slice is writable for its own length.
    let result = unsafe { syscall::read(descriptor, buffer.as_mut_ptr(), buffer.len()) };
    check(result).map(|count| count as usize)
}

/// This process's identifier.
pub fn current_pid() -> process::Id {
    // SAFETY: getpid takes no pointers.
    process::Id(unsafe { syscall::getpid() })
}

/// This process's parent identifier.
pub fn parent_pid() -> process::Id {
    // SAFETY: getppid takes no pointers.
    process::Id(unsafe { syscall::getppid() })
}

/// Timer ticks since boot.
pub fn ticks() -> time::Ticks {
    // SAFETY: getticks takes no pointers.
    unsafe { syscall::ticks() }
}

/// Yield the remainder of this time slice.
pub fn yield_now() {
    // SAFETY: yield takes no pointers.
    unsafe {
        syscall::yield_now();
    }
}

/// Block for `ticks` timer interrupts.
pub fn sleep(ticks: time::Ticks) -> Result<(), Error> {
    // SAFETY: sleep takes no pointers.
    check(unsafe { syscall::sleep(ticks) }).map(|_| ())
}

/// Spawn a child process from a command line.
pub fn spawn(command: &str) -> Result<process::Id, Error> {
    // SAFETY: a str is readable for its own length.
    let result = unsafe { syscall::spawn(command.as_ptr(), command.len()) };
    check(result).map(process::Id)
}

/// Wait for a child process, returning its exit status.
pub fn wait(pid: process::Id) -> Result<u64, Error> {
    // SAFETY: waitpid takes no pointers.
    check(unsafe { syscall::waitpid(pid.0) })
}

/// Terminate this process. Never returns.
pub fn exit(status: u64) -> ! {
    // SAFETY: exit takes no pointers and does not return.
    unsafe { syscall::exit(status) }
}

/// Format `value` as decimal into `buffer`, returning the digits written.
///
/// Applications in this environment have no allocator and no `core::fmt`
/// machinery worth its size, so this covers the common case of reporting a
/// number without pulling in formatting.
pub fn format_u64(mut value: u64, buffer: &mut [u8]) -> &[u8] {
    if buffer.is_empty() {
        return &buffer[..0];
    }
    if value == 0 {
        buffer[0] = b'0';
        return &buffer[..1];
    }
    let mut digits = 0;
    while value > 0 && digits < buffer.len() {
        buffer[digits] = b'0' + (value % 10) as u8;
        value /= 10;
        digits += 1;
    }
    buffer[..digits].reverse();
    &buffer[..digits]
}
