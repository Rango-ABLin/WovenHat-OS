//! Kernel-held compositor surface for client surfaces.
//!
//! Phase 5 of the recovery plan calls for a compositor that places client
//! surfaces on the screen. The honest shape of that compositor is a userspace
//! process whose authority is the one capability that can hand drawing to the
//! real framebuffer, with per-client surfaces delivered as capabilities. This
//! module is the kernel-side primitive that makes such a process possible: a
//! single screen-sized compositor surface, blit and pixel-read operations, and
//! damage tracking the display stack can honour at presentation time.
//!
//! No stacking policy is in here. A client that calls [`blit`] places its
//! surface exactly where it asked, so this is not a window manager; it is the
//! primitive the window manager will drive. A policy layer belongs above this,
//! in userspace, once there is a compositor process that owns it.
//!
//! The compositor surface's dimensions are fixed at [`init`] time rather than
//! derived from the framebuffer, so acceptance tests can target a known
//! screen without depending on real display geometry.

use crate::display::{Damage, Rect, Surface};
use crate::irq_lock::PreemptMutex as Mutex;
use crate::user_graphics;

/// Fixed geometry used by the self-tests that reset and re-initialise the
/// compositor, and as the fallback when no framebuffer exists.
pub const DEFAULT_WIDTH: u32 = 640;
pub const DEFAULT_HEIGHT: u32 = 480;

/// Upper bound on the boot-time surface. The compositor follows the
/// framebuffer, but a 4K display would otherwise cost a 33 MB kernel
/// allocation; 1920×1200 bounds it near 9 MB. A larger display shows the
/// compositor in its top-left corner.
pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1200;

/// Error returned by compositor operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Compositor has not been initialised yet.
    NotReady,
    /// The named client surface does not exist, is stale, or is owned by a
    /// different process.
    InvalidClient,
    /// The requested pixel is outside the compositor surface.
    OutOfRange,
    /// There is no framebuffer to present to.
    NoDisplay,
    /// The compositor does not currently own the display.
    NotPresenting,
    /// Another process holds the compositor lease.
    Busy,
    /// The caller does not hold the compositor lease.
    NotHolder,
}

struct State {
    surface: Surface,
    damage: Damage,
    /// Whether the compositor owns the display. While false, damage keeps
    /// accumulating but nothing reaches the framebuffer.
    presenting: bool,
    /// The one process allowed to drive the compositor from userspace.
    holder: Option<u64>,
}

impl State {
    fn new(width: usize, height: usize) -> Self {
        let mut state = Self {
            surface: Surface::new(width, height),
            damage: Damage::new(),
            presenting: false,
            holder: None,
        };
        // A compositor without an initial fill reads as transparent-looking
        // zeros, which the test cannot distinguish from "a client blitted
        // zeros". Fill once at construction so subsequent reads reflect a
        // deliberate state.
        let bounds = state.surface.bounds();
        state.surface.fill_rect(bounds, 0);
        state.damage.add(bounds);
        state
    }
}

/// Rank 10, matching the user-graphics registry it reads from. A preemption
/// mutex rather than an IRQ mutex: presentation copies up to a full frame, and
/// device interrupts must stay live while it does. Nothing takes this lock
/// from interrupt context. Lock order: STATE, then the terminal's lock inside
/// [`present`]; the terminal never calls back into this module.
static STATE: Mutex<Option<State>> = Mutex::with_rank(None, 10);

/// Initialise the compositor surface. Idempotent — the second call is a no-op.
///
/// The acceptance tests call this; a production boot would call it with the
/// framebuffer's own dimensions once a global framebuffer accessor exists.
pub fn init(width: u32, height: u32) {
    let mut guard = STATE.lock();
    if guard.is_none() {
        *guard = Some(State::new(width as usize, height as usize));
    }
}

/// Initialise the compositor at the framebuffer's size, clamped to
/// [`MAX_WIDTH`]×[`MAX_HEIGHT`], or at the default size without a framebuffer.
/// Idempotent, like [`init`].
pub fn init_for_display() {
    let (width, height) = crate::terminal::framebuffer_info()
        .map(|info| {
            (
                (info.width as u32).clamp(1, MAX_WIDTH),
                (info.height as u32).clamp(1, MAX_HEIGHT),
            )
        })
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
    init(width, height);
}

