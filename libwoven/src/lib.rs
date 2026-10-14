#![cfg_attr(not(test), no_std)]
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

/// The kernel's 5x7 bitmap font, shared source rather than a copy, so text
/// a client renders matches text the kernel renders pixel for pixel.
#[allow(unexpected_cfgs)]
#[path = "../../kernel/src/font.rs"]
pub mod font;

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

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u8)]
    pub enum AuditAction {
        CapabilityGrant = 0,
        CapabilityRevoke = 1,
        IpcSend = 2,
        FileWrite = 3,
        ProcessFault = 4,
        ProcessExec = 5,
        ProcessFork = 6,
        WovenGuardAllow = 7,
        WovenGuardDeny = 8,
        CapabilityLineageIssue = 9,
        CapabilityDerived = 10,
        CapabilityLineageRelease = 11,
        CapabilityLineageRevoke = 12,
        PermissionRequest = 13,
        PermissionDecision = 14,
        PermissionRevoke = 15,
        SandboxProfileBind = 16,
        SandboxProfileDeny = 17,
        SandboxServicePublish = 18,
        SandboxServiceDiscover = 19,
        SandboxServiceUse = 20,
        SandboxFileRead = 21,
        SandboxFileWrite = 22,
        SandboxDeviceAccess = 23,
        SecretStore = 24,
        SecretRevoke = 25,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct AuditEvent {
        pub sequence: u64,
        pub tick: u64,
        pub cpu: u16,
        pub action: AuditAction,
        pub actor: u64,
        pub target: u64,
        pub detail: u64,
        pub allowed: bool,
    }
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
    pub const READDIR: u64 = 18;
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
    pub const GRAPHICS_SURFACE_DAMAGE: u64 = 112;
    pub const GRAPHICS_COMPOSITOR_BLIT_DAMAGE: u64 = 113;
    pub const GRAPHICS_COMPOSITOR_BLEND: u64 = 114;
    pub const GRAPHICS_COMPOSITOR_BLEND_SHARED: u64 = 115;
    pub const GRAPHICS_COMPOSITOR_INFO: u64 = 116;
    pub const GRAPHICS_COMPOSITOR_SCALE: u64 = 117;
    pub const GRAPHICS_SURFACE_WRITE: u64 = 118;
    pub const CLOCK_READ: u64 = 119;
    pub const GRAPHICS_COMPOSITOR_BLIT_REGION: u64 = 120;
    pub const GRAPHICS_COMPOSITOR_BLIT_REGION_SHARED: u64 = 121;
    pub const PERMISSION_REQUEST: u64 = 122;
    pub const PERMISSION_POLL: u64 = 123;
    pub const PERMISSION_DECIDE: u64 = 124;
    pub const PERMISSION_REVOKE: u64 = 125;
    pub const AUDIT_READ: u64 = 126;
    pub const SECRET_STORE: u64 = 127;
    pub const SECRET_READ: u64 = 128;
    pub const SECRET_REVOKE: u64 = 129;
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
        unsafe {
            invoke(
                GRAPHICS_SURFACE_CREATE,
                u64::from(width),
                u64::from(height),
                0,
            )
        }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_fill(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
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
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_damage(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_DAMAGE, handle, packed, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blend(handle: u64, x: i32, y: i32, opacity: u64) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLEND, handle, packed, opacity) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blend_shared(
        handle: u64,
        x: i32,
        y: i32,
        opacity: u64,
    ) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLEND_SHARED, handle, packed, opacity) }
    }

    /// # Safety
    /// `pixels` must be readable for `width * height` `u32`s.
    pub unsafe fn graphics_surface_write(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: *const u32,
    ) -> u64 {
        let packed = u64::from(x)
            | (u64::from(y) << 16)
            | (u64::from(width) << 32)
            | (u64::from(height) << 48);
        unsafe { invoke(GRAPHICS_SURFACE_WRITE, handle, packed, pixels as u64) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_region(
        handle: u64,
        x: i32,
        y: i32,
        region: u64,
        shared: bool,
    ) -> u64 {
        let destination = (x as u32 as u64) | ((y as u32 as u64) << 32);
        let number = if shared {
            GRAPHICS_COMPOSITOR_BLIT_REGION_SHARED
        } else {
            GRAPHICS_COMPOSITOR_BLIT_REGION
        };
        unsafe { invoke(number, handle, destination, region) }
    }

    /// # Safety
    /// `path` must be readable for `path_len` bytes and `name` writable for
    /// 255 bytes, the longest name a directory entry can carry.
    pub unsafe fn readdir(path: *const u8, path_len: usize, index: usize, name: *mut u8) -> u64 {
        let packed = (path_len as u64 & 0xffff) | ((index as u64) << 16);
        unsafe { invoke(READDIR, path as u64, packed, name as u64) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn clock_read() -> u64 {
        unsafe { invoke(CLOCK_READ, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_info() -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_INFO, 0, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_scale(scale: u64) -> u64 {
        unsafe { invoke(GRAPHICS_COMPOSITOR_SCALE, scale, 0, 0) }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_compositor_blit_damage(handle: u64, x: i32, y: i32) -> u64 {
        let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
        unsafe { invoke(GRAPHICS_COMPOSITOR_BLIT_DAMAGE, handle, packed, 0) }
    }

    /// # Safety
    /// `buffer` must be readable for `length` bytes.
    pub unsafe fn message_send(receiver: u64, buffer: *const u8, length: usize) -> u64 {
        unsafe { invoke(MESSAGE_SEND, receiver, buffer as u64, length as u64) }
    }

    /// # Safety
    /// `buffer` must be writable for `capacity` bytes and `sender` for 8.
    pub unsafe fn message_receive(buffer: *mut u8, capacity: usize, sender: *mut u64) -> u64 {
        unsafe {
            invoke(
                MESSAGE_RECEIVE,
                buffer as u64,
                capacity as u64,
                sender as u64,
            )
        }
    }

    /// # Safety
    /// Must be called from WovenHat userspace.
    pub unsafe fn graphics_surface_stroke(
        handle: u64,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
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
        check(result).map(|handle| Self {
            handle,
            width,
            height,
        })
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
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_surface_fill(self.handle, x, y, width, height, color) };
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
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: u32,
    ) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_surface_stroke(self.handle, x, y, width, height, color) };
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

    /// Tell the compositor that `(x, y, width, height)` of this surface
    /// changed. If the surface is shared, the grantee is woken from
    /// [`wait_any`] with [`wait::DAMAGE`].
    pub fn damage(&self, x: u16, y: u16, width: u16, height: u16) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_damage(self.handle, x, y, width, height) };
        check(result).map(|_| ())
    }

    /// Withdraw any grant made with [`share_with`](Self::share_with).
    pub fn unshare(&self) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe { syscall::graphics_surface_share(self.handle, 0) };
        check(result).map(|_| ())
    }

    /// Upload `pixels` (`0x00RRGGBB`, row by row) into the rectangle at
    /// `(x, y)`. The rectangle must lie inside the surface and `pixels` must
    /// hold exactly `width * height` values. This is how anything fill and
    /// stroke cannot express reaches a surface: render into a [`Canvas`],
    /// then upload it.
    pub fn write(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: &[u32],
    ) -> Result<(), Error> {
        if pixels.len() != usize::from(width) * usize::from(height) {
            return Err(Error::Failed);
        }
        // SAFETY: `pixels` is readable for exactly `width * height` values.
        let result = unsafe {
            syscall::graphics_surface_write(self.handle, x, y, width, height, pixels.as_ptr())
        };
        check(result).map(|_| ())
    }

    /// Blit only `region` of this surface (in surface coordinates) so its
    /// top-left lands at `(x, y)` on the compositor. Lease holder only.
    pub fn present_region(&self, region: Region, x: i32, y: i32) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result = unsafe {
            syscall::graphics_compositor_blit_region(self.handle, x, y, region.pack(), false)
        };
        check(result).map(|_| ())
    }

    /// Upload a whole canvas to the top-left of this surface.
    pub fn write_canvas(&self, canvas: &Canvas<'_>) -> Result<(), Error> {
        let (Ok(width), Ok(height)) = (u16::try_from(canvas.width), u16::try_from(canvas.height))
        else {
            return Err(Error::Failed);
        };
        self.write(0, 0, width, height, canvas.pixels())
    }

    /// Blend this surface onto the compositor at `(x, y)` with uniform
    /// `opacity` (0 invisible, 255 opaque). Lease holder only; this is how a
    /// window manager draws shadows and translucent chrome.
    pub fn present_blended(&self, x: i32, y: i32, opacity: u8) -> Result<(), Error> {
        // SAFETY: integer-only call carrying the surface's own handle.
        let result =
            unsafe { syscall::graphics_compositor_blend(self.handle, x, y, u64::from(opacity)) };
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
    /// Damage on a surface shared with this process (`DisplayIo`).
    pub const DAMAGE: u64 = 1 << 2;
}

/// Sleep until any source in `mask` is ready, or until the timer reaches
/// `deadline` (`None` waits indefinitely). Returns the ready sources, or 0 on
/// timeout. Nothing is consumed: follow up with [`input_poll`] or
/// [`message_receive`].
pub fn wait_any(mask: u64, deadline: Option<time::Ticks>) -> Result<u64, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::wait_any(deadline.unwrap_or(u64::MAX), mask) })
}

