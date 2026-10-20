//! The desktop session (Stage 20.15).
//!
//! A system-apps boot starts WovenShell as the session: it takes the
//! display and input focus, and its launcher lists the installed system
//! applications. F12 switches between the session and the kernel's
//! diagnostic console, which keeps working for testers (`note`, `savelog`,
//! `poweroff`): the compositor holds back the session's frames and the
//! terminal gets the display and the keyboard, then F12 hands both back.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[cfg(any(feature = "system-apps", feature = "stage20-15-test"))]
use crate::capability::{Capability, CapabilitySet};

/// The session's process id; 0 before a session starts.
static SESSION: AtomicU64 = AtomicU64::new(0);
static CONSOLE: AtomicBool = AtomicBool::new(false);

#[cfg(any(feature = "system-apps", feature = "stage20-15-test"))]
/// Why the session could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The shell image is not a loadable program.
    Image,
    Spawn,
    Identity,
}

#[cfg(any(feature = "system-apps", feature = "stage20-15-test"))]
/// What the session may do: draw, take input, talk to applications, read
/// the catalog, launch applications, play sound, turn the machine off, and
/// change the signed-in account's own PIN.
pub const CAPABILITIES: CapabilitySet = CapabilitySet::only(Capability::Console)
    .with(Capability::DisplayIo)
    .with(Capability::InputIo)
    .with(Capability::Ipc)
    .with(Capability::FileRead)
    .with(Capability::ProcessCreate)
    .with(Capability::AudioPlayback)
    .with(Capability::PowerControl)
    .with(Capability::AccountManage);

#[cfg(any(feature = "system-apps", feature = "stage20-15-test"))]
/// Start WovenShell from `image` as the session. With accounts, it opens
/// at the sign-in screen.
pub fn start(image: &[u8], accounts: Option<&str>) -> Result<u64, Error> {
    #[cfg(feature = "system-apps")]
    if let Some(path) = accounts {
        crate::identity_runtime::load_durable_or_accounts(path).map_err(|_| Error::Identity)?;
    }
    let program = match accounts {
        Some(path) => {
            crate::userspace::load_elf_with_argv(image, &["/bin/woven-shell", "--accounts", path])
        }
        None => crate::userspace::load_elf_with_argv(image, &["/bin/woven-shell"]),
    }
    .ok_or(Error::Image)?;
    let (pid, _, _) = crate::task::spawn_user_system_service("woven-shell", program, CAPABILITIES)
        .map_err(|_| Error::Spawn)?;
    CONSOLE.store(false, Ordering::Release);
    SESSION.store(pid.as_u64(), Ordering::Release);
    Ok(pid.as_u64())
}

/// The running session's process id.
pub fn active() -> Option<u64> {
    let pid = SESSION.load(Ordering::Acquire);
    (pid != 0 && crate::compositor::holder() == Some(pid)).then_some(pid)
}

/// Whether the diagnostic console is showing instead of the session.
#[cfg(feature = "stage20-15-test")]
pub fn console_active() -> bool {
    CONSOLE.load(Ordering::Acquire)
}

/// F12: show the diagnostic console, or return to the session. Nothing
/// happens without a session.
pub fn toggle_console() {
    let Some(pid) = active() else { return };
    let entering = !CONSOLE.load(Ordering::Acquire);
    CONSOLE.store(entering, Ordering::Release);
    if entering {
        crate::compositor::set_console(true);
        crate::woven_input::set_focus(None);
        crate::terminal::clear();
        crate::terminal::write_bytes(
            b"WOVENHAT DIAGNOSTIC CONSOLE  (F12: BACK TO THE DESKTOP)\n> ",
        );
        crate::serial::write_line(format_args!("[SESSION] diagnostic console"));
    } else {
        crate::compositor::set_console(false);
        crate::woven_input::set_focus(Some(pid));
        crate::serial::write_line(format_args!("[SESSION] back to the desktop"));
    }
}