/// Blit the client surface named by `handle` onto the compositor at `(x, y)`.
///
/// The lookup is performed through `user_graphics` so the ownership and
/// generation rules the registry enforces also apply here: a client cannot
/// blit another client's surface.
///
/// Kernel-side blit with no lease check, for the kernel's own use; userspace
/// goes through [`blit_for`].
#[cfg_attr(
    not(any(
        feature = "stage19-1-test",
        feature = "stage19-2-test",
        feature = "stage19-3-test"
    )),
    allow(dead_code)
)]
pub fn blit(handle: user_graphics::Handle, x: i32, y: i32) -> Result<Rect, Error> {
    // Snapshot the client surface's pixels under the registry lock, then
    // release that lock before taking the compositor's. Holding both would
    // require a ranking rule for user_graphics vs compositor and open the door
    // to inversions; copying out costs one allocation and keeps the lock
    // dependency single.
    //
    // Readiness is checked first so an uninitialised compositor reports
    // NotReady regardless of the handle, and no snapshot is copied for a blit
    // that has nowhere to land. The check is repeated under the lock below
    // because the state may be reset (tests only) between the two.
    if STATE.lock().is_none() {
        return Err(Error::NotReady);
    }
    let snapshot = user_graphics::snapshot(handle).map_err(|_| Error::InvalidClient)?;
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    let touched = state.surface.blit(&snapshot, x, y);
    state.damage.add(touched);
    Ok(touched)
}

/// Take the display from the text terminal and repaint it from the
/// compositor surface on the next [`present`].
#[cfg_attr(
    not(any(feature = "stage19-3-test", feature = "stage19-10-test")),
    allow(dead_code)
)]
pub fn activate_display() -> Result<(), Error> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if !crate::terminal::set_display_owned(true) {
        return Err(Error::NoDisplay);
    }
    state.presenting = true;
    // Whatever the terminal drew is now stale; the whole surface is damage.
    let bounds = state.surface.bounds();
    state.damage.add(bounds);
    Ok(())
}

/// Return the display to the text terminal. Damage keeps accumulating so a
/// later [`activate_display`] repaints correctly.
#[cfg_attr(
    not(any(feature = "stage19-3-test", feature = "stage19-10-test")),
    allow(dead_code)
)]
pub fn deactivate_display() {
    let mut guard = STATE.lock();
    if let Some(state) = guard.as_mut() {
        if state.presenting {
            state.presenting = false;
            crate::terminal::set_display_owned(false);
        }
    }
}

/// Copy the damaged regions of the compositor surface to the framebuffer and
/// clear the damage. Returns the number of pixels written.
///
/// Damage is cleared only after a successful copy, so a failed presentation
/// loses nothing.
#[cfg_attr(
    not(any(feature = "stage19-3-test", feature = "stage19-10-test")),
    allow(dead_code)
)]
pub fn present() -> Result<usize, Error> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if !state.presenting {
        return Err(Error::NotPresenting);
    }
    let written = crate::terminal::with_framebuffer(|buffer, info| {
        crate::display::present(&state.surface, &state.damage, buffer, info)
    })
    .ok_or(Error::NoDisplay)?;
    state.damage.clear();
    Ok(written)
}

/// Take the compositor lease for `pid`.
///
/// The lease is exclusive: one process at a time may blit onto, read from and
/// present the compositor surface on behalf of userspace. Claiming again as the
/// current holder succeeds. The syscall layer requires `DisplayIo` before
/// calling this; the lease is what makes that authority single-holder.
pub fn claim(pid: u64) -> Result<(), Error> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    match state.holder {
        Some(holder) if holder != pid => Err(Error::Busy),
        _ => {
            state.holder = Some(pid);
            Ok(())
        }
    }
}

/// Current lease holder, if any.
#[cfg_attr(not(feature = "stage19-4-test"), allow(dead_code))]
pub fn holder() -> Option<u64> {
    STATE.lock().as_ref().and_then(|state| state.holder)
}

/// Whether the compositor currently owns the display.
#[cfg_attr(not(feature = "stage19-4-test"), allow(dead_code))]
pub fn is_presenting() -> bool {
    STATE.lock().as_ref().is_some_and(|state| state.presenting)
}