/// A rectangle reported by [`compositor_blit_damage`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Region {
    /// `x | y << 16 | width << 32 | height << 48`, the kernel's rectangle
    /// packing.
    pub const fn pack(self) -> u64 {
        self.x as u64
            | (self.y as u64) << 16
            | (self.width as u64) << 32
            | (self.height as u64) << 48
    }
}

/// The process that owns the surface `token` names. A token packs its
/// owner's id in its high 32 bits, and the kernel checks that name against
/// the real owner on every operation, so a token naming process P can only
/// ever reach a surface P owns. A compositor uses this to accept a surface
/// only from its owner.
pub const fn surface_owner(token: u64) -> process::Id {
    process::Id(token >> 32)
}

/// Blit only `region` of another process's shared surface so its top-left
/// lands at `(x, y)`. Same rules as [`compositor_blit_shared`]; this is how a
/// window manager repaints an exposed rectangle without overdrawing windows
/// stacked above it.
pub fn compositor_blit_region_shared(
    handle: u64,
    region: Region,
    x: i32,
    y: i32,
) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result =
        unsafe { syscall::graphics_compositor_blit_region(handle, x, y, region.pack(), true) };
    check(result).map(|_| ())
}

/// Composite only what a client changed: blit the pending damage of a shared
/// surface into a window placed at `(x, y)`. Returns the region copied, in
/// surface coordinates, or `None` when nothing was pending. Lease holder only.
pub fn compositor_blit_damage(handle: u64, x: i32, y: i32) -> Result<Option<Region>, Error> {
    // SAFETY: integer-only call.
    let value = check(unsafe { syscall::graphics_compositor_blit_damage(handle, x, y) })?;
    if value == 0 {
        return Ok(None);
    }
    Ok(Some(Region {
        x: value as u16,
        y: (value >> 16) as u16,
        width: (value >> 32) as u16,
        height: (value >> 48) as u16,
    }))
}

