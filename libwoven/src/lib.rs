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

/// Process arguments as the kernel lays them out on the initial stack: `argc`,
/// then `argc` pointers to NUL-terminated strings, then a NULL.
///
/// Obtained through [`entry!`], which captures the initial stack pointer
/// before any Rust code can move it.
#[derive(Clone, Copy)]
pub struct Args {
    argc: usize,
    argv: *const *const u8,
}

impl Args {
    /// The kernel rejects argument strings of 256 bytes or more, so a scan
    /// that runs this far has found a malformed stack.
    const MAX_ARG_BYTES: usize = 256;

    /// # Safety
    /// `stack` must be the process's initial stack pointer, as captured by
    /// [`entry!`], with the kernel's argument layout still intact.
    pub unsafe fn from_initial_stack(stack: *const u64) -> Self {
        // SAFETY: the caller guarantees the kernel's layout at `stack`.
        let argc = unsafe { stack.read() } as usize;
        Self {
            argc,
            argv: unsafe { stack.add(1) } as *const *const u8,
        }
    }

    pub const fn len(&self) -> usize {
        self.argc
    }

    pub const fn is_empty(&self) -> bool {
        self.argc == 0
    }

    /// Argument `index` without its terminator, or `None` past the end.
    pub fn get(&self, index: usize) -> Option<&'static [u8]> {
        if index >= self.argc {
            return None;
        }
        // SAFETY: `index < argc`, and the kernel wrote `argc` valid pointers
        // to strings that live on the stack for the life of the process.
        let start = unsafe { self.argv.add(index).read() };
        let mut length = 0;
        while length < Self::MAX_ARG_BYTES {
            // SAFETY: bounded by the kernel's own argument size limit.
            if unsafe { start.add(length).read() } == 0 {
                // SAFETY: `length` bytes from `start` were just read.
                return Some(unsafe { core::slice::from_raw_parts(start, length) });
            }
            length += 1;
        }
        None
    }
}

