use core::{
    arch::{asm, global_asm},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use crate::config::{MAX_IO_SIZE, MAX_PATH_SIZE};

global_asm!(
    ".global wovenhat_syscall_entry",
    "wovenhat_syscall_entry:",
    "push rax",
    "push rbx",
    "push rcx",
    "push rdx",
    "push rsi",
    "push rdi",
    "push rbp",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rdi, [rsp + 112]",
    "mov rsi, [rsp + 72]",
    "mov rdx, [rsp + 80]",
    "mov rcx, [rsp + 88]",
    "mov r8, rsp",
    "mov r12, rsp",
    "and rsp, -16",
    "call wovenhat_syscall_dispatch",
    "mov rsp, r12",
    "mov [rsp + 112], rax",
    ".global wovenhat_syscall_resume",
    "wovenhat_syscall_resume:",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rbp",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",
    "pop rax",
    "iretq",
);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct UserForkFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl UserForkFrame {
    pub fn child_return(mut self) -> Self {
        self.rax = 0;
        self
    }
}

static LAST_COMPLETED: AtomicU64 = AtomicU64::new(u64::MAX);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static IO_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static YIELD_PENDING: AtomicBool = AtomicBool::new(false);
static ASYNC_ABI_COMPLETIONS: AtomicU64 = AtomicU64::new(0);

const IO_WRITE: u64 = 1 << 0;
const IO_OPEN: u64 = 1 << 1;
const IO_READ: u64 = 1 << 2;
const IO_CLOSE: u64 = 1 << 3;
const IO_MMAP: u64 = 1 << 4;
const IO_MUNMAP: u64 = 1 << 5;
const IO_FILE_WRITE: u64 = 1 << 6;
const IO_GETUID: u64 = 1 << 7;
const IO_GETGID: u64 = 1 << 8;
const IO_EXEC: u64 = 1 << 9;
const IO_FORK: u64 = 1 << 10;
const IO_STDIN: u64 = 1 << 11;
const IO_STDERR: u64 = 1 << 12;

const ASYNC_CREATE: u64 = 1 << 0;
const ASYNC_POLL: u64 = 1 << 1;
const ASYNC_WAIT: u64 = 1 << 2;
const ASYNC_CANCEL: u64 = 1 << 3;
const ASYNC_COMPLETE: u64 = 1 << 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Number {
    Read = 0,
    Write = 1,
    Open = 2,
    Exit = 3,
    Getpid = 4,
    Waitpid = 5,
    Close = 6,
    Yield = 7,
    Mmap = 8,
    Munmap = 9,
    FileWrite = 10,
    MessageSend = 11,
    MessageReceive = 12,
    Getuid = 13,
    Getgid = 14,
    Exec = 15,
    Fork = 16,
    /// path ptr, path len → packs kind|size|writable into return value
    Stat = 17,
    /// path ptr, path len, entry index → writes DirEntry to user buffer in RSI...
    /// Actually: arg0=path, arg1=path_len, arg2=index; entry copied via a side channel is awkward.
    /// Convention: arg0=path ptr, arg1=path len | (index<<32), arg2=user buffer for name (MAX_DIR_NAME).
    /// Return: name length | (kind<<16), or error.
    Readdir = 18,
    /// path ptr, path len
    Mkdir = 19,
    /// path ptr, path len
    Chdir = 20,
    /// user buffer, capacity → length written
    Getcwd = 21,
    Dup = 22,
    /// returns read_fd in low 32 bits, write_fd in high 32 bits
    Pipe = 23,
    /// oldfd, newfd
    Dup2 = 24,
    Getppid = 25,
    /// pid, sig — minimal: record pending signal on target
    Kill = 26,
    Lseek = 27,
    Unlink = 28,
    Sleep = 29,
    Rename = 30,
    Getticks = 31,
    Sync = 32,
    Ioctl = 33,
    /// sig, handler (0=default, 1=ignore, otherwise userspace address)
    Sigaction = 34,
    Getpgrp = 35,
    /// pid, pgid (0 follows POSIX current/default conventions)
    Setpgid = 36,
    /// kind: 1=UDP, 2=TCP -> process-owned network descriptor
    Socket = 37,
    /// socket, local port
    Bind = 38,
    /// socket, packed IPv4 endpoint
    Connect = 39,
    /// socket, user buffer, length
    NetSend = 40,
    /// socket, user buffer, capacity
    NetRecv = 41,
    NetClose = 42,
    /// user pointer to network::NetInfo
    NetInfo = 43,
    /// hostname pointer, length -> query id
    DnsStart = 44,
    /// query id, user IPv4[4] buffer -> 0 pending, 1 complete
    DnsPoll = 45,
    /// socket -> packed peer endpoint
    NetPeer = 46,
    /// 0=static fallback, nonzero=enable DHCP
    Dhcp = 47,
    /// packed IPv4 in low 32 bits
    PingStart = 48,
    /// 0 pending, otherwise RTT ticks + 1
    PingPoll = 49,
    /// command-line pointer + byte length; parses argv and resolves bare names via /bin
    ExecCommand = 50,
    /// key ptr, key len, value buffer ptr (MAX_ENV_ENTRY bytes) -> value length
    EnvGet = 51,
    /// key ptr, packed key/value lengths, value ptr -> 0
    EnvSet = 52,
    /// number of environment entries
    EnvCount = 53,
    /// index, output ptr, capacity -> entry length (KEY=VALUE)
    EnvEntry = 54,
    /// number of live/zombie process-table entries
    ProcessCount = 55,
    /// index, user ProcessInfo pointer -> 0
    ProcessInfo = 56,
    /// command-line pointer + byte length -> spawn child, return pid
    SpawnCommand = 57,
    /// fd, byte length, page-aligned file offset: read-only private snapshot.
    MmapFile = 58,
    /// fd, byte length, page-aligned file offset: writable private snapshot.
    MmapFileWritable = 59,
    MmapFileLazy = 60,
    MmapFileLazyWritable = 61,
    MmapFileShared = 62,
    Msync = 63,
    /// class -> generation-tagged owner-bound async handle
    AsyncCreate = 64,
    /// handle, user Completion* -> 0 pending, 1 completed/consumed
    AsyncPoll = 65,
    /// handle, user Completion* -> blocks event-driven, returns 1 on copyout
    AsyncWait = 66,
    /// handle -> release pending/completed operation without waiting
    AsyncCancel = 67,
    /// service handle, status, value -> complete an owner-managed service event
    AsyncComplete = 68,
    /// LBA -> generation-tagged Block handle backed by the real block worker
    AsyncBlockRead = 69,
    /// LBA, userspace 512-byte sector -> generation-tagged Block handle
    AsyncBlockWrite = 70,
    /// Block handle, user BlockCompletion* -> 0 pending, 1 copied/consumed
    AsyncBlockPoll = 71,
    /// Block handle, user BlockCompletion* -> blocks event-driven, 1 copied/consumed
    AsyncBlockWait = 72,
    /// fd, user AsyncFileRequest* -> positional read handle
    AsyncFileRead = 73,
    /// fd, user AsyncFileRequest* -> positional write handle; payload copied at submit
    AsyncFileWrite = 74,
    /// File handle, user Completion*, user data* -> 0 pending, 1 copied/consumed
    AsyncFilePoll = 75,
    /// File handle, user Completion*, user data* -> event wait, 1 copied/consumed
    AsyncFileWait = 76,
    /// socket, user buffer, length -> generation-tagged Network send handle
    AsyncNetSend = 77,
    /// socket, capacity -> generation-tagged Network receive handle
    AsyncNetRecv = 78,
    /// Network handle, user Completion*, user data* -> 0 pending, 1 copied/consumed
    AsyncNetPoll = 79,
    /// Network handle, user Completion*, user data* -> event wait, 1 copied/consumed
    AsyncNetWait = 80,
    /// TCP socket, packed IPv4 endpoint -> generation-tagged Network connect handle
    AsyncTcpConnect = 81,
    CompletionPortCreate = 82,
    CompletionPortClose = 83,
    CompletionPortAssociate = 84,
    CompletionPortPoll = 85,
    CompletionPortWait = 86,
    TimerCreate = 87,
    TimerWait = 88,
    EventCreate = 89,
    EventSet = 90,
    EventWait = 91,
    TimerClose = 92,
    EventClose = 93,
    SleepUntil = 94,
    /// user notification buffer -> 0 empty, 1 record copied
    NotificationPoll = 95,
    /// kind, payload -> posts a user notification to the current process
    NotificationPost = 96,
    /// query id -> cancel/reclaim pending DNS query
    DnsCancel = 97,
    /// width, height -> packed handle, or ERROR
    GraphicsSurfaceCreate = 98,
    /// handle, packed (x | y<<16 | w<<32 | h<<48), packed colour -> 0 or ERROR
    GraphicsSurfaceFill = 99,
    /// handle, packed (x | y<<32) -> packed colour, or ERROR
    GraphicsSurfacePixel = 100,
    /// handle -> 0 or ERROR
    GraphicsSurfaceDestroy = 101,
    /// handle, packed (x | y<<32) -> 0 or ERROR
    GraphicsCompositorBlit = 102,
    /// packed (x | y<<32) -> packed colour, or ERROR
    GraphicsCompositorPixel = 103,
    /// handle, packed (x | y<<16 | w<<32 | h<<48), packed colour -> 0 or ERROR
    GraphicsSurfaceStroke = 104,
    /// -> 0 when the caller holds the compositor lease, or ERROR
    GraphicsCompositorClaim = 105,
    /// -> pixels written to the framebuffer, or ERROR
    GraphicsCompositorPresent = 106,
    /// -> packed event (kind << 56 | value), WOULD_BLOCK when none, or ERROR
    InputPoll = 107,
    /// deadline tick (u64::MAX = none) -> packed event, WOULD_BLOCK on
    /// timeout, or ERROR
    InputWait = 108,
    /// own handle, grantee pid (0 revokes) -> 0 or ERROR
    GraphicsSurfaceShare = 109,
    /// owner's handle, packed (x | y<<32) -> 0 or ERROR
    GraphicsCompositorBlitShared = 110,
    /// deadline tick (u64::MAX = none), source mask (1 input, 2 message,
    /// 4 shared-surface damage) -> mask of ready sources, 0 on timeout, or
    /// ERROR
    WaitAny = 111,
    /// own handle, packed rect (x | y<<16 | w<<32 | h<<48) -> 0 or ERROR
    GraphicsSurfaceDamage = 112,
    /// owner's handle, packed window origin (x | y<<32) -> packed source
    /// rect blitted (x | y<<16 | w<<32 | h<<48), 0 when nothing was pending,
    /// or ERROR
    GraphicsCompositorBlitDamage = 113,
    /// own handle, packed (x | y<<32), opacity 0..=255 -> 0 or ERROR
    GraphicsCompositorBlend = 114,
    /// owner's handle, packed (x | y<<32), opacity 0..=255 -> 0 or ERROR
    GraphicsCompositorBlendShared = 115,
    /// () -> packed (width | height<<24 | scale<<48) or ERROR
    GraphicsCompositorInfo = 116,
    /// scale 1..=4 -> packed (width | height<<24 | scale<<48) or ERROR
    GraphicsCompositorScale = 117,
    /// own handle, packed rect (x | y<<16 | w<<32 | h<<48), user pointer to
    /// w*h little-endian 0x00RRGGBB pixels -> 0 or ERROR
    GraphicsSurfaceWrite = 118,
    /// () -> packed UTC date-time (second | minute<<8 | hour<<16 | day<<24 |
    /// month<<32 | year<<40) or ERROR
    ClockRead = 119,
    /// own handle, packed destination (x | y<<32), packed source region
    /// (x | y<<16 | w<<32 | h<<48) -> 0 or ERROR
    GraphicsCompositorBlitRegion = 120,
    /// owner's handle, packed destination, packed source region -> 0 or ERROR
    GraphicsCompositorBlitRegionShared = 121,
    PermissionRequest = 122,
    PermissionPoll = 123,
    PermissionDecide = 124,
    PermissionRevoke = 125,
    /// newest-relative ledger offset, user AuditEvent buffer -> 1, 0, or ERROR
    AuditRead = 126,
    /// user secret pointer, byte length -> generation-tagged secret handle
    SecretStore = 127,
    /// secret handle, output pointer, output capacity -> secret length or ERROR
    SecretRead = 128,
    /// secret handle -> 0 or ERROR
    SecretRevoke = 129,
    /// entry address, stack size -> generation-tagged thread handle
    ThreadCreate = 130,
    /// exit status; never returns for a non-leader thread
    ThreadExit = 131,
    /// thread handle -> exit status, WOULD_BLOCK while live, or ERROR
    ThreadJoin = 132,
    /// -> current thread handle
    ThreadSelf = 133,
    /// TLS base value -> 0 or ERROR
    ThreadSetTls = 134,
    /// -> TLS base value or ERROR
    ThreadGetTls = 135,
    /// thread handle, exit status -> 0 or ERROR; cancels an owned sibling
    ThreadCancel = 136,
    /// Internal ELF PLT resolver: the entry stub supplies the GOT slot in R11
    /// and receives the validated target address in RAX.
    LazyBind = 137,
    /// own handle, packed destination and opacity (x | y<<24 | opacity<<56,
    /// x and y 24-bit signed, bits 48..56 zero), packed source region -> 0 or
    /// ERROR
    GraphicsCompositorBlendRegion = 200,
    /// owner's handle, packed destination and opacity, packed source region
    /// -> 0 or ERROR
    GraphicsCompositorBlendRegionShared = 201,
    /// own handle, packed destination (x | y<<32), packed source region ->
    /// 0 or ERROR; source pixels equal to the transparent key are skipped
    GraphicsCompositorBlitKeyed = 202,
    /// out *AudioInfo (sample_rate u32, max_bytes u32, playback u8, capture
    /// u8, channels u8, bits u8) -> 1, or ERROR if there is no audio output
    AudioInfo = 203,
    /// pcm, length (48 kHz 16-bit stereo, whole frames, <= max_bytes) ->
    /// bytes played once playback has finished; needs AudioPlayback
    AudioPlay = 204,
    /// volume percent (0 mutes) -> the amplifier gain set; needs
    /// AudioPlayback
    AudioSetVolume = 205,
    /// deadline, user array of `{object, kind, mask}` records, count -> ready
    /// mask or 0 on timeout. `kind` is 0 for timer and 1 for event.
    WaitAnyObjects = 206,
    /// out *[x u32, y u32, width u32, height u32; 4] -> the number of
    /// monitors, left to right in surface coordinates. Requires DisplayIo and
    /// the lease. (208 is reserved for audio recording.)
    GraphicsCompositorOutputs = 207,
    /// action (0 power off, 1 restart, 2 suspend) -> does not return when it
    /// works; an error code says why the machine is still running or why
    /// suspend is not yet safely resumable. Needs PowerControl.
    PowerControl = 209,
    /// user buffer packing `[name_len u8][name][old_pin_len u8][old_pin]
    /// [new_pin_len u8][new_pin]` (each part <= 16 bytes), buffer length ->
    /// 0, or ERROR if `name` has no account, `old_pin` does not verify
    /// against it, `new_pin` is empty or too long, or the caller lacks
    /// AccountManage (the desktop session only).
    AccountsChangePin = 210,
    /// output `[power_off, suspend, restart]` capability bytes, length -> 0.
    /// Requires PowerControl; no transition is performed.
    PowerCapabilities = 235,
    /// output `[hlt, monitor_mwait, invariant_tsc]` capability bytes, length -> 0.
    /// Requires PowerControl; no CPU state is changed.
    CpuIdleCapabilities = 236,
    /// output `[available, base_mhz u32, maximum_mhz u32, bus_mhz u32]`, -> 0.
    /// Requires PowerControl; no frequency state is changed.
    CpuFrequencyInfo = 237,
    /// output `[s3, facs, wake32, wake64, facs_version]`, length -> 0.
    /// Requires PowerControl; no suspend state is changed.
    SuspendResumeCapabilities = 238,
    /// -> process-local endpoint handle, or ERROR
    IpcEndpointCreate = 211,
    /// name ptr, packed name length|client rights<<32, endpoint handle -> 0
    IpcServicePublish = 212,
    /// name ptr, name length -> 0
    IpcServiceUnpublish = 213,
    /// name ptr, name length -> process-local client handle, or ERROR
    IpcServiceDiscover = 214,
    /// endpoint handle, message ptr, message length -> 0
    IpcMessageSend = 215,
    /// endpoint handle, output buffer, capacity -> message length; the output
    /// starts with the sender process id as a little-endian u64.
    IpcMessageReceive = 216,
    /// operation, source/name path pointer, path length -> catalog count
    #[cfg(any(feature = "stage22-5-test", feature = "system-apps"))]
    PackageTransaction = 217,
    /// endpoint handle, bounded WAI1 request pointer, request length -> 0;
    /// requires both Ipc and Ai capability
    #[cfg(any(
        feature = "stage23-1-test",
        feature = "stage23-2-test",
        feature = "stage23-3-test",
        feature = "system-apps"
    ))]
    AiMessageSend = 218,
    /// identity open buffer `[name_len][name][password_len][password]` -> session
    #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
    IdentityOpen = 219,
    /// session -> 0; owner-bound lock
    #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
    IdentityLock = 220,
    /// session, password buffer `[length][password]`, length -> 0
    #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
    IdentityUnlock = 221,
    /// session, password buffer `[length][password]`, length -> 0
    #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
    IdentityElevate = 222,
    /// session -> 0; owner-bound close
    #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
    IdentityClose = 223,
    #[cfg(feature = "system-apps")]
    IdentityOpenVerified = 224,
    #[cfg(feature = "system-apps")]
    IdentityUnlockVerified = 225,
    #[cfg(feature = "system-apps")]
    IdentityAdminAddUser = 226,
    #[cfg(feature = "system-apps")]
    IdentityAdminDeleteUser = 227,
    #[cfg(feature = "system-apps")]
    IdentityAdminAddGroup = 228,
    #[cfg(feature = "system-apps")]
    IdentityAdminAddMember = 229,
    #[cfg(feature = "system-apps")]
    IdentityAdminRemoveMember = 230,
    #[cfg(feature = "system-apps")]
    IdentityAdminQuery = 231,
    #[cfg(feature = "system-apps")]
    IdentityAdminSetLockout = 232,
    /// session -> consume one kernel-published TPM credential decision
    #[cfg(feature = "system-apps")]
    IdentityConsumeHardware = 233,
    /// session -> request one physical TPM credential decision
    #[cfg(feature = "system-apps")]
    IdentityRequestHardware = 234,
}