/// Blend another process's shared surface onto the compositor with uniform
/// `opacity`. Same rules as [`compositor_blit_shared`].
pub fn compositor_blend_shared(handle: u64, x: i32, y: i32, opacity: u8) -> Result<(), Error> {
    // SAFETY: integer-only call.
    let result =
        unsafe { syscall::graphics_compositor_blend_shared(handle, x, y, u64::from(opacity)) };
    check(result).map(|_| ())
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

/// Blend `over` onto `under` with `opacity` (0 keeps `under`, 255 gives
/// `over`), per channel, rounding to nearest: the same formula the kernel
/// uses for translucent blits.
pub const fn blend(over: u32, under: u32, opacity: u8) -> u32 {
    let a = opacity as u32;
    let mut result = 0;
    let mut shift = 0;
    while shift <= 16 {
        let top = (over >> shift) & 0xff;
        let bottom = (under >> shift) & 0xff;
        result |= ((top * a + bottom * (255 - a) + 127) / 255) << shift;
        shift += 8;
    }
    result
}

/// A software render target: a caller-owned pixel buffer covering a
/// `width`×`height` rectangle whose top-left is `(x, y)` in the caller's
/// coordinates. Every drawing call takes those coordinates and clips to the
/// rectangle, so a scene can be drawn once per tile without offset
/// arithmetic in the scene code.
pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

impl<'a> Canvas<'a> {
    /// `None` if `pixels` is shorter than `width * height`.
    pub fn new(pixels: &'a mut [u32], x: i32, y: i32, width: usize, height: usize) -> Option<Self> {
        let length = width.checked_mul(height)?;
        let pixels = pixels.get_mut(..length)?;
        Some(Self {
            pixels,
            x,
            y,
            width,
            height,
        })
    }

    pub const fn x(&self) -> i32 {
        self.x
    }

    pub const fn y(&self) -> i32 {
        self.y
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// The rendered pixels, row by row: what [`GraphicsSurface::write`]
    /// uploads.
    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let column = usize::try_from(x.checked_sub(self.x)?).ok()?;
        let row = usize::try_from(y.checked_sub(self.y)?).ok()?;
        (column < self.width && row < self.height).then_some(row * self.width + column)
    }

    pub fn get(&self, x: i32, y: i32) -> Option<u32> {
        self.index(x, y).map(|index| self.pixels[index])
    }

    pub fn set(&mut self, x: i32, y: i32, color: u32) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index] = color & 0x00ff_ffff;
        }
    }

    /// Set every pixel from `shader(x, y)`, in caller coordinates.
    pub fn shade(&mut self, mut shader: impl FnMut(i32, i32) -> u32) {
        for row in 0..self.height {
            let y = self.y + row as i32;
            for column in 0..self.width {
                self.pixels[row * self.width + column] =
                    shader(self.x + column as i32, y) & 0x00ff_ffff;
            }
        }
    }

    /// The part of `(x, y, width, height)` inside the canvas, as buffer
    /// rows and columns.
    fn clip(
        &self,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let left = i64::from(x).max(i64::from(self.x));
        let top = i64::from(y).max(i64::from(self.y));
        let right = (i64::from(x) + width as i64).min(i64::from(self.x) + self.width as i64);
        let bottom = (i64::from(y) + height as i64).min(i64::from(self.y) + self.height as i64);
        if right <= left || bottom <= top {
            return None;
        }
        Some((
            (left - i64::from(self.x)) as usize,
            (top - i64::from(self.y)) as usize,
            (right - left) as usize,
            (bottom - top) as usize,
        ))
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: usize, height: usize, color: u32) {
        self.blend_rect(x, y, width, height, color, u8::MAX);
    }

    /// Blend `color` over a rectangle with uniform `opacity`.
    pub fn blend_rect(
        &mut self,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        color: u32,
        opacity: u8,
    ) {
        let Some((column, row, columns, rows)) = self.clip(x, y, width, height) else {
            return;
        };
        for r in row..row + rows {
            for pixel in
                &mut self.pixels[r * self.width + column..r * self.width + column + columns]
            {
                *pixel = blend(color, *pixel, opacity) & 0x00ff_ffff;
            }
        }
    }

    /// Draw `text` with the kernel's font, top-left at `(x, y)`. Newlines
    /// start a new line at `x`.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: u32) {
        let (mut cursor_x, mut cursor_y) = (x, y);
        for character in text.chars() {
            if character == '\n' {
                cursor_x = x;
                cursor_y += font::LINE_HEIGHT;
                continue;
            }
            for (row, bits) in font::glyph(character).iter().enumerate() {
                for column in 0..font::WIDTH as i32 {
                    if bits & (1 << (font::WIDTH as i32 - 1 - column)) != 0 {
                        self.set(cursor_x + column, cursor_y + row as i32, color);
                    }
                }
            }
            cursor_x += font::ADVANCE;
        }
    }
}