/// Drop `pid`'s lease, if it holds one, and hand the display back to the
/// terminal. Called from the task-exit path so a crashed holder cannot keep
/// the screen.
pub fn release_owner(pid: u64) {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    if state.holder != Some(pid) {
        return;
    }
    state.holder = None;
    crate::woven_input::release_focus(pid);
    if state.presenting {
        state.presenting = false;
        crate::terminal::set_display_owned(false);
    }
}

fn require_holder(pid: u64) -> Result<(), Error> {
    let guard = STATE.lock();
    let state = guard.as_ref().ok_or(Error::NotReady)?;
    if state.holder == Some(pid) {
        Ok(())
    } else {
        Err(Error::NotHolder)
    }
}

/// [`blit`] on behalf of a userspace lease holder.
///
/// The holder check is repeated after the snapshot because the lease can be
/// released by the holder's exit between the two; a blit for a process that
/// no longer holds the lease is refused rather than landing late.
pub fn blit_for(pid: u64, handle: user_graphics::Handle, x: i32, y: i32) -> Result<Rect, Error> {
    require_holder(pid)?;
    let snapshot = user_graphics::snapshot(handle).map_err(|_| Error::InvalidClient)?;
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    let touched = state.surface.blit(&snapshot, x, y);
    state.damage.add(touched);
    Ok(touched)
}

/// Blit another process's surface that its owner has shared with the lease
/// holder `pid`. This is what lets a separate compositor process draw its
/// clients' windows: the client grants, the holder composites.
pub fn blit_shared_for(
    pid: u64,
    handle: user_graphics::Handle,
    x: i32,
    y: i32,
) -> Result<Rect, Error> {
    require_holder(pid)?;
    let snapshot =
        user_graphics::snapshot_shared(handle, pid).map_err(|_| Error::InvalidClient)?;
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    let touched = state.surface.blit(&snapshot, x, y);
    state.damage.add(touched);
    Ok(touched)
}

/// [`blit_for`] with uniform `opacity`, for translucent windows and shadows
/// drawn from the holder's own surfaces.
pub fn blend_for(
    pid: u64,
    handle: user_graphics::Handle,
    x: i32,
    y: i32,
    opacity: u8,
) -> Result<Rect, Error> {
    require_holder(pid)?;
    let snapshot = user_graphics::snapshot(handle).map_err(|_| Error::InvalidClient)?;
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    let touched = state.surface.blend(&snapshot, x, y, opacity);
    state.damage.add(touched);
    Ok(touched)
}

/// [`blit_shared_for`] with uniform `opacity`: a client's window drawn
/// translucently by the compositor process.
pub fn blend_shared_for(
    pid: u64,
    handle: user_graphics::Handle,
    x: i32,
    y: i32,
    opacity: u8,
) -> Result<Rect, Error> {
    require_holder(pid)?;
    let snapshot =
        user_graphics::snapshot_shared(handle, pid).map_err(|_| Error::InvalidClient)?;
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    let touched = state.surface.blend(&snapshot, x, y, opacity);
    state.damage.add(touched);
    Ok(touched)
}

/// Composite only what changed: take the pending damage of a surface shared
/// with the lease holder `pid` and blit just that region, at its position
/// within a window placed at `(x, y)`. Returns the source region, or `None`
/// when nothing was pending.
pub fn blit_damage_for(
    pid: u64,
    handle: user_graphics::Handle,
    x: i32,
    y: i32,
) -> Result<Option<Rect>, Error> {
    require_holder(pid)?;
    let Some((region, copy)) =
        user_graphics::take_damage_shared(handle, pid).map_err(|_| Error::InvalidClient)?
    else {
        return Ok(None);
    };
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    let touched = state
        .surface
        .blit(&copy, x.saturating_add(region.x), y.saturating_add(region.y));
    state.damage.add(touched);
    Ok(Some(region))
}

/// [`pixel`] on behalf of a userspace lease holder. Reading the screen exposes
/// every client's pixels, so it carries the same authority as drawing on it.
pub fn pixel_for(pid: u64, x: i32, y: i32) -> Result<u32, Error> {
    let guard = STATE.lock();
    let state = guard.as_ref().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    state.surface.pixel(x, y).ok_or(Error::OutOfRange)
}

