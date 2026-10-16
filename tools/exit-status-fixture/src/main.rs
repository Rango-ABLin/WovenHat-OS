//! Real Ring-3 regression linked against the production libwoven crate.
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicI32, Ordering};
use libwoven::{Error, process, syscall, thread};

static STATUS: AtomicI32 = AtomicI32::new(0);
const STATUSES: [i32; 6] = [-1, -2, i32::MIN, i32::MAX, 0, 37];

libwoven::entry!(main);

// The kernel enters a thread at a page-aligned stack top, not via CALL.
// These CPU-ABI trampolines establish Rust's call-stack alignment; all actual
// syscalls and result decoding below go through the production library.
core::arch::global_asm!(
    ".global status_worker_entry",
    "status_worker_entry:",
    "and rsp, -16",
    "call {natural}",
    "ud2",
    ".global sleeping_worker_entry",
    "sleeping_worker_entry:",
    "and rsp, -16",
    "call {sleeping}",
    "ud2",
    natural = sym natural_worker,
    sleeping = sym sleeping_worker,
);

unsafe extern "C" {
    fn status_worker_entry();
    fn sleeping_worker_entry();
}

extern "C" fn natural_worker() -> ! {
    // SAFETY: this is an actual WovenHat userspace thread; status is scalar.
    unsafe { syscall::thread_exit(STATUS.load(Ordering::Acquire)) }
}

extern "C" fn sleeping_worker() -> ! {
    loop {
        // SAFETY: scalar timer wait, not a spin loop or retained user pointer.
        unsafe { syscall::sleep(1000) };
    }
}

fn fail(code: u64) -> ! {
    let mut digits = [0; 20];
    let _ = libwoven::print("[S11.STATUS] fixture failure code=");
    let _ = libwoven::write(syscall::STDOUT, libwoven::format_u64(code, &mut digits));
    let _ = libwoven::print("\n");
    libwoven::exit(code)
}

fn require(condition: bool, code: u64) {
    if !condition {
        fail(code);
    }
}

fn fork() -> u64 {
    // SAFETY: Fork takes no user pointer and returns in the cloned context.
    unsafe { syscall::invoke(syscall::FORK, 0, 0, 0) }
}

fn wait(pid: process::Id) -> Result<u64, Error> {
    loop {
        match libwoven::wait(pid) {
            Err(Error::WouldBlock) => {
                // The existing wait ABI is nonblocking. Park on a timer between
                // retries rather than burning a CPU polling the child.
                unsafe { syscall::sleep(1) };
            }
            result => return result,
        }
    }
}

fn create(entry: unsafe extern "C" fn()) -> thread::Id {
    // SAFETY: entry is one of the executable, ABI-aligned trampolines above.
    let raw = unsafe { syscall::thread_create(entry as *const () as u64, 16384) };
    require(raw != syscall::ERROR, 11);
    thread::Id(raw)
}

fn main(_: libwoven::Args) -> ! {
    for (index, status) in STATUSES.into_iter().enumerate() {
        let pid = fork();
        require(pid != syscall::ERROR, 12);
        if pid == 0 {
            libwoven::exit(status as u32 as u64);
        }
        let pid = process::Id(pid);
        require(wait(pid) == Ok(status as u32 as u64), 20 + index as u64);
        require(libwoven::wait(pid) == Err(Error::Failed), 30 + index as u64);
    }
    let _ = libwoven::print("[S11.STATUS] libwoven signed process waits and double-reap: PASSED\n");

    for (index, status) in STATUSES.into_iter().enumerate() {
        STATUS.store(status, Ordering::Release);
        let id = create(status_worker_entry);
        // SAFETY: id belongs to this process and is joined exactly once.
        require(
            unsafe { syscall::thread_join(id) } == thread::Join::Exited(status),
            40 + index as u64,
        );
        require(
            unsafe { syscall::thread_join(id) } == thread::Join::Invalid,
            50 + index as u64,
        );
        let id = create(sleeping_worker_entry);
        require(
            unsafe { syscall::thread_cancel(id, status) } == 0,
            60 + index as u64,
        );
        require(
            unsafe { syscall::thread_join(id) } == thread::Join::Exited(status),
            70 + index as u64,
        );
        require(
            unsafe { syscall::thread_join(id) } == thread::Join::Invalid,
            80 + index as u64,
        );
    }
    let _ = libwoven::print(
        "[S11.STATUS] libwoven signed natural/canceled joins and stale IDs: PASSED\n",
    );

    let parent = libwoven::current_pid();
    let owned = create(sleeping_worker_entry);
    let pid = fork();
    require(pid != syscall::ERROR, 90);
    if pid == 0 {
        require(
            unsafe { syscall::thread_join(owned) } == thread::Join::Invalid,
            91,
        );
        require(libwoven::wait(parent) == Err(Error::Failed), 92);
        libwoven::exit(0);
    }
    require(wait(process::Id(pid)) == Ok(0), 93);
    require(unsafe { syscall::thread_cancel(owned, -2) } == 0, 94);
    require(
        unsafe { syscall::thread_join(owned) } == thread::Join::Exited(-2),
        95,
    );

    let pid = fork();
    require(pid != syscall::ERROR, 96);
    if pid == 0 {
        sleeping_worker();
    }
    require(
        libwoven::wait(process::Id(pid)) == Err(Error::WouldBlock),
        97,
    );
    // SAFETY: Kill targets only the fixture's own newly created sleeping child.
    require(unsafe { syscall::invoke(26, pid, 9, 0) } == 0, 98);
    require(wait(process::Id(pid)) == Ok(137), 99);
    require(
        libwoven::wait(process::Id(u64::MAX)) == Err(Error::Failed),
        100,
    );
    let _ = libwoven::print("[S11.STATUS] ownership and genuine wait errors: PASSED\n");
    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    fail(101)
}