/// A calendar date and time of day from the hardware clock, UTC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Read the wall clock. The hardware clock keeps UTC and carries no time
/// zone.
pub fn clock() -> Result<DateTime, Error> {
    // SAFETY: argument-free call.
    check(unsafe { syscall::clock_read() }).map(|value| DateTime {
        second: value as u8,
        minute: (value >> 8) as u8,
        hour: (value >> 16) as u8,
        day: (value >> 24) as u8,
        month: (value >> 32) as u8,
        year: (value >> 40) as u16,
    })
}

/// The compositor surface's geometry and the display scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompositorInfo {
    /// Surface width in logical pixels: the framebuffer width over `scale`.
    pub width: u32,
    /// Surface height in logical pixels.
    pub height: u32,
    /// Each logical pixel is shown as a `scale`×`scale` block.
    pub scale: u32,
}

impl CompositorInfo {
    const fn unpack(value: u64) -> Self {
        Self {
            width: (value & 0xff_ffff) as u32,
            height: ((value >> 24) & 0xff_ffff) as u32,
            scale: ((value >> 48) & 0xffff) as u32,
        }
    }
}

/// Largest display scale the kernel accepts.
pub const MAX_DISPLAY_SCALE: u32 = 4;

/// The compositor's current geometry. Lease holder only.
pub fn compositor_info() -> Result<CompositorInfo, Error> {
    // SAFETY: argument-free call.
    check(unsafe { syscall::graphics_compositor_info() }).map(CompositorInfo::unpack)
}

/// Set the integer display scale (1 to [`MAX_DISPLAY_SCALE`]). The surface
/// becomes the framebuffer divided by `scale` and starts black, so redraw
/// everything at the returned geometry. Lease holder only; a new holder
/// always starts at scale 1.
pub fn compositor_set_scale(scale: u32) -> Result<CompositorInfo, Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::graphics_compositor_scale(u64::from(scale)) })
        .map(CompositorInfo::unpack)
}

/// What ended a [`FrameClock::wait`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameEvent {
    /// The next frame is due. `skipped` counts whole frames that went by
    /// unrendered because the caller ran late; an animation advances by
    /// `skipped + 1` steps to stay on time.
    Frame { skipped: u64 },
    /// A [`wait_any`] source became ready before the frame was due.
    Ready(u64),
}