/// Present on behalf of the lease holder, taking the display from the terminal
/// on first use. Returns the number of pixels written.
pub fn present_for(pid: u64) -> Result<usize, Error> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(Error::NotReady)?;
    if state.holder != Some(pid) {
        return Err(Error::NotHolder);
    }
    if !state.presenting {
        if !crate::terminal::set_display_owned(true) {
            return Err(Error::NoDisplay);
        }
        state.presenting = true;
        let bounds = state.surface.bounds();
        state.damage.add(bounds);
        // The user is now looking at the holder's screen, so keystrokes are
        // meant for it rather than for the hidden terminal.
        crate::woven_input::set_focus(Some(pid));
    }
    let written = crate::terminal::with_framebuffer(|buffer, info| {
        crate::display::present(&state.surface, &state.damage, buffer, info)
    })
    .ok_or(Error::NoDisplay)?;
    state.damage.clear();
    Ok(written)
}

/// Read one pixel from the compositor surface.
///
/// Kernel-side read with no lease check; userspace goes through [`pixel_for`].
#[cfg_attr(
    not(any(
        feature = "stage19-1-test",
        feature = "stage19-2-test",
        feature = "stage19-3-test"
    )),
    allow(dead_code)
)]
pub fn pixel(x: i32, y: i32) -> Result<u32, Error> {
    let guard = STATE.lock();
    let state = guard.as_ref().ok_or(Error::NotReady)?;
    state.surface.pixel(x, y).ok_or(Error::OutOfRange)
}

/// Dimensions of the compositor surface, or `None` before [`init`].
///
/// Currently used only by the Stage 19.1 self-test; a userspace compositor
/// will reach the same values through its own idempotent `init` call once that
/// process exists.
#[cfg_attr(
    not(any(
        feature = "stage19-1-test",
        feature = "stage19-3-test",
        feature = "stage19-10-test"
    )),
    allow(dead_code)
)]
pub fn dimensions() -> Option<(u32, u32)> {
    let guard = STATE.lock();
    guard
        .as_ref()
        .map(|state| (state.surface.width() as u32, state.surface.height() as u32))
}

/// Reset the compositor state. Intended for tests only.
///
/// Dropping and re-initialising is the only way to shrink the surface or clear
/// damage between acceptance runs; a production kernel would never call this.
#[cfg_attr(
    not(any(feature = "stage19-1-test", feature = "stage19-3-test")),
    allow(dead_code)
)]
pub fn reset() {
    // Dropping the state also drops any lease; the display goes back to the
    // terminal below if the compositor held it.
    let mut guard = STATE.lock();
    if let Some(holder) = guard.as_ref().and_then(|state| state.holder) {
        crate::woven_input::release_focus(holder);
    }
    if guard.as_ref().is_some_and(|state| state.presenting) {
        crate::terminal::set_display_owned(false);
    }
    *guard = None;
}