/// Define the process entry point and hand `main` the process arguments.
///
/// `main` must have the signature `fn(libwoven::Args) -> !`. The stub is
/// assembly because the arguments are found relative to the entry stack
/// pointer, which Rust code cannot observe reliably: it passes `rsp` as the
/// first argument, keeps the stack 16-byte aligned for the call, and traps if
/// `main` ever returns.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        core::arch::global_asm!(
            ".globl _start",
            "_start:",
            "mov rdi, rsp",
            "and rsp, -16",
            "call {start}",
            "ud2",
            start = sym __libwoven_start,
        );

        #[doc(hidden)]
        unsafe extern "C" fn __libwoven_start(stack: *const u64) -> ! {
            // SAFETY: `stack` is the untouched entry stack pointer.
            let args = unsafe { $crate::Args::from_initial_stack(stack) };
            $main(args)
        }
    };
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
    pub const GRAPHICS_SURFACE_CREATE: u64 = 98;
    pub const GRAPHICS_SURFACE_FILL: u64 = 99;
    pub const GRAPHICS_SURFACE_PIXEL: u64 = 100;
    pub const GRAPHICS_SURFACE_DESTROY: u64 = 101;
    pub const GRAPHICS_COMPOSITOR_BLIT: u64 = 102;
    pub const GRAPHICS_COMPOSITOR_PIXEL: u64 = 103;
    pub const GRAPHICS_SURFACE_STROKE: u64 = 104;
    pub const GRAPHICS_COMPOSITOR_CLAIM: u64 = 105;
    pub const GRAPHICS_COMPOSITOR_PRESENT: u64 = 106;
    pub const INPUT_POLL: u64 = 107;
    pub const INPUT_WAIT: u64 = 108;
    pub const GRAPHICS_SURFACE_SHARE: u64 = 109;
    pub const GRAPHICS_COMPOSITOR_BLIT_SHARED: u64 = 110;
    pub const WAIT_ANY: u64 = 111;
    pub const MESSAGE_SEND: u64 = 11;
    pub const MESSAGE_RECEIVE: u64 = 12;

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

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_create(width: u32, height: u32) -> u64 {
        unsafe { invoke(GRAPHICS_SURFACE_CREATE, u64::from(width), u64::from(height), 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_fill(
        handle: u64,
        x: u16, y: u16, width: u16, height: u16,
        color: u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_FILL, handle, packed, u64::from(color)) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_pixel(handle: u64, x: u32, y: u32) -> u64 {
        let packed = u64::from(x) | (u64::from(y) << 32);
        unsafe { invoke(GRAPHICS_SURFACE_PIXEL, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_destroy(handle: u64) -> u64 {
        unsafe { invoke(GRAPHICS_SURFACE_DESTROY, handle, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_pixel(x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_PIXEL, packed, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_claim() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_CLAIM, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_present() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_PRESENT, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn input_poll() -> u64 {
        unsafe { invoke(INPUT_POLL, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn input_wait(deadline: u64) -> u64 {
        unsafe { invoke(INPUT_WAIT, deadline, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_share(handle: u64, grantee: u64) -> u64 {
        unsafe { invoke(GRAPHICS_SURFACE_SHARE, handle, grantee, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_shared(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT_SHARED, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn wait_any(deadline: u64, mask: u64) -> u64 {
        unsafe { invoke(WAIT_ANY, deadline, mask, 0) }
    }

    /// # Safety
    /// `buffer` must be readable for `length` bytes.
    pub unsafe fn message_send(receiver: u64, buffer: *const u8, length: usize) -> u64 {
        unsafe { invoke(MESSAGE_SEND, receiver, buffer as u64, length as u64) }
    }

    /// # Safety
    /// `buffer` must be writable for `capacity` bytes and `sender` for 8.
    pub unsafe fn message_receive(buffer: *mut u8, capacity: usize, sender: *mut u64) -> u64 {
        unsafe { invoke(MESSAGE_RECEIVE, buffer as u64, capacity as u64, sender as u64) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_stroke(
        handle: u64,
        x: u16, y: u16, width: u16, height: u16,
        color: u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_STROKE, handle, packed, u64::from(color)) }
    }
}

/// Pack a colour into the kernel's `0x00RRGGBB` representation.
pub const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

/// Typed wrapper for a client-owned drawing surface.
///
/// The underlying token packs a slot, a generation and the owning process, so
/// a `GraphicsSurface` held across a destroy is refused by the kernel rather
/// than driving a reused slot. Dropping a `GraphicsSurface` releases it.
pub struct GraphicsSurface {
    handle: u64,
    width: u32,
    height: u32,
}

impl GraphicsSurface {
    /// Allocate a surface. Caps enforced by the kernel.
    pub fn create(width: u32, height: u32) -> Result<Self, Error> {
        // SAFETY: both arguments are plain integers.
        let result = unsafe { syscall::graphics_surface_create(width, height) };
        check(result).map(|handle| Self { handle, width, height })
    }

    pub const fn handle(&self) -> u64 {
        self.handle
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Fill `(x, y)` through `(x + w - 1, y + h - 1)` with `color`.
    pub fn fill_rect(
        &self,
        x: u16, y: u16, width: u16, height: u16, color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe {
            syscall::graphics_surface_fill(self.handle, x, y, width, height, color)
        };
        check(result).map(|_| ())
    }

    /// Read one pixel back for verification.
    pub fn pixel(&self, x: u32, y: u32) -> Result<u32, Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_pixel(self.handle, x, y) };
        check(result).map(|value| value as u32)
    }

    /// Stroke a one-pixel outline of a rectangle inside this surface.
    pub fn stroke_rect(
        &self,
        x: u16, y: u16, width: u16, height: u16, color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe {
            syscall::graphics_surface_stroke(self.handle, x, y, width, height, color)
        };
        check(result).map(|_| ())
    }

    /// Allow process `grantee` to read this surface, so a compositor process
    /// can draw it with [`compositor_blit_shared`]. Replaces any earlier
    /// grant; the grant ends when the surface is destroyed.
    pub fn share_with(&self, grantee: process::Id) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_share(self.handle, grantee.0) };
        check(result).map(|_| ())
    }

    /// Withdraw any grant made with [`share_with`](Self::share_with).
    pub fn unshare(&self) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_share(self.handle, 0) };
        check(result).map(|_| ())
    }

    /// Blit this surface onto the compositor at `(x, y)`.
    ///
    /// Policy is up to the compositor: today it draws exactly where asked,
    /// which is the primitive a future window manager will drive.
    pub fn present(&self, x: i32, y: i32) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_compositor_blit(self.handle, x, y) };
        check(result).map(|_| ())
    }
}

/// Blit another process's surface onto the compositor at `(x, y)`.
///
/// `handle` is the owner's token, received out of band (for example over
/// IPC). It works only for the lease holder, and only if the owner shared the
/// surface with this process.
pub fn compositor_blit_shared(handle: u64, x: i32, y: i32) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result = unsafe { syscall::graphics_compositor_blit_shared(handle, x, y) };
    check(result).map(|_| ())
}

/// Sources for [`wait_any`].
pub mod wait {
    /// Input for the focused process (`InputIo`).
    pub const INPUT: u64 = 1 << 0;
    /// A message in this process's mailbox (`Ipc`).
    pub const MESSAGE: u64 = 1 << 1;
}

/// Sleep until any source in `mask` is ready, or until the timer reaches
/// `deadline` (`None` waits indefinitely). Returns the ready sources, or 0 on
/// timeout. Nothing is consumed: follow up with [`input_poll`] or
/// [`message_receive`].
pub fn wait_any(mask: u64, deadline: Option<time::Ticks>) -> Result<u64, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::wait_any(deadline.unwrap_or(u64::MAX), mask) })
}

/// Send `bytes` to process `receiver`. Requires the `Ipc` capability.
pub fn message_send(receiver: process::Id, bytes: &[u8]) -> Result<(), Error> {
    // SAFETY: the pointer and length describe `bytes`.
    let result = unsafe { syscall::message_send(receiver.0, bytes.as_ptr(), bytes.len()) };
    check(result).map(|_| ())
}

/// Take the next message, returning its length and sender. Fails when no
/// message is queued, or when it does not fit in `buffer`.
pub fn message_receive(buffer: &mut [u8]) -> Result<(usize, process::Id), Error> {
    let mut sender = 0u64;
    // SAFETY: the pointers describe `buffer` and `sender`, both writable.
    let result =
        unsafe { syscall::message_receive(buffer.as_mut_ptr(), buffer.len(), &mut sender) };
    check(result).map(|length| (length as usize, process::Id(sender)))
}

/// Take the compositor lease.
///
/// Requires the `DisplayIo` capability, and only one process holds the lease
/// at a time. Blitting with [`GraphicsSurface::present`], reading the screen
/// with [`screen_pixel`] and presenting with [`compositor_present`] all
/// require it. The kernel releases the lease when the holder exits.
pub fn compositor_claim() -> Result<(), Error> {
    // SAFETY: argument-free call.
    let result = unsafe { syscall::graphics_compositor_claim() };
    check(result).map(|_| ())
}

/// Copy compositor damage to the framebuffer, taking the display from the text
/// terminal on first use. Returns the number of pixels written.
pub fn compositor_present() -> Result<u64, Error> {
    // SAFETY: argument-free call.
    let result = unsafe { syscall::graphics_compositor_present() };
    check(result)
}

/// A keyboard event delivered to the focused process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    F1,
}

/// Take the next input event, if any.
///
/// Requires the `InputIo` capability and input focus, which the kernel grants
/// to the compositor lease holder while it owns the display. Returns
/// `Ok(None)` when no event is pending; the call never blocks.
pub fn input_poll() -> Result<Option<Key>, Error> {
    // SAFETY: argument-free call.
    decode_input(unsafe { syscall::input_poll() })
}

/// Sleep until the next input event, or until the timer reaches `deadline`
/// (`None` waits indefinitely). Returns `Ok(None)` on timeout.
///
/// Same authority as [`input_poll`]. Only one task of a process may wait at a
/// time; a second concurrent waiter is refused.
pub fn input_wait(deadline: Option<time::Ticks>) -> Result<Option<Key>, Error> {
    // SAFETY: integer-only call.
    decode_input(unsafe { syscall::input_wait(deadline.unwrap_or(u64::MAX)) })
}

fn decode_input(result: u64) -> Result<Option<Key>, Error> {
    if result == syscall::WOULD_BLOCK {
        return Ok(None);
    }
    let value = check(result)?;
    let payload = value & 0x00ff_ffff_ffff_ffff;
    Ok(Some(match value >> 56 {
        1 => Key::Char(char::from_u32(payload as u32).ok_or(Error::Failed)?),
        2 => Key::Enter,
        3 => Key::Backspace,
        4 => Key::Tab,
        5 => Key::F1,
        _ => return Err(Error::Failed),
    }))
}

/// Read one pixel from the compositor surface. Requires the lease, because the
/// screen carries every client's pixels.
pub fn screen_pixel(x: i32, y: i32) -> Result<u32, Error> {
    // SAFETY: integer-only call.
    let result = unsafe { syscall::graphics_compositor_pixel(x, y) };
    check(result).map(|value| value as u32)
}

impl Drop for GraphicsSurface {
    fn drop(&mut self) {
        // SAFETY: destroying a surface the current process owns is always safe;
        // the kernel ignores a stale handle rather than hitting another slot.
        unsafe {
            let _ = syscall::graphics_surface_destroy(self.handle);
        }
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