/// Paces an animation at a fixed period on the kernel timer.
///
/// Frame boundaries are absolute (`start + n * period`), so lateness in one
/// frame does not push back every later one, and no frame is ever early.
/// There is no vertical-sync source yet, so frames are not aligned to the
/// display's refresh; at the 100 Hz timer the shortest period is 10 ms.
pub struct FrameClock {
    period: time::Ticks,
    next: time::Ticks,
}

impl FrameClock {
    /// Start a clock whose first frame is due one `period` from now. A
    /// period of 0 is treated as 1 tick.
    pub fn new(period: time::Ticks) -> Self {
        let period = period.max(1);
        Self {
            period,
            next: ticks().saturating_add(period),
        }
    }

    pub const fn period(&self) -> time::Ticks {
        self.period
    }

    /// Tick at which the next frame is due.
    pub const fn deadline(&self) -> time::Ticks {
        self.next
    }

    /// Sleep until the next frame is due, or until a source in `mask` is
    /// ready (`mask` 0 waits for the frame alone). A ready source does not
    /// consume the frame: call again to keep waiting for it.
    pub fn wait(&mut self, mask: u64) -> Result<FrameEvent, Error> {
        loop {
            let now = ticks();
            if now >= self.next {
                let skipped = (now - self.next) / self.period;
                self.next = self
                    .next
                    .saturating_add(self.period.saturating_mul(skipped + 1));
                return Ok(FrameEvent::Frame { skipped });
            }
            if mask == 0 {
                // Relative sleep from a tick read earlier can only overshoot,
                // never wake before the deadline.
                sleep(self.next - now)?;
            } else {
                let ready = wait_any(mask, Some(self.next))?;
                if ready != 0 {
                    return Ok(FrameEvent::Ready(ready));
                }
            }
        }
    }
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

/// An input event delivered to the focused process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEvent {
    Key(Key),
    /// Relative motion; Y grows downwards.
    PointerMove {
        dx: i16,
        dy: i16,
    },
    /// Button 0 is left, 1 right, 2 middle.
    PointerButton {
        button: u8,
        pressed: bool,
    },
}

/// Take the next input event of any kind, if any. Same authority as
/// [`input_poll`].
pub fn input_poll_event() -> Result<Option<InputEvent>, Error> {
    // SAFETY: argument-free call.
    decode_event(unsafe { syscall::input_poll() })
}

/// Take the next key, if any, discarding pointer events ahead of it.
///
/// Requires the `InputIo` capability and input focus, which the kernel grants
/// to the compositor lease holder while it owns the display. Returns
/// `Ok(None)` when no key is pending; the call never blocks.
pub fn input_poll() -> Result<Option<Key>, Error> {
    loop {
        match input_poll_event()? {
            None => return Ok(None),
            Some(InputEvent::Key(key)) => return Ok(Some(key)),
            Some(_) => {}
        }
    }
}

/// Sleep until the next input event, or until the timer reaches `deadline`
/// (`None` waits indefinitely). Returns `Ok(None)` on timeout.
///
/// Same authority as [`input_poll`]. Only one task of a process may wait at a
/// time; a second concurrent waiter is refused.
pub fn input_wait(deadline: Option<time::Ticks>) -> Result<Option<Key>, Error> {
    loop {
        // SAFETY: integer-only call.
        match decode_event(unsafe { syscall::input_wait(deadline.unwrap_or(u64::MAX)) })? {
            None => return Ok(None),
            Some(InputEvent::Key(key)) => return Ok(Some(key)),
            Some(_) => {}
        }
    }
}

fn decode_event(result: u64) -> Result<Option<InputEvent>, Error> {
    if result == syscall::WOULD_BLOCK {
        return Ok(None);
    }
    let value = check(result)?;
    let payload = value & 0x00ff_ffff_ffff_ffff;
    Ok(Some(match value >> 56 {
        1 => InputEvent::Key(Key::Char(
            char::from_u32(payload as u32).ok_or(Error::Failed)?,
        )),
        2 => InputEvent::Key(Key::Enter),
        3 => InputEvent::Key(Key::Backspace),
        4 => InputEvent::Key(Key::Tab),
        5 => InputEvent::Key(Key::F1),
        6 => InputEvent::PointerMove {
            dx: payload as u16 as i16,
            dy: (payload >> 16) as u16 as i16,
        },
        7 => InputEvent::PointerButton {
            button: payload as u8,
            pressed: (payload >> 8) & 1 != 0,
        },
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

/// Runtime authorities that may be requested from the permission broker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Permission {
    Camera = 0,
    Microphone = 1,
    Network = 2,
    FilesystemRead = 3,
    FilesystemWrite = 4,
    Gpu = 5,
    Usb = 6,
    Clipboard = 7,
    Location = 8,
    Processes = 9,
    Ipc = 10,
    Storage = 11,
    Display = 12,
    Input = 13,
    Ai = 14,
    Secrets = 15,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Resolving,
    Granted,
    Denied,
    Revoked,
}

/// Ask the broker for one authority. `reason` is an application-defined
/// stable digest; user-facing brokers can resolve it to localized text.
pub fn permission_request(permission: Permission, reason: u64) -> Result<u64, Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_REQUEST, permission as u64, reason, 0) })
}

