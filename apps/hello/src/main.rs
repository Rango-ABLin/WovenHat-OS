//! The first WovenHat application built outside the kernel.
//!
//! Every program before this one was inline assembly inside
//! `kernel/src/userspace.rs`, wrapped in a synthetic ELF at runtime. This is a
//! separate crate compiled to a static ELF that the kernel loads from the
//! filesystem like any other executable, which is what makes an application
//! platform possible at all.
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Entry point named by `link.x`.
///
/// The kernel enters here with a Ring-3 stack already mapped, so an ordinary
/// function prologue is safe.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let _ = libwoven::print("hello from wovenhat userspace
");

    // Prove this is a real process rather than a fragment of the kernel: ask
    // the kernel who we are and report it back.
    let mut digits = [0u8; 20];
    let pid = libwoven::format_u64(libwoven::current_pid().0, &mut digits);
    let _ = libwoven::print("pid=");
    let _ = libwoven::write(libwoven::syscall::STDOUT, pid);
    let _ = libwoven::print("
");

    libwoven::exit(0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    // A userspace panic must terminate the process, not spin.
    libwoven::exit(101)
}