pub fn entry_address() -> u64 {
    unsafe extern "C" {
        fn wovenhat_syscall_entry();
    }

    wovenhat_syscall_entry as *const () as u64
}

pub fn emit(number: Number) -> u64 {
    invoke(number, 0, 0, 0)
}

pub fn invoke(number: Number, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    // SAFETY: Vector 0x80 is a DPL3 interrupt gate whose assembly entry saves
    // all general registers and returns the dispatch result in RAX.
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") number as u64 => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            options(preserves_flags),
        );
    }

    result
}
fn fork_frame_layout_valid() -> bool {
    let frame = UserForkFrame {
        rax: 7,
        rip: 0x4000,
        rsp: 0x8000,
        ..UserForkFrame::default()
    };
    let child = frame.child_return();
    core::mem::size_of::<UserForkFrame>() == 20 * core::mem::size_of::<u64>()
        && child.rax == 0
        && child.rip == frame.rip
        && child.rsp == frame.rsp
}
pub fn test() -> bool {
    let before = COMPLETIONS.load(Ordering::Acquire);
    let result = emit(Number::Getpid);
    fork_frame_layout_valid()
        && COMPLETIONS.load(Ordering::Acquire) == before + 1
        && LAST_COMPLETED.load(Ordering::Acquire) == Number::Getpid as u64
        && result == crate::task::current_process_id()
}

pub fn user_io_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & (IO_WRITE | IO_OPEN | IO_READ | IO_CLOSE)
        == IO_WRITE | IO_OPEN | IO_READ | IO_CLOSE
}

pub fn user_standard_streams_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & (IO_STDIN | IO_STDERR) == IO_STDIN | IO_STDERR
}
pub fn user_memory_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & (IO_MMAP | IO_MUNMAP) == IO_MMAP | IO_MUNMAP
}

pub fn user_identity_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & (IO_GETUID | IO_GETGID) == IO_GETUID | IO_GETGID
}

pub fn user_exec_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & IO_EXEC == IO_EXEC
}
pub fn user_fork_verified() -> bool {
    IO_COMPLETIONS.load(Ordering::Acquire) & IO_FORK == IO_FORK
}

pub fn user_async_verified() -> bool {
    let required = ASYNC_CREATE | ASYNC_POLL | ASYNC_WAIT | ASYNC_CANCEL | ASYNC_COMPLETE;
    ASYNC_ABI_COMPLETIONS.load(Ordering::Acquire) & required == required
}
pub fn last_completed(number: Number) -> bool {
    LAST_COMPLETED.load(Ordering::Acquire) == number as u64
}

pub fn service_pending() {
    if YIELD_PENDING.swap(false, Ordering::AcqRel) {
        crate::task::yield_now();
    }
}

const SYSCALL_ERROR: u64 = u64::MAX;