pub fn permission_poll(handle: u64) -> Result<PermissionState, Error> {
    match check(unsafe { syscall::invoke(syscall::PERMISSION_POLL, handle, 0, 0) })? {
        0 => Ok(PermissionState::Pending),
        1 => Ok(PermissionState::Resolving),
        2 => Ok(PermissionState::Granted),
        3 => Ok(PermissionState::Denied),
        4 => Ok(PermissionState::Revoked),
        _ => Err(Error::Failed),
    }
}

/// Resolve a request. The kernel accepts this only from an authorized policy
/// service holding `TaskControl`.
pub fn permission_decide(handle: u64, allow: bool) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_DECIDE, handle, u64::from(allow), 0) })
        .map(|_| ())
}

pub fn permission_revoke(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::PERMISSION_REVOKE, handle, 0, 0) }).map(|_| ())
}

/// Store a bounded secret in the caller-owned volatile WovenVault and return a
/// generation-tagged handle. The input is copied before the syscall returns.
pub fn secret_store(secret: &[u8]) -> Result<u64, Error> {
    check(unsafe {
        syscall::invoke(
            syscall::SECRET_STORE,
            secret.as_ptr() as u64,
            secret.len() as u64,
            0,
        )
    })
}

/// Copy a secret into a caller-owned buffer. The handle never exposes a kernel
/// pointer and is invalidated when the owner exits or revokes it.
pub fn secret_read(handle: u64, output: &mut [u8]) -> Result<usize, Error> {
    let length = check(unsafe {
        syscall::invoke(
            syscall::SECRET_READ,
            handle,
            output.as_mut_ptr() as u64,
            output.len() as u64,
        )
    })? as usize;
    if length > output.len() {
        return Err(Error::Failed);
    }
    Ok(length)
}

pub fn secret_revoke(handle: u64) -> Result<(), Error> {
    check(unsafe { syscall::invoke(syscall::SECRET_REVOKE, handle, 0, 0) }).map(|_| ())
}

fn check(value: u64) -> Result<u64, Error> {
    match value {
        syscall::ERROR => Err(Error::Failed),
        syscall::WOULD_BLOCK => Err(Error::WouldBlock),
        other => Ok(other),
    }
}