#[cfg(feature = "stage19-1-test")]
pub fn structural_self_test() -> bool {
    use crate::display::rgb;

    reset();

    // Operations before init must refuse rather than silently create state.
    let fake = user_graphics::Handle::from_u64(0);
    if blit(fake, 0, 0) != Err(Error::NotReady) || pixel(0, 0) != Err(Error::NotReady) {
        return false;
    }

    init(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    // A second init is a no-op; dimensions do not shift under the caller.
    init(100, 100);
    if dimensions() != Some((DEFAULT_WIDTH, DEFAULT_HEIGHT)) {
        return false;
    }

    // After init the compositor reads as its background fill (0), not as the
    // zeroed heap it just allocated.
    if pixel(0, 0) != Ok(0) || pixel(100, 100) != Ok(0) {
        return false;
    }
    if pixel(DEFAULT_WIDTH as i32, 0) != Err(Error::OutOfRange)
        || pixel(-1, 0) != Err(Error::OutOfRange)
    {
        return false;
    }

    // A real client surface, filled and blitted onto the compositor.
    let owner = 42u64;
    let Ok(client) = user_graphics::create(owner, 32, 16) else {
        return false;
    };
    if user_graphics::fill_rect(client, Rect::new(0, 0, 32, 16), rgb(10, 20, 30)).is_err() {
        return false;
    }
    let touched = match blit(client, 100, 50) {
        Ok(rect) => rect,
        Err(_) => return false,
    };
    if touched != Rect::new(100, 50, 32, 16) {
        return false;
    }
    // The compositor must carry the colour the client wrote, exactly, at the
    // right offset.
    if pixel(100, 50) != Ok(rgb(10, 20, 30))
        || pixel(131, 65) != Ok(rgb(10, 20, 30))
        || pixel(99, 50) != Ok(0)
        || pixel(132, 65) != Ok(0)
    {
        return false;
    }

    // Blit clipped by the compositor's edges: the reported touched region is
    // the on-screen part only.
    let touched = match blit(client, DEFAULT_WIDTH as i32 - 10, DEFAULT_HEIGHT as i32 - 5) {
        Ok(rect) => rect,
        Err(_) => return false,
    };
    if touched.width != 10 || touched.height != 5 {
        return false;
    }

    // A handle that does not belong to any client is refused with
    // InvalidClient rather than being treated as a wildcard.
    if blit(fake, 0, 0) != Err(Error::InvalidClient) {
        return false;
    }

    // After the owning process goes away, the client's handle is stale and
    // another blit must be refused rather than drawing from a reused slot.
    user_graphics::release_owner(owner);
    if blit(client, 0, 0) != Err(Error::InvalidClient) {
        return false;
    }

    reset();
    true
}

#[cfg(feature = "stage19-3-test")]
pub fn presentation_self_test() -> bool {
    use crate::display::rgb;
    use bootloader_api::info::PixelFormat;

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S19.3] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    /// Read a framebuffer pixel back as 0x00RRGGBB, independently of the
    /// compositor's own state.
    fn screen_pixel(x: usize, y: usize) -> Option<u32> {
        crate::terminal::with_framebuffer(|buffer, info| {
            if x >= info.width || y >= info.height || info.bytes_per_pixel < 3 {
                return None;
            }
            let offset = (y * info.stride + x) * info.bytes_per_pixel;
            let bytes = buffer.get(offset..offset + 3)?;
            match info.pixel_format {
                PixelFormat::Rgb => Some(rgb(bytes[0], bytes[1], bytes[2])),
                PixelFormat::Bgr => Some(rgb(bytes[2], bytes[1], bytes[0])),
                _ => None,
            }
        })
        .flatten()
    }

    /// FNV-1a over the whole framebuffer, to detect any write at all.
    fn screen_checksum() -> Option<u64> {
        crate::terminal::with_framebuffer(|buffer, _| {
            buffer.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
    }

    reset();
    init(DEFAULT_WIDTH, DEFAULT_HEIGHT);

    let Some((screen_width, screen_height)) =
        crate::terminal::with_framebuffer(|_, info| (info.width, info.height))
    else {
        return step("framebuffer available", false);
    };
    crate::serial::write_line(format_args!(
        "[S19.3] framebuffer {}x{}",
        screen_width, screen_height
    ));
    let visible =
        screen_width.min(DEFAULT_WIDTH as usize) * screen_height.min(DEFAULT_HEIGHT as usize);

    if !step(
        "present refused before taking the display",
        present() == Err(Error::NotPresenting),
    ) {
        return false;
    }
    if !step(
        "display taken from the terminal",
        activate_display().is_ok(),
    ) {
        return false;
    }
    // Taking the display repaints everything the compositor covers. The
    // terminal's background is not black, so reading black proves the
    // compositor wrote it.
    if !step(
        "takeover repaints the visible surface",
        present() == Ok(visible),
    ) || !step(
        "framebuffer shows the compositor background",
        screen_pixel(10, 10) == Some(0),
    ) {
        return false;
    }
    if !step("damage cleared after present", present() == Ok(0)) {
        return false;
    }

    let owner = 77u64;
    let color = rgb(200, 100, 50);
    let Ok(client) = user_graphics::create(owner, 32, 16) else {
        return step("client surface created", false);
    };
    let drawn = user_graphics::fill_rect(client, Rect::new(0, 0, 32, 16), color).is_ok()
        && blit(client, 100, 50) == Ok(Rect::new(100, 50, 32, 16));
    if !step("client blitted", drawn) {
        return false;
    }
    if !step(
        "only the damaged region is presented",
        present() == Ok(32 * 16),
    ) {
        return false;
    }
    let on_screen = screen_pixel(100, 50) == Some(color)
        && screen_pixel(131, 65) == Some(color)
        && screen_pixel(99, 50) == Some(0)
        && screen_pixel(132, 65) == Some(0);
    if !step("framebuffer carries the client at its offset", on_screen) {
        return false;
    }

    // While the compositor owns the display, terminal output must not reach
    // the framebuffer.
    let before = screen_checksum();
    crate::terminal::write_bytes(b"terminal text while compositing\n");
    if !step(
        "terminal suppressed while compositing",
        before.is_some() && screen_checksum() == before,
    ) {
        return false;
    }

    deactivate_display();
    if !step(
        "present refused after returning the display",
        present() == Err(Error::NotPresenting),
    ) {
        return false;
    }
    let before = screen_checksum();
    crate::terminal::write_bytes(b"XXXX");
    if !step(
        "terminal draws again after release",
        before.is_some() && screen_checksum() != before,
    ) {
        return false;
    }

    user_graphics::release_owner(owner);
    reset();
    true
}

/// Kernel-level checks of the lease rules. The Stage 19.4 gate pairs this with
/// a userspace probe that exercises the same rules through the syscalls.
#[cfg(feature = "stage19-4-test")]
pub fn lease_self_test() -> bool {
    use crate::display::rgb;

    let (first, second) = (1001u64, 1002u64);
    if holder().is_some() {
        return false;
    }
    if claim(first).is_err() || claim(first).is_err() || holder() != Some(first) {
        return false;
    }
    if claim(second) != Err(Error::Busy) {
        return false;
    }
    let Ok(client) = user_graphics::create(second, 4, 4) else {
        return false;
    };
    let _ = user_graphics::fill_rect(client, Rect::new(0, 0, 4, 4), rgb(1, 2, 3));
    if blit_for(second, client, 0, 0) != Err(Error::NotHolder)
        || pixel_for(second, 0, 0) != Err(Error::NotHolder)
        || present_for(second) != Err(Error::NotHolder)
    {
        return false;
    }
    // A process that is not the holder releasing has no effect.
    release_owner(second);
    if holder() != Some(first) {
        return false;
    }
    release_owner(first);
    if holder().is_some() || claim(second).is_err() || pixel_for(second, 0, 0).is_err() {
        return false;
    }
    release_owner(second);
    user_graphics::release_owner(second);
    holder().is_none()
}

/// Stage 19.10: the boot compositor follows the framebuffer, and with every
/// writer behind the terminal lock, taking the display silences the boot
/// console as well as the userspace terminal.
#[cfg(feature = "stage19-10-test")]
pub fn geometry_self_test(console: &mut crate::console::Console) -> bool {
    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S19.10] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    fn screen_checksum() -> Option<u64> {
        crate::terminal::with_framebuffer(|buffer, _| {
            buffer.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
    }
    fn screen_pixel(x: usize, y: usize) -> Option<[u8; 3]> {
        crate::terminal::with_framebuffer(|buffer, info| {
            let offset = (y * info.stride + x) * info.bytes_per_pixel;
            buffer.get(offset..offset + 3).map(|bytes| [bytes[0], bytes[1], bytes[2]])
        })
        .flatten()
    }

    let Some(info) = crate::terminal::framebuffer_info() else {
        return step("framebuffer available", false);
    };
    let expected = (
        (info.width as u32).min(MAX_WIDTH),
        (info.height as u32).min(MAX_HEIGHT),
    );
    crate::serial::write_line(format_args!(
        "[S19.10] framebuffer {}x{}, compositor {:?}",
        info.width,
        info.height,
        dimensions()
    ));
    if !step("boot compositor matches the framebuffer", dimensions() == Some(expected)) {
        return false;
    }

    // Taking the display repaints the whole screen, corner to corner, rather
    // than a fixed 640x480 window in its top-left.
    if !step("display taken", activate_display().is_ok()) {
        return false;
    }
    let covered = (expected.0 as usize) * (expected.1 as usize);
    if !step("takeover repaints the whole framebuffer", present() == Ok(covered))
        || !step(
            "far corner shows the compositor background",
            screen_pixel(expected.0 as usize - 1, expected.1 as usize - 1) == Some([0, 0, 0]),
        )
    {
        deactivate_display();
        return false;
    }

    // The boot console now draws through the terminal lock, so while the
    // compositor owns the display its output is suppressed too.
    let before = screen_checksum();
    console.println("boot console text while compositing");
    let silent = before.is_some() && screen_checksum() == before;
    if !step("boot console suppressed while compositing", silent) {
        deactivate_display();
        return false;
    }
    deactivate_display();
    let before = screen_checksum();
    console.println("boot console text after release");
    step(
        "boot console draws again after release",
        before.is_some() && screen_checksum() != before,
    )
}