fn sys_mmap(length: u64, flags: u64) -> u64 {
    if flags & !1 != 0 {
        return SYSCALL_ERROR;
    }
    match crate::task::mmap_current(length, flags & 1 != 0) {
        Ok(address) => {
            IO_COMPLETIONS.fetch_or(IO_MMAP, Ordering::Release);
            address
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_munmap(address: u64, length: u64) -> u64 {
    match crate::task::munmap_current(address, length) {
        Ok(()) => {
            IO_COMPLETIONS.fetch_or(IO_MUNMAP, Ordering::Release);
            0
        }
        Err(_) => SYSCALL_ERROR,
    }
}
fn sys_write(descriptor: u64, user_buffer: u64, length: u64) -> u64 {
    if !matches!(descriptor, 1 | 2) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0_u8; MAX_IO_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }

    // If stdout/stderr has been explicitly redirected with dup2(), honor the
    // process file table. Otherwise these descriptors remain the console.
    if crate::task::descriptor_is_open(descriptor) {
        let Ok(count) = crate::task::write_current(descriptor, &buffer[..length]) else {
            return SYSCALL_ERROR;
        };
        IO_COMPLETIONS.fetch_or(IO_WRITE, Ordering::Release);
        if descriptor == 2 {
            IO_COMPLETIONS.fetch_or(IO_STDERR, Ordering::Release);
        }
        return count as u64;
    }

    if !crate::task::current_has(crate::capability::Capability::Console) {
        return SYSCALL_ERROR;
    }
    crate::serial::write_bytes(&buffer[..length]);
    crate::terminal::write_bytes(&buffer[..length]);
    IO_COMPLETIONS.fetch_or(IO_WRITE, Ordering::Release);
    if descriptor == 2 {
        IO_COMPLETIONS.fetch_or(IO_STDERR, Ordering::Release);
    }
    length as u64
}

fn sys_open(user_path: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path[..length]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::open_current(path) {
        Ok(descriptor) => {
            IO_COMPLETIONS.fetch_or(IO_OPEN, Ordering::Release);
            descriptor
        }
        Err(_) => SYSCALL_ERROR,
    }
}

#[cfg(any(feature = "stage22-5-test", feature = "system-apps"))]
fn sys_package_transaction(operation: u64, user_path: u64, length: u64) -> u64 {
    if !matches!(operation, 1..=3)
        || !crate::task::current_has(crate::capability::Capability::FileRead)
        || !crate::task::current_has(crate::capability::Capability::FileWrite)
        || !crate::task::current_has(crate::capability::Capability::StorageIo)
    {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let Some(mut path) = crate::heap::try_filled(0u8, MAX_PATH_SIZE) else {
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_from_current_user(user_path, &mut path[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path[..length]) else {
        return SYSCALL_ERROR;
    };
    crate::package_service::transact(operation as u8, path, crate::timer::ticks())
        .unwrap_or(SYSCALL_ERROR)
}

/// `stat(path)` → packed result:
/// bits[7:0]   = kind (0=file, 1=directory)
/// bits[39:8]  = size (24 bits)
/// bit 40      = writable
fn sys_stat(user_path: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path_buf = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path_buf[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path_buf[..length]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::stat_path(path) {
        Ok(stat) => {
            let kind = match stat.kind {
                crate::vfs::NodeKind::File => 0u64,
                crate::vfs::NodeKind::Directory => 1u64,
            };
            let size = (stat.size as u64) & 0x00FF_FFFF;
            let writable = u64::from(stat.writable);
            kind | (size << 8) | (writable << 40)
        }
        Err(_) => SYSCALL_ERROR,
    }
}

/// `readdir(path, index, name_buf)`
/// arg0 = path pointer, arg1 = path length, arg2 = (index << 32) | name_buf_ptr low...
/// Simpler: arg0=path, arg1=path_len, arg2=index.
/// Name is written to a fixed side-buffer? No — use:
///   arg0 = path ptr
///   arg1 = path_len | (index << 16)
///   arg2 = user buffer for name (at least MAX_DIR_NAME bytes)
/// Return: name_length | (kind << 8) on success.
fn sys_readdir(user_path: u64, path_len_and_index: u64, user_name_buf: u64) -> u64 {
    let path_len = (path_len_and_index & 0xFFFF) as usize;
    let index = (path_len_and_index >> 16) as usize;
    if path_len == 0 || path_len > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path_buf = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path_buf[..path_len]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path_buf[..path_len]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::readdir_path(path, index) {
        Ok(entry) => {
            if crate::paging::copy_to_current_user(user_name_buf, &entry.name[..entry.name_length])
                .is_err()
            {
                return SYSCALL_ERROR;
            }
            let kind = match entry.kind {
                crate::vfs::NodeKind::File => 0u64,
                crate::vfs::NodeKind::Directory => 1u64,
            };
            (entry.name_length as u64) | (kind << 8)
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_chdir(user_path: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path_buf = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path_buf[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path_buf[..length]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::chdir_current(path) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_getcwd(user_buffer: u64, capacity: u64) -> u64 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    if capacity == 0 || capacity > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path_buf = [0_u8; MAX_PATH_SIZE];
    let Ok(len) = crate::task::current_cwd_str(&mut path_buf) else {
        return SYSCALL_ERROR;
    };
    if len > capacity {
        return SYSCALL_ERROR;
    }
    if crate::paging::copy_to_current_user(user_buffer, &path_buf[..len]).is_err() {
        return SYSCALL_ERROR;
    }
    len as u64
}

fn sys_mkdir(user_path: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path_buf = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path_buf[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path_buf[..length]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::mkdir_path(path) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_read(descriptor: u64, user_buffer: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0_u8; MAX_IO_SIZE];
    if descriptor == 0 && !crate::task::descriptor_is_open(0) {
        if !crate::task::current_has(crate::capability::Capability::Console) {
            return SYSCALL_ERROR;
        }
        let count = crate::keyboard::read_bytes(&mut buffer[..length]);
        if crate::paging::copy_to_current_user(user_buffer, &buffer[..count]).is_err() {
            return SYSCALL_ERROR;
        }
        IO_COMPLETIONS.fetch_or(IO_STDIN, Ordering::Release);
        return count as u64;
    }
    let Ok(count) = crate::task::read_current(descriptor, &mut buffer[..length]) else {
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_to_current_user(user_buffer, &buffer[..count]).is_err() {
        return SYSCALL_ERROR;
    }
    IO_COMPLETIONS.fetch_or(IO_READ, Ordering::Release);
    count as u64
}

fn sys_file_write(descriptor: u64, user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_process_id();
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0_u8; MAX_IO_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let result = crate::task::write_current(descriptor, &buffer[..length]);
    crate::audit::record(
        actor,
        crate::audit::Action::FileWrite,
        descriptor,
        result.is_ok(),
    );
    match result {
        Ok(count) => {
            IO_COMPLETIONS.fetch_or(IO_FILE_WRITE, Ordering::Release);
            count as u64
        }
        Err(_) => SYSCALL_ERROR,
    }
}
fn sys_message_send(receiver: u64, user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_process_id();
    if !crate::task::current_has(crate::capability::Capability::Ipc)
        || !crate::task::may_ipc_with(receiver)
    {
        crate::audit::record(actor, crate::audit::Action::IpcSend, receiver, false);
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > crate::ipc::MAX_MESSAGE_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0_u8; crate::ipc::MAX_MESSAGE_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let sender = crate::task::current_process_id();
    let result = crate::ipc::send(sender, receiver, &buffer[..length]);
    crate::audit::record(
        actor,
        crate::audit::Action::IpcSend,
        receiver,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |()| 0)
}
fn sys_message_receive(user_buffer: u64, capacity: u64, user_sender: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    let receiver = crate::task::current_process_id();
    // Peek, copy out with no lock held (the copy can fault), then dequeue
    // only that message. If another thread of this process dequeued first,
    // the copy is stale: take the next message instead.
    loop {
        let Ok((message, serial)) = crate::ipc::peek(receiver) else {
            return SYSCALL_ERROR;
        };
        if message.payload().len() > capacity
            || crate::paging::copy_to_current_user(user_buffer, message.payload()).is_err()
            || crate::paging::copy_to_current_user(user_sender, &message.sender.to_le_bytes())
                .is_err()
        {
            return SYSCALL_ERROR;
        }
        match crate::ipc::receive_if(receiver, serial) {
            Ok(_) => return message.payload().len() as u64,
            Err(crate::ipc::Error::QueueEmpty) => continue,
            Err(_) => return SYSCALL_ERROR,
        }
    }
}

fn copy_service_name(
    user_name: u64,
    length: u64,
) -> Option<([u8; crate::config::MAX_IPC_SERVICE_NAME], usize)> {
    let length = usize::try_from(length).ok()?;
    if length == 0 || length > crate::config::MAX_IPC_SERVICE_NAME {
        return None;
    }
    let mut name = [0u8; crate::config::MAX_IPC_SERVICE_NAME];
    crate::paging::copy_from_current_user(user_name, &mut name[..length]).ok()?;
    Some((name, length))
}

fn sys_ipc_endpoint_create() -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    crate::ipc::create_endpoint_object(crate::task::current_process_id())
        .map_or(SYSCALL_ERROR, |handle| u64::from(handle.as_u32()))
}

fn sys_ipc_service_publish(user_name: u64, packed: u64, raw_handle: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc)
        || raw_handle > u64::from(u32::MAX)
    {
        return SYSCALL_ERROR;
    }
    let name_length = packed & 0xffff_ffff;
    let client_rights = (packed >> 32) as u8;
    let Some((name, length)) = copy_service_name(user_name, name_length) else {
        return SYSCALL_ERROR;
    };
    crate::ipc::publish_service(
        crate::task::current_process_id(),
        &name[..length],
        crate::ipc::Handle::from_u32(raw_handle as u32),
        client_rights,
    )
    .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_ipc_service_unpublish(user_name: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Some((name, length)) = copy_service_name(user_name, length) else {
        return SYSCALL_ERROR;
    };
    crate::ipc::unpublish_service(crate::task::current_process_id(), &name[..length])
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_ipc_service_discover(user_name: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Some((name, length)) = copy_service_name(user_name, length) else {
        return SYSCALL_ERROR;
    };
    crate::ipc::discover_service(crate::task::current_process_id(), &name[..length])
        .map_or(SYSCALL_ERROR, |handle| u64::from(handle.as_u32()))
}

fn raw_ipc_handle(raw: u64) -> Option<crate::ipc::Handle> {
    u32::try_from(raw).ok().map(crate::ipc::Handle::from_u32)
}

fn sys_ipc_message_send(raw_handle: u64, user_buffer: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Some(handle) = raw_ipc_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > crate::ipc::MAX_MESSAGE_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; crate::ipc::MAX_MESSAGE_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    crate::ipc::send_handle(crate::task::current_process_id(), handle, &buffer[..length])
        .map_or(SYSCALL_ERROR, |()| 0)
}

#[cfg(any(
    feature = "stage23-1-test",
    feature = "stage23-2-test",
    feature = "stage23-3-test",
    feature = "system-apps"
))]
fn sys_ai_message_send(raw_handle: u64, user_buffer: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc)
        || !crate::task::current_has(crate::capability::Capability::Ai)
    {
        return SYSCALL_ERROR;
    }
    sys_ipc_message_send(raw_handle, user_buffer, length)
}

fn sys_ipc_message_receive(raw_handle: u64, user_buffer: u64, capacity: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Some(handle) = raw_ipc_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    if capacity < core::mem::size_of::<u64>()
        || capacity > crate::ipc::MAX_MESSAGE_SIZE + core::mem::size_of::<u64>()
    {
        return SYSCALL_ERROR;
    }
    let message = match crate::ipc::receive_handle(crate::task::current_process_id(), handle) {
        Ok(message) => message,
        Err(_) => return SYSCALL_ERROR,
    };
    let output_length = core::mem::size_of::<u64>().saturating_add(message.payload().len());
    if output_length > capacity {
        return SYSCALL_ERROR;
    }
    let mut output = [0u8; crate::ipc::MAX_MESSAGE_SIZE + core::mem::size_of::<u64>()];
    output[..8].copy_from_slice(&message.sender.to_le_bytes());
    output[8..output_length].copy_from_slice(message.payload());
    if crate::paging::copy_to_current_user(user_buffer, &output[..output_length]).is_err() {
        return SYSCALL_ERROR;
    }
    output_length as u64
}

fn sys_pipe() -> u64 {
    match crate::task::pipe_current() {
        Ok((r, w)) => r | (w << 32),
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_dup2(old: u64, new: u64) -> u64 {
    match crate::task::dup2_current(old, new) {
        Ok(fd) => fd,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_getppid() -> u64 {
    crate::task::getppid_current()
}

fn sys_lseek(fd: u64, offset: u64) -> u64 {
    match crate::task::seek_current(fd, offset) {
        Ok(pos) => pos,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_unlink(user_path: u64, length: u64) -> u64 {
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path[..length]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::unlink_current(path) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_rename(user_old: u64, old_len_and_new_len: u64, user_new: u64) -> u64 {
    let old_len = (old_len_and_new_len & 0xffff) as usize;
    let new_len = ((old_len_and_new_len >> 16) & 0xffff) as usize;
    if old_len == 0 || new_len == 0 || old_len > MAX_PATH_SIZE || new_len > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut old = [0_u8; MAX_PATH_SIZE];
    let mut new = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_old, &mut old[..old_len]).is_err() {
        return SYSCALL_ERROR;
    }
    if crate::paging::copy_from_current_user(user_new, &mut new[..new_len]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(old) = core::str::from_utf8(&old[..old_len]) else {
        return SYSCALL_ERROR;
    };
    let Ok(new) = core::str::from_utf8(&new[..new_len]) else {
        return SYSCALL_ERROR;
    };
    match crate::task::rename_current(old, new) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_getticks() -> u64 {
    crate::timer::ticks()
}

fn sys_sync() -> u64 {
    crate::storage::sync_all_mounted()
        .map(|count| count as u64)
        .unwrap_or(SYSCALL_ERROR)
}

fn sys_ioctl(_fd: u64, _req: u64, _arg: u64) -> u64 {
    0 // no-op placeholder for future TTY control
}

fn sys_sleep(ticks: u64) -> u64 {
    crate::task::sleep_current(ticks);
    0
}

fn sys_kill(pid: u64, sig: u64) -> u64 {
    match crate::task::kill_process_from_user(pid, sig) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_dup(descriptor: u64) -> u64 {
    match crate::task::dup_current(descriptor) {
        Ok(fd) => fd,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_close(descriptor: u64) -> u64 {
    match crate::task::close_current(descriptor) {
        Ok(()) => {
            IO_COMPLETIONS.fetch_or(IO_CLOSE, Ordering::Release);
            0
        }
        Err(_) => SYSCALL_ERROR,
    }
}
fn sys_fork(frame: *const UserForkFrame) -> u64 {
    let actor = crate::task::current_process_id();
    if !crate::task::current_has(crate::capability::Capability::ProcessCreate) {
        crate::audit::record(actor, crate::audit::Action::ProcessFork, 0, false);
        return SYSCALL_ERROR;
    }
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        crate::audit::record(actor, crate::audit::Action::ProcessFork, 0, false);
        return SYSCALL_ERROR;
    };
    match crate::task::fork_current(*frame) {
        Ok(child) => {
            IO_COMPLETIONS.fetch_or(IO_FORK, Ordering::Release);
            crate::audit::record(
                actor,
                crate::audit::Action::ProcessFork,
                child.as_u64(),
                true,
            );
            child.as_u64()
        }
        Err(_) => {
            crate::audit::record(actor, crate::audit::Action::ProcessFork, 0, false);
            SYSCALL_ERROR
        }
    }
}
fn sys_sigaction(sig: u64, handler: u64) -> u64 {
    crate::task::sigaction_current(sig, handler).unwrap_or(SYSCALL_ERROR)
}

fn sys_setpgid(pid: u64, pgid: u64) -> u64 {
    match crate::task::set_process_group(pid, pgid) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

/// Arrange a caught signal to enter its userspace handler after this syscall.
/// The handler receives `sig` in RDI and a normal `ret` resumes the interrupted RIP.
fn prepare_signal_delivery(frame: *const UserForkFrame) {
    let Some((sig, handler)) = crate::task::take_pending_signal_current() else {
        return;
    };
    if frame.is_null() {
        return;
    }
    // `sigaction` admits only executable user addresses; never return to a
    // non-user RIP, which would fault on `iretq` in ring 0.
    if crate::user_address::check_user_range(handler, 1).is_err() {
        return;
    }
    let frame = unsafe { &mut *(frame as *mut UserForkFrame) };
    let Some(new_rsp) = frame.rsp.checked_sub(8) else {
        return;
    };
    if crate::paging::copy_to_current_user(new_rsp, &frame.rip.to_le_bytes()).is_err() {
        return;
    }
    frame.rsp = new_rsp;
    frame.rip = handler;
    frame.rdi = sig;
}

fn sys_exec(user_path: u64, length: u64) -> u64 {
    let actor = crate::task::current_process_id();
    if !crate::task::current_has(crate::capability::Capability::FileRead)
        || !crate::task::current_has(crate::capability::Capability::ProcessCreate)
    {
        crate::audit::record(actor, crate::audit::Action::ProcessExec, length, false);
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut path = [0_u8; MAX_PATH_SIZE];
    if crate::paging::copy_from_current_user(user_path, &mut path[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(path) = core::str::from_utf8(&path[..length]) else {
        return SYSCALL_ERROR;
    };
    let Some(mut image) = crate::heap::try_filled(0_u8, crate::vfs::NODE_CAPACITY) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Ok(image_length) = crate::vfs::read_all(path, &mut image) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Some(program) =
        crate::userspace::load_elf_with_argv_verified(&image[..image_length], &[path])
    else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    drop(image);
    IO_COMPLETIONS.fetch_or(IO_EXEC, Ordering::Release);
    crate::audit::record(
        actor,
        crate::audit::Action::ProcessExec,
        length as u64,
        true,
    );
    crate::task::exec_current(program)
}

/// Execute a whitespace-delimited command line from userspace.
///
/// Stage 6 keeps the existing `exec(path,len)` ABI intact and adds this
/// argument-bearing form for the interactive shell. Bare command names are
/// resolved under `/bin`; paths containing `/` are used verbatim. The parser
/// intentionally supports simple ASCII whitespace only for now (no quoting or
/// environment expansion), bounded by userspace::MAX_ARGV/MAX_ARGV_BYTES.
fn sys_exec_command(user_line: u64, length: u64) -> u64 {
    let actor = crate::task::current_process_id();
    if !crate::task::current_has(crate::capability::Capability::FileRead)
        || !crate::task::current_has(crate::capability::Capability::ProcessCreate)
    {
        crate::audit::record(actor, crate::audit::Action::ProcessExec, length, false);
        return SYSCALL_ERROR;
    }

    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > crate::userspace::MAX_ARGV_BYTES {
        return SYSCALL_ERROR;
    }

    let mut command = [0_u8; crate::userspace::MAX_ARGV_BYTES];
    if crate::paging::copy_from_current_user(user_line, &mut command[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    if !command[..length].is_ascii() {
        return SYSCALL_ERROR;
    }

    let mut ranges = [(0_usize, 0_usize); crate::userspace::MAX_ARGV];
    let mut argc = 0_usize;
    let mut cursor = 0_usize;
    while cursor < length {
        while cursor < length && command[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == length {
            break;
        }
        if argc == crate::userspace::MAX_ARGV {
            return SYSCALL_ERROR;
        }
        let start = cursor;
        while cursor < length && !command[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        ranges[argc] = (start, cursor);
        argc += 1;
    }
    if argc == 0 {
        return SYSCALL_ERROR;
    }

    let Ok(command_name) = core::str::from_utf8(&command[ranges[0].0..ranges[0].1]) else {
        return SYSCALL_ERROR;
    };
    let resolved = if command_name.as_bytes().contains(&b'/') {
        alloc::string::String::from(command_name)
    } else {
        alloc::format!("/bin/{command_name}")
    };
    if resolved.len() > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }

    let mut argv = [""; crate::userspace::MAX_ARGV];
    argv[0] = resolved.as_str();
    for index in 1..argc {
        let (start, end) = ranges[index];
        let Ok(argument) = core::str::from_utf8(&command[start..end]) else {
            return SYSCALL_ERROR;
        };
        argv[index] = argument;
    }

    let Some(mut image) = crate::heap::try_filled(0_u8, crate::vfs::NODE_CAPACITY) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Ok(image_length) = crate::vfs::read_all(resolved.as_str(), &mut image) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Some(program) =
        crate::userspace::load_elf_with_argv_verified(&image[..image_length], &argv[..argc])
    else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    drop(image);
    IO_COMPLETIONS.fetch_or(IO_EXEC, Ordering::Release);
    crate::audit::record(
        actor,
        crate::audit::Action::ProcessExec,
        length as u64,
        true,
    );
    crate::task::exec_current(program)
}

/// Spawn a simple command as a new userspace process without requiring the
/// userspace shell to fork first. This is the reliable fast path for ordinary
/// foreground/background commands; fork remains available for pipelines and
/// redirections that need inherited descriptor wiring.
fn sys_spawn_command(user_line: u64, length: u64) -> u64 {
    let actor = crate::task::current_process_id();
    if !crate::task::current_has(crate::capability::Capability::FileRead)
        || !crate::task::current_has(crate::capability::Capability::ProcessCreate)
    {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > crate::userspace::MAX_ARGV_BYTES {
        return SYSCALL_ERROR;
    }
    let mut command = [0_u8; crate::userspace::MAX_ARGV_BYTES];
    if crate::paging::copy_from_current_user(user_line, &mut command[..length]).is_err()
        || !command[..length].is_ascii()
    {
        return SYSCALL_ERROR;
    }

    let mut ranges = [(0_usize, 0_usize); crate::userspace::MAX_ARGV];
    let mut argc = 0_usize;
    let mut cursor = 0_usize;
    while cursor < length {
        while cursor < length && command[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == length {
            break;
        }
        if argc == crate::userspace::MAX_ARGV {
            return SYSCALL_ERROR;
        }
        let start = cursor;
        while cursor < length && !command[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        ranges[argc] = (start, cursor);
        argc += 1;
    }
    if argc == 0 {
        return SYSCALL_ERROR;
    }
    let Ok(command_name) = core::str::from_utf8(&command[ranges[0].0..ranges[0].1]) else {
        return SYSCALL_ERROR;
    };
    let resolved = if command_name.as_bytes().contains(&b'/') {
        alloc::string::String::from(command_name)
    } else {
        alloc::format!("/bin/{command_name}")
    };
    if resolved.len() > MAX_PATH_SIZE {
        return SYSCALL_ERROR;
    }
    let mut argv = [""; crate::userspace::MAX_ARGV];
    argv[0] = resolved.as_str();
    for index in 1..argc {
        let (start, end) = ranges[index];
        let Ok(argument) = core::str::from_utf8(&command[start..end]) else {
            return SYSCALL_ERROR;
        };
        argv[index] = argument;
    }
    let Some(mut image) = crate::heap::try_filled(0_u8, crate::vfs::NODE_CAPACITY) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Ok(image_length) = crate::vfs::read_all(resolved.as_str(), &mut image) else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    let Some(program) =
        crate::userspace::load_elf_with_argv_verified(&image[..image_length], &argv[..argc])
    else {
        crate::audit::record(
            actor,
            crate::audit::Action::ProcessExec,
            length as u64,
            false,
        );
        return SYSCALL_ERROR;
    };
    drop(image);
    match crate::task::spawn_user_process("cmd", program) {
        Ok((pid, _)) => {
            crate::audit::record(
                actor,
                crate::audit::Action::ProcessExec,
                length as u64,
                true,
            );
            pid.as_u64()
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_env_get(user_key: u64, key_len: u64, user_out: u64) -> u64 {
    let Ok(key_len) = usize::try_from(key_len) else {
        return SYSCALL_ERROR;
    };
    if key_len == 0 || key_len >= crate::task::MAX_ENV_ENTRY {
        return SYSCALL_ERROR;
    }
    let mut key = [0u8; crate::task::MAX_ENV_ENTRY];
    if crate::paging::copy_from_current_user(user_key, &mut key[..key_len]).is_err() {
        return SYSCALL_ERROR;
    }
    let mut value = [0u8; crate::task::MAX_ENV_ENTRY];
    let Ok(len) = crate::task::env_get(&key[..key_len], &mut value) else {
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_to_current_user(user_out, &value[..len]).is_err() {
        return SYSCALL_ERROR;
    }
    len as u64
}

fn sys_env_set(user_key: u64, packed_lengths: u64, user_value: u64) -> u64 {
    let key_len = (packed_lengths & 0xffff_ffff) as usize;
    let value_len = (packed_lengths >> 32) as usize;
    if key_len == 0
        || key_len >= crate::task::MAX_ENV_ENTRY
        || value_len >= crate::task::MAX_ENV_ENTRY
    {
        return SYSCALL_ERROR;
    }
    let mut key = [0u8; crate::task::MAX_ENV_ENTRY];
    let mut value = [0u8; crate::task::MAX_ENV_ENTRY];
    if crate::paging::copy_from_current_user(user_key, &mut key[..key_len]).is_err()
        || crate::paging::copy_from_current_user(user_value, &mut value[..value_len]).is_err()
    {
        return SYSCALL_ERROR;
    }
    crate::task::env_set(&key[..key_len], &value[..value_len]).map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_env_entry(index: u64, user_out: u64, capacity: u64) -> u64 {
    let Ok(index) = usize::try_from(index) else {
        return SYSCALL_ERROR;
    };
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    let capacity = core::cmp::min(capacity, crate::task::MAX_ENV_ENTRY);
    if capacity == 0 {
        return SYSCALL_ERROR;
    }
    let mut entry = [0u8; crate::task::MAX_ENV_ENTRY];
    let Ok(len) = crate::task::env_entry(index, &mut entry[..capacity]) else {
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_to_current_user(user_out, &entry[..len]).is_err() {
        return SYSCALL_ERROR;
    }
    len as u64
}

fn sys_process_info(index: u64, user_out: u64) -> u64 {
    let Ok(index) = usize::try_from(index) else {
        return SYSCALL_ERROR;
    };
    let Some(info) = crate::task::process_info(index) else {
        return SYSCALL_ERROR;
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&info as *const crate::task::ProcessInfo).cast::<u8>(),
            core::mem::size_of::<crate::task::ProcessInfo>(),
        )
    };
    if crate::paging::copy_to_current_user(user_out, bytes).is_err() {
        return SYSCALL_ERROR;
    }
    0
}

fn sys_socket(kind: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let kind = match kind {
        1 => crate::network::SocketKind::Udp,
        2 => crate::network::SocketKind::Tcp,
        _ => return SYSCALL_ERROR,
    };
    crate::network::socket_open(crate::task::current_process_id(), kind).unwrap_or(SYSCALL_ERROR)
}

fn sys_net_bind(socket: u64, port: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(port) = u16::try_from(port) else {
        return SYSCALL_ERROR;
    };
    crate::network::socket_bind(crate::task::current_process_id(), socket, port)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_net_connect(socket: u64, packed: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(endpoint) = crate::network::endpoint_from_packed(packed) else {
        return SYSCALL_ERROR;
    };
    crate::network::socket_connect(crate::task::current_process_id(), socket, endpoint)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_net_send(socket: u64, user_buffer: u64, length: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_IO_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    crate::network::socket_send(crate::task::current_process_id(), socket, &buffer[..length])
        .map_or(SYSCALL_ERROR, |n| n as u64)
}

fn sys_net_recv(socket: u64, user_buffer: u64, capacity: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    if capacity > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_IO_SIZE];
    match crate::network::socket_recv(
        crate::task::current_process_id(),
        socket,
        &mut buffer[..capacity],
    ) {
        Ok((n, _)) => {
            if crate::paging::copy_to_current_user(user_buffer, &buffer[..n]).is_err() {
                SYSCALL_ERROR
            } else {
                n as u64
            }
        }
        Err(crate::network::SocketError::WouldBlock) => u64::MAX - 1,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_net_close(socket: u64) -> u64 {
    crate::network::socket_close(crate::task::current_process_id(), socket)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_net_info(user_buffer: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let info = crate::network::net_info();
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&info as *const crate::network::NetInfo).cast::<u8>(),
            core::mem::size_of::<crate::network::NetInfo>(),
        )
    };
    if crate::paging::copy_to_current_user(user_buffer, bytes).is_err() {
        SYSCALL_ERROR
    } else {
        bytes.len() as u64
    }
}

fn sys_dns_start(user_name: u64, length: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > 253 {
        return SYSCALL_ERROR;
    }
    let mut name = [0u8; 253];
    if crate::paging::copy_from_current_user(user_name, &mut name[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Ok(name) = core::str::from_utf8(&name[..length]) else {
        return SYSCALL_ERROR;
    };
    crate::network::dns_start(name).unwrap_or(SYSCALL_ERROR)
}

fn sys_dns_poll(query: u64, user_ipv4: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    match crate::network::dns_poll(query) {
        Ok(None) => 0,
        Ok(Some(ip)) => {
            let octets = ip.octets();
            if crate::paging::copy_to_current_user(user_ipv4, &octets).is_err() {
                SYSCALL_ERROR
            } else {
                1
            }
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_dns_cancel(query: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    crate::network::dns_cancel(query).map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_net_peer(socket: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    match crate::network::socket_peer(crate::task::current_process_id(), socket) {
        Ok(Some(endpoint)) => crate::network::endpoint_to_packed(endpoint),
        Ok(None) => 0,
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_dhcp(enabled: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    crate::network::set_dhcp(enabled != 0).map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_ping_start(ip_packed: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let raw = (ip_packed & 0xffff_ffff) as u32;
    let ip = smoltcp::wire::Ipv4Address::from_octets(raw.to_be_bytes());
    crate::network::ping_start(ip).map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_ping_poll() -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    match crate::network::ping_poll() {
        Ok(None) => 0,
        Ok(Some(ticks)) => ticks.saturating_add(1),
        Err(_) => SYSCALL_ERROR,
    }
}

fn authorize_async_class(class: crate::async_op::AsyncClass) -> bool {
    match class {
        crate::async_op::AsyncClass::Block => {
            crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Storage)
        }
        crate::async_op::AsyncClass::Network => {
            crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network)
        }
        crate::async_op::AsyncClass::Device => {
            crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Generic)
        }
        crate::async_op::AsyncClass::Service => {
            crate::task::current_has(crate::capability::Capability::Ipc)
        }
        // File operations are created only through the dedicated Stage 10.5
        // submission syscalls, which authorize the exact descriptor/scope.
        crate::async_op::AsyncClass::File => false,
        crate::async_op::AsyncClass::Timer => {
            crate::task::current_has(crate::capability::Capability::TimerRead)
        }
        crate::async_op::AsyncClass::Event => {
            crate::task::current_has(crate::capability::Capability::Ipc)
        }
    }
}

fn async_handle(raw: u64) -> Result<crate::async_op::Handle, ()> {
    crate::async_op::Handle::from_raw(raw).ok_or(())
}

fn copy_async_completion_to_user(
    user_completion: u64,
    completion: crate::async_op::Completion,
) -> Result<(), ()> {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&completion.status.to_ne_bytes());
    bytes[4..8].copy_from_slice(&completion.reserved.to_ne_bytes());
    bytes[8..16].copy_from_slice(&completion.value.to_ne_bytes());
    crate::paging::copy_to_current_user(user_completion, &bytes).map_err(|_| ())
}

fn sys_async_create(class: u64) -> u64 {
    let Some(class) = crate::async_op::AsyncClass::from_u64(class) else {
        return SYSCALL_ERROR;
    };
    // Ring 3 may directly manufacture only Service operations. Hardware-backed
    // classes receive handles from their dedicated submission syscalls.
    if !matches!(
        class,
        crate::async_op::AsyncClass::Service
            | crate::async_op::AsyncClass::Timer
            | crate::async_op::AsyncClass::Event
    ) || !authorize_async_class(class)
    {
        return SYSCALL_ERROR;
    }
    match crate::async_op::allocate_current(class) {
        Ok(handle) => {
            ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_CREATE, Ordering::Release);
            handle.to_raw()
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_async_poll(raw_handle: u64, user_completion: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    let Ok(class) = crate::async_op::class_current(handle) else {
        return SYSCALL_ERROR;
    };
    if !matches!(
        class,
        crate::async_op::AsyncClass::Service
            | crate::async_op::AsyncClass::Timer
            | crate::async_op::AsyncClass::Event
    ) || !authorize_async_class(class)
    {
        return SYSCALL_ERROR;
    }
    match crate::async_op::peek_current(handle) {
        Ok(None) => {
            ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_POLL, Ordering::Release);
            0
        }
        Ok(Some(completion)) => {
            if copy_async_completion_to_user(user_completion, completion).is_err() {
                return SYSCALL_ERROR;
            }
            if crate::async_op::release_current(handle).is_err() {
                return SYSCALL_ERROR;
            }
            ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_POLL, Ordering::Release);
            1
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_async_wait(raw_handle: u64, user_completion: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    let Ok(class) = crate::async_op::class_current(handle) else {
        return SYSCALL_ERROR;
    };
    if !matches!(
        class,
        crate::async_op::AsyncClass::Service
            | crate::async_op::AsyncClass::Timer
            | crate::async_op::AsyncClass::Event
    ) || !authorize_async_class(class)
    {
        return SYSCALL_ERROR;
    }
    let Ok(completion) = crate::async_op::wait_ready(handle) else {
        return SYSCALL_ERROR;
    };
    if copy_async_completion_to_user(user_completion, completion).is_err() {
        return SYSCALL_ERROR;
    }
    if crate::async_op::release_current(handle).is_err() {
        return SYSCALL_ERROR;
    }
    ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_WAIT, Ordering::Release);
    1
}

fn sys_timer_create(relative: u64, period: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::TimerRead) {
        return SYSCALL_ERROR;
    }
    let deadline = crate::timer::ticks()
        .checked_add(relative)
        .filter(|&value| value != u64::MAX);
    deadline
        .and_then(|value| {
            crate::async_events::create_timer(crate::task::current_process_id(), value, period).ok()
        })
        .unwrap_or(SYSCALL_ERROR)
}

fn sys_timer_wait(raw: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::TimerRead) {
        return SYSCALL_ERROR;
    }
    crate::async_events::wait_current(raw, true).map_or(SYSCALL_ERROR, |handle| handle.to_raw())
}

fn sys_event_create(initial: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    crate::async_events::create_event(crate::task::current_process_id(), initial != 0)
        .unwrap_or(SYSCALL_ERROR)
}

fn sys_event_set(raw: u64, state: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    crate::async_events::set_event(crate::task::current_process_id(), raw, state != 0)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_event_wait(raw: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    crate::async_events::wait_current(raw, false).map_or(SYSCALL_ERROR, |handle| handle.to_raw())
}

fn sys_timer_close(raw: u64) -> u64 {
    crate::async_events::close(crate::task::current_process_id(), raw, true)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_event_close(raw: u64) -> u64 {
    crate::async_events::close(crate::task::current_process_id(), raw, false)
        .map_or(SYSCALL_ERROR, |()| 0)
}

fn sys_sleep_until(deadline: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::TimerRead) {
        return SYSCALL_ERROR;
    }
    crate::task::wait_for_event_until(deadline);
    0
}

fn sys_notification_poll(user_buffer: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) {
        return SYSCALL_ERROR;
    }
    let Some(note) = crate::notifications::receive_for(crate::task::current_process_id()) else {
        return 0;
    };
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(note.kind as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&note.source.to_le_bytes());
    bytes[16..24].copy_from_slice(&note.payload.to_le_bytes());
    if crate::paging::copy_to_current_user(user_buffer, &bytes).is_err() {
        return SYSCALL_ERROR;
    }
    1
}
fn sys_notification_post(kind: u64, payload: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Ipc) || !(1..=6).contains(&kind) {
        return SYSCALL_ERROR;
    }
    let kind = match kind {
        1 => crate::notifications::Kind::Terminate,
        2 => crate::notifications::Kind::Suspend,
        3 => crate::notifications::Kind::Resume,
        4 => crate::notifications::Kind::ChildExit,
        5 => crate::notifications::Kind::Exception,
        _ => crate::notifications::Kind::User,
    };
    if crate::notifications::publish(crate::notifications::Notification {
        recipient: crate::task::current_process_id(),
        kind,
        source: crate::task::current_process_id(),
        payload,
    }) {
        0
    } else {
        SYSCALL_ERROR
    }
}

fn sys_async_cancel(raw_handle: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    let Ok(class) = crate::async_op::class_current(handle) else {
        return SYSCALL_ERROR;
    };
    // Cancellation is always permitted to the owner. Policy tightening must
    // never prevent safe teardown of authority that was valid when created.
    // Block requests additionally detach their kernel bounce-buffer queue slot.
    if class == crate::async_op::AsyncClass::Block {
        let _ = crate::block_io::cancel_user_request(handle);
    } else if class == crate::async_op::AsyncClass::File {
        let _ = crate::async_file::cancel(handle);
    } else if class == crate::async_op::AsyncClass::Network {
        let _ = crate::async_network::cancel(handle);
    } else if matches!(
        class,
        crate::async_op::AsyncClass::Timer | crate::async_op::AsyncClass::Event
    ) {
        crate::async_events::cancel(handle);
    }
    match crate::async_op::cancel_current(handle) {
        Ok(()) => {
            ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_CANCEL, Ordering::Release);
            0
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_async_complete(raw_handle: u64, status: u64, value: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    if crate::async_op::class_current(handle) != Ok(crate::async_op::AsyncClass::Service)
        || !crate::task::current_has(crate::capability::Capability::Ipc)
    {
        return SYSCALL_ERROR;
    }
    match crate::async_op::complete_current_service(
        handle,
        crate::async_op::Completion::new(status as i32, value),
    ) {
        Ok(()) => {
            ASYNC_ABI_COMPLETIONS.fetch_or(ASYNC_COMPLETE, Ordering::Release);
            0
        }
        Err(_) => SYSCALL_ERROR,
    }
}

fn copy_block_completion_to_user(
    user_result: u64,
    completion: crate::async_op::Completion,
    result: crate::block_io::UserResult,
) -> Result<(), ()> {
    // ABI: 16-byte generic Completion header followed by one 512-byte sector.
    // For writes and failed reads the payload is zeroed so kernel bounce-buffer
    // contents are never exposed accidentally.
    let mut bytes = [0u8; 16 + crate::block::SECTOR_SIZE];
    bytes[0..4].copy_from_slice(&completion.status.to_ne_bytes());
    bytes[4..8].copy_from_slice(&completion.reserved.to_ne_bytes());
    bytes[8..16].copy_from_slice(&completion.value.to_ne_bytes());
    if result.is_read && result.result.is_ok() {
        bytes[16..].copy_from_slice(&result.data);
    }
    crate::paging::copy_to_current_user(user_result, &bytes).map_err(|_| ())
}

fn sys_async_block_read(lba: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Storage) {
        return SYSCALL_ERROR;
    }
    match crate::block_io::submit_user_read(lba) {
        Ok(handle) => handle.to_raw(),
        Err(_) => SYSCALL_ERROR,
    }
}

fn sys_async_block_write(lba: u64, user_sector: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Storage) {
        return SYSCALL_ERROR;
    }
    let mut data = [0u8; crate::block::SECTOR_SIZE];
    if crate::paging::copy_from_current_user(user_sector, &mut data).is_err() {
        return SYSCALL_ERROR;
    }
    match crate::block_io::submit_user_write(lba, data) {
        Ok(handle) => handle.to_raw(),
        Err(_) => SYSCALL_ERROR,
    }
}

fn finish_async_block(handle: crate::async_op::Handle, user_result: u64, wait: bool) -> u64 {
    if crate::async_op::class_current(handle) != Ok(crate::async_op::AsyncClass::Block)
        || !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Storage)
    {
        return SYSCALL_ERROR;
    }
    let completion = if wait {
        match crate::async_op::wait_ready(handle) {
            Ok(completion) => completion,
            Err(_) => return SYSCALL_ERROR,
        }
    } else {
        match crate::async_op::peek_current(handle) {
            Ok(Some(completion)) => completion,
            Ok(None) => return 0,
            Err(_) => return SYSCALL_ERROR,
        }
    };
    let Some(result) = crate::block_io::peek_user_result(handle) else {
        return SYSCALL_ERROR;
    };
    if copy_block_completion_to_user(user_result, completion, result).is_err() {
        return SYSCALL_ERROR;
    }
    if !crate::block_io::consume_user_result(handle)
        || crate::async_op::release_current(handle).is_err()
    {
        return SYSCALL_ERROR;
    }
    1
}

fn sys_async_block_poll(raw_handle: u64, user_result: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_block(handle, user_result, false)
}

fn sys_async_block_wait(raw_handle: u64, user_result: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_block(handle, user_result, true)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AsyncFileRequest {
    offset: u64,
    buffer: u64,
    length: u64,
}

fn copy_async_file_request(user_request: u64) -> Result<AsyncFileRequest, ()> {
    let mut bytes = [0u8; 24];
    crate::paging::copy_from_current_user(user_request, &mut bytes).map_err(|_| ())?;
    Ok(AsyncFileRequest {
        offset: u64::from_ne_bytes(bytes[0..8].try_into().map_err(|_| ())?),
        buffer: u64::from_ne_bytes(bytes[8..16].try_into().map_err(|_| ())?),
        length: u64::from_ne_bytes(bytes[16..24].try_into().map_err(|_| ())?),
    })
}
fn sys_async_file_read(descriptor: u64, user_request: u64) -> u64 {
    let Ok(request) = copy_async_file_request(user_request) else {
        return SYSCALL_ERROR;
    };
    let Ok(offset) = usize::try_from(request.offset) else {
        return SYSCALL_ERROR;
    };
    let Ok(length) = usize::try_from(request.length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    crate::async_file::submit_read(descriptor, offset, length).map_or(SYSCALL_ERROR, |h| h.to_raw())
}
fn sys_async_file_write(descriptor: u64, user_request: u64) -> u64 {
    let Ok(request) = copy_async_file_request(user_request) else {
        return SYSCALL_ERROR;
    };
    let Ok(offset) = usize::try_from(request.offset) else {
        return SYSCALL_ERROR;
    };
    let Ok(length) = usize::try_from(request.length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut data = [0u8; MAX_IO_SIZE];
    if crate::paging::copy_from_current_user(request.buffer, &mut data[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    crate::async_file::submit_write(descriptor, offset, &data[..length])
        .map_or(SYSCALL_ERROR, |h| h.to_raw())
}
fn copy_file_completion(
    user_completion: u64,
    completion: crate::async_op::Completion,
) -> Result<(), ()> {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&completion.status.to_ne_bytes());
    bytes[4..8].copy_from_slice(&completion.reserved.to_ne_bytes());
    bytes[8..16].copy_from_slice(&completion.value.to_ne_bytes());
    crate::paging::copy_to_current_user(user_completion, &bytes).map_err(|_| ())
}
fn finish_async_file(
    handle: crate::async_op::Handle,
    user_completion: u64,
    user_data: u64,
    wait: bool,
) -> u64 {
    if crate::async_op::class_current(handle) != Ok(crate::async_op::AsyncClass::File) {
        return SYSCALL_ERROR;
    }
    let completion = if wait {
        match crate::async_op::wait_ready(handle) {
            Ok(c) => c,
            Err(_) => return SYSCALL_ERROR,
        }
    } else {
        match crate::async_op::peek_current(handle) {
            Ok(Some(c)) => c,
            Ok(None) => return 0,
            Err(_) => return SYSCALL_ERROR,
        }
    };
    let Some(result) = crate::async_file::peek_result(handle) else {
        return SYSCALL_ERROR;
    };
    let write = result.operation == crate::async_file::Operation::Write;
    if !crate::task::authorize_current_file_scope(result.scope, write) {
        return SYSCALL_ERROR;
    }
    if copy_file_completion(user_completion, completion).is_err() {
        return SYSCALL_ERROR;
    }
    if !write && result.result.is_ok() {
        let count = result.result.unwrap_or(0).min(result.length);
        if crate::paging::copy_to_current_user(user_data, &result.data[..count]).is_err() {
            return SYSCALL_ERROR;
        }
    }
    if !crate::async_file::consume(handle) || crate::async_op::release_current(handle).is_err() {
        return SYSCALL_ERROR;
    }
    1
}
fn sys_async_file_poll(raw_handle: u64, user_completion: u64, user_data: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_file(handle, user_completion, user_data, false)
}
fn sys_async_file_wait(raw_handle: u64, user_completion: u64, user_data: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_file(handle, user_completion, user_data, true)
}

fn sys_port_read(port: u64, destination: u64, argument: u64, wait: bool) -> u64 {
    let (capacity, deadline) = if wait {
        let mut options = [0u8; 16];
        if crate::paging::copy_from_current_user(argument, &mut options).is_err() {
            return SYSCALL_ERROR;
        }
        let timeout = u64::from_le_bytes(options[8..].try_into().unwrap());
        let deadline = if timeout == u64::MAX {
            u64::MAX
        } else {
            let Some(deadline) = crate::timer::ticks()
                .checked_add(timeout)
                .filter(|&d| d != u64::MAX)
            else {
                return SYSCALL_ERROR;
            };
            deadline
        };
        (
            u64::from_le_bytes(options[..8].try_into().unwrap()),
            deadline,
        )
    } else {
        (argument, 0)
    };
    if capacity == 0 || capacity > crate::completion_queue::BATCH as u64 {
        return SYSCALL_ERROR;
    }
    let owner = crate::task::current_process_id();
    let Some(task) = crate::task::current_task_id_if_running() else {
        return SYSCALL_ERROR;
    };
    loop {
        let result = crate::completion_port::begin(owner, port, task, capacity as usize, wait);
        let Ok((events, count)) = result else {
            return SYSCALL_ERROR;
        };
        if count != 0 {
            let mut bytes = [0u8; crate::completion_queue::BATCH * 40];
            for (event, out) in events[..count]
                .iter()
                .zip(bytes.as_chunks_mut::<40>().0.iter_mut())
            {
                event.encode(out);
            }
            let copied =
                crate::paging::copy_to_current_user(destination, &bytes[..count * 40]).is_ok();
            let acknowledged =
                crate::completion_port::finish(owner, port, task, &events[..count], copied).is_ok();
            return if copied && acknowledged {
                count as u64
            } else {
                SYSCALL_ERROR
            };
        }
        if !wait || (deadline != u64::MAX && crate::timer::ticks() >= deadline) {
            crate::completion_port::unwatch(port, task);
            return 0;
        }
        crate::task::wait_for_event_until(deadline);
        crate::completion_port::unwatch(port, task);
    }
}

fn sys_async_tcp_connect(socket: u64, packed: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(endpoint) = crate::network::endpoint_from_packed(packed) else {
        return SYSCALL_ERROR;
    };
    crate::async_network::submit_connect(socket, endpoint).map_or(SYSCALL_ERROR, |h| h.to_raw())
}

fn sys_async_net_send(socket: u64, user_buffer: u64, length: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    let mut data = [0u8; MAX_IO_SIZE];
    if crate::paging::copy_from_current_user(user_buffer, &mut data[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    crate::async_network::submit_send(socket, &data[..length]).map_or(SYSCALL_ERROR, |h| h.to_raw())
}

fn sys_async_net_recv(socket: u64, capacity: u64) -> u64 {
    if !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network) {
        return SYSCALL_ERROR;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    if capacity > MAX_IO_SIZE {
        return SYSCALL_ERROR;
    }
    crate::async_network::submit_recv(socket, capacity).map_or(SYSCALL_ERROR, |h| h.to_raw())
}

fn finish_async_net(
    handle: crate::async_op::Handle,
    user_completion: u64,
    user_data: u64,
    wait: bool,
) -> u64 {
    if crate::async_op::class_current(handle) != Ok(crate::async_op::AsyncClass::Network)
        || !crate::task::authorize_current_device(crate::wovenguard::DeviceClass::Network)
    {
        return SYSCALL_ERROR;
    }
    let completion = if wait {
        match crate::async_op::wait_ready(handle) {
            Ok(c) => c,
            Err(_) => return SYSCALL_ERROR,
        }
    } else {
        match crate::async_op::peek_current(handle) {
            Ok(Some(c)) => c,
            Ok(None) => return 0,
            Err(_) => return SYSCALL_ERROR,
        }
    };
    let Some(result) = crate::async_network::peek_result(handle) else {
        return SYSCALL_ERROR;
    };
    if copy_file_completion(user_completion, completion).is_err() {
        return SYSCALL_ERROR;
    }
    if result.operation == crate::async_network::Operation::Recv && result.result.is_ok() {
        let count = result.result.unwrap_or(0).min(result.length);
        if crate::paging::copy_to_current_user(user_data, &result.data[..count]).is_err() {
            return SYSCALL_ERROR;
        }
    }
    if !crate::async_network::consume(handle) || crate::async_op::release_current(handle).is_err() {
        return SYSCALL_ERROR;
    }
    1
}

fn sys_async_net_poll(raw_handle: u64, user_completion: u64, user_data: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_net(handle, user_completion, user_data, false)
}
fn sys_async_net_wait(raw_handle: u64, user_completion: u64, user_data: u64) -> u64 {
    let Ok(handle) = async_handle(raw_handle) else {
        return SYSCALL_ERROR;
    };
    finish_async_net(handle, user_completion, user_data, true)
}

fn sys_audit_read(offset: u64, user_event: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::TaskInspect) {
        return SYSCALL_ERROR;
    }
    let Ok(offset) = usize::try_from(offset) else {
        return SYSCALL_ERROR;
    };
    let Some(event) = crate::audit::from_latest(offset) else {
        return 0;
    };
    let mut bytes = [0_u8; 48];
    bytes[0..8].copy_from_slice(&event.sequence.to_le_bytes());
    bytes[8..16].copy_from_slice(&event.tick.to_le_bytes());
    bytes[16..18].copy_from_slice(&event.cpu.to_le_bytes());
    bytes[18] = event.action as u8;
    bytes[19] = u8::from(event.allowed);
    bytes[20..28].copy_from_slice(&event.actor.to_le_bytes());
    bytes[28..36].copy_from_slice(&event.target.to_le_bytes());
    bytes[36..44].copy_from_slice(&event.detail.to_le_bytes());
    if crate::paging::copy_to_current_user(user_event, &bytes).is_err() {
        return SYSCALL_ERROR;
    }
    1
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn sys_audio_info(user_info: u64) -> u64 {
    let Some(info) = crate::hda::audio_info() else {
        return SYSCALL_ERROR;
    };
    let mut bytes = [0u8; 12];
    bytes[0..4].copy_from_slice(&info.sample_rate.to_le_bytes());
    bytes[4..8].copy_from_slice(&info.max_bytes.to_le_bytes());
    bytes[8] = info.playback_streams;
    bytes[9] = info.capture_streams;
    bytes[10] = info.channels;
    bytes[11] = info.bits;
    if crate::paging::copy_to_current_user(user_info, &bytes).is_err() {
        return SYSCALL_ERROR;
    }
    1
}

#[cfg(not(any(not(feature = "qemu-test"), feature = "stage13-12-test")))]
fn sys_audio_info(_user_info: u64) -> u64 {
    SYSCALL_ERROR
}

/// The PCM is copied into kernel memory first: user pages may fault, and the
/// HDA controller lock must not be held across that.
#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn sys_audio_play(user_pcm: u64, length: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    if !crate::task::current_has(crate::capability::Capability::AudioPlayback) {
        crate::audit::record(actor, crate::audit::Action::AudioPlay, 0, false);
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > crate::hda::MAX_PLAY_BYTES {
        return SYSCALL_ERROR;
    }
    let Some(mut pcm) = crate::heap::try_filled(0u8, length) else {
        crate::audit::record(actor, crate::audit::Action::AudioPlay, length as u64, false);
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_from_current_user(user_pcm, &mut pcm).is_err() {
        return SYSCALL_ERROR;
    }
    let result = crate::hda::play(&pcm);
    crate::audit::record(
        actor,
        crate::audit::Action::AudioPlay,
        length as u64,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |played| played as u64)
}

#[cfg(not(any(not(feature = "qemu-test"), feature = "stage13-12-test")))]
fn sys_audio_play(_user_pcm: u64, _length: u64) -> u64 {
    SYSCALL_ERROR
}

#[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
fn sys_audio_set_volume(percent: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::AudioPlayback) || percent > 100 {
        return SYSCALL_ERROR;
    }
    crate::hda::set_volume(percent as u8).map_or(SYSCALL_ERROR, u64::from)
}

#[cfg(not(any(not(feature = "qemu-test"), feature = "stage13-12-test")))]
fn sys_audio_set_volume(_percent: u64) -> u64 {
    SYSCALL_ERROR
}

fn sys_secret_store(user_secret: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Secrets) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > crate::secret_vault::MAX_SECRET_BYTES {
        return SYSCALL_ERROR;
    }
    let mut secret = [0_u8; crate::secret_vault::MAX_SECRET_BYTES];
    if crate::paging::copy_from_current_user(user_secret, &mut secret[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let actor = crate::task::current_task_id().as_u64();
    let result = crate::secret_vault::store(actor, &secret[..length]);
    for byte in &mut secret {
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    match result {
        Ok(handle) => {
            crate::audit::record(actor, crate::audit::Action::SecretStore, handle.0, true);
            handle.0
        }
        Err(_) => {
            crate::audit::record(actor, crate::audit::Action::SecretStore, 0, false);
            SYSCALL_ERROR
        }
    }
}

fn sys_secret_read(raw_handle: u64, user_output: u64, capacity: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Secrets) {
        return SYSCALL_ERROR;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return SYSCALL_ERROR;
    };
    if capacity > crate::secret_vault::MAX_SECRET_BYTES {
        return SYSCALL_ERROR;
    }
    let mut secret = [0_u8; crate::secret_vault::MAX_SECRET_BYTES];
    let result = crate::secret_vault::read(
        crate::task::current_task_id().as_u64(),
        crate::secret_vault::SecretHandle(raw_handle),
        &mut secret,
    );
    let Ok(length) = result else {
        for byte in &mut secret {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        return SYSCALL_ERROR;
    };
    if length > capacity {
        for byte in &mut secret {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        return SYSCALL_ERROR;
    }
    if crate::paging::copy_to_current_user(user_output, &secret[..length]).is_err() {
        for byte in &mut secret {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        return SYSCALL_ERROR;
    }
    for byte in &mut secret {
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    length as u64
}

fn sys_secret_revoke(raw_handle: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::Secrets) {
        return SYSCALL_ERROR;
    }
    let actor = crate::task::current_task_id().as_u64();
    match crate::secret_vault::revoke(actor, crate::secret_vault::SecretHandle(raw_handle)) {
        Ok(()) => {
            crate::audit::record(actor, crate::audit::Action::SecretRevoke, raw_handle, true);
            0
        }
        Err(_) => {
            crate::audit::record(actor, crate::audit::Action::SecretRevoke, raw_handle, false);
            SYSCALL_ERROR
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wovenhat_syscall_dispatch(
    number: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    frame: *const UserForkFrame,
) -> u64 {
    let result = match number {
        value if value == Number::Mmap as u64 => sys_mmap(arg0, arg1),
        value if value == Number::Munmap as u64 => sys_munmap(arg0, arg1),
        value if value == Number::Read as u64 => sys_read(arg0, arg1, arg2),
        value if value == Number::FileWrite as u64 => sys_file_write(arg0, arg1, arg2),
        value if value == Number::Write as u64 => sys_write(arg0, arg1, arg2),
        value if value == Number::Open as u64 => sys_open(arg0, arg1),
        value if value == Number::Close as u64 => sys_close(arg0),
        value if value == Number::Exec as u64 => sys_exec(arg0, arg1),
        value if value == Number::ExecCommand as u64 => sys_exec_command(arg0, arg1),
        value if value == Number::EnvGet as u64 => sys_env_get(arg0, arg1, arg2),
        value if value == Number::EnvSet as u64 => sys_env_set(arg0, arg1, arg2),
        value if value == Number::EnvCount as u64 => crate::task::env_count() as u64,
        value if value == Number::EnvEntry as u64 => sys_env_entry(arg0, arg1, arg2),
        value if value == Number::ProcessCount as u64 => crate::task::process_count() as u64,
        value if value == Number::ProcessInfo as u64 => sys_process_info(arg0, arg1),
        value if value == Number::SpawnCommand as u64 => sys_spawn_command(arg0, arg1),
        value if value == Number::MmapFileShared as u64 => {
            crate::task::mmap_file_current(arg0, arg1, arg2, true, true, true)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::Msync as u64 => crate::task::msync_current(arg0, arg1)
            .map(|_| 0)
            .unwrap_or(SYSCALL_ERROR),
        value if value == Number::MmapFileLazy as u64 => {
            crate::task::mmap_file_current(arg0, arg1, arg2, false, true, false)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::MmapFileLazyWritable as u64 => {
            crate::task::mmap_file_current(arg0, arg1, arg2, true, true, false)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::MmapFile as u64 => {
            crate::task::mmap_file_current(arg0, arg1, arg2, false, false, false)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::MmapFileWritable as u64 => {
            crate::task::mmap_file_current(arg0, arg1, arg2, true, false, false)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::Fork as u64 => sys_fork(frame),
        value if value == Number::MessageSend as u64 => sys_message_send(arg0, arg1, arg2),
        value if value == Number::MessageReceive as u64 => sys_message_receive(arg0, arg1, arg2),
        value if value == Number::IpcEndpointCreate as u64 => sys_ipc_endpoint_create(),
        value if value == Number::IpcServicePublish as u64 => {
            sys_ipc_service_publish(arg0, arg1, arg2)
        }
        value if value == Number::IpcServiceUnpublish as u64 => {
            sys_ipc_service_unpublish(arg0, arg1)
        }
        value if value == Number::IpcServiceDiscover as u64 => sys_ipc_service_discover(arg0, arg1),
        value if value == Number::IpcMessageSend as u64 => sys_ipc_message_send(arg0, arg1, arg2),
        value if value == Number::IpcMessageReceive as u64 => {
            sys_ipc_message_receive(arg0, arg1, arg2)
        }
        #[cfg(any(feature = "stage22-5-test", feature = "system-apps"))]
        value if value == Number::PackageTransaction as u64 => {
            sys_package_transaction(arg0, arg1, arg2)
        }
        #[cfg(any(
            feature = "stage23-1-test",
            feature = "stage23-2-test",
            feature = "stage23-3-test",
            feature = "system-apps"
        ))]
        value if value == Number::AiMessageSend as u64 => sys_ai_message_send(arg0, arg1, arg2),
        value if value == Number::Getuid as u64 => {
            IO_COMPLETIONS.fetch_or(IO_GETUID, Ordering::Release);
            u64::from(crate::task::current_credentials().uid)
        }
        value if value == Number::Getgid as u64 => {
            IO_COMPLETIONS.fetch_or(IO_GETGID, Ordering::Release);
            u64::from(crate::task::current_credentials().gid)
        }
        value if value == Number::Yield as u64 => {
            // Yield immediately from the syscall frame so a userspace
            // fork/wait loop does not depend on a later timer interrupt to
            // schedule another Ready process. The syscall frame resumes here
            // when this task is selected again, then returns to Ring 3.
            crate::task::yield_now();
            0
        }
        value if value == Number::Exit as u64 => {
            if crate::task::begin_user_destructor_exit(frame, arg0 as i32) {
                0
            } else {
                crate::task::exit_current_process(arg0 as i32)
            }
        }
        value if value == Number::ThreadCreate as u64 => {
            crate::task::create_user_thread(arg0, arg1)
                .map(|id| id.raw())
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::ThreadExit as u64 => {
            crate::task::exit_current_thread(arg0 as i32)
        }
        value if value == Number::ThreadJoin as u64 => loop {
            match crate::task::join_user_thread(arg0) {
                Ok(exit) => break crate::exit_status::encode(exit),
                Err(crate::task::ThreadJoinError::Running) => {
                    let owner = crate::task::current_process_id();
                    let waiter = crate::task::current_task_id().as_u64();
                    match crate::thread::register_waiter(
                        owner,
                        crate::thread::ThreadId::from_raw(arg0),
                        waiter,
                    ) {
                        Ok(true) => continue,
                        Ok(false) => crate::task::wait_for_event(),
                        Err(_) => break SYSCALL_ERROR,
                    }
                }
                Err(_) => break SYSCALL_ERROR,
            }
        },
        value if value == Number::ThreadSelf as u64 => crate::task::current_thread_id()
            .map(|id| id.raw())
            .unwrap_or(SYSCALL_ERROR),
        value if value == Number::ThreadSetTls as u64 => crate::task::current_thread_id()
            .filter(|id| crate::thread::set_tls(*id, arg0))
            .map_or(SYSCALL_ERROR, |_| 0),
        value if value == Number::ThreadGetTls as u64 => crate::task::current_thread_id()
            .and_then(crate::thread::tls)
            .unwrap_or(SYSCALL_ERROR),
        value if value == Number::ThreadCancel as u64 => {
            crate::task::cancel_user_thread(arg0, arg1 as i32).map_or(SYSCALL_ERROR, |_| 0)
        }
        value if value == Number::LazyBind as u64 => crate::task::resolve_lazy_binding(frame),
        value if value == Number::Getpid as u64 => crate::task::current_process_id(),
        value if value == Number::Waitpid as u64 => match crate::task::wait_process(arg0) {
            Ok(exit_code) => crate::exit_status::encode(exit_code),
            Err(crate::task::WaitError::StillRunning) => u64::MAX - 1,
            Err(crate::task::WaitError::NoSuchChild) => u64::MAX,
        },
        value if value == Number::Stat as u64 => sys_stat(arg0, arg1),
        value if value == Number::Readdir as u64 => sys_readdir(arg0, arg1, arg2),
        value if value == Number::Mkdir as u64 => sys_mkdir(arg0, arg1),
        value if value == Number::Chdir as u64 => sys_chdir(arg0, arg1),
        value if value == Number::Getcwd as u64 => sys_getcwd(arg0, arg1),
        value if value == Number::Dup as u64 => sys_dup(arg0),
        value if value == Number::Pipe as u64 => sys_pipe(),
        value if value == Number::Dup2 as u64 => sys_dup2(arg0, arg1),
        value if value == Number::Getppid as u64 => sys_getppid(),
        value if value == Number::Kill as u64 => sys_kill(arg0, arg1),
        value if value == Number::Lseek as u64 => sys_lseek(arg0, arg1),
        value if value == Number::Unlink as u64 => sys_unlink(arg0, arg1),
        value if value == Number::Sleep as u64 => sys_sleep(arg0),
        value if value == Number::Rename as u64 => sys_rename(arg0, arg1, arg2),
        value if value == Number::Getticks as u64 => sys_getticks(),
        value if value == Number::Sync as u64 => sys_sync(),
        value if value == Number::Ioctl as u64 => sys_ioctl(arg0, arg1, arg2),
        value if value == Number::Sigaction as u64 => sys_sigaction(arg0, arg1),
        value if value == Number::Getpgrp as u64 => crate::task::current_process_group(),
        value if value == Number::Setpgid as u64 => sys_setpgid(arg0, arg1),
        value if value == Number::Socket as u64 => sys_socket(arg0),
        value if value == Number::Bind as u64 => sys_net_bind(arg0, arg1),
        value if value == Number::Connect as u64 => sys_net_connect(arg0, arg1),
        value if value == Number::NetSend as u64 => sys_net_send(arg0, arg1, arg2),
        value if value == Number::NetRecv as u64 => sys_net_recv(arg0, arg1, arg2),
        value if value == Number::NetClose as u64 => sys_net_close(arg0),
        value if value == Number::NetInfo as u64 => sys_net_info(arg0),
        value if value == Number::DnsStart as u64 => sys_dns_start(arg0, arg1),
        value if value == Number::DnsPoll as u64 => sys_dns_poll(arg0, arg1),
        value if value == Number::DnsCancel as u64 => sys_dns_cancel(arg0),
        value if value == Number::NetPeer as u64 => sys_net_peer(arg0),
        value if value == Number::Dhcp as u64 => sys_dhcp(arg0),
        value if value == Number::PingStart as u64 => sys_ping_start(arg0),
        value if value == Number::PingPoll as u64 => sys_ping_poll(),
        value if value == Number::AsyncCreate as u64 => sys_async_create(arg0),
        value if value == Number::AsyncPoll as u64 => sys_async_poll(arg0, arg1),
        value if value == Number::AsyncWait as u64 => sys_async_wait(arg0, arg1),
        value if value == Number::NotificationPoll as u64 => sys_notification_poll(arg0),
        value if value == Number::NotificationPost as u64 => sys_notification_post(arg0, arg1),
        value if value == Number::AsyncCancel as u64 => sys_async_cancel(arg0),
        value if value == Number::AsyncComplete as u64 => sys_async_complete(arg0, arg1, arg2),
        value if value == Number::AsyncBlockRead as u64 => sys_async_block_read(arg0),
        value if value == Number::AsyncBlockWrite as u64 => sys_async_block_write(arg0, arg1),
        value if value == Number::AsyncBlockPoll as u64 => sys_async_block_poll(arg0, arg1),
        value if value == Number::AsyncBlockWait as u64 => sys_async_block_wait(arg0, arg1),
        value if value == Number::AsyncFileRead as u64 => sys_async_file_read(arg0, arg1),
        value if value == Number::AsyncFileWrite as u64 => sys_async_file_write(arg0, arg1),
        value if value == Number::AsyncFilePoll as u64 => sys_async_file_poll(arg0, arg1, arg2),
        value if value == Number::AsyncFileWait as u64 => sys_async_file_wait(arg0, arg1, arg2),
        value if value == Number::AsyncNetSend as u64 => sys_async_net_send(arg0, arg1, arg2),
        value if value == Number::AsyncNetRecv as u64 => sys_async_net_recv(arg0, arg1),
        value if value == Number::AsyncNetPoll as u64 => sys_async_net_poll(arg0, arg1, arg2),
        value if value == Number::AsyncNetWait as u64 => sys_async_net_wait(arg0, arg1, arg2),
        value if value == Number::AsyncTcpConnect as u64 => sys_async_tcp_connect(arg0, arg1),
        value if value == Number::CompletionPortCreate as u64 => {
            crate::completion_port::create(crate::task::current_process_id())
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::CompletionPortClose as u64 => {
            crate::completion_port::close(crate::task::current_process_id(), arg0)
                .map_or(SYSCALL_ERROR, |()| 0)
        }
        value if value == Number::CompletionPortAssociate as u64 => async_handle(arg0)
            .ok()
            .and_then(|handle| crate::async_op::associate_current(handle, arg1, arg2).ok())
            .map_or(SYSCALL_ERROR, |()| 0),
        value if value == Number::CompletionPortPoll as u64 => {
            sys_port_read(arg0, arg1, arg2, false)
        }
        value if value == Number::CompletionPortWait as u64 => {
            sys_port_read(arg0, arg1, arg2, true)
        }
        value if value == Number::TimerCreate as u64 => sys_timer_create(arg0, arg1),
        value if value == Number::TimerWait as u64 => sys_timer_wait(arg0),
        value if value == Number::EventCreate as u64 => sys_event_create(arg0),
        value if value == Number::EventSet as u64 => sys_event_set(arg0, arg1),
        value if value == Number::EventWait as u64 => sys_event_wait(arg0),
        value if value == Number::TimerClose as u64 => sys_timer_close(arg0),
        value if value == Number::EventClose as u64 => sys_event_close(arg0),
        value if value == Number::SleepUntil as u64 => sys_sleep_until(arg0),
        value if value == Number::GraphicsSurfaceCreate as u64 => sys_graphics_create(arg0, arg1),
        value if value == Number::GraphicsSurfaceFill as u64 => sys_graphics_fill(arg0, arg1, arg2),
        value if value == Number::GraphicsSurfacePixel as u64 => sys_graphics_pixel(arg0, arg1),
        value if value == Number::GraphicsSurfaceDestroy as u64 => sys_graphics_destroy(arg0),
        value if value == Number::GraphicsCompositorBlit as u64 => sys_compositor_blit(arg0, arg1),
        value if value == Number::GraphicsCompositorPixel as u64 => sys_compositor_pixel(arg0),
        value if value == Number::GraphicsSurfaceStroke as u64 => {
            sys_graphics_stroke(arg0, arg1, arg2)
        }
        value if value == Number::GraphicsCompositorClaim as u64 => sys_compositor_claim(),
        value if value == Number::GraphicsCompositorPresent as u64 => sys_compositor_present(),
        value if value == Number::InputPoll as u64 => sys_input_poll(),
        value if value == Number::InputWait as u64 => sys_input_wait(arg0),
        value if value == Number::GraphicsSurfaceShare as u64 => sys_graphics_share(arg0, arg1),
        value if value == Number::GraphicsCompositorBlitShared as u64 => {
            sys_compositor_blit_shared(arg0, arg1)
        }
        value if value == Number::WaitAny as u64 => sys_wait_any(arg0, arg1),
        value if value == Number::WaitAnyObjects as u64 => sys_wait_any_objects(arg0, arg1, arg2),
        value if value == Number::GraphicsSurfaceDamage as u64 => sys_graphics_damage(arg0, arg1),
        value if value == Number::GraphicsCompositorBlitDamage as u64 => {
            sys_compositor_blit_damage(arg0, arg1)
        }
        value if value == Number::GraphicsCompositorBlend as u64 => {
            sys_compositor_blend(arg0, arg1, arg2, false)
        }
        value if value == Number::GraphicsCompositorBlendShared as u64 => {
            sys_compositor_blend(arg0, arg1, arg2, true)
        }
        value if value == Number::GraphicsCompositorInfo as u64 => sys_compositor_info(),
        value if value == Number::GraphicsCompositorScale as u64 => sys_compositor_scale(arg0),
        value if value == Number::GraphicsSurfaceWrite as u64 => {
            sys_graphics_write(arg0, arg1, arg2)
        }
        value if value == Number::ClockRead as u64 => sys_clock_read(),
        value if value == Number::GraphicsCompositorBlitRegion as u64 => {
            sys_compositor_blit_region(arg0, arg1, arg2, false)
        }
        value if value == Number::GraphicsCompositorBlitRegionShared as u64 => {
            sys_compositor_blit_region(arg0, arg1, arg2, true)
        }
        value if value == Number::PermissionRequest as u64 => {
            match crate::permission::Permission::from_u64(arg0) {
                Some(permission) => {
                    crate::permission::request_current(permission, arg1).unwrap_or(SYSCALL_ERROR)
                }
                None => SYSCALL_ERROR,
            }
        }
        value if value == Number::PermissionPoll as u64 => {
            match crate::permission::state_current(arg0) {
                Ok(crate::permission::State::Pending) => 0,
                Ok(crate::permission::State::Resolving) => 1,
                Ok(crate::permission::State::Granted) => 2,
                Ok(crate::permission::State::Denied) => 3,
                Ok(crate::permission::State::Revoked) => 4,
                Err(_) => SYSCALL_ERROR,
            }
        }
        value if value == Number::PermissionDecide as u64 => {
            crate::permission::decide(arg0, arg1 != 0)
                .map(|_| 0)
                .unwrap_or(SYSCALL_ERROR)
        }
        value if value == Number::PermissionRevoke as u64 => crate::permission::revoke(arg0)
            .map(|_| 0)
            .unwrap_or(SYSCALL_ERROR),
        value if value == Number::AuditRead as u64 => sys_audit_read(arg0, arg1),
        value if value == Number::SecretStore as u64 => sys_secret_store(arg0, arg1),
        value if value == Number::SecretRead as u64 => sys_secret_read(arg0, arg1, arg2),
        value if value == Number::SecretRevoke as u64 => sys_secret_revoke(arg0),
        value if value == Number::GraphicsCompositorBlitKeyed as u64 => {
            sys_compositor_blit_keyed(arg0, arg1, arg2)
        }
        value if value == Number::GraphicsCompositorBlendRegion as u64 => {
            sys_compositor_blend_region(arg0, arg1, arg2, false)
        }
        value if value == Number::GraphicsCompositorBlendRegionShared as u64 => {
            sys_compositor_blend_region(arg0, arg1, arg2, true)
        }
        value if value == Number::AudioInfo as u64 => sys_audio_info(arg0),
        value if value == Number::AudioPlay as u64 => sys_audio_play(arg0, arg1),
        value if value == Number::AudioSetVolume as u64 => sys_audio_set_volume(arg0),
        value if value == Number::GraphicsCompositorOutputs as u64 => sys_compositor_outputs(arg0),
        value if value == Number::PowerControl as u64 => sys_power_control(arg0),
        value if value == Number::PowerCapabilities as u64 => sys_power_capabilities(arg0, arg1),
        value if value == Number::CpuIdleCapabilities as u64 => {
            sys_cpu_idle_capabilities(arg0, arg1)
        }
        value if value == Number::CpuFrequencyInfo as u64 => sys_cpu_frequency_info(arg0, arg1),
        value if value == Number::SuspendResumeCapabilities as u64 => {
            sys_suspend_resume_capabilities(arg0, arg1)
        }
        value if value == Number::AccountsChangePin as u64 => sys_accounts_change_pin(arg0, arg1),
        #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
        value if value == Number::IdentityOpen as u64 => sys_identity_open(arg0, arg1),
        #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
        value if value == Number::IdentityLock as u64 => sys_identity_lock(arg0),
        #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
        value if value == Number::IdentityUnlock as u64 => {
            sys_identity_password_action(arg0, arg1, arg2, false)
        }
        #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
        value if value == Number::IdentityElevate as u64 => {
            sys_identity_password_action(arg0, arg1, arg2, true)
        }
        #[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
        value if value == Number::IdentityClose as u64 => sys_identity_close(arg0),
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityOpenVerified as u64 => {
            sys_identity_open_verified(arg0, arg1)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityUnlockVerified as u64 => {
            sys_identity_unlock_verified(arg0, arg1, arg2)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminAddUser as u64 => {
            sys_identity_admin_add_user(arg0, arg1)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminDeleteUser as u64 => {
            sys_identity_admin_delete_user(arg0)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminAddGroup as u64 => {
            sys_identity_admin_add_group(arg0, arg1)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminAddMember as u64 => {
            sys_identity_admin_group_member(arg0, arg1, true)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminRemoveMember as u64 => {
            sys_identity_admin_group_member(arg0, arg1, false)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminQuery as u64 => {
            sys_identity_admin_query(arg0, arg1, arg2)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityAdminSetLockout as u64 => {
            sys_identity_admin_set_lockout(arg0)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityConsumeHardware as u64 => {
            sys_identity_consume_hardware(arg0)
        }
        #[cfg(feature = "system-apps")]
        value if value == Number::IdentityRequestHardware as u64 => {
            sys_identity_request_hardware(arg0)
        }
        _ => u64::MAX,
    };

    // Scheduler-owned termination requests must be acknowledged before the
    // syscall returns to ring 3. This primarily covers self-termination; a
    // remotely running task is retired after its next physical hand-off.
    crate::task::honor_current_termination_request();
    prepare_signal_delivery(frame);
    LAST_COMPLETED.store(number, Ordering::Release);
    COMPLETIONS.fetch_add(1, Ordering::Release);
    result
}

// -------------------------------------------------------------- graphics

/// Allocate a client surface for the current process.
///
/// `arg0`/`arg1` carry width and height as u32. The returned token packs the
/// slot, generation and owner, so an impostor process holding a leaked value
/// cannot drive the surface. Caps are enforced in `user_graphics`.
fn sys_graphics_create(width: u64, height: u64) -> u64 {
    let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
        return SYSCALL_ERROR;
    };
    let owner = crate::task::current_process_id();
    crate::user_graphics::create(owner, width, height)
        .map(|handle| handle.to_u64())
        .unwrap_or(SYSCALL_ERROR)
}

/// Fill a rectangle in a client surface.
///
/// `arg1` packs `x | (y << 16) | (w << 32) | (h << 48)` as four u16 fields.
/// Clients that need larger coordinates will reach a differently packed
/// successor; the surface caps keep every value well within u16.
fn sys_graphics_fill(handle: u64, packed: u64, color: u64) -> u64 {
    let x = (packed & 0xffff) as u16 as i32;
    let y = ((packed >> 16) & 0xffff) as u16 as i32;
    let width = ((packed >> 32) & 0xffff) as usize;
    let height = ((packed >> 48) & 0xffff) as usize;
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    crate::user_graphics::fill_rect(
        handle,
        crate::display::Rect::new(x, y, width, height),
        color as u32,
    )
    .map_or(SYSCALL_ERROR, |()| 0)
}

/// Upload pixels into a rectangle of one of the caller's surfaces. This is
/// how a client draws anything fill and stroke cannot express: text,
/// gradients, icons, rendered in its own memory and copied in.
///
/// The rectangle is checked against the surface before anything is copied,
/// which also bounds the copy by the surface caps. The pixels are copied out
/// of user memory with no lock held (the copy can fault), and the registry
/// re-resolves the handle when writing, so a surface destroyed meanwhile is
/// refused rather than written.
fn sys_graphics_write(handle: u64, packed: u64, user_pixels: u64) -> u64 {
    let x = (packed & 0xffff) as usize;
    let y = ((packed >> 16) & 0xffff) as usize;
    let width = ((packed >> 32) & 0xffff) as usize;
    let height = ((packed >> 48) & 0xffff) as usize;
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    let Ok((surface_width, surface_height)) = crate::user_graphics::dimensions(handle) else {
        return SYSCALL_ERROR;
    };
    if width == 0 || height == 0 || x + width > surface_width || y + height > surface_height {
        return SYSCALL_ERROR;
    }
    let Some(mut bytes) = crate::heap::try_filled(0u8, width * height * 4) else {
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_from_current_user(user_pixels, &mut bytes).is_err() {
        return SYSCALL_ERROR;
    }
    let rect = crate::display::Rect::new(x as i32, y as i32, width, height);
    crate::user_graphics::write_pixels(handle, rect, &bytes).map_or(SYSCALL_ERROR, |()| 0)
}

/// The wall-clock date and time from the CMOS clock, UTC. Needs no
/// capability, like `Getticks`: the time of day is not a secret.
fn sys_clock_read() -> u64 {
    crate::rtc::read().map_or(SYSCALL_ERROR, crate::rtc::DateTime::pack)
}

/// Read one pixel from a client surface, returning its packed colour.
///
/// `arg1` packs `x | (y << 32)` as two u32s. Returning the colour directly
/// means a client can verify what it drew without a buffer copy.
fn sys_graphics_pixel(handle: u64, packed: u64) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    crate::user_graphics::pixel(handle, x, y)
        .map(u64::from)
        .unwrap_or(SYSCALL_ERROR)
}

/// Release a client surface.
fn sys_graphics_destroy(handle: u64) -> u64 {
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    crate::user_graphics::destroy(handle).map_or(SYSCALL_ERROR, |()| 0)
}

/// Resolve a client surface token for the calling process.
///
/// The token's owner field comes from userspace, so it must name the caller;
/// otherwise a process could forge a token naming another process as owner.
fn caller_surface(raw: u64) -> Option<crate::user_graphics::Handle> {
    let handle = crate::user_graphics::Handle::from_u64(raw);
    handle
        .is_owned_by(crate::task::current_process_id())
        .then_some(handle)
}

/// Whether the caller may use the compositor at all. The lease checked inside
/// the compositor narrows this to a single holder.
fn display_authorized() -> bool {
    crate::task::current_has(crate::capability::Capability::DisplayIo)
}

/// Blit a client surface onto the compositor at `packed = x | (y << 32)`.
///
/// Requires `DisplayIo` and the compositor lease, and the surface must belong
/// to the caller.
fn sys_compositor_blit(handle: u64, packed: u64) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    let pid = crate::task::current_process_id();
    crate::compositor::blit_for(pid, handle, x, y).map_or(SYSCALL_ERROR, |_| 0)
}

/// Share one of the caller's own surfaces with process `grantee`, or revoke
/// with 0. No capability: a process may always decide who reads its pixels.
fn sys_graphics_share(handle: u64, grantee: u64) -> u64 {
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    let grantee = (grantee != 0).then_some(grantee);
    crate::user_graphics::share(handle, grantee).map_or(SYSCALL_ERROR, |()| 0)
}

/// Blit a surface another process shared with the caller. Requires
/// `DisplayIo` and the lease. The handle is the owner's token, so it is not
/// bound to the caller; the registry's grant is what authorises the read.
fn sys_compositor_blit_shared(handle: u64, packed: u64) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let pid = crate::task::current_process_id();
    let handle = crate::user_graphics::Handle::from_u64(handle);
    crate::compositor::blit_shared_for(pid, handle, x, y).map_or(SYSCALL_ERROR, |_| 0)
}

/// Blit only a region of a surface onto the compositor. Same authority as the
/// corresponding whole-surface blit: `DisplayIo` and the lease, and an own
/// surface must be the caller's, a shared one must carry a grant naming the
/// caller.
fn sys_compositor_blit_region(handle: u64, destination: u64, region: u64, shared: bool) -> u64 {
    let x = (destination & 0xffff_ffff) as i32;
    let y = ((destination >> 32) & 0xffff_ffff) as i32;
    let region = crate::display::Rect::new(
        (region & 0xffff) as i32,
        ((region >> 16) & 0xffff) as i32,
        ((region >> 32) & 0xffff) as usize,
        ((region >> 48) & 0xffff) as usize,
    );
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let pid = crate::task::current_process_id();
    let handle = if shared {
        crate::user_graphics::Handle::from_u64(handle)
    } else {
        let Some(handle) = caller_surface(handle) else {
            return SYSCALL_ERROR;
        };
        handle
    };
    crate::compositor::blit_region_for(pid, handle, region, x, y, shared)
        .map_or(SYSCALL_ERROR, |_| 0)
}

/// Region blit of one of the caller's own surfaces that skips pixels equal
/// to the transparent key (`display::TRANSPARENT_KEY`): an overlay of text,
/// borders or a pointer drawn over what is already composed. Same authority
/// as the region blit.
fn sys_compositor_blit_keyed(handle: u64, destination: u64, region: u64) -> u64 {
    let x = (destination & 0xffff_ffff) as i32;
    let y = ((destination >> 32) & 0xffff_ffff) as i32;
    let region = crate::display::Rect::new(
        (region & 0xffff) as i32,
        ((region >> 16) & 0xffff) as i32,
        ((region >> 32) & 0xffff) as usize,
        ((region >> 48) & 0xffff) as usize,
    );
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    crate::compositor::blit_region_keyed_for(
        crate::task::current_process_id(),
        handle,
        region,
        x,
        y,
    )
    .map_or(SYSCALL_ERROR, |_| 0)
}

/// Sign-extend the low 24 bits of `value`.
const fn signed_24(value: u64) -> i32 {
    ((value as u32) << 8) as i32 >> 8
}

/// Blend only a region of a surface onto the compositor with uniform
/// opacity. Three registers carry four values, so the destination word packs
/// x and y as 24-bit signed fields (ample for a compositor bounded at
/// 1920x1200) with the opacity in its top byte. The byte between them is
/// reserved and must be zero. Same authority as the region blit.
fn sys_compositor_blend_region(handle: u64, destination: u64, region: u64, shared: bool) -> u64 {
    if (destination >> 48) & 0xff != 0 {
        return SYSCALL_ERROR;
    }
    let x = signed_24(destination);
    let y = signed_24(destination >> 24);
    let opacity = (destination >> 56) as u8;
    let region = crate::display::Rect::new(
        (region & 0xffff) as i32,
        ((region >> 16) & 0xffff) as i32,
        ((region >> 32) & 0xffff) as usize,
        ((region >> 48) & 0xffff) as usize,
    );
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let pid = crate::task::current_process_id();
    let handle = if shared {
        crate::user_graphics::Handle::from_u64(handle)
    } else {
        let Some(handle) = caller_surface(handle) else {
            return SYSCALL_ERROR;
        };
        handle
    };
    crate::compositor::blend_region_for(pid, handle, region, x, y, opacity, shared)
        .map_or(SYSCALL_ERROR, |_| 0)
}

/// Blend a surface onto the compositor with uniform opacity. Same authority
/// as the corresponding blit: `DisplayIo` and the lease; an own surface must
/// be the caller's, a shared one must carry a grant naming the caller. An
/// opacity above 255 is refused rather than truncated.
fn sys_compositor_blend(handle: u64, packed: u64, opacity: u64, shared: bool) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    let Ok(opacity) = u8::try_from(opacity) else {
        return SYSCALL_ERROR;
    };
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let pid = crate::task::current_process_id();
    let result = if shared {
        crate::compositor::blend_shared_for(
            pid,
            crate::user_graphics::Handle::from_u64(handle),
            x,
            y,
            opacity,
        )
    } else {
        let Some(handle) = caller_surface(handle) else {
            return SYSCALL_ERROR;
        };
        crate::compositor::blend_for(pid, handle, x, y, opacity)
    };
    result.map_or(SYSCALL_ERROR, |_| 0)
}

/// Pack surface geometry and scale as `width | height << 24 | scale << 48`.
/// The surface is at most `MAX_WIDTH`×`MAX_HEIGHT`, well inside 24 bits.
fn pack_compositor_info(width: u32, height: u32, scale: u32) -> u64 {
    u64::from(width) | (u64::from(height) << 24) | (u64::from(scale) << 48)
}

/// Turn the machine off (action 0) or restart it (action 1). Requires
/// PowerControl, which only the desktop session holds; every request is in
/// the security ledger. Returns only when the machine is still running:
/// 1 no ACPI power-off, 2 the firmware refused ACPI mode, 3 ignored.
fn sys_power_control(action: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let allowed =
        crate::task::current_has(crate::capability::Capability::PowerControl) && action <= 2;
    crate::audit::record(actor, crate::audit::Action::PowerControl, action, allowed);
    if !allowed {
        return SYSCALL_ERROR;
    }
    if action == 2 {
        // The kernel wake path currently restores a kernel continuation that
        // owns rollback, not the interrupted userspace syscall frame. Refuse
        // the transition until a task-safe return target is committed; do not
        // leave drivers, CPUs, or firmware state partially prepared.
        crate::serial::write_line(format_args!(
            "[POWER] suspend denied: no safe userspace resume target"
        ));
        return 4;
    }
    if action == 1 {
        crate::power::reboot();
    }
    match crate::power::power_off() {
        crate::power::Error::NoAcpi => 1,
        crate::power::Error::AcpiModeRefused => 2,
        crate::power::Error::StillRunning => 3,
    }
}

/// Report the firmware power transitions without performing one. The fixed
/// three-byte result is copied only after validating the userspace range.
fn sys_power_capabilities(user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let allowed = crate::task::current_has(crate::capability::Capability::PowerControl);
    crate::audit::record(actor, crate::audit::Action::PowerControl, 2, allowed);
    if !allowed || length < 3 {
        return SYSCALL_ERROR;
    }
    let Some(power) = crate::hal::acpi::power() else {
        return SYSCALL_ERROR;
    };
    let result = [
        u8::from(power.s5.is_some()),
        u8::from(power.s3.is_some()),
        1,
    ];
    if crate::paging::copy_to_current_user(user_buffer, &result).is_err() {
        return SYSCALL_ERROR;
    }
    0
}

/// Report CPU idle primitives without executing an idle instruction or
/// changing per-CPU state. The fixed three-byte result is copied only after
/// validating the userspace range.
fn sys_cpu_idle_capabilities(user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let allowed = crate::task::current_has(crate::capability::Capability::PowerControl);
    crate::audit::record(actor, crate::audit::Action::PowerControl, 3, allowed);
    if !allowed || length < 3 {
        return SYSCALL_ERROR;
    }
    let capabilities = crate::hal::cpu::idle_capabilities();
    let result = [
        u8::from(capabilities.hlt),
        u8::from(capabilities.monitor_mwait),
        u8::from(capabilities.invariant_tsc),
    ];
    if crate::paging::copy_to_current_user(user_buffer, &result).is_err() {
        return SYSCALL_ERROR;
    }
    0
}

/// Report CPUID leaf 0x16 frequency inventory without changing CPU state.
/// The fixed 13-byte result is copied only after validating the user range.
fn sys_cpu_frequency_info(user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let allowed = crate::task::current_has(crate::capability::Capability::PowerControl);
    crate::audit::record(actor, crate::audit::Action::PowerControl, 4, allowed);
    if !allowed || length < 13 {
        return SYSCALL_ERROR;
    }
    let mut result = [0u8; 13];
    if let Some(info) = crate::hal::cpu::frequency_info() {
        result[0] = 1;
        result[1..5].copy_from_slice(&info.base_mhz.to_le_bytes());
        result[5..9].copy_from_slice(&info.maximum_mhz.to_le_bytes());
        result[9..13].copy_from_slice(&info.bus_mhz.to_le_bytes());
    }
    if crate::paging::copy_to_current_user(user_buffer, &result).is_err() {
        return SYSCALL_ERROR;
    }
    0
}

/// Report the validated prerequisites for a future ACPI S3 transition and
/// firmware wake path. This query never writes FACS or PM1 control state.
fn sys_suspend_resume_capabilities(user_buffer: u64, length: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let allowed = crate::task::current_has(crate::capability::Capability::PowerControl);
    crate::audit::record(actor, crate::audit::Action::PowerControl, 5, allowed);
    if !allowed || length < 5 {
        return SYSCALL_ERROR;
    }
    let Some(power) = crate::hal::acpi::power() else {
        return SYSCALL_ERROR;
    };
    let mut result = [0_u8; 5];
    result[0] = u8::from(power.s3.is_some());
    if let Some(facs) = power.facs {
        result[1] = 1;
        result[2] = u8::from(facs.waking_vector_32 != 0);
        result[3] = u8::from(facs.waking_vector_64 != 0);
        result[4] = facs.version;
    }
    if crate::paging::copy_to_current_user(user_buffer, &result).is_err() {
        return SYSCALL_ERROR;
    }
    0
}

/// Pull one `[len u8][bytes]` field (`len <= max`) from the front of
/// `packed`, returning it and the remainder.
fn split_length_prefixed(packed: &[u8], max: usize) -> Option<(&[u8], &[u8])> {
    let (&length, rest) = packed.split_first()?;
    let length = length as usize;
    if length > max || length > rest.len() {
        return None;
    }
    Some(rest.split_at(length))
}

/// Change a local account's own PIN. The buffer packs `[name_len u8][name]
/// [old_pin_len u8][old_pin][new_pin_len u8][new_pin]`. Requires
/// AccountManage, which only the desktop session holds; every attempt is in
/// the security ledger, but never the PINs themselves.
fn sys_accounts_change_pin(user_buffer: u64, length: u64) -> u64 {
    const MAX_BUFFER: usize = 3 + crate::accounts::MAX_NAME + 2 * crate::accounts::MAX_PIN;
    let actor = crate::task::current_task_id().as_u64();
    if !crate::task::current_has(crate::capability::Capability::AccountManage) {
        crate::audit::record(actor, crate::audit::Action::AccountChangePin, 0, false);
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        crate::audit::record(actor, crate::audit::Action::AccountChangePin, 0, false);
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        crate::audit::record(actor, crate::audit::Action::AccountChangePin, 0, false);
        return SYSCALL_ERROR;
    }
    let packed = &buffer[..length];
    let parsed =
        split_length_prefixed(packed, crate::accounts::MAX_NAME).and_then(|(name, rest)| {
            let (old_pin, rest) = split_length_prefixed(rest, crate::accounts::MAX_PIN)?;
            let (new_pin, rest) = split_length_prefixed(rest, crate::accounts::MAX_PIN)?;
            rest.is_empty().then_some((name, old_pin, new_pin))
        });
    let Some((name, old_pin, new_pin)) = parsed else {
        crate::audit::record(actor, crate::audit::Action::AccountChangePin, 0, false);
        return SYSCALL_ERROR;
    };
    #[cfg(feature = "system-apps")]
    let result = crate::identity_runtime::change_pin(
        crate::task::current_process_id(),
        name,
        old_pin,
        new_pin,
    );
    #[cfg(not(feature = "system-apps"))]
    let result = crate::accounts::change_pin(name, old_pin, new_pin);
    crate::audit::record(
        actor,
        crate::audit::Action::AccountChangePin,
        0,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |()| 0)
}

#[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
fn sys_identity_open(user_buffer: u64, length: u64) -> u64 {
    const MAX_BUFFER: usize = 2 + crate::accounts::MAX_NAME + crate::accounts::MAX_PIN;
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Some((name, password)) = crate::identity_runtime::parse_open_buffer(&buffer[..length])
    else {
        return SYSCALL_ERROR;
    };
    let result = crate::identity_runtime::open(crate::task::current_process_id(), name, password);
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityOpen,
        0,
        result.is_ok(),
    );
    result.map(|session| session.0).unwrap_or(SYSCALL_ERROR)
}

#[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
fn sys_identity_lock(session: u64) -> u64 {
    let result = crate::identity_runtime::lock(
        crate::task::current_process_id(),
        libwoven::identity::SessionId(session),
    );
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityLock,
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
fn sys_identity_password_action(session: u64, user_buffer: u64, length: u64, elevate: bool) -> u64 {
    const MAX_BUFFER: usize = 1 + crate::accounts::MAX_PIN;
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Some(password) = crate::identity_runtime::parse_password_buffer(&buffer[..length]) else {
        return SYSCALL_ERROR;
    };
    let owner = crate::task::current_process_id();
    let result = if elevate {
        crate::identity_runtime::elevate(owner, libwoven::identity::SessionId(session), password)
    } else {
        crate::identity_runtime::unlock(owner, libwoven::identity::SessionId(session), password)
    };
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        if elevate {
            crate::audit::Action::IdentityElevate
        } else {
            crate::audit::Action::IdentityUnlock
        },
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(any(feature = "stage24-5-test", feature = "system-apps"))]
fn sys_identity_close(session: u64) -> u64 {
    let result = crate::identity_runtime::close(
        crate::task::current_process_id(),
        libwoven::identity::SessionId(session),
    );
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityClose,
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_consume_hardware(session: u64) -> u64 {
    let result = crate::identity_runtime::consume_pending_hardware_decision(
        crate::task::current_process_id(),
        libwoven::identity::SessionId(session),
    );
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityHardware,
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_request_hardware(session: u64) -> u64 {
    let result = crate::identity_runtime::request_hardware_decision(
        crate::task::current_process_id(),
        libwoven::identity::SessionId(session),
    );
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityHardware,
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_open_verified(user_buffer: u64, length: u64) -> u64 {
    const MAX_BUFFER: usize = 1 + crate::accounts::MAX_NAME + 32;
    if !crate::task::current_has(crate::capability::Capability::AccountManage) {
        return SYSCALL_ERROR;
    }
    let Ok(length) = usize::try_from(length) else {
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        return SYSCALL_ERROR;
    }
    let Some((name, rest)) = crate::identity_runtime::parse_length_prefixed(
        &buffer[..length],
        crate::accounts::MAX_NAME,
    ) else {
        return SYSCALL_ERROR;
    };
    let Ok(digest) = rest.try_into() else {
        return SYSCALL_ERROR;
    };
    let result =
        crate::identity_runtime::open_digest(crate::task::current_process_id(), name, &digest);
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityOpen,
        0,
        result.is_ok(),
    );
    result.map(|session| session.0).unwrap_or(SYSCALL_ERROR)
}

#[cfg(feature = "system-apps")]
fn sys_identity_unlock_verified(session: u64, user_buffer: u64, length: u64) -> u64 {
    if !crate::task::current_has(crate::capability::Capability::AccountManage) || length != 32 {
        return SYSCALL_ERROR;
    }
    let mut digest = [0u8; 32];
    if crate::paging::copy_from_current_user(user_buffer, &mut digest).is_err() {
        return SYSCALL_ERROR;
    }
    let result = crate::identity_runtime::unlock_digest(
        crate::task::current_process_id(),
        libwoven::identity::SessionId(session),
        &digest,
    );
    crate::audit::record(
        crate::task::current_task_id().as_u64(),
        crate::audit::Action::IdentityUnlock,
        session,
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_add_user(user_buffer: u64, length: u64) -> u64 {
    const MAX_BUFFER: usize = 3 + crate::accounts::MAX_NAME + crate::accounts::MAX_PIN;
    let actor = crate::task::current_task_id().as_u64();
    let Ok(length) = usize::try_from(length) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    }
    let Some((name, rest)) = crate::identity_runtime::parse_length_prefixed(
        &buffer[..length],
        crate::accounts::MAX_NAME,
    ) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    };
    let Some((password, rest)) =
        crate::identity_runtime::parse_length_prefixed(rest, crate::accounts::MAX_PIN)
    else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    };
    if rest.len() != 1 {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddUser, 0, false);
        return SYSCALL_ERROR;
    }
    let result = crate::identity_runtime::admin_add_user(
        crate::task::current_process_id(),
        name,
        password,
        rest[0] != 0,
    );
    crate::audit::record(
        actor,
        crate::audit::Action::IdentityAdminAddUser,
        0,
        result.is_ok(),
    );
    result.map(|user| user.0 as u64).unwrap_or(SYSCALL_ERROR)
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_delete_user(user: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let Ok(user) = u32::try_from(user) else {
        crate::audit::record(
            actor,
            crate::audit::Action::IdentityAdminDeleteUser,
            0,
            false,
        );
        return SYSCALL_ERROR;
    };
    let result = crate::identity_runtime::admin_delete_user(
        crate::task::current_process_id(),
        libwoven::identity::UserId(user),
    );
    crate::audit::record(
        actor,
        crate::audit::Action::IdentityAdminDeleteUser,
        u64::from(user),
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_add_group(user_buffer: u64, length: u64) -> u64 {
    const MAX_BUFFER: usize = 1 + crate::accounts::MAX_NAME;
    let actor = crate::task::current_task_id().as_u64();
    let Ok(length) = usize::try_from(length) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddGroup, 0, false);
        return SYSCALL_ERROR;
    };
    if length == 0 || length > MAX_BUFFER {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddGroup, 0, false);
        return SYSCALL_ERROR;
    }
    let mut buffer = [0u8; MAX_BUFFER];
    if crate::paging::copy_from_current_user(user_buffer, &mut buffer[..length]).is_err() {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddGroup, 0, false);
        return SYSCALL_ERROR;
    }
    let Some((name, rest)) = crate::identity_runtime::parse_length_prefixed(
        &buffer[..length],
        crate::accounts::MAX_NAME,
    ) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddGroup, 0, false);
        return SYSCALL_ERROR;
    };
    if !rest.is_empty() {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminAddGroup, 0, false);
        return SYSCALL_ERROR;
    }
    let result = crate::identity_runtime::admin_add_group(crate::task::current_process_id(), name);
    crate::audit::record(
        actor,
        crate::audit::Action::IdentityAdminAddGroup,
        0,
        result.is_ok(),
    );
    result.map(|group| group.0 as u64).unwrap_or(SYSCALL_ERROR)
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_group_member(user: u64, group: u64, add: bool) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let action = if add {
        crate::audit::Action::IdentityAdminAddMember
    } else {
        crate::audit::Action::IdentityAdminRemoveMember
    };
    let (Ok(user), Ok(group)) = (u32::try_from(user), u32::try_from(group)) else {
        crate::audit::record(actor, action, 0, false);
        return SYSCALL_ERROR;
    };
    let result = crate::identity_runtime::admin_group_member(
        crate::task::current_process_id(),
        libwoven::identity::UserId(user),
        libwoven::identity::GroupId(group),
        add,
    );
    crate::audit::record(
        actor,
        action,
        (u64::from(group) << 32) | u64::from(user),
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_query(user_output: u64, capacity: u64, kind: u64) -> u64 {
    const MAX_OUTPUT: usize = libwoven::identity::ADMIN_QUERY_MAX_BYTES;
    let actor = crate::task::current_task_id().as_u64();
    let Ok(capacity) = usize::try_from(capacity) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminQuery, kind, false);
        return SYSCALL_ERROR;
    };
    let Ok(kind) = u8::try_from(kind) else {
        crate::audit::record(actor, crate::audit::Action::IdentityAdminQuery, 0, false);
        return SYSCALL_ERROR;
    };
    if capacity == 0 || capacity > MAX_OUTPUT || kind > libwoven::identity::ADMIN_QUERY_GROUPS {
        crate::audit::record(
            actor,
            crate::audit::Action::IdentityAdminQuery,
            u64::from(kind),
            false,
        );
        return SYSCALL_ERROR;
    }
    let mut output = [0u8; MAX_OUTPUT];
    let result = crate::identity_runtime::admin_query(
        crate::task::current_process_id(),
        kind,
        &mut output[..capacity],
    );
    let Ok(length) = result else {
        crate::audit::record(
            actor,
            crate::audit::Action::IdentityAdminQuery,
            u64::from(kind),
            false,
        );
        return SYSCALL_ERROR;
    };
    if crate::paging::copy_to_current_user(user_output, &output[..length]).is_err() {
        crate::audit::record(
            actor,
            crate::audit::Action::IdentityAdminQuery,
            u64::from(kind),
            false,
        );
        return SYSCALL_ERROR;
    }
    crate::audit::record(
        actor,
        crate::audit::Action::IdentityAdminQuery,
        u64::from(kind),
        true,
    );
    length as u64
}

#[cfg(feature = "system-apps")]
fn sys_identity_admin_set_lockout(failures: u64) -> u64 {
    let actor = crate::task::current_task_id().as_u64();
    let Ok(failures) = u8::try_from(failures) else {
        crate::audit::record(
            actor,
            crate::audit::Action::IdentityAdminSetLockout,
            0,
            false,
        );
        return SYSCALL_ERROR;
    };
    let result =
        crate::identity_runtime::admin_set_lockout(crate::task::current_process_id(), failures);
    crate::audit::record(
        actor,
        crate::audit::Action::IdentityAdminSetLockout,
        u64::from(failures),
        result.is_ok(),
    );
    result.map_or(SYSCALL_ERROR, |_| 0)
}

/// The compositor surface's size and display scale. Requires `DisplayIo` and
/// the lease: the geometry is the holder's to draw on.
fn sys_compositor_outputs(user_outputs: u64) -> u64 {
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let Ok((outputs, count)) = crate::compositor::outputs_for(crate::task::current_process_id())
    else {
        return SYSCALL_ERROR;
    };
    let mut bytes = [0u8; crate::compositor::MAX_OUTPUTS * 16];
    for (output, chunk) in outputs.iter().zip(bytes.as_chunks_mut::<16>().0) {
        for (value, field) in [output.0, output.1, output.2, output.3]
            .iter()
            .zip(chunk.as_chunks_mut::<4>().0)
        {
            *field = value.to_le_bytes();
        }
    }
    if crate::paging::copy_to_current_user(user_outputs, &bytes).is_err() {
        return SYSCALL_ERROR;
    }
    count as u64
}

fn sys_compositor_info() -> u64 {
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    crate::compositor::info_for(crate::task::current_process_id())
        .map_or(SYSCALL_ERROR, |(width, height, scale)| {
            pack_compositor_info(width, height, scale)
        })
}

/// Set the integer display scale (1..=`MAX_SCALE`). The surface is
/// reallocated at the framebuffer's size divided by the scale and starts
/// black. Requires `DisplayIo` and the lease. Returns the new geometry.
fn sys_compositor_scale(scale: u64) -> u64 {
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let Ok(scale) = u32::try_from(scale) else {
        return SYSCALL_ERROR;
    };
    crate::compositor::set_scale_for(crate::task::current_process_id(), scale)
        .map_or(SYSCALL_ERROR, |(width, height)| {
            pack_compositor_info(width, height, scale)
        })
}

/// Take the compositor lease. Requires `DisplayIo`.
fn sys_compositor_claim() -> u64 {
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    crate::compositor::claim(crate::task::current_process_id()).map_or(SYSCALL_ERROR, |()| 0)
}

/// Event kinds in the top byte of an `InputPoll` result.
const INPUT_KIND_CHAR: u64 = 1;
const INPUT_KIND_ENTER: u64 = 2;
const INPUT_KIND_BACKSPACE: u64 = 3;
const INPUT_KIND_TAB: u64 = 4;
const INPUT_KIND_F1: u64 = 5;
const INPUT_KIND_F2: u64 = 8;
const INPUT_KIND_F3: u64 = 14;
const INPUT_KIND_F4: u64 = 15;
const INPUT_KIND_POINTER_MOVE: u64 = 6;
const INPUT_KIND_POINTER_BUTTON: u64 = 7;
const INPUT_KIND_SCROLL: u64 = 9;
const INPUT_KIND_POINTER_ABSOLUTE: u64 = 10;
const INPUT_KIND_TOUCH: u64 = 11;
const INPUT_KIND_PEN: u64 = 12;
const INPUT_KIND_GAME_CONTROLLER: u64 = 13;

fn encode_input(event: crate::woven_input::Event) -> Option<u64> {
    use crate::woven_input::{Event, Key};
    let packed = |kind: u64, value: u64| (kind << 56) | value;
    match event {
        Event::Key(Key::Char(character)) => {
            Some(packed(INPUT_KIND_CHAR, u64::from(u32::from(character))))
        }
        Event::Key(Key::Enter) => Some(packed(INPUT_KIND_ENTER, 0)),
        Event::Key(Key::Backspace) => Some(packed(INPUT_KIND_BACKSPACE, 0)),
        Event::Key(Key::Tab) => Some(packed(INPUT_KIND_TAB, 0)),
        Event::Key(Key::F1) => Some(packed(INPUT_KIND_F1, 0)),
        Event::Key(Key::F2) => Some(packed(INPUT_KIND_F2, 0)),
        Event::Key(Key::F3) => Some(packed(INPUT_KIND_F3, 0)),
        Event::Key(Key::F4) => Some(packed(INPUT_KIND_F4, 0)),
        // The kernel's own key; never published to a focus holder.
        Event::Key(Key::F12) => None,
        // dx and dy as two's-complement u16 halves.
        Event::PointerMove { dx, dy } => Some(packed(
            INPUT_KIND_POINTER_MOVE,
            u64::from(dx as u16) | (u64::from(dy as u16) << 16),
        )),
        Event::PointerButton { button, pressed } => Some(packed(
            INPUT_KIND_POINTER_BUTTON,
            u64::from(button) | (u64::from(pressed) << 8),
        )),
        Event::Scroll { vertical } => Some(packed(INPUT_KIND_SCROLL, u64::from(vertical as u16))),
        Event::PointerAbsolute { x, y } => Some(packed(
            INPUT_KIND_POINTER_ABSOLUTE,
            u64::from(x) | (u64::from(y) << 16),
        )),
        // contact | active << 8 | x << 16 | y << 32
        Event::Touch {
            contact,
            x,
            y,
            active,
        } => Some(packed(
            INPUT_KIND_TOUCH,
            u64::from(contact)
                | (u64::from(active) << 8)
                | (u64::from(x) << 16)
                | (u64::from(y) << 32),
        )),
        // x | y << 16 | pressure (10 bits) << 32 | touching << 48
        Event::Pen {
            x,
            y,
            pressure,
            touching,
        } => Some(packed(
            INPUT_KIND_PEN,
            u64::from(x)
                | (u64::from(y) << 16)
                | (u64::from(pressure & 0x3ff) << 32)
                | (u64::from(touching) << 48),
        )),
        Event::GameController { control, value } => Some(packed(
            INPUT_KIND_GAME_CONTROLLER,
            u64::from(control) | (u64::from(value as u16) << 16),
        )),
    }
}

/// Take the next input event for the caller. Requires `InputIo`, and the
/// caller must hold input focus, which the compositor grants to its lease
/// holder while it owns the display. Non-blocking: WOULD_BLOCK when empty.
fn sys_input_poll() -> u64 {
    const WOULD_BLOCK: u64 = u64::MAX - 1;
    if !crate::task::current_has(crate::capability::Capability::InputIo) {
        return SYSCALL_ERROR;
    }
    let owner = crate::task::current_process_id();
    loop {
        match crate::woven_input::poll_for(owner) {
            Err(_) => return SYSCALL_ERROR,
            Ok(None) => return WOULD_BLOCK,
            Ok(Some(event)) => {
                if let Some(encoded) = encode_input(event) {
                    return encoded;
                }
            }
        }
    }
}

/// Sleep until the next input event for the caller or until `deadline`.
/// Same authority as `InputPoll`; returns WOULD_BLOCK on timeout.
fn sys_input_wait(deadline: u64) -> u64 {
    const WOULD_BLOCK: u64 = u64::MAX - 1;
    if !crate::task::current_has(crate::capability::Capability::InputIo) {
        return SYSCALL_ERROR;
    }
    let owner = crate::task::current_process_id();
    let task = crate::task::current_task_id();
    loop {
        match crate::woven_input::wait_for(owner, task, deadline) {
            Err(_) => return SYSCALL_ERROR,
            Ok(None) => return WOULD_BLOCK,
            Ok(Some(event)) => {
                if let Some(encoded) = encode_input(event) {
                    return encoded;
                }
            }
        }
    }
}

const WAIT_INPUT: u64 = 1 << 0;
const WAIT_MESSAGE: u64 = 1 << 1;
const WAIT_DAMAGE: u64 = 1 << 2;

/// Mark a region of one of the caller's own surfaces as changed. If the
/// surface is shared, its grantee is woken. No capability: it concerns only
/// the caller's pixels.
fn sys_graphics_damage(handle: u64, packed: u64) -> u64 {
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    let rect = crate::display::Rect::new(
        (packed & 0xffff) as u16 as i32,
        ((packed >> 16) & 0xffff) as u16 as i32,
        ((packed >> 32) & 0xffff) as usize,
        ((packed >> 48) & 0xffff) as usize,
    );
    crate::user_graphics::mark_damage(handle, rect).map_or(SYSCALL_ERROR, |()| 0)
}

/// Blit only the pending damage of a surface shared with the caller.
/// Requires `DisplayIo` and the lease; authorised by the slot's grant.
fn sys_compositor_blit_damage(handle: u64, packed: u64) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    let pid = crate::task::current_process_id();
    let handle = crate::user_graphics::Handle::from_u64(handle);
    match crate::compositor::blit_damage_for(pid, handle, x, y) {
        Err(_) => SYSCALL_ERROR,
        Ok(None) => 0,
        // Surface caps keep every field within u16.
        Ok(Some(region)) => {
            (region.x as u64 & 0xffff)
                | ((region.y as u64 & 0xffff) << 16)
                | ((region.width as u64 & 0xffff) << 32)
                | ((region.height as u64 & 0xffff) << 48)
        }
    }
}

/// Sleep until any requested source is ready or the timer reaches `deadline`.
///
/// Sources: input (`InputIo`, and only while the caller holds focus) and the
/// caller's pid mailbox (`Ipc`). Nothing is consumed; the caller follows up
/// with `InputPoll` or `MessageReceive`. Returns the ready mask, or 0 on
/// timeout.
///
/// The task registers with every requested source, then re-checks readiness,
/// then sleeps: an event between registration and sleep latches the
/// scheduler's pending permit, so it is not lost. Each source wakes with its
/// own lock released. Registrations are dropped before returning.
fn sys_wait_any(deadline: u64, mask: u64) -> u64 {
    if mask == 0 || mask & !(WAIT_INPUT | WAIT_MESSAGE | WAIT_DAMAGE) != 0 {
        return SYSCALL_ERROR;
    }
    let want_input = mask & WAIT_INPUT != 0;
    let want_message = mask & WAIT_MESSAGE != 0;
    let want_damage = mask & WAIT_DAMAGE != 0;
    if (want_input && !crate::task::current_has(crate::capability::Capability::InputIo))
        || (want_message && !crate::task::current_has(crate::capability::Capability::Ipc))
        || (want_damage && !display_authorized())
    {
        return SYSCALL_ERROR;
    }
    let owner = crate::task::current_process_id();
    let task = crate::task::current_task_id();

    let result = 'wait: loop {
        // Input is a source only while the caller holds focus; waiting on
        // input alone without focus could never complete, so it is refused.
        let mut input_live = false;
        if want_input {
            match crate::woven_input::register_waiter(owner, task) {
                Ok(None) => input_live = true,
                Ok(Some(other)) => {
                    // Liveness is checked with FOCUS released.
                    if crate::task::task_live(other) {
                        break 'wait SYSCALL_ERROR;
                    }
                    crate::woven_input::clear_waiter(other);
                    continue;
                }
                Err(_) if !want_message && !want_damage => break 'wait SYSCALL_ERROR,
                Err(_) => {}
            }
        }
        if want_message {
            match crate::ipc::register_message_waiter(owner, task) {
                Ok(None) => {}
                Ok(Some(other)) => {
                    if crate::task::task_live(other) {
                        break 'wait SYSCALL_ERROR;
                    }
                    crate::ipc::clear_message_waiter(owner, other);
                    continue;
                }
                Err(_) => break 'wait SYSCALL_ERROR,
            }
        }
        if want_damage {
            match crate::user_graphics::register_damage_waiter(owner, task) {
                Ok(None) => {}
                Ok(Some(other)) => {
                    if crate::task::task_live(other) {
                        break 'wait SYSCALL_ERROR;
                    }
                    crate::user_graphics::clear_damage_waiter(owner, other);
                    continue;
                }
                Err(_) => break 'wait SYSCALL_ERROR,
            }
        }
        let mut ready = 0;
        if want_damage && crate::user_graphics::damage_pending_for(owner) {
            ready |= WAIT_DAMAGE;
        }
        if input_live && crate::woven_input::pending(owner) {
            ready |= WAIT_INPUT;
        }
        if want_message && crate::ipc::message_pending(owner) {
            ready |= WAIT_MESSAGE;
        }
        if ready != 0 {
            break 'wait ready;
        }
        if deadline != u64::MAX && crate::timer::ticks() >= deadline {
            break 'wait 0;
        }
        crate::task::wait_for_event_until(deadline);
    };
    if want_input {
        crate::woven_input::clear_waiter(task);
    }
    if want_message {
        crate::ipc::clear_message_waiter(owner, task);
    }
    if want_damage {
        crate::user_graphics::clear_damage_waiter(owner, task);
    }
    result
}

const MAX_WAIT_OBJECTS: usize = 8;
const WAIT_OBJECT_TIMER: u64 = 0;
const WAIT_OBJECT_EVENT: u64 = 1;

/// Wait on generation-safe timer/event objects without retaining a userspace
/// pointer asynchronously. The complete source array is copied before any
/// registration, and every registration is removed before return.
fn sys_wait_any_objects(deadline: u64, user_sources: u64, count: u64) -> u64 {
    let Ok(count) = usize::try_from(count) else {
        return SYSCALL_ERROR;
    };
    if count == 0 || count > MAX_WAIT_OBJECTS {
        return SYSCALL_ERROR;
    }
    let byte_count = count * 24;
    let mut bytes = [0u8; MAX_WAIT_OBJECTS * 24];
    if crate::paging::copy_from_current_user(user_sources, &mut bytes[..byte_count]).is_err() {
        return SYSCALL_ERROR;
    }
    let mut sources = [(0u64, false, 0u64); MAX_WAIT_OBJECTS];
    let mut used_masks = 0u64;
    for (index, source) in sources.iter_mut().take(count).enumerate() {
        let offset = index * 24;
        let raw = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        let kind = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
        let mask = u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().unwrap());
        if mask < (1 << 3)
            || !mask.is_power_of_two()
            || used_masks & mask != 0
            || !matches!(kind, WAIT_OBJECT_TIMER | WAIT_OBJECT_EVENT)
        {
            return SYSCALL_ERROR;
        }
        if (kind == WAIT_OBJECT_TIMER
            && !crate::task::current_has(crate::capability::Capability::TimerRead))
            || (kind == WAIT_OBJECT_EVENT
                && !crate::task::current_has(crate::capability::Capability::Ipc))
        {
            return SYSCALL_ERROR;
        }
        *source = (raw, kind == WAIT_OBJECT_TIMER, mask);
        used_masks |= mask;
    }
    let owner = crate::task::current_process_id();
    let task = crate::task::current_task_id();
    let mut registered = [false; MAX_WAIT_OBJECTS];
    let result = 'wait: loop {
        let mut ready = 0u64;
        for (index, (raw, is_timer, mask)) in sources.iter().take(count).enumerate() {
            match crate::async_events::register_task(owner, *raw, *is_timer, task) {
                Ok(crate::async_events::TaskWaitRegistration::Ready) => ready |= *mask,
                Ok(crate::async_events::TaskWaitRegistration::Registered) => {
                    registered[index] = true
                }
                Ok(crate::async_events::TaskWaitRegistration::Busy) | Err(()) => {
                    break 'wait SYSCALL_ERROR
                }
            }
        }
        if ready != 0 {
            break 'wait ready;
        }
        if deadline != u64::MAX && crate::timer::ticks() >= deadline {
            break 'wait 0;
        }
        crate::task::wait_for_event_until(deadline);
        for (index, (raw, is_timer, _)) in sources.iter().take(count).enumerate() {
            if registered[index] {
                crate::async_events::clear_task_waiter(owner, *raw, *is_timer, task);
                registered[index] = false;
            }
        }
    };
    for (index, (raw, is_timer, _)) in sources.iter().take(count).enumerate() {
        if registered[index] {
            crate::async_events::clear_task_waiter(owner, *raw, *is_timer, task);
        }
    }
    result
}

/// Present compositor damage to the framebuffer. Requires `DisplayIo` and the
/// lease; the first call takes the display from the text terminal, and the
/// holder's exit gives it back.
fn sys_compositor_present() -> u64 {
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    crate::compositor::present_for(crate::task::current_process_id())
        .map_or(SYSCALL_ERROR, |written| written as u64)
}

/// Read one pixel of the compositor surface at `packed = x | (y << 32)`.
///
/// Returns the packed colour or SYSCALL_ERROR. The screen carries every
/// client's pixels, so reading it requires the same authority as drawing:
/// `DisplayIo` and the lease.
fn sys_compositor_pixel(packed: u64) -> u64 {
    let x = (packed & 0xffff_ffff) as i32;
    let y = ((packed >> 32) & 0xffff_ffff) as i32;
    if !display_authorized() {
        return SYSCALL_ERROR;
    }
    crate::compositor::pixel_for(crate::task::current_process_id(), x, y)
        .map(u64::from)
        .unwrap_or(SYSCALL_ERROR)
}

/// Stroke a rectangle outline into a client surface.
///
/// Shares the fill encoding so the client has one packing convention: four
/// u16 fields in `packed`.
fn sys_graphics_stroke(handle: u64, packed: u64, color: u64) -> u64 {
    let x = (packed & 0xffff) as u16 as i32;
    let y = ((packed >> 16) & 0xffff) as u16 as i32;
    let width = ((packed >> 32) & 0xffff) as usize;
    let height = ((packed >> 48) & 0xffff) as usize;
    let Some(handle) = caller_surface(handle) else {
        return SYSCALL_ERROR;
    };
    crate::user_graphics::stroke_rect(
        handle,
        crate::display::Rect::new(x, y, width, height),
        color as u32,
    )
    .map_or(SYSCALL_ERROR, |()| 0)
}