/// Read one retained security-ledger record, newest first. The caller needs
/// the kernel's `TaskInspect` authority; no user pointer is retained.
pub fn audit_read(offset: u64) -> Result<Option<security::AuditEvent>, Error> {
    let mut bytes = [0_u8; 48];
    let result = check(unsafe {
        syscall::invoke(syscall::AUDIT_READ, offset, bytes.as_mut_ptr() as u64, 0)
    })?;
    if result == 0 {
        return Ok(None);
    }
    if result != 1 {
        return Err(Error::Failed);
    }
    let action = match bytes[18] {
        0 => security::AuditAction::CapabilityGrant,
        1 => security::AuditAction::CapabilityRevoke,
        2 => security::AuditAction::IpcSend,
        3 => security::AuditAction::FileWrite,
        4 => security::AuditAction::ProcessFault,
        5 => security::AuditAction::ProcessExec,
        6 => security::AuditAction::ProcessFork,
        7 => security::AuditAction::WovenGuardAllow,
        8 => security::AuditAction::WovenGuardDeny,
        9 => security::AuditAction::CapabilityLineageIssue,
        10 => security::AuditAction::CapabilityDerived,
        11 => security::AuditAction::CapabilityLineageRelease,
        12 => security::AuditAction::CapabilityLineageRevoke,
        13 => security::AuditAction::PermissionRequest,
        14 => security::AuditAction::PermissionDecision,
        15 => security::AuditAction::PermissionRevoke,
        16 => security::AuditAction::SandboxProfileBind,
        17 => security::AuditAction::SandboxProfileDeny,
        18 => security::AuditAction::SandboxServicePublish,
        19 => security::AuditAction::SandboxServiceDiscover,
        20 => security::AuditAction::SandboxServiceUse,
        21 => security::AuditAction::SandboxFileRead,
        22 => security::AuditAction::SandboxFileWrite,
        23 => security::AuditAction::SandboxDeviceAccess,
        24 => security::AuditAction::SecretStore,
        25 => security::AuditAction::SecretRevoke,
        _ => return Err(Error::Failed),
    };
    Ok(Some(security::AuditEvent {
        sequence: u64::from_le_bytes(bytes[0..8].try_into().unwrap_or([0; 8])),
        tick: u64::from_le_bytes(bytes[8..16].try_into().unwrap_or([0; 8])),
        cpu: u16::from_le_bytes(bytes[16..18].try_into().unwrap_or([0; 2])),
        action,
        actor: u64::from_le_bytes(bytes[20..28].try_into().unwrap_or([0; 8])),
        target: u64::from_le_bytes(bytes[28..36].try_into().unwrap_or([0; 8])),
        detail: u64::from_le_bytes(bytes[36..44].try_into().unwrap_or([0; 8])),
        allowed: bytes[19] != 0,
    }))
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

/// Open a file, returning its descriptor. Requires `FileRead`.
pub fn open(path: &str) -> Result<u64, Error> {
    // SAFETY: a str is readable for its own length.
    check(unsafe { syscall::open(path.as_ptr(), path.len(), 0) })
}

/// Close a descriptor.
pub fn close(descriptor: u64) -> Result<(), Error> {
    // SAFETY: integer-only call.
    check(unsafe { syscall::close(descriptor) }).map(|_| ())
}

/// The longest name a directory entry can carry.
pub const MAX_NAME: usize = 255;

/// Entry `index` of directory `path`: its name, copied into `name`, and
/// whether it is itself a directory. Fails past the last entry. Requires
/// `FileRead`.
pub fn read_dir(
    path: &str,
    index: usize,
    name: &mut [u8; MAX_NAME],
) -> Result<(usize, bool), Error> {
    // SAFETY: the path is readable for its length, and `name` is writable
    // for the longest name the kernel copies.
    let value =
        check(unsafe { syscall::readdir(path.as_ptr(), path.len(), index, name.as_mut_ptr()) })?;
    Ok(((value & 0xff) as usize, (value >> 8) & 0xff == 1))
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

// Host unit tests for the parts of the API that are pure computation. The
// crate is freestanding everywhere else; under `cargo test` it links std so
// these run on the build machine.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_is_exact_at_both_ends_and_rounds_to_nearest() {
        let (over, under) = (rgb(200, 100, 0), rgb(0, 50, 255));
        assert_eq!(blend(over, under, 0), under);
        assert_eq!(blend(over, under, 255), over);
        // Per channel: (over * a + under * (255 - a) + 127) / 255.
        let mid = blend(over, under, 128);
        assert_eq!(mid >> 16, (200 * 128 + 127) / 255);
        assert_eq!(mid >> 8 & 0xff, (100 * 128 + 50 * 127 + 127) / 255);
        assert_eq!(mid & 0xff, (255 * 127 + 127) / 255);
        for opacity in [1, 77, 128, 254] {
            assert_eq!(blend(over, over, opacity), over, "a colour over itself");
        }
    }

    #[test]
    fn canvas_needs_a_buffer_large_enough_for_its_rectangle() {
        let mut pixels = [0u32; 12];
        assert!(Canvas::new(&mut pixels, 0, 0, 4, 3).is_some());
        assert!(Canvas::new(&mut pixels, 0, 0, 4, 4).is_none());
        assert!(
            Canvas::new(&mut pixels, 0, 0, usize::MAX, 2).is_none(),
            "overflow refused"
        );
        let canvas = Canvas::new(&mut pixels, 0, 0, 2, 2).unwrap();
        assert_eq!(canvas.pixels().len(), 4, "only the rectangle is exposed");
    }

    #[test]
    fn canvas_uses_caller_coordinates_and_clips_to_its_rectangle() {
        let mut pixels = [0u32; 16];
        let mut canvas = Canvas::new(&mut pixels, 100, 50, 4, 4).unwrap();
        canvas.set(101, 52, 0xffab_cdef);
        assert_eq!(canvas.get(101, 52), Some(0x00ab_cdef), "high byte cleared");
        // Outside the rectangle: ignored on write, absent on read.
        for (x, y) in [
            (99, 50),
            (104, 50),
            (100, 49),
            (100, 54),
            (i32::MIN, i32::MAX),
        ] {
            canvas.set(x, y, 1);
            assert_eq!(canvas.get(x, y), None);
        }
        assert_eq!(
            canvas.pixels().iter().filter(|pixel| **pixel != 0).count(),
            1
        );
        assert_eq!(
            pixels[2 * 4 + 1],
            0x00ab_cdef,
            "row-major from the top-left"
        );
    }

    #[test]
    fn fill_and_blend_rect_touch_only_the_overlap() {
        let mut pixels = [0u32; 16];
        let mut canvas = Canvas::new(&mut pixels, 10, 10, 4, 4).unwrap();
        canvas.fill_rect(0, 0, 100, 100, rgb(9, 9, 9));
        assert!(canvas.pixels().iter().all(|pixel| *pixel == rgb(9, 9, 9)));
        // Hanging off the bottom-right corner: a 2x2 overlap.
        canvas.fill_rect(12, 12, 10, 10, rgb(1, 2, 3));
        let changed = canvas
            .pixels()
            .iter()
            .filter(|pixel| **pixel == rgb(1, 2, 3))
            .count();
        assert_eq!(changed, 4);
        assert_eq!(canvas.get(11, 11), Some(rgb(9, 9, 9)));
        // Entirely outside, or empty: nothing.
        canvas.fill_rect(20, 20, 5, 5, 0);
        canvas.fill_rect(10, 10, 0, 5, 0);
        assert!(canvas.pixels().iter().all(|pixel| *pixel != 0));
        // Blending is the formula over what is there.
        canvas.blend_rect(10, 10, 1, 1, rgb(255, 255, 255), 100);
        assert_eq!(
            canvas.get(10, 10),
            Some(blend(rgb(255, 255, 255), rgb(9, 9, 9), 100))
        );
        assert_eq!(canvas.get(11, 10), Some(rgb(9, 9, 9)));
    }

    #[test]
    fn shade_passes_caller_coordinates() {
        let mut pixels = [0u32; 6];
        let mut canvas = Canvas::new(&mut pixels, -1, 7, 3, 2).unwrap();
        canvas.shade(|x, y| ((x + 1) as u32) << 8 | y as u32);
        assert_eq!(canvas.get(-1, 7), Some(7));
        assert_eq!(canvas.get(1, 8), Some(2 << 8 | 8));
    }

    #[test]
    fn draw_text_renders_the_kernel_font_and_honours_newlines() {
        let (width, height) = (16usize, 20usize);
        let mut pixels = vec![0u32; width * height];
        let mut canvas = Canvas::new(&mut pixels, 0, 0, width, height).unwrap();
        canvas.draw_text(1, 2, "Ab\nc", 7);
        let lit = |canvas: &Canvas<'_>, x: i32, y: i32| canvas.get(x, y) == Some(7);
        for (character, x0, y0) in [
            ('A', 1, 2),
            ('b', 1 + font::ADVANCE, 2),
            ('c', 1, 2 + font::LINE_HEIGHT),
        ] {
            for (row, bits) in font::glyph(character).iter().enumerate() {
                for column in 0..font::WIDTH as i32 {
                    let set = bits & (1 << (font::WIDTH as i32 - 1 - column)) != 0;
                    assert_eq!(
                        lit(&canvas, x0 + column, y0 + row as i32),
                        set,
                        "{character} ({column}, {row})"
                    );
                }
            }
        }
        // The blank column between glyphs stays clear.
        assert!((2..2 + font::HEIGHT as i32).all(|y| !lit(&canvas, 1 + font::WIDTH as i32, y)));
    }

    #[test]
    fn region_packing_matches_the_kernel_rectangle_layout() {
        let region = Region {
            x: 0x0102,
            y: 0x0304,
            width: 0x0506,
            height: 0x0708,
        };
        assert_eq!(region.pack(), 0x0708_0506_0304_0102);
    }

    #[test]
    fn compositor_info_unpacks_the_kernel_packing() {
        // The kernel packs width | height << 24 | scale << 48.
        let packed = 640 | (400 << 24) | (2 << 48);
        assert_eq!(
            CompositorInfo::unpack(packed),
            CompositorInfo {
                width: 640,
                height: 400,
                scale: 2
            }
        );
        let largest = 1920 | (1200 << 24) | (4 << 48);
        assert_eq!(
            CompositorInfo::unpack(largest),
            CompositorInfo {
                width: 1920,
                height: 1200,
                scale: 4
            }
        );
    }

    #[test]
    fn surface_owner_reads_the_high_half_of_a_token() {
        // slot | generation << 8 | owner << 32, as the kernel packs it.
        let token = 3 | (0x00ab_cdef << 8) | (4242u64 << 32);
        assert_eq!(surface_owner(token), process::Id(4242));
        assert_eq!(surface_owner(0xffff_ffff), process::Id(0));
    }
}
