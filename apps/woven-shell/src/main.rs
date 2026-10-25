//! WovenShell: the WovenHat desktop.
//!
//! A userspace program that takes the compositor lease and runs the desktop:
//! a procedural wallpaper, a translucent taskbar with the WovenStart button,
//! one entry per window and a clock, a system tray, notification toasts,
//! client windows with title bars, and a pointer.
//!
//! # Drawing
//!
//! Client surfaces are capped at 320×240, so the desktop is never one
//! surface. The shell's own layers (desktop, title bars, toast, pointer) are
//! rendered in software into a [`Canvas`] in chunks of at most 320×40,
//! uploaded into one staging surface and blitted with a region blit.
//!
//! Every change becomes a repaint of an exposed rectangle, which recomposes
//! that rectangle completely, bottom to top: desktop, tray icons, each window
//! (title bar, then the client's shared surface copied with a region blit),
//! the toast, and the pointer. Region blits copy only the part of each layer
//! inside the rectangle, so nothing stacked above is overdrawn outside it.
//!
//! # Protocol
//!
//! Applications talk to the shell over IPC. Every message starts with a
//! four-byte tag:
//! - `WOPN`, token (u64), width (u16), height (u16), title (up to 24 bytes):
//!   open a window showing a surface the sender shared with the shell.
//! - `TRAY`, token (u64): show a 16×16 surface in the system tray.
//! - `NOTE`, text: show a notification toast for four seconds.
//! - `WPMV`, x (u16), y (u16): report a pointer move in client coordinates.
//! - `WPBT`, button (u8), pressed (u8), x (u16), y (u16): report a bounded
//!   client button edge in client coordinates.
//!
//! A surface is accepted only if the sender owns it: the token names its
//! owner, and the kernel enforces that name on every operation. A window or
//! icon goes away when its surface does, normally because its owner exited.
//! ASCII keystrokes go to the focused window's owner as one-byte messages.
//! Enter and Backspace use the bounded `WKEY` message (`WKEY`, code), keeping
//! special keys distinguishable from text without exposing input authority.
//!
//! A new window fades in: its opacity follows a [`Spring`] from 0 to 255,
//! and its title bar, client area and shadow are composed at that opacity.
//! While anything animates the shell runs a 20 ms frame clock; otherwise
//! 250 ms.
//!
//! The launcher panel and the toast are translucent over whatever lies
//! beneath, windows included: their background is region-blended, and their
//! border and text are an overlay drawn on the transparent key and copied with
//! a keyed blit, as is the pointer. Nothing is read back from the screen.
//!
//! Every window casts a soft shadow of two translucent bands, offset down and
//! right; the focused window sits higher, so its shadow falls further and
//! darker. Shadows stay off the taskbar.
//!
//! Windows change size by request: the shell sends the owner `WSIZ` with a
//! client size, and the owner answers `WRSZ` with a new surface of that size
//! (token, width, height), releasing the old one. Dropping a window on the
//! left or right edge snaps it to that half of the screen. F2 toggles
//! tiling: master and stack, the first window on the left half and the rest
//! sharing the right, re-tiled as windows open and close.
//!
//! A window is moved by dragging its title bar. Its close control sends the
//! owner `WCLS`, a request: the window goes when the owner releases its
//! surface, so an application can finish first, or decline.
//!
//! # Notification Center
//!
//! Every notification is kept after its toast expires: the last
//! [`MAX_NOTES`], newest first, each labelled with the sender's window title.
//! A badge beside the clock counts the unread ones. Clicking the clock opens
//! the Notification Center, a translucent panel listing them, and marks them
//! read; its Clear button empties the history.
//!
//! # Quick Settings
//!
//! The button left of the tray opens Quick Settings, a translucent panel of
//! toggles that act on the system today: the display scale (1x or 2x, with
//! every window laid out again at the new geometry), Do Not Disturb (no
//! toasts; notifications are still remembered and counted), and tiling (as
//! F2). Settings the system cannot act on yet are not offered.
//!
//! # Lock screen
//!
//! Quick Settings' last row locks the desktop. The lock screen covers
//! everything: no window is drawn, no keystroke reaches an application, and
//! a notification is remembered and counted but never shown. It shows the
//! time and a PIN prompt, with one masked dot per digit typed.
//!
//! The PIN is given with `--pin <digits>` and kept in the shell's own
//! memory: the kernel's secret vault needs the `Secrets` capability, which
//! the security model reserves for broker-granted applications and keeps out
//! of system services such as the shell. Attempts are compared in constant
//! time and typed digits are wiped after each one. Three wrong PINs in a row
//! lock entry out for five seconds. Without `--pin`, Enter unlocks: real
//! accounts and credentials come with the login manager.
//!
//! # Login
//!
//! With `--accounts <file>` the shell starts at a sign-in screen. The file
//! holds one local account per line, `name:salt-hex:hash-hex`, the hash
//! being `PBKDF2-HMAC-SHA256(PIN, salt, PBKDF2_ITERATIONS)` (see
//! `libwoven::accounts`); no PIN is stored.
//! Tab chooses the account, digits enter its PIN, Enter signs in, with the
//! same masking and lockout as the lock screen. Once signed in, the
//! account's name is shown in the taskbar, and the lock screen asks for that
//! account's PIN. The kernel identity service owns the authenticated session
//! and applies its credentials to this process.
//!
//! # Monitors
//!
//! With several monitors (a multi-head display), the desktop spans them
//! left to right. The first is the primary: the taskbar, panels, launcher,
//! new windows and tiling use it, while the wallpaper, pointer and window
//! dragging cover every monitor.
//!
//! # Workspaces
//!
//! Four workspaces, each with its own windows and stacking order. The
//! switcher left of the Quick Settings button shows them, the current one
//! highlighted; clicking one, or F3, switches. F4 moves the focused window
//! to the next workspace and follows it. Windows open on the current
//! workspace; tiling and the taskbar's window entries cover only it.
//!
//! # WovenStart
//!
//! The start button, or F1, opens the launcher: the applications listed in
//! `/share/applications`, filtered as you type. Enter or a click launches
//! one. Each catalog entry is a small file of `Name=` and `Exec=` lines;
//! `%p` in the command becomes the shell's process id, which is how a
//! launched application finds the shell. Launched applications are the
//! shell's children, and it reaps them as they exit.
//!
//! On a display at least 1280×800 the shell switches to display scale 2, so
//! the 5×7 font stays legible. With `--seconds` the clock shows seconds.
#![no_std]
#![no_main]

use core::cell::UnsafeCell;
use core::panic::PanicInfo;

use libwoven::graphics::{Camera, DepthBuffer, Renderer, Vertex3, Vertex3Field, Vertex3Tex};
use libwoven::layout::{self, Frame};
use libwoven::{
    blend, font, process, rgb, wait, Args, Canvas, DateTime, FrameEvent, GraphicsSurface,
    InputEvent, Key, Region, Spring, TRANSPARENT,
};

libwoven::entry!(main);

/// Wallpaper gradient, top to bottom.
const WALLPAPER_TOP: u32 = rgb(11, 29, 58);
const WALLPAPER_BOTTOM: u32 = rgb(16, 78, 82);
/// The weave: `WEAVE_PITCH`-sized blocks alternately tinted light and dark,
/// basket-weave style, with a thin highlight seam at each block boundary.
const WEAVE: u32 = rgb(255, 255, 255);
const WEAVE_OPACITY: u8 = 10;
const WEAVE_SHADE: u32 = rgb(0, 0, 0);
const WEAVE_TINT_OPACITY: u8 = 12;
const WEAVE_PITCH: i32 = 96;

const TASKBAR_HEIGHT: i32 = 24;
const TASKBAR: u32 = rgb(10, 18, 30);
const TASKBAR_OPACITY: u8 = 216;
const ACCENT: u32 = rgb(76, 190, 188);
const START_X: i32 = 4;
const START_WIDTH: usize = 44;
const START_HEIGHT: usize = 16;
const START_FILL: u32 = rgb(28, 115, 145);
const START_LABEL: &str = "Woven";
/// The WovenStart icon: a basket-weave glyph, two shades suggesting strands
/// crossing over and under. `A` is the strand that is on top, `D` the one
/// beneath it; `.` is transparent (the button fill shows through).
const START_ICON: [&[u8; 8]; 8] = [
    b"AA..AA..",
    b"AA..AA..",
    b"..DD..DD",
    b"..DD..DD",
    b"AA..AA..",
    b"AA..AA..",
    b"..DD..DD",
    b"..DD..DD",
];
const START_ICON_SIZE: i32 = 8;
const START_ICON_A: u32 = rgb(235, 225, 190);
const START_ICON_D: u32 = rgb(165, 125, 70);
/// Left margin from the button's edge to the icon, chosen so it clears the
/// background-fill sample point Stage 20.1's self-test reads at `(6, top+6)`.
const START_ICON_MARGIN: i32 = 3;
const START_ICON_GAP: i32 = 2;
const TEXT: u32 = rgb(255, 255, 255);
/// Clock text is right-aligned this far from the right edge.
const CLOCK_MARGIN: i32 = 6;
/// Room for the widest clock reading, `HH:MM:SS`, plus its margin.
const CLOCK_WIDTH: i32 = 8 * font::ADVANCE + CLOCK_MARGIN;

/// Taskbar window entries, left to right after the start button.
const ENTRY_X: i32 = START_X + START_WIDTH as i32 + 8;
const ENTRY_WIDTH: i32 = 80;
const ENTRY_GAP: i32 = 4;
const ENTRY_ACTIVE: u32 = rgb(70, 120, 210);
const ENTRY_INACTIVE: u32 = rgb(44, 52, 68);
/// Characters of a title that fit in an entry.
const ENTRY_CHARS: usize = ((ENTRY_WIDTH - 8) / font::ADVANCE) as usize;

/// Tray icons, right to left from the clock.
const TRAY_ICON: i32 = 16;
const TRAY_GAP: i32 = 4;
const MAX_TRAY: usize = 4;

const TITLE_HEIGHT: i32 = 14;
const TITLE_ACTIVE: u32 = rgb(70, 120, 210);
const TITLE_INACTIVE: u32 = rgb(60, 66, 80);
const MAX_TITLE: usize = 24;
/// Shadow bands, outermost first, as (offset, opacity) of black over what
/// lies beneath. The focused window's shadow is deeper: its elevation.
const FOCUSED_SHADOW: [(i32, u8); 2] = [(8, 24), (4, 48)];
const UNFOCUSED_SHADOW: [(i32, u8); 2] = [(4, 20), (2, 40)];
/// The furthest any shadow band reaches beyond its window.
const SHADOW_REACH: i32 = 8;
const SHADOW: u32 = rgb(0, 0, 0);

/// The close control: a square at the right of the title bar.
const CLOSE_SIZE: i32 = 10;
const CLOSE_FILL: u32 = rgb(170, 60, 60);
const MAX_WINDOWS: usize = 4;
/// Workspaces, and the switcher boxes that show them.
const WORKSPACES: usize = 4;
const WORKSPACE_BOX: i32 = 12;
const WORKSPACE_GAP: i32 = 2;
/// New windows cascade from here, this far apart.
const CASCADE_ORIGIN: i32 = 40;
const CASCADE_STEP: i32 = 24;

const TOAST_WIDTH: i32 = 180;
const TOAST_HEIGHT: i32 = 22;
const TOAST_MARGIN: i32 = 8;
const TOAST_FILL: u32 = rgb(30, 34, 46);
const TOAST_OPACITY: u8 = 216;
const TOAST_CHARS: usize = ((TOAST_WIDTH - 12) / font::ADVANCE) as usize;
const TOAST_TICKS: u64 = 4 * libwoven::time::TICKS_PER_SECOND;

/// The pointer: `B` black outline, `W` white fill, `.` transparent. The tip
/// is the top-left pixel.
const CURSOR: [&[u8; 7]; 10] = [
    b"B......",
    b"BB.....",
    b"BWB....",
    b"BWWB...",
    b"BWWWB..",
    b"BWWWWB.",
    b"BWWWWWB",
    b"BWWBBBB",
    b"BWB....",
    b"BB.....",
];
const CURSOR_WIDTH: i32 = 7;
const CURSOR_HEIGHT: i32 = 10;

/// The application catalog: one `*.app` file per application.
const CATALOG: &str = "/share/applications";
// Production system-apps publishes ten bounded catalog entries. Keep the
// launcher capacity aligned with that manifest so alphabetical loading does
// not silently drop the last applications (notably Update and Terminal).
const MAX_APPS: usize = 10;
const MAX_APP_NAME: usize = 20;
const MAX_EXEC: usize = 96;
const MAX_QUERY: usize = 16;
const MAX_CHILDREN: usize = 8;

const PANEL_X: i32 = 4;
const PANEL_WIDTH: i32 = 160;
const PANEL_ROW: i32 = 14;
const PANEL_FILL: u32 = rgb(28, 32, 44);
const PANEL_OPACITY: u8 = 216;
const PANEL_TITLE: &str = "WovenStart";

/// Notifications remembered for the Notification Center.
const MAX_NOTES: usize = 8;
const CENTER_WIDTH: i32 = 164;
const CENTER_TITLE: &str = "Notifications";
const CENTER_CLEAR: &str = "Clear";
const QS_TITLE: &str = "Quick Settings";
const QS_ROWS: usize = 10;

/// How dark the lock screen makes the wallpaper behind it.
const LOCK_DIM: u8 = 160;
const MAX_PIN: usize = 16;
/// Wrong PINs in a row before entry is locked out, and for how long.
const PIN_ATTEMPTS: u32 = 3;
const PIN_LOCKOUT: u64 = 5 * libwoven::time::TICKS_PER_SECOND;

/// Accounts the sign-in screen offers.
const MAX_ACCOUNTS: usize = 4;

/// Whether the locked screen is the sign-in screen, the lock screen, or the
/// signed-in account changing its own PIN.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LockMode {
    SignIn,
    Lock,
    ChangePin,
}

/// A step of `LockMode::ChangePin`, in order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChangePinStep {
    /// Verify the account's current PIN.
    Current,
    /// Collect the new PIN.
    New,
    /// Collect it again, and only proceed if the two match.
    Confirm,
}

/// What the lock screen's prompt says.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LockPrompt {
    Enter,
    Wrong,
    LockedOut,
}

/// Characters of a `source: text` row that fit.
const CENTER_CHARS: usize = ((CENTER_WIDTH - 12) / font::ADVANCE) as usize;

const TILE_WIDTH: usize = 320;
const TILE_HEIGHT: usize = 40;
const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;

/// The shell wakes at least four times a second for the clock, the toast
/// timer and window liveness.
const FRAME_PERIOD: u64 = libwoven::time::TICKS_PER_SECOND / 4;
/// How long a resize may take before a vanished surface counts as an exit.
// A resize client may need to copy a bounded replacement Canvas before its
// WRSZ message reaches the shell on a slow, multi-CPU-free QEMU guest.
const RESIZE_GRACE: u64 = 5 * libwoven::time::TICKS_PER_SECOND;
/// The frame clock while something animates: 20 ms.
const ANIMATION_PERIOD: u64 = 2;
const KEY_ENTER: u8 = 1;
const KEY_BACKSPACE: u8 = 2;

/// The one tile buffer, in .bss rather than on the stack.
struct TileBuffer(UnsafeCell<[u32; TILE_PIXELS]>);

// SAFETY: the shell is single-threaded, and `main` takes the only reference.
unsafe impl Sync for TileBuffer {}

static TILE: TileBuffer = TileBuffer(UnsafeCell::new([0; TILE_PIXELS]));

/// The Quick Settings panel's maximum pixel footprint, flat-rendered once
/// per repaint into this offscreen texture, then sampled by the 3D desktop
/// mode's perspective projection. `.bss`, not the stack, for the same reason
/// as `TILE`.
const QS_PANEL_MAX_HEIGHT: usize = (1 + QS_ROWS) * PANEL_ROW as usize + 2;
const QS_PANEL_PIXELS: usize = CENTER_WIDTH as usize * QS_PANEL_MAX_HEIGHT;

struct QsPanelTexture(UnsafeCell<[u32; QS_PANEL_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_qs_3d` takes the only reference.
unsafe impl Sync for QsPanelTexture {}

static QS_PANEL_TEXTURE: QsPanelTexture = QsPanelTexture(UnsafeCell::new([0; QS_PANEL_PIXELS]));

/// The launcher panel's maximum pixel footprint (every catalog entry
/// matching, `MAX_APPS` rows), flat-rendered once per repaint into this
/// offscreen texture, then sampled by the 3D desktop mode's perspective
/// projection.
const LAUNCHER_PANEL_MAX_HEIGHT: usize = (2 + MAX_APPS) * PANEL_ROW as usize + 2;
const LAUNCHER_PANEL_PIXELS: usize = PANEL_WIDTH as usize * LAUNCHER_PANEL_MAX_HEIGHT;

struct LauncherPanelTexture(UnsafeCell<[u32; LAUNCHER_PANEL_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_panel_3d` takes the
// only reference.
unsafe impl Sync for LauncherPanelTexture {}

static LAUNCHER_PANEL_TEXTURE: LauncherPanelTexture =
    LauncherPanelTexture(UnsafeCell::new([0; LAUNCHER_PANEL_PIXELS]));

/// The Notification Center's maximum pixel footprint (`MAX_NOTES` rows),
/// flat-rendered once per repaint into this offscreen texture, then
/// sampled by the 3D desktop mode's perspective projection.
const CENTER_MAX_HEIGHT: usize = (1 + MAX_NOTES) * PANEL_ROW as usize + 2;
const CENTER_PIXELS: usize = CENTER_WIDTH as usize * CENTER_MAX_HEIGHT;

struct CenterTexture(UnsafeCell<[u32; CENTER_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_center_3d` takes the
// only reference.
unsafe impl Sync for CenterTexture {}

static CENTER_TEXTURE: CenterTexture = CenterTexture(UnsafeCell::new([0; CENTER_PIXELS]));

/// The toast's fixed pixel footprint, flat-rendered once per repaint into
/// this offscreen texture, then sampled by the 3D desktop mode's
/// perspective projection.
const TOAST_PIXELS: usize = TOAST_WIDTH as usize * TOAST_HEIGHT as usize;

struct ToastTexture(UnsafeCell<[u32; TOAST_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_toast_3d` takes the
// only reference.
unsafe impl Sync for ToastTexture {}

static TOAST_TEXTURE: ToastTexture = ToastTexture(UnsafeCell::new([0; TOAST_PIXELS]));

/// The taskbar's maximum pixel footprint, flat-rendered once per repaint
/// into this offscreen texture, then sampled by the 3D desktop mode's
/// perspective projection. `MAX_TASKBAR_WIDTH` mirrors the kernel
/// compositor's own `MAX_WIDTH` bound (two 1280-pixel monitors side by
/// side), since the taskbar spans the primary monitor's full width.
const MAX_TASKBAR_WIDTH: usize = 2560;
const TASKBAR_PIXELS: usize = MAX_TASKBAR_WIDTH * TASKBAR_HEIGHT as usize;

struct TaskbarTexture(UnsafeCell<[u32; TASKBAR_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_taskbar_3d` takes the
// only reference.
unsafe impl Sync for TaskbarTexture {}

static TASKBAR_TEXTURE: TaskbarTexture = TaskbarTexture(UnsafeCell::new([0; TASKBAR_PIXELS]));

/// A depth buffer sized for one `TILE`-sized render pass, shared by every 3D
/// surface (the Quick Settings panel, title bars, the wallpaper, and so on):
/// each one clears it before use and none holds it across a call, so they
/// never need their own copy.
struct TileDepthBuffer(UnsafeCell<[u16; TILE_PIXELS]>);

// SAFETY: the shell is single-threaded, and each 3D render function takes
// the only reference, for the duration of that call only.
unsafe impl Sync for TileDepthBuffer {}

static TILE_DEPTH_3D: TileDepthBuffer = TileDepthBuffer(UnsafeCell::new([0; TILE_PIXELS]));

/// The desktop's state, in .bss too: the user stack is 16 KiB, and the state
/// built in `main`'s frame, with the catalog loader's on top, overflowed it.
struct DesktopState(UnsafeCell<Desktop>);

// SAFETY: the shell is single-threaded, and `main` takes the only reference.
unsafe impl Sync for DesktopState {}

static DESKTOP: DesktopState = DesktopState(UnsafeCell::new(Desktop::INITIAL));

/// Exit with a numeric reason so the gate can tell which step failed.
fn fail(code: u64) -> ! {
    let _ = libwoven::print("woven-shell: failure at step ");
    let mut digits = [0u8; 20];
    let _ = libwoven::write(libwoven::syscall::STDOUT, libwoven::format_u64(code, &mut digits));
    let _ = libwoven::print("\n");
    libwoven::exit(code)
}

fn log(parts: &[&[u8]]) {
    for part in parts {
        let _ = libwoven::write(libwoven::syscall::STDOUT, part);
    }
    let _ = libwoven::print("\n");
}

/// Wallpaper colour at `(x, y)` on a screen `height` pixels tall: a vertical
/// gradient with a basket-weave texture over it, alternating light and dark
/// `WEAVE_PITCH`-sized blocks with a thin highlight seam between them.
fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
    let span = (height - 1).max(1);
    let t = (y.clamp(0, span) * 255 / span) as u8;
    let base = blend(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
    let over = (x / WEAVE_PITCH + y / WEAVE_PITCH) % 2 == 0;
    let tint = if over { WEAVE } else { WEAVE_SHADE };
    let woven = blend(tint, base, WEAVE_TINT_OPACITY);
    if x % WEAVE_PITCH == 0 || y % WEAVE_PITCH == 0 {
        blend(WEAVE, woven, WEAVE_OPACITY)
    } else {
        woven
    }
}

/// Clock text for `time`: `HH:MM`, or `HH:MM:SS` with `seconds`, and dashes
/// when the clock could not be read. Returns the bytes and their length.
fn clock_text(time: Option<DateTime>, seconds: bool) -> ([u8; 8], usize) {
    let digits = |value: u8| [b'0' + value / 10 % 10, b'0' + value % 10];
    let text = match time {
        Some(time) => {
            let [h0, h1] = digits(time.hour);
            let [m0, m1] = digits(time.minute);
            let [s0, s1] = digits(time.second);
            [h0, h1, b':', m0, m1, b':', s0, s1]
        }
        None => *b"--:--:--",
    };
    (text, if seconds { 8 } else { 5 })
}

/// Fill an already-allocated, already-zeroed catalog in place. Never builds
/// or returns a `Catalog` by value: at `MAX_TRANSLATIONS` entries it is
/// several KiB, and the user stack is 16 KiB (see `DesktopState`), so even a
/// transient by-value copy on the way into a static overflowed it.
fn fill_lock_catalog(catalog: &mut libwoven::i18n::Catalog, language: libwoven::i18n::Language) {
    use libwoven::i18n::Language;
    let entries: &[(&str, &str)] = match language {
        Language::English => &[
            ("sign_in", "Sign in"),
            ("locked", "Locked"),
            ("change_pin", "Change PIN"),
            ("current_pin", "Current PIN"),
            ("new_pin", "New PIN"),
            ("confirm_new_pin", "Confirm new PIN"),
            ("enter_pin", "Enter PIN"),
            ("wrong_pin", "Wrong PIN"),
            ("try_again_shortly", "Try again shortly"),
        ],
        Language::Chichewa => &[
            ("sign_in", "Lowani"),
            ("locked", "Yatsekedwa"),
            ("change_pin", "Sinthani PIN"),
            ("current_pin", "PIN Yamakono"),
            ("new_pin", "PIN Yatsopano"),
            ("confirm_new_pin", "Tsimikizani PIN Yatsopano"),
            ("enter_pin", "Lowetsani PIN"),
            ("wrong_pin", "PIN Yolakwika"),
            ("try_again_shortly", "Yesaninso Posachedwa"),
        ],
    };
    for (key, value) in entries {
        let _ = catalog.insert(key, value);
    }
}

/// Both lock-screen catalogs, const-initialized to `Catalog::new()` (an
/// all-zero value LLVM places in `.bss`, not `.data`) and filled in place on
/// first use, so no by-value `Catalog` is ever constructed on the stack.
struct LockCatalogs {
    english: UnsafeCell<libwoven::i18n::Catalog>,
    chichewa: UnsafeCell<libwoven::i18n::Catalog>,
    built: UnsafeCell<bool>,
}

// SAFETY: the shell is single-threaded, and `lock_catalog` takes the only reference.
unsafe impl Sync for LockCatalogs {}

static LOCK_CATALOGS: LockCatalogs = LockCatalogs {
    english: UnsafeCell::new(libwoven::i18n::Catalog::new()),
    chichewa: UnsafeCell::new(libwoven::i18n::Catalog::new()),
    built: UnsafeCell::new(false),
};

/// The lock screen's translatable strings for `language`, built once
/// (lazily, in place) through `libwoven::i18n::Catalog` (Stage 26.5).
/// English values are byte-for-byte the literals every already-accepted
/// lock-screen gate expects, so the default language changes nothing;
/// Chichewa is a best-effort demo translation, not a reviewed localization.
fn lock_catalog(language: libwoven::i18n::Language) -> &'static libwoven::i18n::Catalog {
    use libwoven::i18n::Language;
    // SAFETY: single-threaded; build-once guarded by `built`, and every
    // reference handed out is immutable.
    unsafe {
        if !*LOCK_CATALOGS.built.get() {
            fill_lock_catalog(&mut *LOCK_CATALOGS.english.get(), Language::English);
            fill_lock_catalog(&mut *LOCK_CATALOGS.chichewa.get(), Language::Chichewa);
            *LOCK_CATALOGS.built.get() = true;
        }
        match language {
            Language::English => &*LOCK_CATALOGS.english.get(),
            Language::Chichewa => &*LOCK_CATALOGS.chichewa.get(),
        }
    }
}

/// `bytes` as text, cut to `limit` characters; anything not printable ASCII
/// is shown as `?`, so the font's replacement box never appears.
fn label(bytes: &[u8], limit: usize, buffer: &mut [u8; 32]) -> usize {
    let length = bytes.len().min(limit).min(buffer.len());
    for (slot, byte) in buffer.iter_mut().zip(&bytes[..length]) {
        *slot = if (b' '..=b'~').contains(byte) { *byte } else { b'?' };
    }
    length
}

/// A one-pixel outline of `area` in `color`.
fn outline(canvas: &mut Canvas<'_>, area: Area, color: u32) {
    let (width, height) = (area.width as usize, area.height as usize);
    canvas.fill_rect(area.x, area.y, width, 1, color);
    canvas.fill_rect(area.x, area.y + area.height - 1, width, 1, color);
    canvas.fill_rect(area.x, area.y, 1, height, color);
    canvas.fill_rect(area.x + area.width - 1, area.y, 1, height, color);
}

/// A rectangle in screen coordinates.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Area {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Area {
    const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        Self::new(x, y, right - x, bottom - y)
    }

    fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self::new(x, y, right - x, bottom - y)
    }

    /// This area as a region of a layer whose top-left is `(x, y)`.
    fn relative_to(self, x: i32, y: i32) -> Region {
        Region {
            x: (self.x - x) as u16,
            y: (self.y - y) as u16,
            width: self.width as u16,
            height: self.height as u16,
        }
    }
}

/// Most separate areas [`Dirty`] keeps.
const DIRTY_AREAS: usize = 4;

/// The areas to repaint this frame, kept apart while merging them would
/// repaint pixels that did not change: one bounding box over a window at the
/// top left and the tray at the bottom right covers most of the screen.
struct Dirty {
    areas: [Area; DIRTY_AREAS],
    len: usize,
}

impl Dirty {
    const fn new() -> Self {
        Self { areas: [Area::new(0, 0, 0, 0); DIRTY_AREAS], len: 0 }
    }

    fn of(area: Area) -> Self {
        let mut dirty = Self::new();
        dirty.add(area);
        dirty
    }

    fn add(&mut self, area: Area) {
        if area.is_empty() {
            return;
        }
        let size = |area: Area| i64::from(area.width) * i64::from(area.height);
        // Absorb every area whose merge costs no more than the two apart.
        let mut area = area;
        let mut index = 0;
        while index < self.len {
            let other = self.areas[index];
            if size(other.union(area)) <= size(other) + size(area) {
                area = area.union(other);
                self.len -= 1;
                self.areas[index] = self.areas[self.len];
                index = 0;
            } else {
                index += 1;
            }
        }
        if self.len < DIRTY_AREAS {
            self.areas[self.len] = area;
            self.len += 1;
            return;
        }
        // Full: merge into the area that grows least.
        let mut best = 0;
        for index in 1..self.len {
            let growth = |i: usize| size(self.areas[i].union(area)) - size(self.areas[i]);
            if growth(index) < growth(best) {
                best = index;
            }
        }
        self.areas[best] = self.areas[best].union(area);
    }

    fn areas(&self) -> &[Area] {
        &self.areas[..self.len]
    }
}

#[derive(Clone, Copy)]
struct Window {
    owner: process::Id,
    token: u64,
    /// Top-left of the client area; the title bar sits above it.
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: [u8; MAX_TITLE],
    title_length: usize,
    /// Opacity, 0 to 255: animated from 0 as the window opens.
    fade: Spring,
    /// The workspace the window belongs to.
    workspace: usize,
    /// Until this tick, a resize has been requested and the owner may be
    /// swapping surfaces: a failed blit of the old one does not mean it
    /// exited.
    resizing_until: libwoven::time::Ticks,
}

impl Window {
    fn alpha(&self) -> u8 {
        self.fade.value().clamp(0, 255) as u8
    }

    fn title_area(&self) -> Area {
        Area::new(self.x, self.y - TITLE_HEIGHT, self.width, TITLE_HEIGHT)
    }

    fn body(&self) -> Area {
        Area::new(self.x, self.y, self.width, self.height)
    }

    fn frame(&self) -> Area {
        self.title_area().union(self.body())
    }

    /// Everything the window and its deepest shadow can cover: what to
    /// repaint when it opens, closes, moves or changes elevation.
    fn footprint(&self) -> Area {
        let frame = self.frame();
        frame.union(Area::new(frame.x + SHADOW_REACH, frame.y + SHADOW_REACH, frame.width, frame.height))
    }

    /// The close control, or nothing on a window too narrow to carry one.
    fn close_area(&self) -> Area {
        if self.width < 4 * CLOSE_SIZE {
            return Area::new(0, 0, 0, 0);
        }
        Area::new(self.x + self.width - CLOSE_SIZE - 2, self.y - TITLE_HEIGHT + 2, CLOSE_SIZE, CLOSE_SIZE)
    }
}

#[derive(Clone, Copy)]
struct TrayIcon {
    token: u64,
}

#[derive(Clone, Copy)]
struct App {
    name: [u8; MAX_APP_NAME],
    name_length: usize,
    exec: [u8; MAX_EXEC],
    exec_length: usize,
}

impl App {
    fn name(&self) -> &[u8] {
        &self.name[..self.name_length]
    }

    /// Parse a catalog entry: `Name=` and `Exec=` lines, both required.
    fn parse(text: &[u8]) -> Option<Self> {
        let mut app = Self {
            name: [0; MAX_APP_NAME],
            name_length: 0,
            exec: [0; MAX_EXEC],
            exec_length: 0,
        };
        for line in text.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if let Some(name) = line.strip_prefix(b"Name=") {
                if name.is_empty() || name.len() > MAX_APP_NAME {
                    return None;
                }
                app.name[..name.len()].copy_from_slice(name);
                app.name_length = name.len();
            } else if let Some(exec) = line.strip_prefix(b"Exec=") {
                if exec.is_empty() || exec.len() > MAX_EXEC {
                    return None;
                }
                app.exec[..exec.len()].copy_from_slice(exec);
                app.exec_length = exec.len();
            }
        }
        (app.name_length > 0 && app.exec_length > 0).then_some(app)
    }

    /// The command line, with `%p` replaced by the shell's process id.
    fn command<'a>(&self, shell: process::Id, buffer: &'a mut [u8; 128]) -> Option<&'a str> {
        let mut digits = [0u8; 20];
        let pid = libwoven::format_u64(shell.0, &mut digits);
        let exec = &self.exec[..self.exec_length];
        let mut length = 0;
        let mut index = 0;
        while index < exec.len() {
            let part: &[u8] = if exec[index..].starts_with(b"%p") {
                index += 2;
                pid
            } else {
                index += 1;
                &exec[index - 1..index]
            };
            buffer.get_mut(length..length + part.len())?.copy_from_slice(part);
            length += part.len();
        }
        core::str::from_utf8(&buffer[..length]).ok()
    }
}

/// Load the catalog, sorted by name. A missing catalog, or no `FileRead`,
/// leaves the launcher empty rather than stopping the shell.
/// The apps are written into `apps`, which starts empty; returns how many.
fn load_catalog(apps: &mut [Option<App>; MAX_APPS]) -> usize {
    let mut count = 0;
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut index = 0;
    while count < MAX_APPS {
        let Ok((length, is_directory)) = libwoven::read_dir(CATALOG, index, &mut name) else {
            break;
        };
        index += 1;
        if is_directory || !name[..length].ends_with(b".app") {
            continue;
        }
        let mut path = [0u8; 128];
        let prefix = CATALOG.as_bytes();
        let total = prefix.len() + 1 + length;
        if total > path.len() {
            continue;
        }
        path[..prefix.len()].copy_from_slice(prefix);
        path[prefix.len()] = b'/';
        path[prefix.len() + 1..total].copy_from_slice(&name[..length]);
        let Ok(path) = core::str::from_utf8(&path[..total]) else { continue };
        let Ok(descriptor) = libwoven::open(path) else { continue };
        let mut text = [0u8; 256];
        let read = libwoven::read(descriptor, &mut text);
        let _ = libwoven::close(descriptor);
        let Some(app) = read.ok().and_then(|length| App::parse(&text[..length])) else {
            continue;
        };
        // Insertion sort by name, so the list reads alphabetically.
        let mut position = count;
        while position > 0 && apps[position - 1].is_some_and(|other: App| other.name() > app.name()) {
            apps[position] = apps[position - 1];
            position -= 1;
        }
        apps[position] = Some(app);
        count += 1;
    }
    count
}

/// Whether `name` contains `query`, ignoring ASCII case.
fn matches(name: &[u8], query: &[u8]) -> bool {
    query.is_empty()
        || name
            .windows(query.len())
            .any(|window| window.eq_ignore_ascii_case(query))
}

#[derive(Clone, Copy)]
struct Child {
    pid: process::Id,
    app: usize,
}

/// One remembered notification, already formatted for its row:
/// `source: text`, cut to what fits.
#[derive(Clone, Copy)]
struct Note {
    text: [u8; 32],
    length: usize,
}

struct Toast {
    text: [u8; 32],
    length: usize,
    until: libwoven::time::Ticks,
}

/// The staging surface and the tile buffer the shell renders into.
struct Painter<'a> {
    staging: GraphicsSurface,
    tile: &'a mut [u32],
}

impl Painter<'_> {
    /// [`draw`](Self::draw), blended at `opacity` over what is composed.
    fn draw_blended(&mut self, area: Area, opacity: u8, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(16);
                };
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region_blended(region, x, y, opacity).is_err()
                {
                    fail(17);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Render an overlay over `area` with `render`, on a canvas pre-filled
    /// with the transparent key, and copy it with a keyed blit: only what
    /// `render` drew lands, over what is already composed.
    fn overlay(&mut self, area: Area, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(14);
                };
                canvas.fill_rect(x, y, width as usize, height as usize, TRANSPARENT);
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region_keyed(region, x, y).is_err()
                {
                    fail(15);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Blend `color` over `area` with uniform `opacity`, in chunks no larger
    /// than the staging surface.
    fn blend(&mut self, area: Area, color: u32, opacity: u8) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(12);
                };
                canvas.fill_rect(x, y, width as usize, height as usize, color);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region_blended(region, x, y, opacity).is_err()
                {
                    fail(13);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Render `area` with `render` in chunks no larger than the staging
    /// surface, uploading and blitting each.
    fn draw(&mut self, area: Area, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(10);
                };
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region(region, x, y).is_err()
                {
                    fail(11);
                }
                x += width;
            }
            y += height;
        }
    }
}

struct Desktop {
    /// The primary monitor's width; the taskbar spans it.
    width: i32,
    /// The whole desktop's width, across every monitor.
    desktop_width: i32,
    height: i32,
    seconds: bool,
    /// The time on screen, `None` if the clock could not be read.
    time: Option<DateTime>,
    windows: [Option<Window>; MAX_WINDOWS],
    /// The current workspace's window slots bottom to top; the last is
    /// focused.
    stack: [usize; MAX_WINDOWS],
    stacked: usize,
    /// The current workspace, and every other workspace's saved stack.
    workspace: usize,
    saved: [([usize; MAX_WINDOWS], usize); WORKSPACES],
    /// Windows opened so far, for the cascade.
    opened: i32,
    tray: [Option<TrayIcon>; MAX_TRAY],
    toast: Option<Toast>,
    /// Remembered notifications, newest first.
    notes: [Option<Note>; MAX_NOTES],
    /// How many of them arrived since the Notification Center was last
    /// opened.
    unread: usize,
    center_open: bool,
    qs_open: bool,
    /// Do Not Disturb: notifications are remembered but raise no toast.
    dnd: bool,
    /// The display scale the shell set.
    scale: u32,
    /// The lock screen's language. Defaults to English, which keeps the
    /// exact strings every already-accepted lock-screen gate expects;
    /// Quick Settings can switch it to Chichewa.
    language: libwoven::i18n::Language,
    /// Whether the desktop renders in 3D: the wallpaper and lock screen as
    /// a receding floor, the taskbar, title bars, and every floating panel
    /// (Quick Settings, launcher, Notification Center, toast) as tilted
    /// cards. Production sessions start in this mode; Quick Settings can
    /// still switch to the flat compatibility presentation.
    three_d: bool,
    /// Accessibility: skip the window-open fade-in animation, appearing at
    /// full opacity immediately. Defaults to off, matching
    /// `libwoven::accessibility::Settings::reduced_motion`'s own default.
    reduced_motion: bool,
    locked: bool,
    /// The PIN and its length, if one was given.
    pin: Option<([u8; MAX_PIN], usize)>,
    /// Digits typed at the lock screen so far.
    entry: [u8; MAX_PIN],
    entry_length: usize,
    failures: u32,
    lockout_until: libwoven::time::Ticks,
    prompt: LockPrompt,
    mode: LockMode,
    accounts: [Option<libwoven::accounts::Account>; MAX_ACCOUNTS],
    /// The account chosen on the sign-in screen.
    chosen: usize,
    /// The account signed in, if any.
    session: Option<usize>,
    /// Kernel-owned authentication token for the signed-in account.
    kernel_session: Option<libwoven::identity::SessionId>,
    /// Which step of changing the signed-in account's PIN is active, under
    /// `LockMode::ChangePin`.
    change_pin: Option<ChangePinStep>,
    /// The current PIN, verified at the `Current` step and kept only long
    /// enough to reach the `Confirm` step; wiped once the change completes,
    /// fails, or is cancelled.
    change_pin_old: [u8; MAX_PIN],
    change_pin_old_length: usize,
    /// The new PIN typed at the `New` step, kept to compare against the
    /// `Confirm` step's entry.
    change_pin_new: [u8; MAX_PIN],
    change_pin_new_length: usize,
    pointer: (i32, i32),
    pointer_visible: bool,
    /// The window being dragged, and where in it the pointer took hold.
    drag: Option<(usize, i32, i32)>,
    /// The client receiving the current primary-button click sequence.
    pointer_capture: Option<usize>,
    /// Whether windows are tiled.
    tiled: bool,
    apps: [Option<App>; MAX_APPS],
    app_count: usize,
    launcher_open: bool,
    query: [u8; MAX_QUERY],
    query_length: usize,
    selected: usize,
    children: [Option<Child>; MAX_CHILDREN],
}

impl Desktop {
    /// The state before `main` fills in the display, options and catalog.
    const INITIAL: Desktop = Desktop {
        width: 0,
        desktop_width: 0,
        height: 0,
        seconds: false,
        time: None,
        windows: [None; MAX_WINDOWS],
        stack: [0; MAX_WINDOWS],
        stacked: 0,
        workspace: 0,
        saved: [([0; MAX_WINDOWS], 0); WORKSPACES],
        opened: 0,
        tray: [None; MAX_TRAY],
        toast: None,
        notes: [None; MAX_NOTES],
        unread: 0,
        center_open: false,
        qs_open: false,
        dnd: false,
        scale: 1,
        language: libwoven::i18n::Language::English,
        // Production presentation is 3D by default. The 3D path still calls
        // the bounded 2D painters to populate textures before projecting
        // them through the software depth-tested renderer.
        three_d: true,
        reduced_motion: false,
        locked: false,
        pin: None,
        entry: [0; MAX_PIN],
        entry_length: 0,
        failures: 0,
        lockout_until: 0,
        prompt: LockPrompt::Enter,
        mode: LockMode::SignIn,
        accounts: [None; MAX_ACCOUNTS],
        chosen: 0,
        session: None,
        kernel_session: None,
        change_pin: None,
        change_pin_old: [0; MAX_PIN],
        change_pin_old_length: 0,
        change_pin_new: [0; MAX_PIN],
        change_pin_new_length: 0,
        pointer: (0, 0),
        pointer_visible: false,
        drag: None,
        pointer_capture: None,
        tiled: false,
        apps: [None; MAX_APPS],
        app_count: 0,
        launcher_open: false,
        query: [0; MAX_QUERY],
        query_length: 0,
        selected: 0,
        children: [None; MAX_CHILDREN],
    };

    fn screen(&self) -> Area {
        Area::new(0, 0, self.desktop_width, self.height)
    }

    /// The lock-screen account rows and PIN prompt are the only pixels that
    /// change while choosing an account. Keeping this damage bounded avoids
    /// repainting the full 1280x800 wallpaper for every key on SMP boots.
    fn lock_account_area(&self) -> Area {
        let middle = self.height / 2;
        self.screen().intersect(Area::new(0, middle - 50, self.desktop_width, 80))
    }

    /// The prompt and masked PIN dots change independently of the wallpaper.
    fn lock_input_area(&self) -> Area {
        let middle = self.height / 2;
        self.screen().intersect(Area::new(0, middle - 8, self.desktop_width, 32))
    }

    fn taskbar_top(&self) -> i32 {
        self.height - TASKBAR_HEIGHT
    }

    fn taskbar(&self) -> Area {
        Area::new(0, self.taskbar_top(), self.width, TASKBAR_HEIGHT)
    }

    /// Row for taskbar text: seven glyph rows centred in the bar below its
    /// one-pixel accent line.
    fn text_y(&self) -> i32 {
        self.taskbar_top() + 9
    }

    fn clock_area(&self) -> Area {
        Area::new(self.width - CLOCK_WIDTH, self.taskbar_top(), CLOCK_WIDTH, TASKBAR_HEIGHT)
    }

    fn entry_area(&self, index: usize) -> Area {
        let x = ENTRY_X + index as i32 * (ENTRY_WIDTH + ENTRY_GAP);
        Area::new(x, self.taskbar_top() + 4, ENTRY_WIDTH, START_HEIGHT as i32)
    }

    fn tray_area(&self, index: usize) -> Area {
        let x = self.width - CLOCK_WIDTH - (index as i32 + 1) * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    fn toast_area(&self) -> Area {
        Area::new(self.width - TOAST_WIDTH - TOAST_MARGIN, TOAST_MARGIN, TOAST_WIDTH, TOAST_HEIGHT)
    }

    fn cursor_area(&self) -> Area {
        if !self.pointer_visible {
            return Area::new(0, 0, 0, 0);
        }
        Area::new(self.pointer.0, self.pointer.1, CURSOR_WIDTH, CURSOR_HEIGHT)
    }

    /// Catalog indices matching the query, in catalog order.
    fn filtered(&self) -> ([usize; MAX_APPS], usize) {
        let mut found = [0; MAX_APPS];
        let mut count = 0;
        let query = &self.query[..self.query_length];
        for (index, app) in self.apps[..self.app_count].iter().enumerate() {
            if app.is_some_and(|app| matches(app.name(), query)) {
                found[count] = index;
                count += 1;
            }
        }
        (found, count)
    }

    fn start_area(&self) -> Area {
        Area::new(START_X, self.taskbar_top() + 4, START_WIDTH as i32, START_HEIGHT as i32)
    }

    /// The launcher panel, sized to its rows, above the start button; empty
    /// while closed.
    fn panel_area(&self) -> Area {
        if !self.launcher_open {
            return Area::new(0, 0, 0, 0);
        }
        let rows = 2 + self.filtered().1.max(1) as i32;
        let height = rows * PANEL_ROW + 2;
        Area::new(PANEL_X, self.taskbar_top() - 2 - height, PANEL_WIDTH, height)
    }

    /// The panel's overlay: border, title row, selection and text, drawn at
    /// `area`'s own position in `canvas`. Used both for the ordinary flat
    /// 2D panel and, rendered once at local origin into an offscreen
    /// texture, as the source the 3D mode's perspective projection samples.
    /// Its background is blended separately, so what lies beneath shows
    /// through.
    fn paint_panel_content(&self, canvas: &mut Canvas<'_>, area: Area) {
        let (x, y) = (area.x, area.y);
        let inner = (area.width - 2) as usize;
        outline(canvas, area, ACCENT);
        canvas.fill_rect(x + 1, y + 1, inner, PANEL_ROW as usize, START_FILL);
        canvas.draw_text(x + 6, y + 5, PANEL_TITLE, TEXT);

        let mut search = [0u8; MAX_QUERY + 3];
        search[..2].copy_from_slice(b"> ");
        search[2..2 + self.query_length].copy_from_slice(&self.query[..self.query_length]);
        search[2 + self.query_length] = b'_';
        let text = core::str::from_utf8(&search[..3 + self.query_length]).unwrap_or("");
        canvas.draw_text(x + 6, y + 1 + PANEL_ROW + 4, text, TEXT);

        let (found, count) = self.filtered();
        if count == 0 {
            canvas.draw_text(x + 6, y + 1 + 2 * PANEL_ROW + 4, "No matches", TEXT);
        }
        for (row, &index) in found[..count].iter().enumerate() {
            let Some(app) = &self.apps[index] else { continue };
            let row_y = y + 1 + (2 + row as i32) * PANEL_ROW;
            if row == self.selected {
                canvas.fill_rect(x + 1, row_y, inner, PANEL_ROW as usize, ENTRY_ACTIVE);
            }
            let text = core::str::from_utf8(app.name()).unwrap_or("?");
            canvas.draw_text(x + 6, row_y + 4, text, TEXT);
        }
    }

    fn render_panel(&self, canvas: &mut Canvas<'_>) {
        let area = self.panel_area();
        if self.three_d {
            self.render_panel_3d(canvas, area);
        } else {
            self.paint_panel_content(canvas, area);
        }
    }

    /// The launcher panel, perspective-projected as a card tilted back in
    /// 3D (its bottom edge stays put; its top edge recedes and narrows)
    /// instead of blitted flat. Same technique as
    /// [`Self::render_qs_3d`]: the flat content is rendered once into
    /// `LAUNCHER_PANEL_TEXTURE` at local origin, then sampled by two
    /// `textured_triangle_3d` calls per tile `canvas` this is invoked for.
    fn render_panel_3d(&self, canvas: &mut Canvas<'_>, area: Area) {
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let texture_pixels = unsafe { &mut *LAUNCHER_PANEL_TEXTURE.0.get() };
        let Some(mut texture) =
            Canvas::new(texture_pixels, 0, 0, area.width as usize, area.height as usize)
        else {
            return self.paint_panel_content(canvas, area);
        };
        // The content painter intentionally draws only the panel's visible
        // overlay pixels because the flat path starts from a transparent
        // keyed canvas.  The 3D source texture is persistent scratch storage,
        // so seed its active footprint with the translucent panel background
        // before every repaint or a prior selection/glyph can leak into the
        // next projected frame.
        texture.shade(|x, y| {
            blend(
                PANEL_FILL,
                wallpaper(area.x + x, area.y + y, self.height),
                PANEL_OPACITY,
            )
        });
        self.paint_panel_content(&mut texture, Area::new(0, 0, area.width, area.height));
        // The projected quad is narrower than the flat panel's footprint at
        // its receded top edge, so the corners it no longer covers still
        // need the real desktop background rather than whatever this
        // tile's shared scratch buffer held. Painted before the depth
        // buffer below is (re-)cleared: `render_desktop_background` clears
        // and uses that same shared buffer for its own projection, so
        // calling it after this function's own clear would corrupt the
        // depth test this function's own triangles depend on.
        self.render_desktop_background(canvas);

        let half_width = (area.width / 2) << 16;
        let half_height = ((area.height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 70 << 16;
        let camera = Camera::new(
            DISTANCE,
            DISTANCE,
            area.x + area.width / 2,
            area.y + area.height / 2,
        );
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return self.paint_panel_content(canvas, area);
        };
        let vertex = |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return self.paint_panel_content(canvas, area);
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, 255, 0),
            vertex(bottom_left, 0, 255),
            &texture,
        );
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_right, 255, 0),
            vertex(bottom_right, 255, 255),
            vertex(bottom_left, 0, 255),
            &texture,
        );
    }

    /// Open or close the launcher. Returns the area to repaint.
    fn toggle_launcher(&mut self) -> Area {
        let mut before = self.panel_area();
        if !self.launcher_open && (self.center_open || self.qs_open) {
            // Only one panel at a time.
            before = before.union(self.center_area()).union(self.qs_area()).union(self.qs_button());
            self.center_open = false;
            self.qs_open = false;
        }
        self.launcher_open = !self.launcher_open;
        self.query_length = 0;
        self.selected = 0;
        before.union(self.panel_area())
    }

    /// Launch catalog entry `index` and close the launcher. A failure is
    /// reported in a toast. Returns the area to repaint.
    fn launch(&mut self, index: usize) -> Area {
        let mut dirty = self.toggle_launcher();
        let Some(app) = self.apps[index] else { return dirty };
        let slot = self.children.iter().position(Option::is_none);
        let mut buffer = [0u8; 128];
        let started = match (slot, app.command(libwoven::current_pid(), &mut buffer)) {
            (Some(slot), Some(command)) => match libwoven::spawn(command) {
                Ok(pid) => {
                    self.children[slot] = Some(Child { pid, app: index });
                    log(&[b"woven-shell: launched ", app.name()]);
                    true
                }
                Err(_) => false,
            },
            _ => false,
        };
        if !started {
            let mut message = [0u8; 64];
            let prefix = b"Could not start ";
            let length = (prefix.len() + app.name_length).min(message.len());
            message[..prefix.len()].copy_from_slice(prefix);
            message[prefix.len()..length].copy_from_slice(&app.name()[..length - prefix.len()]);
            let mut text = [0u8; 32];
            let shown = label(&message[..length], TOAST_CHARS, &mut text);
            self.toast = Some(Toast {
                text,
                length: shown,
                until: libwoven::ticks() + TOAST_TICKS,
            });
            log(&[b"woven-shell: could not start ", app.name()]);
            dirty = dirty.union(self.toast_area());
        }
        dirty
    }

    /// Keys while the launcher is open edit the search and choose.
    fn launcher_key(&mut self, key: Key) -> Area {
        let before = self.panel_area();
        match key {
            Key::Char(character) if (' '..='~').contains(&character) => {
                if self.query_length < MAX_QUERY {
                    self.query[self.query_length] = character as u8;
                    self.query_length += 1;
                }
                self.selected = 0;
            }
            Key::Backspace => {
                self.query_length = self.query_length.saturating_sub(1);
                self.selected = 0;
            }
            Key::Tab => {
                let count = self.filtered().1;
                if count > 0 {
                    self.selected = (self.selected + 1) % count;
                }
            }
            Key::Enter => {
                let (found, count) = self.filtered();
                if self.selected < count {
                    return before.union(self.launch(found[self.selected]));
                }
            }
            Key::F1 => return self.toggle_launcher(),
            // Tiling waits until the launcher closes.
            Key::Char(_) | Key::F2 | Key::F3 | Key::F4 => {}
        }
        before.union(self.panel_area())
    }

    /// Reap launched applications that have exited.
    fn reap(&mut self) {
        for child in self.children.iter_mut() {
            let Some(running) = *child else { continue };
            match libwoven::wait(running.pid) {
                Err(libwoven::Error::WouldBlock) => {}
                result => {
                    let app = self.apps[running.app];
                    let name = app.as_ref().map_or(&b"?"[..], |app| app.name());
                    if result.is_ok() {
                        log(&[b"woven-shell: reaped ", name]);
                    }
                    *child = None;
                }
            }
        }
    }

    fn focused(&self) -> Option<usize> {
        self.stacked.checked_sub(1).map(|top| self.stack[top])
    }

    /// Open windows in slot order: the taskbar's entries.
    /// The current workspace's windows, by slot.
    fn entries(&self) -> impl Iterator<Item = (usize, &Window)> {
        let workspace = self.workspace;
        self.windows
            .iter()
            .enumerate()
            .filter_map(|(slot, window)| window.as_ref().map(|window| (slot, window)))
            .filter(move |(_, window)| window.workspace == workspace)
    }

    /// The switcher box for workspace `index`, left of the Quick Settings
    /// button.
    fn workspace_area(&self, index: usize) -> Area {
        let span = WORKSPACES as i32 * (WORKSPACE_BOX + WORKSPACE_GAP) - WORKSPACE_GAP;
        let x = self.qs_button().x - 6 - span + index as i32 * (WORKSPACE_BOX + WORKSPACE_GAP);
        Area::new(x, self.taskbar_top() + 4, WORKSPACE_BOX, START_HEIGHT as i32)
    }

    /// Show workspace `target`: save the current stack, restore the target's.
    fn switch_workspace(&mut self, target: usize) -> Area {
        if target >= WORKSPACES || target == self.workspace {
            return Area::new(0, 0, 0, 0);
        }
        let _ = self.close_panels();
        self.drag = None;
        self.saved[self.workspace] = (self.stack, self.stacked);
        (self.stack, self.stacked) = self.saved[target];
        self.workspace = target;
        let digit = [b'1' + target as u8];
        log(&[b"woven-shell: workspace ", &digit]);
        let _ = self.retile();
        self.screen()
    }

    /// Move the focused window to the next workspace and follow it.
    fn move_focused_to_next_workspace(&mut self) -> Area {
        let Some(slot) = self.focused() else { return Area::new(0, 0, 0, 0) };
        let target = (self.workspace + 1) % WORKSPACES;
        // Off this workspace's stack, onto the top of the target's.
        self.stacked -= 1;
        let (stack, count) = &mut self.saved[target];
        if *count < MAX_WINDOWS {
            stack[*count] = slot;
            *count += 1;
        }
        if let Some(window) = self.windows[slot].as_mut() {
            window.workspace = target;
        }
        let _ = self.retile();
        self.switch_workspace(target)
    }

    /// A full-screen background, as a flat fill in 2D mode or, in 3D mode,
    /// perspective-projected as a floor receding into the distance -- the
    /// near edge, at the bottom of the screen, stays anchored; the far
    /// edge, at the top, recedes and narrows. This can span the whole
    /// virtual desktop, far larger than any fixed-size offscreen texture,
    /// so 3D mode samples `color` procedurally via
    /// [`libwoven::graphics::Renderer::procedural_triangle_3d`] instead of
    /// texturing a pre-rendered copy. Shared by the desktop wallpaper and
    /// the lock screen's dimmed wallpaper -- the two full-screen
    /// backgrounds, differing only in their per-pixel formula.
    fn render_floor_3d(&self, canvas: &mut Canvas<'_>, color: impl Fn(i32, i32) -> u32) {
        if !self.three_d {
            canvas.shade(&color);
            return;
        }
        let height = self.height;
        let width = self.desktop_width;
        let half_width = (width / 2) << 16;
        let half_height = ((height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 70 << 16;
        let camera = Camera::new(DISTANCE, DISTANCE, width / 2, height / 2);
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            canvas.shade(&color);
            return;
        };
        let vertex = |(x, y, depth): (i32, i32, u16), source_x: i32, source_y: i32| {
            Vertex3Field::new(x, y, depth, source_x, source_y)
        };
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            canvas.shade(&color);
            return;
        };
        depth.clear(u16::MAX);
        // The projected quad narrows at its receded top edge, so the
        // corners it no longer covers still need a sensible background.
        canvas.shade(&color);
        let mut renderer = Renderer::new(canvas);
        renderer.procedural_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, width, 0),
            vertex(bottom_left, 0, height),
            &color,
        );
        renderer.procedural_triangle_3d(
            &mut depth,
            vertex(top_right, width, 0),
            vertex(bottom_right, width, height),
            vertex(bottom_left, 0, height),
            &color,
        );
    }

    /// The desktop background: the wallpaper.
    fn render_desktop_background(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        self.render_floor_3d(canvas, |x, y| wallpaper(x, y, height));
    }

    /// The desktop layer: wallpaper and taskbar, with its start button,
    /// window entries and clock.
    fn render_desktop(&self, canvas: &mut Canvas<'_>) {
        self.render_desktop_background(canvas);
        if self.three_d {
            self.render_taskbar_3d(canvas);
            // Keep the desktop's depth and perspective while presenting the
            // taskbar as a crisp foreground surface. Projecting a 24px bar
            // compresses the bitmap font enough to make labels unreadable.
            self.paint_taskbar_content(canvas);
        } else {
            self.paint_taskbar_content(canvas);
        }
    }

    /// The taskbar's content -- its translucent fill, accent top border,
    /// start button, window entries, clock, workspace switcher, account
    /// name, Quick Settings button, and unread badge -- drawn at its own
    /// real screen position in `canvas`. Used both for the ordinary flat
    /// 2D taskbar and, rendered once into an offscreen texture at that same
    /// position (so every `self.taskbar_top()`-based Area in here lands in
    /// the texture exactly where it would on screen), as the source the 3D
    /// mode's perspective projection samples.
    fn paint_taskbar_content(&self, canvas: &mut Canvas<'_>) {
        let top = self.taskbar_top();
        let width = self.width as usize;
        let height = self.height;
        canvas.shade(|x, y| wallpaper(x, y, height));
        canvas.blend_rect(0, top, width, TASKBAR_HEIGHT as usize, TASKBAR, TASKBAR_OPACITY);
        canvas.fill_rect(0, top, width, 1, ACCENT);

        canvas.fill_rect(START_X, top + 4, START_WIDTH, START_HEIGHT, START_FILL);
        let icon_x = START_X + START_ICON_MARGIN;
        let icon_y = top + 4 + (START_HEIGHT as i32 - START_ICON_SIZE) / 2;
        for (row, line) in START_ICON.iter().enumerate() {
            for (column, cell) in line.iter().enumerate() {
                let color = match cell {
                    b'A' => START_ICON_A,
                    b'D' => START_ICON_D,
                    _ => continue,
                };
                canvas.set(icon_x + column as i32, icon_y + row as i32, color);
            }
        }
        let label_x = icon_x + START_ICON_SIZE + START_ICON_GAP;
        canvas.draw_text(label_x, self.text_y(), START_LABEL, TEXT);

        let focused = self.focused();
        for (index, (slot, window)) in self.entries().enumerate() {
            let area = self.entry_area(index);
            let fill = if Some(slot) == focused { ENTRY_ACTIVE } else { ENTRY_INACTIVE };
            canvas.fill_rect(area.x, area.y, area.width as usize, area.height as usize, fill);
            let mut buffer = [0u8; 32];
            let length = label(&window.title[..window.title_length], ENTRY_CHARS, &mut buffer);
            let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
            canvas.draw_text(area.x + 4, self.text_y(), text, TEXT);
        }

        let (bytes, length) = clock_text(self.time, self.seconds);
        let text = core::str::from_utf8(&bytes[..length]).unwrap_or("");
        let clock_x = self.width - CLOCK_MARGIN - font::text_width(text) as i32;
        canvas.draw_text(clock_x, self.text_y(), text, TEXT);

        // The workspace switcher, left of the Quick Settings button.
        for index in 0..WORKSPACES {
            let area = self.workspace_area(index);
            let fill = if index == self.workspace { ENTRY_ACTIVE } else { ENTRY_INACTIVE };
            canvas.fill_rect(area.x, area.y, area.width as usize, area.height as usize, fill);
            let digit = [b'1' + index as u8];
            canvas.draw_text(area.x + 3, self.text_y(), core::str::from_utf8(&digit).unwrap_or("?"), TEXT);
        }

        // The signed-in account's name, left of the workspace switcher.
        if let Some(account) = self.session.and_then(|index| self.accounts[index]) {
            let name = core::str::from_utf8(account.name()).unwrap_or("?");
            let x = self.workspace_area(0).x - 6 - font::text_width(name) as i32;
            canvas.draw_text(x, self.text_y(), name, TEXT);
        }

        // The Quick Settings button.
        let button = self.qs_button();
        let fill = if self.qs_open { ENTRY_ACTIVE } else { ENTRY_INACTIVE };
        canvas.fill_rect(button.x, button.y, button.width as usize, button.height as usize, fill);
        canvas.draw_text(button.x + 5, self.text_y(), "Q", TEXT);

        // The unread badge, just left of the clock: a count up to 9.
        if self.unread > 0 {
            let badge = self.badge_area();
            canvas.fill_rect(badge.x, badge.y, badge.width as usize, badge.height as usize, ACCENT);
            let digit = [b'0' + self.unread.min(9) as u8];
            canvas.draw_text(badge.x + 3, badge.y + 2, core::str::from_utf8(&digit).unwrap_or("?"), TEXT);
        }
    }

    /// The taskbar, perspective-projected as a card tilted back in 3D (its
    /// bottom edge -- the bottom of the screen -- stays put; its top edge
    /// recedes and narrows slightly, a shallower tilt than the desktop
    /// floor since it is a much thinner strip) instead of blitted flat.
    /// The flat content is rendered once into `TASKBAR_TEXTURE` at the
    /// taskbar's own real position (see [`Self::paint_taskbar_content`]),
    /// then sampled by two `textured_triangle_3d` calls per tile `canvas`
    /// this is invoked for.
    fn render_taskbar_3d(&self, canvas: &mut Canvas<'_>) {
        let top = self.taskbar_top();
        let width = self.width;
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let texture_pixels = unsafe { &mut *TASKBAR_TEXTURE.0.get() };
        let Some(mut texture) = Canvas::new(
            texture_pixels,
            0,
            top,
            width as usize,
            TASKBAR_HEIGHT as usize,
        ) else {
            return self.paint_taskbar_content(canvas);
        };
        self.paint_taskbar_content(&mut texture);
        // The projected quad is narrower than the flat taskbar's footprint
        // at its receded top edge, so the sliver it no longer covers still
        // needs the real desktop background (itself 3D-projected, since 3D
        // mode is already on here) rather than stale tile content. Painted
        // before the depth buffer below is (re-)cleared:
        // `render_desktop_background` clears and uses that same shared
        // buffer for its own projection, so calling it after this
        // function's own clear would corrupt the depth test this
        // function's own triangles depend on.
        self.render_desktop_background(canvas);

        let half_width = (width / 2) << 16;
        let half_height = ((TASKBAR_HEIGHT - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 30 << 16;
        // Keep the taskbar's receded edge attached to the full primary
        // monitor width. Perspective would otherwise pull the top corners
        // inward, leaving the outer taskbar pixels uncovered. The 23/20
        // compensation is the exact inverse of DISTANCE/(DISTANCE+depth)
        // for this fixed 30-on-200 tilt, while retaining the depth gradient.
        let top_half_width = half_width.saturating_mul(23) / 20;
        let camera = Camera::new(DISTANCE, DISTANCE, width / 2, top + TASKBAR_HEIGHT / 2);
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-top_half_width, -half_height, TILT_DEPTH),
            camera.project(top_half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return self.paint_taskbar_content(canvas);
        };
        let vertex = |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return self.paint_taskbar_content(canvas);
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, 255, 0),
            vertex(bottom_left, 0, 255),
            &texture,
        );
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_right, 255, 0),
            vertex(bottom_right, 255, 255),
            vertex(bottom_left, 0, 255),
            &texture,
        );
    }

    /// The Quick Settings button, left of the tray's last icon slot.
    fn qs_button(&self) -> Area {
        let x = self.width - CLOCK_WIDTH - (MAX_TRAY as i32 + 1) * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    /// Quick Settings: a title row and one row per toggle, above the
    /// taskbar's right end; empty while closed.
    fn qs_area(&self) -> Area {
        if !self.qs_open {
            return Area::new(0, 0, 0, 0);
        }
        let height = (1 + QS_ROWS as i32) * PANEL_ROW + 2;
        Area::new(self.width - 4 - CENTER_WIDTH, self.taskbar_top() - 2 - height, CENTER_WIDTH, height)
    }

    /// The toggles as they currently read.
    fn qs_rows(&self) -> [&'static str; QS_ROWS] {
        [
            "Power off",
            "Restart",
            if self.scale == 2 { "Display scale: 2x" } else { "Display scale: 1x" },
            if self.dnd { "Do not disturb: On" } else { "Do not disturb: Off" },
            if self.tiled { "Tiling: On" } else { "Tiling: Off" },
            "Lock screen",
            "Change PIN",
            match self.language {
                libwoven::i18n::Language::English => "Language: English",
                libwoven::i18n::Language::Chichewa => "Language: Chichewa",
            },
            if self.three_d { "3D Desktop: On" } else { "3D Desktop: Off" },
            if self.reduced_motion { "Reduced Motion: On" } else { "Reduced Motion: Off" },
        ]
    }

    /// The lock screen: the wallpaper dimmed, "Locked", the time, the
    /// prompt, and a masked dot per digit typed, all centred.
    fn render_lock(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        self.render_floor_3d(canvas, |x, y| blend(rgb(0, 0, 0), wallpaper(x, y, height), LOCK_DIM));
        let middle = self.height / 2;
        let centred = |text: &str| (self.width - font::text_width(text) as i32) / 2;
        let catalog = lock_catalog(self.language);
        if self.mode == LockMode::SignIn {
            // The sign-in prompt, then the accounts, the chosen one marked.
            let text = catalog.lookup("sign_in", "Sign in");
            canvas.draw_text(centred(text), middle - 60, text, TEXT);
            for (index, account) in self.accounts.iter().flatten().enumerate() {
                let mut line = [b' '; 2 + libwoven::accounts::MAX_NAME];
                if index == self.chosen {
                    line[0] = b'>';
                }
                line[2..2 + account.name().len()].copy_from_slice(account.name());
                let text = core::str::from_utf8(&line[..2 + account.name().len()]).unwrap_or("");
                canvas.draw_text(centred(text), middle - 46 + index as i32 * 10, text, TEXT);
            }
        } else if self.mode == LockMode::ChangePin {
            let text = catalog.lookup("change_pin", "Change PIN");
            canvas.draw_text(centred(text), middle - 34, text, TEXT);
        } else {
            let text = catalog.lookup("locked", "Locked");
            canvas.draw_text(centred(text), middle - 34, text, TEXT);
            let (bytes, length) = clock_text(self.time, false);
            let clock = core::str::from_utf8(&bytes[..length]).unwrap_or("");
            canvas.draw_text(centred(clock), middle - 20, clock, TEXT);
        }
        let prompt = if self.mode == LockMode::ChangePin && self.prompt == LockPrompt::Enter {
            match self.change_pin {
                Some(ChangePinStep::Current) => catalog.lookup("current_pin", "Current PIN"),
                Some(ChangePinStep::New) => catalog.lookup("new_pin", "New PIN"),
                Some(ChangePinStep::Confirm) => catalog.lookup("confirm_new_pin", "Confirm new PIN"),
                None => catalog.lookup("enter_pin", "Enter PIN"),
            }
        } else {
            match self.prompt {
                LockPrompt::Enter => catalog.lookup("enter_pin", "Enter PIN"),
                LockPrompt::Wrong => catalog.lookup("wrong_pin", "Wrong PIN"),
                LockPrompt::LockedOut => catalog.lookup("try_again_shortly", "Try again shortly"),
            }
        };
        canvas.draw_text(centred(prompt), middle - 4, prompt, TEXT);
        let dots = self.entry_length as i32;
        let left = (self.width - (dots * 10 - 4)) / 2;
        for index in 0..dots {
            canvas.fill_rect(left + index * 10, middle + 12, 6, 6, TEXT);
        }
    }

    fn lock(&mut self) -> Area {
        let _ = self.close_panels();
        if let Some(session) = self.kernel_session {
            if libwoven::identity::session_lock(session).is_err() {
                log(&[b"woven-shell: kernel identity lock failed"]);
                return self.screen();
            }
        }
        self.locked = true;
        self.mode = LockMode::Lock;
        self.forget_entry();
        self.prompt = LockPrompt::Enter;
        log(&[b"woven-shell: locked"]);
        self.screen()
    }

    /// Enter `LockMode::ChangePin` for the signed-in account, from Quick
    /// Settings. A toast, not the lock screen, reports the `--pin`-only case
    /// (no persistent account to update) since there is nothing to prompt
    /// for.
    fn start_change_pin(&mut self) -> Area {
        let dirty = self.close_panels();
        if self.session.is_none() {
            let mut text = [0u8; 32];
            let length = label(b"No account signed in", TOAST_CHARS, &mut text);
            self.toast = Some(Toast { text, length, until: libwoven::ticks() + TOAST_TICKS });
            return dirty.union(self.toast_area());
        }
        self.locked = true;
        self.mode = LockMode::ChangePin;
        self.change_pin = Some(ChangePinStep::Current);
        self.forget_entry();
        self.prompt = LockPrompt::Enter;
        log(&[b"woven-shell: change pin started"]);
        self.screen()
    }

    /// Wipe the typed digits.
    fn forget_entry(&mut self) {
        for byte in &mut self.entry {
            // SAFETY: a valid, aligned byte of our own array; volatile so the
            // wipe is not optimised away.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.entry_length = 0;
    }

    /// Check the typed digits: against the chosen account when signing in,
    /// the signed-in account when locked or confirming its current PIN to
    /// change it, or else the `--pin` PIN.
    fn pin_matches(&self) -> bool {
        let account = match self.mode {
            LockMode::SignIn => Some(self.chosen),
            LockMode::Lock | LockMode::ChangePin => self.session,
        };
        if let Some(account) = account.and_then(|index| self.accounts[index]) {
            return account.verify(&self.entry[..self.entry_length]);
        }
        let Some((stored, length)) = &self.pin else { return true };
        // Constant time over the whole buffer: no early exit on a mismatch.
        let mut difference = u8::from(*length != self.entry_length);
        for (stored, typed) in stored.iter().zip(&self.entry) {
            difference |= stored ^ typed;
        }
        difference == 0
    }

    /// A key at the lock screen. Returns the area to repaint.
    fn lock_key(&mut self, key: Key) -> Area {
        match key {
            Key::Char(digit) if digit.is_ascii_digit() => {
                if self.entry_length < MAX_PIN {
                    self.entry[self.entry_length] = digit as u8;
                    self.entry_length += 1;
                }
                self.lock_input_area()
            }
            Key::Backspace => {
                if self.entry_length > 0 {
                    self.entry_length -= 1;
                    self.entry[self.entry_length] = 0;
                } else if self.mode == LockMode::ChangePin {
                    // Backspace on an empty entry cancels: there is no other
                    // key bound to "leave this screen without changing
                    // anything", and the alternative is simply leaving it
                    // through a successful step instead.
                    return self.cancel_change_pin();
                }
                self.lock_input_area()
            }
            Key::Tab if self.mode == LockMode::SignIn => {
                let count = self.accounts.iter().flatten().count().max(1);
                self.chosen = (self.chosen + 1) % count;
                self.forget_entry();
                self.prompt = LockPrompt::Enter;
                self.lock_account_area()
            }
            Key::Enter if self.mode == LockMode::ChangePin => self.change_pin_enter(),
            Key::Enter => {
                if libwoven::ticks() < self.lockout_until {
                    self.prompt = LockPrompt::LockedOut;
                } else if match self.mode {
                    LockMode::SignIn | LockMode::Lock
                        if self.accounts.iter().any(Option::is_some) => true,
                    _ => self.pin_matches(),
                } {
                    let kernel_authenticated = match self.mode {
                        LockMode::SignIn => {
                            match self.accounts[self.chosen] {
                                Some(account) => {
                                    let digest = account.password_digest(&self.entry[..self.entry_length]);
                                    match libwoven::identity::session_open_verified(
                                        account.name(),
                                        &digest,
                                    ) {
                                        Ok(session) => {
                                            self.kernel_session = Some(session);
                                            true
                                        }
                                        Err(_) => false,
                                    }
                                }
                                None => false,
                            }
                        }
                        LockMode::Lock => match (
                            self.kernel_session,
                            self.session.and_then(|index| self.accounts[index]),
                        ) {
                            (Some(session), Some(account)) => {
                                let digest = account.password_digest(&self.entry[..self.entry_length]);
                                libwoven::identity::session_unlock_verified(session, &digest).is_ok()
                            }
                            // No account ever signed in: this is the legacy
                            // `--pin`-only mode, which has no kernel session
                            // to re-verify against. `pin_matches()` already
                            // checked the entry above against `self.pin`.
                            (None, None) => true,
                            _ => false,
                        },
                        LockMode::ChangePin => true,
                    };
                    if !kernel_authenticated {
                        self.failures += 1;
                        self.prompt = LockPrompt::Wrong;
                        log(&[b"woven-shell: kernel identity rejected PIN"]);
                        self.forget_entry();
                        return self.lock_input_area();
                    }
                    self.locked = false;
                    self.failures = 0;
                    if self.mode == LockMode::SignIn {
                        self.session = Some(self.chosen);
                        let name = self.accounts[self.chosen];
                        log(&[b"woven-shell: signed in: ", name.as_ref().map_or(&b"?"[..], |account| account.name())]);
                    } else {
                        log(&[b"woven-shell: unlocked"]);
                    }
                } else {
                    self.failures += 1;
                    self.prompt = LockPrompt::Wrong;
                    log(&[b"woven-shell: wrong PIN"]);
                    if self.failures >= PIN_ATTEMPTS {
                        self.failures = 0;
                        self.lockout_until = libwoven::ticks() + PIN_LOCKOUT;
                        self.prompt = LockPrompt::LockedOut;
                    }
                }
                self.forget_entry();
                if self.locked {
                    self.lock_input_area()
                } else {
                    self.screen()
                }
            }
            _ => Area::new(0, 0, 0, 0),
        }
    }

    /// Securely wipe the stashed current PIN.
    fn wipe_change_pin_old(&mut self) {
        for byte in &mut self.change_pin_old {
            // SAFETY: a valid, aligned byte of our own array; volatile so the
            // wipe is not optimised away.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.change_pin_old_length = 0;
    }

    /// Securely wipe the stashed new PIN.
    fn wipe_change_pin_new(&mut self) {
        for byte in &mut self.change_pin_new {
            // SAFETY: as `wipe_change_pin_old`.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.change_pin_new_length = 0;
    }

    /// Leave `LockMode::ChangePin` without changing anything.
    fn cancel_change_pin(&mut self) -> Area {
        self.wipe_change_pin_old();
        self.wipe_change_pin_new();
        self.change_pin = None;
        self.locked = false;
        self.mode = LockMode::Lock;
        log(&[b"woven-shell: pin change cancelled"]);
        self.screen()
    }

    /// Enter at the active `ChangePin` step: verify the current PIN, collect
    /// the new one, collect it again, and on a matching confirmation, call
    /// `libwoven::accounts::change_pin` and report the result as a toast.
    fn change_pin_enter(&mut self) -> Area {
        match self.change_pin {
            Some(ChangePinStep::Current) => {
                if libwoven::ticks() < self.lockout_until {
                    self.prompt = LockPrompt::LockedOut;
                    self.forget_entry();
                    return self.lock_input_area();
                }
                if self.pin_matches() {
                    self.failures = 0;
                    self.change_pin_old[..self.entry_length]
                        .copy_from_slice(&self.entry[..self.entry_length]);
                    self.change_pin_old_length = self.entry_length;
                    self.change_pin = Some(ChangePinStep::New);
                    self.prompt = LockPrompt::Enter;
                } else {
                    self.failures += 1;
                    self.prompt = LockPrompt::Wrong;
                    if self.failures >= PIN_ATTEMPTS {
                        self.failures = 0;
                        self.lockout_until = libwoven::ticks() + PIN_LOCKOUT;
                        self.prompt = LockPrompt::LockedOut;
                    }
                }
                self.forget_entry();
                self.lock_input_area()
            }
            Some(ChangePinStep::New) => {
                // An empty new PIN is refused silently rather than accepted
                // as "no PIN": Enter on nothing just does nothing here.
                if self.entry_length == 0 {
                    return self.lock_input_area();
                }
                self.change_pin_new[..self.entry_length]
                    .copy_from_slice(&self.entry[..self.entry_length]);
                self.change_pin_new_length = self.entry_length;
                self.change_pin = Some(ChangePinStep::Confirm);
                self.prompt = LockPrompt::Enter;
                self.forget_entry();
                self.lock_input_area()
            }
            Some(ChangePinStep::Confirm) => {
                let matches = self.entry_length == self.change_pin_new_length
                    && self.entry[..self.entry_length] == self.change_pin_new[..self.change_pin_new_length];
                self.forget_entry();
                if !matches {
                    // Does not match: back to entering the new PIN, keeping
                    // the already-verified current one.
                    self.wipe_change_pin_new();
                    self.change_pin = Some(ChangePinStep::New);
                    self.prompt = LockPrompt::Wrong;
                    return self.lock_input_area();
                }
                let name = self.session.and_then(|index| self.accounts[index]);
                let session_name = name.map(|account| {
                    let mut value = [0u8; libwoven::accounts::MAX_NAME];
                    let length = account.name().len();
                    value[..length].copy_from_slice(account.name());
                    (value, length)
                });
                let result = name.map(|account| {
                    let mut buffer = [0u8; libwoven::accounts::MAX_NAME];
                    let length = account.name().len();
                    buffer[..length].copy_from_slice(account.name());
                    libwoven::accounts::change_pin(
                        &buffer[..length],
                        &self.change_pin_old[..self.change_pin_old_length],
                        &self.change_pin_new[..self.change_pin_new_length],
                    )
                });
                self.wipe_change_pin_old();
                self.wipe_change_pin_new();
                self.change_pin = None;
                self.locked = false;
                self.mode = LockMode::Lock;
                let succeeded = matches!(result, Some(Ok(())));
                if succeeded {
                    self.accounts = [None; MAX_ACCOUNTS];
                    load_accounts("/etc/wovenhat/accounts", &mut self.accounts);
                    self.session = session_name.and_then(|(value, length)| {
                        self.accounts.iter().position(|account| {
                            account.is_some_and(|account| account.name() == &value[..length])
                        })
                    });
                }
                let message: &[u8] = match result {
                    Some(Ok(())) => b"PIN changed",
                    Some(Err(_)) => b"Could not change PIN",
                    None => b"No account signed in",
                };
                let mut text = [0u8; 32];
                let length = label(message, TOAST_CHARS, &mut text);
                self.toast = Some(Toast { text, length, until: libwoven::ticks() + TOAST_TICKS });
                log(&[
                    b"woven-shell: pin change ",
                    if succeeded { b"succeeded" } else { b"failed" },
                ]);
                self.screen()
            }
            None => self.lock_input_area(),
        }
    }

    /// The Quick Settings panel's content -- border, title fill, title text,
    /// and every row -- drawn at `area`'s own position in `canvas`. Used
    /// both for the ordinary flat 2D panel and, rendered once at local
    /// origin into an offscreen texture, as the source the 3D mode's
    /// perspective projection samples.
    fn paint_qs_content(&self, canvas: &mut Canvas<'_>, area: Area) {
        let (x, y) = (area.x, area.y);
        outline(canvas, area, ACCENT);
        canvas.fill_rect(x + 1, y + 1, (area.width - 2) as usize, PANEL_ROW as usize, START_FILL);
        canvas.draw_text(x + 6, y + 5, QS_TITLE, TEXT);
        for (row, text) in self.qs_rows().iter().enumerate() {
            canvas.draw_text(x + 6, y + 1 + (1 + row as i32) * PANEL_ROW + 4, text, TEXT);
        }
    }

    fn render_qs(&self, canvas: &mut Canvas<'_>) {
        let area = self.qs_area();
        if self.three_d {
            self.render_qs_3d(canvas, area);
        } else {
            self.paint_qs_content(canvas, area);
        }
    }

    /// The Quick Settings panel, perspective-projected as a card tilted back
    /// in 3D (its bottom edge stays put; its top edge recedes and narrows)
    /// instead of blitted flat. The flat content is rendered once into
    /// `QS_PANEL_TEXTURE` at local origin, then sampled by two
    /// `textured_triangle_3d` calls per tile `canvas` this is invoked for
    /// (the Quick Settings area spans several `TILE_HEIGHT`-tall tiles);
    /// each call clips to its own `canvas`/depth bounds, so painting the
    /// full projected quad on every tile is safe and only the tile's own
    /// pixels are touched.
    fn render_qs_3d(&self, canvas: &mut Canvas<'_>, area: Area) {
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let texture_pixels = unsafe { &mut *QS_PANEL_TEXTURE.0.get() };
        let Some(mut texture) =
            Canvas::new(texture_pixels, 0, 0, area.width as usize, area.height as usize)
        else {
            return self.paint_qs_content(canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                PANEL_FILL,
                wallpaper(area.x + x, area.y + y, self.height),
                PANEL_OPACITY,
            )
        });
        self.paint_qs_content(&mut texture, Area::new(0, 0, area.width, area.height));
        // The projected quad is narrower than the flat panel's footprint at
        // its receded top edge, so the corners it no longer covers would
        // otherwise still show whatever the previous frame drew there (the
        // flat panel, if 3D mode was just switched on) rather than the real
        // desktop background underneath -- itself 3D-projected, since 3D
        // mode is already on here. Painted before the depth buffer below
        // is (re-)cleared: `render_desktop_background` clears and uses
        // that same shared buffer for its own projection, so calling it
        // after this function's own clear would corrupt the depth test
        // this function's own triangles depend on.
        self.render_desktop_background(canvas);

        let half_width = (area.width / 2) << 16;
        // `area.height - 1`, not `area.height`: the bottom edge is meant to
        // land exactly on the panel's last pixel row (an inclusive bound),
        // not one row past it (the exclusive size `area.height` would give).
        let half_height = ((area.height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 70 << 16;
        let camera = Camera::new(
            DISTANCE, // focal_length == distance: the (closest) bottom edge
            // keeps its original flat scale; only the receding top edge
            // narrows.
            DISTANCE,
            area.x + area.width / 2,
            area.y + area.height / 2,
        );
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return self.paint_qs_content(canvas, area);
        };
        let vertex = |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return self.paint_qs_content(canvas, area);
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, 255, 0),
            vertex(bottom_left, 0, 255),
            &texture,
        );
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_right, 255, 0),
            vertex(bottom_right, 255, 255),
            vertex(bottom_left, 0, 255),
            &texture,
        );
    }

    /// Close whichever panel is open. Returns the area to repaint.
    fn close_panels(&mut self) -> Area {
        let mut dirty = Area::new(0, 0, 0, 0);
        if self.launcher_open {
            dirty = dirty.union(self.toggle_launcher());
        }
        if self.center_open {
            dirty = dirty.union(self.center_area());
            self.center_open = false;
        }
        if self.qs_open {
            dirty = dirty.union(self.qs_area()).union(self.qs_button());
            self.qs_open = false;
        }
        dirty
    }

    fn toggle_qs(&mut self) -> Area {
        let opening = !self.qs_open;
        let mut dirty = self.close_panels();
        if opening {
            self.qs_open = true;
            dirty = dirty.union(self.qs_area()).union(self.qs_button());
        }
        dirty
    }

    /// Power off, or restart. Returns only if the machine is still
    /// running; a toast says why.
    fn power(&mut self, restart: bool) -> Area {
        log(if restart { &[b"woven-shell: restart"] } else { &[b"woven-shell: power off"] });
        let result = if restart { libwoven::restart() } else { libwoven::power_off() };
        let reason: &[u8] = match result {
            Err(_) => b"Not allowed to turn the machine off",
            Ok(1) => b"This machine has no ACPI power-off",
            Ok(2) => b"The firmware refused ACPI power-off",
            Ok(_) => b"The machine ignored the request",
        };
        let mut text = [0u8; 32];
        let length = label(reason, TOAST_CHARS, &mut text);
        self.toast = Some(Toast { text, length, until: libwoven::ticks() + TOAST_TICKS });
        self.close_panels().union(self.toast_area())
    }

    fn toggle_tiling(&mut self) -> Area {
        self.tiled = !self.tiled;
        log(if self.tiled { &[b"woven-shell: tiling on"] } else { &[b"woven-shell: tiling off"] });
        self.retile().union(self.qs_area())
    }

    /// Switch between display scales 2 and 1: the compositor surface is
    /// reallocated at the new geometry, every window is kept on screen (and
    /// tiled again if tiling is on), and the whole desktop is repainted.
    fn toggle_scale(&mut self) -> Area {
        let target = if self.scale == 2 { 1 } else { 2 };
        let Ok(info) = libwoven::compositor_set_scale(target) else {
            return Area::new(0, 0, 0, 0);
        };
        self.scale = info.scale;
        // The monitors scale together; the first stays the primary.
        let (outputs, monitors) = libwoven::compositor_outputs().unwrap_or(([libwoven::Output::default(); 4], 0));
        self.desktop_width = info.width as i32;
        self.width = if monitors > 0 { (outputs[0].width as i32).clamp(1, self.desktop_width) } else { self.desktop_width };
        self.height = info.height as i32;
        let top = self.taskbar_top();
        for window in self.windows.iter_mut().flatten() {
            window.x = window.x.clamp(0, (self.desktop_width - window.width).max(0));
            window.y = window.y.clamp(TITLE_HEIGHT, (top - window.height).max(TITLE_HEIGHT));
        }
        self.pointer = (self.pointer.0.min(self.desktop_width - 1), self.pointer.1.min(self.height - 1));
        log(if target == 1 { &[b"woven-shell: display scale 1x"] } else { &[b"woven-shell: display scale 2x"] });
        let _ = self.retile();
        self.screen()
    }

    /// Where the unread badge sits: a 10x11 box just left of the clock text.
    fn badge_area(&self) -> Area {
        let (_, length) = clock_text(self.time, self.seconds);
        let clock_width = length as i32 * font::ADVANCE - (font::ADVANCE - font::WIDTH as i32);
        let clock_x = self.width - CLOCK_MARGIN - clock_width;
        Area::new(clock_x - 13, self.taskbar_top() + 7, 10, 11)
    }

    /// The Notification Center, sized to its rows, above the taskbar's right
    /// end; empty while closed.
    fn center_area(&self) -> Area {
        if !self.center_open {
            return Area::new(0, 0, 0, 0);
        }
        let count = self.notes.iter().flatten().count();
        let height = (1 + count.max(1) as i32) * PANEL_ROW + 2;
        Area::new(self.width - 4 - CENTER_WIDTH, self.taskbar_top() - 2 - height, CENTER_WIDTH, height)
    }

    /// The Notification Center's overlay: border, title row with Clear, and
    /// one row per remembered notification, drawn at `area`'s own position
    /// in `canvas`. Used both for the ordinary flat 2D panel and, rendered
    /// once at local origin into an offscreen texture, as the source the 3D
    /// mode's perspective projection samples.
    fn paint_center_content(&self, canvas: &mut Canvas<'_>, area: Area) {
        let (x, y) = (area.x, area.y);
        outline(canvas, area, ACCENT);
        canvas.fill_rect(x + 1, y + 1, (area.width - 2) as usize, PANEL_ROW as usize, START_FILL);
        canvas.draw_text(x + 6, y + 5, CENTER_TITLE, TEXT);
        let clear_x = x + area.width - 6 - font::text_width(CENTER_CLEAR) as i32;
        canvas.draw_text(clear_x, y + 5, CENTER_CLEAR, TEXT);
        let mut any = false;
        for (row, note) in self.notes.iter().flatten().enumerate() {
            any = true;
            let text = core::str::from_utf8(&note.text[..note.length]).unwrap_or("");
            canvas.draw_text(x + 6, y + 1 + (1 + row as i32) * PANEL_ROW + 4, text, TEXT);
        }
        if !any {
            canvas.draw_text(x + 6, y + 1 + PANEL_ROW + 4, "No notifications", TEXT);
        }
    }

    fn render_center(&self, canvas: &mut Canvas<'_>) {
        let area = self.center_area();
        if self.three_d {
            self.render_center_3d(canvas, area);
        } else {
            self.paint_center_content(canvas, area);
        }
    }

    /// The Notification Center, perspective-projected as a card tilted back
    /// in 3D, the same technique as [`Self::render_qs_3d`]: the flat
    /// content is rendered once into `CENTER_TEXTURE` at local origin, then
    /// sampled by two `textured_triangle_3d` calls per tile `canvas` this
    /// is invoked for.
    fn render_center_3d(&self, canvas: &mut Canvas<'_>, area: Area) {
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let texture_pixels = unsafe { &mut *CENTER_TEXTURE.0.get() };
        let Some(mut texture) =
            Canvas::new(texture_pixels, 0, 0, area.width as usize, area.height as usize)
        else {
            return self.paint_center_content(canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                PANEL_FILL,
                wallpaper(area.x + x, area.y + y, self.height),
                PANEL_OPACITY,
            )
        });
        self.paint_center_content(&mut texture, Area::new(0, 0, area.width, area.height));
        // As in `render_qs_3d`: paint the (3D-projected) desktop background
        // before the depth buffer below is (re-)cleared, since
        // `render_desktop_background` clears and uses that same shared
        // buffer for its own projection and would otherwise corrupt the
        // depth test this function's own triangles depend on.
        self.render_desktop_background(canvas);

        let half_width = (area.width / 2) << 16;
        let half_height = ((area.height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 70 << 16;
        let camera = Camera::new(
            DISTANCE,
            DISTANCE,
            area.x + area.width / 2,
            area.y + area.height / 2,
        );
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return self.paint_center_content(canvas, area);
        };
        let vertex = |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return self.paint_center_content(canvas, area);
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, 255, 0),
            vertex(bottom_left, 0, 255),
            &texture,
        );
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_right, 255, 0),
            vertex(bottom_right, 255, 255),
            vertex(bottom_left, 0, 255),
            &texture,
        );
    }

    /// Open or close the Notification Center. Opening it closes the
    /// launcher and marks every notification read. Returns the area to
    /// repaint.
    fn toggle_center(&mut self) -> Area {
        let opening = !self.center_open;
        let mut dirty = self.close_panels();
        self.center_open = opening;
        if self.center_open && self.unread > 0 {
            self.unread = 0;
            dirty = dirty.union(self.taskbar());
        }
        dirty.union(self.center_area())
    }

    /// Remember a notification from `sender`. Returns the area to repaint.
    fn remember(&mut self, sender: process::Id, text: &[u8]) -> Area {
        // The sender's window title names it; a sender without a window is
        // "app".
        let window = self.windows.iter().flatten().find(|window| window.owner == sender);
        let source = window.map_or(&b"app"[..], |window| &window.title[..window.title_length]);
        let mut row = [0u8; 64];
        let mut length = 0;
        for part in [source, b": ", text] {
            let take = part.len().min(row.len() - length);
            row[length..length + take].copy_from_slice(&part[..take]);
            length += take;
        }
        let mut note = Note { text: [0; 32], length: 0 };
        note.length = label(&row[..length], CENTER_CHARS, &mut note.text);
        let before = self.center_area();
        self.notes.rotate_right(1);
        self.notes[0] = Some(note);
        if !self.center_open {
            self.unread += 1;
        }
        before.union(self.center_area()).union(self.taskbar())
    }

    /// Forget every notification. Returns the area to repaint.
    fn clear_notes(&mut self) -> Area {
        let before = self.center_area();
        self.notes = [None; MAX_NOTES];
        self.unread = 0;
        log(&[b"woven-shell: notifications cleared"]);
        before.union(self.center_area()).union(self.taskbar())
    }

    fn render_title(window: &Window, focused: bool, canvas: &mut Canvas<'_>, three_d: bool) {
        let area = window.title_area();
        let fill = if focused { TITLE_ACTIVE } else { TITLE_INACTIVE };
        if three_d {
            Self::render_title_3d(area, fill, canvas);
        } else {
            canvas.fill_rect(area.x, area.y, area.width as usize, area.height as usize, fill);
        }
        let close = window.close_area();
        if !close.is_empty() {
            canvas.fill_rect(close.x, close.y, close.width as usize, close.height as usize, CLOSE_FILL);
            canvas.draw_text(close.x + 3, close.y + 2, "x", TEXT);
        }
        let mut buffer = [0u8; 32];
        let reserved = if close.is_empty() { 8 } else { 8 + CLOSE_SIZE + 4 };
        let chars = ((window.width - reserved) / font::ADVANCE).max(0) as usize;
        let length = label(&window.title[..window.title_length], chars, &mut buffer);
        let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
        canvas.draw_text(area.x + 4, area.y + 4, text, TEXT);
    }

    /// A window's title-bar fill, perspective-projected as a card tilted
    /// back in 3D (bottom edge anchored, top edge receded and narrowed),
    /// instead of flat. Flat-colour, not textured -- unlike the Quick
    /// Settings panel, there is no bounded-size offscreen render to sample,
    /// since a title bar's width is the window's own and windows can be
    /// much wider than `QS_PANEL_TEXTURE` is sized for. The flat colour is
    /// filled first so any gap between the narrowed top edge and the
    /// original flat footprint still shows a sensible colour (the window's
    /// own fill) rather than whatever this tile's shared scratch buffer
    /// held from an unrelated previous repaint.
    fn render_title_3d(area: Area, fill: u32, canvas: &mut Canvas<'_>) {
        canvas.fill_rect(area.x, area.y, area.width as usize, area.height as usize, fill);
        let half_width = (area.width / 2) << 16;
        let half_height = ((area.height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 20 << 16;
        let camera = Camera::new(
            DISTANCE,
            DISTANCE,
            area.x + area.width / 2,
            area.y + area.height / 2,
        );
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return;
        };
        let color = libwoven::graphics::Color::rgb(
            (fill >> 16) as u8,
            (fill >> 8) as u8,
            fill as u8,
        );
        let vertex = |(x, y, depth): (i32, i32, u16)| Vertex3::new(x, y, depth, color);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return;
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.triangle_3d(&mut depth, vertex(top_left), vertex(top_right), vertex(bottom_left));
        renderer.triangle_3d(&mut depth, vertex(top_right), vertex(bottom_right), vertex(bottom_left));
    }

    /// The toast's overlay: border and text over its blended background,
    /// drawn at `area`'s own position in `canvas`. Used both for the
    /// ordinary flat 2D toast and, rendered once at local origin into an
    /// offscreen texture, as the source the 3D mode's perspective
    /// projection samples.
    fn paint_toast_content(&self, toast: &Toast, canvas: &mut Canvas<'_>, area: Area) {
        outline(canvas, area, ACCENT);
        let text = core::str::from_utf8(&toast.text[..toast.length]).unwrap_or("");
        canvas.draw_text(area.x + 6, area.y + 8, text, TEXT);
    }

    fn render_toast(&self, toast: &Toast, canvas: &mut Canvas<'_>) {
        let area = self.toast_area();
        if self.three_d {
            self.render_toast_3d(toast, canvas, area);
        } else {
            self.paint_toast_content(toast, canvas, area);
        }
    }

    /// The toast, perspective-projected as a card tilted back in 3D, the
    /// same technique as [`Self::render_qs_3d`]: the flat content is
    /// rendered once into `TOAST_TEXTURE` at local origin, then sampled by
    /// two `textured_triangle_3d` calls per tile `canvas` this is invoked
    /// for.
    fn render_toast_3d(&self, toast: &Toast, canvas: &mut Canvas<'_>, area: Area) {
        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let texture_pixels = unsafe { &mut *TOAST_TEXTURE.0.get() };
        let Some(mut texture) =
            Canvas::new(texture_pixels, 0, 0, area.width as usize, area.height as usize)
        else {
            return self.paint_toast_content(toast, canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                TOAST_FILL,
                wallpaper(area.x + x, area.y + y, self.height),
                TOAST_OPACITY,
            )
        });
        self.paint_toast_content(toast, &mut texture, Area::new(0, 0, area.width, area.height));
        // As in `render_qs_3d`: paint the (3D-projected) desktop background
        // before the depth buffer below is (re-)cleared, since
        // `render_desktop_background` clears and uses that same shared
        // buffer for its own projection and would otherwise corrupt the
        // depth test this function's own triangles depend on.
        self.render_desktop_background(canvas);

        let half_width = (area.width / 2) << 16;
        let half_height = ((area.height - 1) << 16) / 2;
        const DISTANCE: i32 = 200 << 16;
        const TILT_DEPTH: i32 = 30 << 16;
        let camera = Camera::new(
            DISTANCE,
            DISTANCE,
            area.x + area.width / 2,
            area.y + area.height / 2,
        );
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            camera.project(-half_width, -half_height, TILT_DEPTH),
            camera.project(half_width, -half_height, TILT_DEPTH),
            camera.project(-half_width, half_height, 0),
            camera.project(half_width, half_height, 0),
        ) else {
            return self.paint_toast_content(toast, canvas, area);
        };
        let vertex = |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) =
            DepthBuffer::new(depth_values, canvas.x(), canvas.y(), canvas.width(), canvas.height())
        else {
            return self.paint_toast_content(toast, canvas, area);
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_left, 0, 0),
            vertex(top_right, 255, 0),
            vertex(bottom_left, 0, 255),
            &texture,
        );
        renderer.textured_triangle_3d(
            &mut depth,
            vertex(top_right, 255, 0),
            vertex(bottom_right, 255, 255),
            vertex(bottom_left, 0, 255),
            &texture,
        );
    }

    /// The pointer, as an overlay: its transparent cells stay on the key.
    fn render_cursor(&self, canvas: &mut Canvas<'_>) {
        let (left, top) = self.pointer;
        for (row, line) in CURSOR.iter().enumerate() {
            for (column, cell) in line.iter().enumerate() {
                let color = match cell {
                    b'B' => rgb(0, 0, 0),
                    b'W' => TEXT,
                    _ => continue,
                };
                canvas.set(left + column as i32, top + row as i32, color);
            }
        }
    }

    /// Recompose `area` completely, bottom to top.
    fn repaint(&self, painter: &mut Painter<'_>, area: Area) {
        let area = area.intersect(self.screen());
        if area.is_empty() {
            return;
        }
        // Locked, nothing but the lock screen is drawn: no window, no
        // notification, no pointer.
        if self.locked {
            painter.draw(area, |canvas| self.render_lock(canvas));
            return;
        }
        painter.draw(area, |canvas| self.render_desktop(canvas));

        for (index, icon) in self.tray.iter().enumerate() {
            let Some(icon) = icon else { continue };
            let place = self.tray_area(index);
            let part = place.intersect(area);
            if !part.is_empty() {
                // A failure means the icon's surface is gone; the next
                // liveness check removes it.
                let region = part.relative_to(place.x, place.y);
                let _ = libwoven::compositor_blit_region_shared(icon.token, region, part.x, part.y);
            }
        }

        let focused = self.focused();
        // Windows may sit on any monitor; only the taskbar band is excluded.
        let above_taskbar = Area::new(0, 0, self.desktop_width, self.taskbar_top());
        for &slot in &self.stack[..self.stacked] {
            let Some(window) = &self.windows[slot] else { continue };
            let bands = if Some(slot) == focused { FOCUSED_SHADOW } else { UNFOCUSED_SHADOW };
            let frame = window.frame();
            // Everything about a fading window, its shadow included, is
            // composed at its current opacity.
            let alpha = window.alpha();
            if alpha == 0 {
                continue;
            }
            for (offset, opacity) in bands {
                let band = Area::new(frame.x + offset, frame.y + offset, frame.width, frame.height);
                let part = band.intersect(area).intersect(above_taskbar);
                let opacity = (u32::from(opacity) * u32::from(alpha) / 255) as u8;
                if !part.is_empty() && opacity != 0 {
                    painter.blend(part, SHADOW, opacity);
                }
            }
            let title = window.title_area().intersect(area);
            if !title.is_empty() {
                let active = Some(slot) == focused;
                if alpha == u8::MAX {
                    painter.draw(title, |canvas| Self::render_title(window, active, canvas, self.three_d));
                } else {
                    painter.draw_blended(title, alpha, |canvas| Self::render_title(window, active, canvas, self.three_d));
                }
            }
            let body = window.body().intersect(area);
            if !body.is_empty() {
                let region = body.relative_to(window.x, window.y);
                let result = if alpha == u8::MAX {
                    libwoven::compositor_blit_region_shared(window.token, region, body.x, body.y)
                } else {
                    libwoven::compositor_blend_region_shared(window.token, region, body.x, body.y, alpha)
                };
                if result.is_err() {
                    log(&[b"woven-shell: shared surface compose failed: ", &window.title[..window.title_length]]);
                }
            }
        }

        if let Some(toast) = &self.toast {
            let part = self.toast_area().intersect(area);
            if !part.is_empty() {
                painter.blend(part, TOAST_FILL, TOAST_OPACITY);
                painter.overlay(part, |canvas| self.render_toast(toast, canvas));
            }
        }

        let panel = self.panel_area().intersect(area);
        if !panel.is_empty() {
            painter.blend(panel, PANEL_FILL, PANEL_OPACITY);
            painter.overlay(panel, |canvas| self.render_panel(canvas));
        }

        let qs = self.qs_area().intersect(area);
        if !qs.is_empty() {
            painter.blend(qs, PANEL_FILL, PANEL_OPACITY);
            painter.overlay(qs, |canvas| self.render_qs(canvas));
        }

        let center = self.center_area().intersect(area);
        if !center.is_empty() {
            painter.blend(center, PANEL_FILL, PANEL_OPACITY);
            painter.overlay(center, |canvas| self.render_center(canvas));
        }

        let cursor = self.cursor_area().intersect(area);
        if !cursor.is_empty() {
            painter.overlay(cursor, |canvas| self.render_cursor(canvas));
        }
    }

    fn raise(&mut self, slot: usize) {
        let Some(position) = self.stack[..self.stacked].iter().position(|&s| s == slot) else {
            return;
        };
        self.stack[position..self.stacked].rotate_left(1);
    }

    /// Focus window `slot`, raising it. Returns the area to repaint.
    fn focus(&mut self, slot: usize) -> Area {
        if self.focused() == Some(slot) {
            return Area::new(0, 0, 0, 0);
        }
        let previous = self.focused().and_then(|old| self.windows[old]).map(|w| w.footprint());
        self.raise(slot);
        let current = self.windows[slot].map(|w| w.footprint()).unwrap_or(Area::new(0, 0, 0, 0));
        current.union(previous.unwrap_or(Area::new(0, 0, 0, 0))).union(self.taskbar())
    }

    /// Open a window for `owner`. Returns the areas to repaint.
    fn open(&mut self, owner: process::Id, message: &[u8]) -> Dirty {
        if message.len() < 16 {
            return Dirty::new();
        }
        let token = u64::from_le_bytes(message[4..12].try_into().unwrap_or([0; 8]));
        let width = i32::from(u16::from_le_bytes([message[12], message[13]]));
        let height = i32::from(u16::from_le_bytes([message[14], message[15]]));
        if libwoven::surface_owner(token) != owner
            || !(1..=self.width).contains(&width)
            || !(1..=self.taskbar_top() - TITLE_HEIGHT).contains(&height)
        {
            log(&[b"woven-shell: window open rejected"]);
            return Dirty::new();
        }
        let Some(slot) = self.windows.iter().position(Option::is_none) else {
            return Dirty::new();
        };
        let offset = CASCADE_ORIGIN + (self.opened % 6) * CASCADE_STEP;
        self.opened += 1;
        // Keep the whole frame above the taskbar and on screen.
        let x = offset.min((self.width - width).max(0));
        let y = offset.min((self.taskbar_top() - height).max(TITLE_HEIGHT));
        let mut title = [0u8; MAX_TITLE];
        let title_length = (message.len() - 16).min(MAX_TITLE);
        title[..title_length].copy_from_slice(&message[16..16 + title_length]);
        // Reduced motion: appear at full opacity immediately rather than
        // animate in, matching libwoven::accessibility::Settings's own
        // "duration 0" convention for this preference.
        let mut fade = Spring::new(if self.reduced_motion { i32::from(u8::MAX) } else { 0 });
        fade.set_target(i32::from(u8::MAX));
        let window = Window {
            owner,
            token,
            x,
            y,
            width,
            height,
            title,
            title_length,
            fade,
            workspace: self.workspace,
            resizing_until: 0,
        };
        self.windows[slot] = Some(window);
        let previous = self.focused().and_then(|old| self.windows[old]).map(|w| w.footprint());
        self.stack[self.stacked] = slot;
        self.stacked += 1;
        log(&[b"woven-shell: window opened: ", &title[..title_length]]);
        // Apart: the new window, the one it unfocused, the taskbar and any
        // retiled windows, rather than one box over most of the screen.
        let mut dirty = Dirty::of(window.footprint());
        dirty.add(previous.unwrap_or(Area::new(0, 0, 0, 0)));
        dirty.add(self.taskbar());
        dirty.add(self.retile());
        // Opening a client changes the composition order across the newly
        // focused surface. Keep the first frame conservative so an exposed
        // tile cannot retain pixels from the pre-open desktop.
        dirty.add(self.screen());
        dirty
    }

    /// Where windows may go: the screen above the taskbar.
    fn work_area(&self) -> Frame {
        Frame::new(0, 0, self.width, self.taskbar_top())
    }

    /// Put window `slot` in `frame`, title bar included: move it now, and if
    /// its client area changes size, ask the owner for a surface of the new
    /// size. Returns the area to repaint.
    fn place(&mut self, slot: usize, frame: Frame) -> Area {
        let Some(window) = self.windows[slot].as_mut() else {
            return Area::new(0, 0, 0, 0);
        };
        let before = window.footprint();
        let client = frame.client(TITLE_HEIGHT);
        window.x = client.x;
        window.y = client.y;
        if (client.width, client.height) != (window.width, window.height) {
            let mut request = [0u8; 8];
            request[..4].copy_from_slice(b"WSIZ");
            request[4..6].copy_from_slice(&(client.width as u16).to_le_bytes());
            request[6..].copy_from_slice(&(client.height as u16).to_le_bytes());
            window.resizing_until = libwoven::ticks() + RESIZE_GRACE;
            let _ = libwoven::message_send(window.owner, &request);
        }
        before.union(window.footprint())
    }

    /// Lay every window out again, if tiling is on. Returns the area to
    /// repaint.
    fn retile(&mut self) -> Area {
        let mut dirty = Area::new(0, 0, 0, 0);
        if !self.tiled {
            return dirty;
        }
        let mut slots = [0usize; MAX_WINDOWS];
        let mut count = 0;
        for (slot, _) in self.entries() {
            slots[count] = slot;
            count += 1;
        }
        let (frames, placed) = layout::tile(count, self.work_area());
        for index in 0..placed {
            dirty = dirty.union(self.place(slots[index], frames[index]));
        }
        dirty
    }

    /// `WRSZ`: the owner replaced a window's surface with one of a new size.
    /// Returns the area to repaint.
    fn resized(&mut self, owner: process::Id, message: &[u8]) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        if message.len() != 16 {
            return empty;
        }
        let token = u64::from_le_bytes(message[4..12].try_into().unwrap_or([0; 8]));
        let width = i32::from(u16::from_le_bytes([message[12], message[13]]));
        let height = i32::from(u16::from_le_bytes([message[14], message[15]]));
        let top = self.taskbar_top();
        if libwoven::surface_owner(token) != owner
            || !(1..=self.width).contains(&width)
            || !(1..=top - TITLE_HEIGHT).contains(&height)
        {
            log(&[b"woven-shell: WRSZ rejected: token or geometry"]);
            return empty;
        }
        let Some(window) = self.windows.iter_mut().flatten().find(|window| window.owner == owner) else {
            log(&[b"woven-shell: WRSZ rejected: owner window missing"]);
            return empty;
        };
        let before = window.footprint();
        window.token = token;
        window.width = width;
        window.height = height;
        window.resizing_until = 0;
        log(&[b"woven-shell: window resized: ", &window.title[..window.title_length]]);
        before.union(window.footprint())
    }

    /// Remove window `slot`. Returns the area to repaint.
    fn close(&mut self, slot: usize) -> Area {
        let Some(window) = self.windows[slot].take() else {
            return Area::new(0, 0, 0, 0);
        };
        if window.workspace == self.workspace {
            if let Some(position) = self.stack[..self.stacked].iter().position(|&s| s == slot) {
                self.stack[position..self.stacked].rotate_left(1);
                self.stacked -= 1;
            }
        } else {
            let (stack, count) = &mut self.saved[window.workspace];
            if let Some(position) = stack[..*count].iter().position(|&s| s == slot) {
                stack[position..*count].rotate_left(1);
                *count -= 1;
            }
        }
        if self.drag.is_some_and(|(dragged, _, _)| dragged == slot) {
            self.drag = None;
        }
        log(&[b"woven-shell: window closed: ", &window.title[..window.title_length]]);
        // The newly focused window's title and shadow change too.
        let next = self.focused().and_then(|top| self.windows[top]).map(|w| w.footprint());
        window
            .footprint()
            .union(next.unwrap_or(Area::new(0, 0, 0, 0)))
            .union(self.taskbar())
            .union(self.retile())
    }

    fn handle_message(&mut self, sender: process::Id, message: &[u8]) -> Dirty {
        let empty = Dirty::new();
        match message.get(..4) {
            Some(b"WOPN") => self.open(sender, message),
            Some(b"WRSZ") => Dirty::of(self.resized(sender, message)),
            Some(b"TRAY") if message.len() == 12 => {
                let token = u64::from_le_bytes(message[4..12].try_into().unwrap_or([0; 8]));
                if libwoven::surface_owner(token) != sender {
                    return empty;
                }
                let Some(index) = self.tray.iter().position(Option::is_none) else {
                    return empty;
                };
                self.tray[index] = Some(TrayIcon { token });
                log(&[b"woven-shell: tray icon added"]);
                Dirty::of(self.tray_area(index))
            }
            Some(b"NOTE") => {
                let mut text = [0u8; 32];
                let length = label(&message[4..], TOAST_CHARS, &mut text);
                log(&[b"woven-shell: notification: ", &text[..length]]);
                let remembered = self.remember(sender, &message[4..]);
                // Do Not Disturb, or the lock screen, keeps the notification
                // but raises no toast.
                if self.dnd || self.locked {
                    return Dirty::of(remembered);
                }
                self.toast = Some(Toast {
                    text,
                    length,
                    until: libwoven::ticks() + TOAST_TICKS,
                });
                let mut dirty = Dirty::of(self.toast_area());
                dirty.add(remembered);
                dirty
            }
            _ => empty,
        }
    }

    fn handle_input(&mut self, event: InputEvent) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        // Locked, keys go to the PIN prompt and nowhere else; the pointer
        // still moves, unseen, so it is where it was left once unlocked.
        if self.locked {
            return match event {
                InputEvent::Key(key) => self.lock_key(key),
                InputEvent::PointerMove { dx, dy } => {
                    self.pointer = (
                        (self.pointer.0 + i32::from(dx)).clamp(0, self.desktop_width - 1),
                        (self.pointer.1 + i32::from(dy)).clamp(0, self.height - 1),
                    );
                    empty
                }
                _ => empty,
            };
        }
        match event {
            InputEvent::PointerMove { dx, dy } => {
                let before = self.cursor_area();
                self.pointer = (
                    (self.pointer.0 + i32::from(dx)).clamp(0, self.desktop_width - 1),
                    (self.pointer.1 + i32::from(dy)).clamp(0, self.height - 1),
                );
                self.pointer_visible = true;
                let moved = self.drag_to_pointer();
                if self.drag.is_none() {
                    self.forward_pointer_move();
                }
                before.union(self.cursor_area()).union(moved)
            }
            InputEvent::PointerButton { button: 0, pressed: false } => {
                if let Some(slot) = self.pointer_capture.take() {
                    self.forward_pointer_button(slot, false);
                }
                // Dropped on the left or right edge: snap to that half.
                let Some((slot, _, _)) = self.drag.take() else { return empty };
                let (x, _) = self.pointer;
                // The desktop's outer edges: the left snaps on the primary,
                // the right on the last monitor (monitors are the same size).
                if x <= 0 || x >= self.desktop_width - 1 {
                    let mut half = layout::half(self.work_area(), x > 0);
                    if x > 0 {
                        half.x += self.desktop_width - self.width;
                    }
                    log(&[b"woven-shell: window snapped"]);
                    return self.place(slot, half);
                }
                empty
            }
            InputEvent::PointerButton { button: 0, pressed: true } => {
                let (x, y) = self.pointer;
                if let Some(index) = (0..WORKSPACES).find(|&index| self.workspace_area(index).contains(x, y)) {
                    return self.switch_workspace(index);
                }
                if self.start_area().contains(x, y) {
                    return self.toggle_launcher();
                }
                // The Quick Settings button, and its panel's rows.
                if self.qs_button().contains(x, y) {
                    return self.toggle_qs();
                }
                if self.qs_open {
                    let panel = self.qs_area();
                    if !panel.contains(x, y) {
                        return self.close_panels();
                    }
                    return match (y - panel.y - 1) / PANEL_ROW - 1 {
                        0 => self.power(false),
                        1 => self.power(true),
                        2 => self.toggle_scale(),
                        3 => {
                            self.dnd = !self.dnd;
                            log(if self.dnd { &[b"woven-shell: do not disturb on"] } else { &[b"woven-shell: do not disturb off"] });
                            panel
                        }
                        4 => self.toggle_tiling(),
                        5 => self.lock(),
                        6 => self.start_change_pin(),
                        7 => {
                            self.language = match self.language {
                                libwoven::i18n::Language::English => libwoven::i18n::Language::Chichewa,
                                libwoven::i18n::Language::Chichewa => libwoven::i18n::Language::English,
                            };
                            log(&[b"woven-shell: language toggled"]);
                            panel
                        }
                        8 => {
                            self.three_d = !self.three_d;
                            log(if self.three_d { &[b"woven-shell: 3D desktop on"] } else { &[b"woven-shell: 3D desktop off"] });
                            // Every open window's title bar also renders
                            // differently in 3D mode, not just this panel.
                            panel.union(self.screen())
                        }
                        9 => {
                            self.reduced_motion = !self.reduced_motion;
                            log(if self.reduced_motion { &[b"woven-shell: reduced motion on"] } else { &[b"woven-shell: reduced motion off"] });
                            panel
                        }
                        _ => empty,
                    };
                }
                // The clock opens and closes the Notification Center.
                if self.clock_area().contains(x, y) || (self.unread > 0 && self.badge_area().contains(x, y)) {
                    return self.toggle_center();
                }
                if self.center_open {
                    let center = self.center_area();
                    if !center.contains(x, y) {
                        return self.toggle_center();
                    }
                    // Clear, at the right of the title row.
                    if y < center.y + 1 + PANEL_ROW && x >= center.x + center.width - 40 {
                        return self.clear_notes();
                    }
                    return empty;
                }
                if self.launcher_open {
                    // A click on an application launches it; anywhere else
                    // outside the panel closes the launcher.
                    let panel = self.panel_area();
                    if !panel.contains(x, y) {
                        return self.toggle_launcher();
                    }
                    let row = (y - panel.y - 1) / PANEL_ROW - 2;
                    let (found, count) = self.filtered();
                    if row >= 0 && (row as usize) < count {
                        return self.launch(found[row as usize]);
                    }
                    return empty;
                }
                // A taskbar entry focuses its window.
                let entry = self
                    .entries()
                    .enumerate()
                    .find(|(index, _)| self.entry_area(*index).contains(x, y))
                    .map(|(_, (slot, _))| slot);
                if let Some(slot) = entry {
                    return self.focus(slot);
                }
                // Otherwise the topmost window under the pointer: its close
                // control asks the owner to close; anywhere else focuses it,
                // and its title bar also starts a drag.
            let mut pointer_x = [0u8; 20];
            let mut pointer_y = [0u8; 20];
            let pointer_x = libwoven::format_u64(self.pointer.0.max(0) as u64, &mut pointer_x);
            let pointer_y = libwoven::format_u64(self.pointer.1.max(0) as u64, &mut pointer_y);
            log(&[b"woven-shell: pointer press x=", pointer_x, b" y=", pointer_y]);
                let hit = self.stack[..self.stacked].iter().rev().copied().find(|&slot| {
                    self.windows[slot].is_some_and(|window| window.frame().contains(x, y))
                });
                let Some(slot) = hit else { return empty };
                let Some(window) = self.windows[slot] else { return empty };
                if window.close_area().contains(x, y) {
                    let _ = libwoven::message_send(window.owner, b"WCLS");
                    log(&[b"woven-shell: close requested: ", &window.title[..window.title_length]]);
                    return empty;
                }
                if window.title_area().contains(x, y) {
                    self.drag = Some((slot, x - window.x, y - window.y));
                } else if window.body().contains(x, y) {
                    self.pointer_capture = Some(slot);
                    self.forward_pointer_button(slot, true);
                }
                self.focus(slot)
            }
            InputEvent::Key(key) if self.launcher_open => self.launcher_key(key),
            InputEvent::Key(Key::F1) => self.toggle_launcher(),
            InputEvent::Key(Key::F2) => self.toggle_tiling(),
            InputEvent::Key(Key::F3) => self.switch_workspace((self.workspace + 1) % WORKSPACES),
            InputEvent::Key(Key::F4) => self.move_focused_to_next_workspace(),
            InputEvent::Key(Key::Enter) => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ = libwoven::message_send(window.owner, &[b'W', b'K', b'E', b'Y', KEY_ENTER]);
                }
                empty
            }
            InputEvent::Key(Key::Backspace) => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ = libwoven::message_send(window.owner, &[b'W', b'K', b'E', b'Y', KEY_BACKSPACE]);
                }
                empty
            }
            InputEvent::Key(Key::Char(character)) if character.is_ascii() => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ = libwoven::message_send(window.owner, &[character as u8]);
                    log(&[b"woven-shell: key forwarded"]);
                }
                empty
            }
            _ => empty,
        }
    }

    /// Forward a pointer move to the topmost client under the pointer. Shell
    /// chrome, title bars and window drags remain shell-owned.
    fn forward_pointer_move(&self) {
        let (x, y) = self.pointer;
        let Some(slot) = self.stack[..self.stacked].iter().rev().copied().find(|&slot| {
            self.windows[slot].is_some_and(|window| window.body().contains(x, y))
        }) else {
            return;
        };
        let Some(window) = self.windows[slot] else { return };
        let local_x = (x - window.x).clamp(0, window.width.saturating_sub(1)) as u16;
        let local_y = (y - window.y).clamp(0, window.height.saturating_sub(1)) as u16;
        let mut message = [0u8; 8];
        message[..4].copy_from_slice(b"WPMV");
        message[4..6].copy_from_slice(&local_x.to_le_bytes());
        message[6..8].copy_from_slice(&local_y.to_le_bytes());
        let _ = libwoven::message_send(window.owner, &message);
    }

    /// Forward a bounded primary-button edge to a client that owns the
    /// pointer capture. Coordinates are clamped to the shared surface.
    fn forward_pointer_button(&self, slot: usize, pressed: bool) {
        let (x, y) = self.pointer;
        let Some(window) = self.windows[slot] else { return };
        let local_x = (x - window.x).clamp(0, window.width.saturating_sub(1)) as u16;
        let local_y = (y - window.y).clamp(0, window.height.saturating_sub(1)) as u16;
        let mut message = [0u8; 10];
        message[..4].copy_from_slice(b"WPBT");
        message[4] = 0;
        message[5] = u8::from(pressed);
        message[6..8].copy_from_slice(&local_x.to_le_bytes());
        message[8..10].copy_from_slice(&local_y.to_le_bytes());
        let _ = libwoven::message_send(window.owner, &message);
    }

    /// Move the dragged window under the pointer, keeping its frame on screen
    /// and above the taskbar. Returns the area to repaint.
    fn drag_to_pointer(&mut self) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        let Some((slot, grab_x, grab_y)) = self.drag else { return empty };
        let Some(window) = self.windows[slot].as_mut() else { return empty };
        let before = window.footprint();
        let top = self.height - TASKBAR_HEIGHT;
        window.x = (self.pointer.0 - grab_x).clamp(0, (self.desktop_width - window.width).max(0));
        window.y = (self.pointer.1 - grab_y).clamp(TITLE_HEIGHT, (top - window.height).max(TITLE_HEIGHT));
        let after = window.footprint();
        if after == before { empty } else { before.union(after) }
    }

    /// Composite client damage, and drop windows and icons whose surface is
    /// gone. Returns the area to repaint.
    fn check_clients(&mut self) -> Area {
        let mut dirty = Area::new(0, 0, 0, 0);
        for slot in 0..MAX_WINDOWS {
            let Some(window) = self.windows[slot] else { continue };
            // A damage blit may draw over windows stacked above; repainting
            // the region afterwards, before presenting, puts them back.
            match libwoven::compositor_blit_damage(window.token, window.x, window.y) {
                Ok(Some(region)) => {
                    let changed = Area::new(
                        window.x + i32::from(region.x),
                        window.y + i32::from(region.y),
                        i32::from(region.width),
                        i32::from(region.height),
                    );
                    // Recompose the complete client body after a client-side
                    // update. The damage blit has already copied the changed
                    // pixels into the compositor, but a full body repaint
                    // restores stacking order and guarantees the next
                    // present cannot retain a stale tile behind overlays.
                    dirty = dirty.union(changed).union(window.body());
                }
                Ok(None) => {}
                // While a resize is outstanding the owner may have released
                // the old surface before its new one arrives.
                Err(_) if libwoven::ticks() < window.resizing_until => {}
                Err(_) => {
                    // A released client surface can leave stale pixels in a
                    // partially damaged scanout. Recompose the full screen
                    // after removing the window so its wallpaper/taskbar
                    // and all remaining layers are presented atomically.
                    dirty = dirty.union(self.close(slot)).union(self.screen());
                }
            }
        }
        for index in 0..MAX_TRAY {
            let Some(icon) = self.tray[index] else { continue };
            let place = self.tray_area(index);
            match libwoven::compositor_blit_damage(icon.token, place.x, place.y) {
                Ok(Some(_)) => dirty = dirty.union(place),
                Ok(None) => {}
                Err(_) => {
                    self.tray[index] = None;
                    log(&[b"woven-shell: tray icon removed"]);
                    dirty = dirty.union(place);
                }
            }
        }
        dirty
    }

    /// Whether any window is still fading.
    fn animating(&self) -> bool {
        self.windows.iter().flatten().any(|window| !window.fade.is_settled())
    }

    /// Advance every animation by `frames`. Returns the area to repaint.
    fn animate(&mut self, frames: u64) -> Area {
        let mut dirty = Area::new(0, 0, 0, 0);
        for window in self.windows.iter_mut().flatten() {
            if !window.fade.is_settled() {
                window.fade.step(frames);
                dirty = dirty.union(window.footprint());
            }
        }
        dirty
    }

    /// Advance the clock and the toast timer, and reap exited
    /// applications. Returns the area to repaint.
    fn tick(&mut self) -> Area {
        self.reap();
        let mut dirty = Area::new(0, 0, 0, 0);
        let time = libwoven::clock().ok();
        if clock_text(time, self.seconds) != clock_text(self.time, self.seconds) {
            self.time = time;
            // The lock screen's clock is in the middle of the screen.
            dirty = dirty.union(if self.locked { self.screen() } else { self.clock_area() });
        }
        if self.toast.as_ref().is_some_and(|toast| libwoven::ticks() >= toast.until) {
            self.toast = None;
            dirty = dirty.union(self.toast_area());
        }
        dirty
    }
}

/// Read up to [`MAX_ACCOUNTS`] accounts from an accounts file into
/// `accounts`, which starts empty; malformed lines are skipped.
fn load_accounts(path: &str, accounts: &mut [Option<libwoven::accounts::Account>; MAX_ACCOUNTS]) {
    let Ok(descriptor) = libwoven::open(path) else { return };
    let mut text = [0u8; 1024];
    let read = libwoven::read(descriptor, &mut text);
    let _ = libwoven::close(descriptor);
    let Ok(length) = read else { return };
    let mut count = 0;
    for line in text[..length].split(|byte| *byte == b'\n') {
        if count == MAX_ACCOUNTS {
            break;
        }
        if let Some(account) = libwoven::accounts::Account::parse(line) {
            accounts[count] = Some(account);
            count += 1;
        }
    }
}

fn main(args: Args) -> ! {
    // SAFETY: the only reference to the desktop's state, taken once.
    let desktop = unsafe { &mut *DESKTOP.0.get() };
    let mut index = 1;
    while let Some(argument) = args.get(index) {
        match argument {
            b"--seconds" => desktop.seconds = true,
            // The Stage 20.5 contract isolates the flat compositor's window
            // depth/shadow behavior. Production remains 3D by default; this
            // explicit switch keeps that acceptance independent of the later
            // 3D presentation stages.
            b"--no-3d" => desktop.three_d = false,
            b"--pin" => {
                index += 1;
                match args.get(index) {
                    Some(digits)
                        if (1..=MAX_PIN).contains(&digits.len()) && digits.iter().all(u8::is_ascii_digit) =>
                    {
                        let mut stored = [0u8; MAX_PIN];
                        stored[..digits.len()].copy_from_slice(digits);
                        desktop.pin = Some((stored, digits.len()));
                    }
                    _ => fail(1),
                }
            }
            b"--accounts" => {
                index += 1;
                let path = args.get(index).and_then(|path| core::str::from_utf8(path).ok());
                desktop.accounts = [None; MAX_ACCOUNTS];
                if let Some(path) = path {
                    load_accounts(path, &mut desktop.accounts);
                }
                // Asked to sign users in but none can be read: refuse to start
                // rather than open the desktop to anyone.
                if desktop.accounts.iter().all(Option::is_none) {
                    fail(9);
                }
            }
            _ => fail(1),
        }
        index += 1;
    }
    // SAFETY: the only reference to the tile buffer, taken once.
    let tile = unsafe { &mut *TILE.0.get() };

    if libwoven::compositor_claim().is_err() {
        fail(2);
    }
    let mut info = match libwoven::compositor_info() {
        Ok(info) => info,
        Err(_) => fail(3),
    };
    if info.width >= 1280 && info.height >= 800 {
        info = match libwoven::compositor_set_scale(2) {
            Ok(info) => info,
            Err(_) => fail(4),
        };
    }
    let staging = match GraphicsSurface::create(TILE_WIDTH as u32, TILE_HEIGHT as u32) {
        Ok(surface) => surface,
        Err(_) => fail(5),
    };
    let mut painter = Painter { staging, tile };

    let (width, height) = (info.width as i32, info.height as i32);
    // The first monitor is the primary; the desktop spans them all.
    let (outputs, monitors) = libwoven::compositor_outputs().unwrap_or(([libwoven::Output::default(); 4], 0));
    desktop.width = if monitors > 0 { (outputs[0].width as i32).clamp(1, width) } else { width };
    desktop.desktop_width = width;
    if monitors > 1 {
        let count = [b'0' + monitors as u8];
        log(&[b"woven-shell: monitors ", &count]);
    }
    desktop.height = height;
    desktop.time = libwoven::clock().ok();
    desktop.scale = info.scale;
    // With accounts, the desktop starts at the sign-in screen.
    desktop.locked = desktop.accounts.iter().any(Option::is_some);
    desktop.pointer = (width / 2, height / 2);
    desktop.app_count = load_catalog(&mut desktop.apps);
    desktop.repaint(&mut painter, desktop.screen());
    // The first present takes the display from the text terminal.
    if libwoven::compositor_present().is_err() {
        fail(6);
    }
    let mut pid = [0u8; 20];
    let pid = libwoven::format_u64(libwoven::current_pid().0, &mut pid);
    log(&[b"woven-shell: desktop ready pid=", pid]);

    let mut clock = libwoven::FrameClock::new(FRAME_PERIOD);
    // Applications reach the shell over IPC. Without the Ipc capability the
    // kernel refuses a wait on messages; the desktop then runs without
    // applications rather than not at all.
    let mut sources = wait::INPUT | wait::MESSAGE | wait::DAMAGE;
    // Large enough for any message: one that did not fit would stay queued
    // and keep the mailbox ready forever.
    let mut message = [0u8; 256];
    // Whether the clock is running at the animation rate.
    let mut fast = false;
    loop {
        // Switch frame rate as animations start and finish.
        if desktop.animating() != fast {
            fast = !fast;
            clock = libwoven::FrameClock::new(if fast { ANIMATION_PERIOD } else { FRAME_PERIOD });
        }
        let mut dirty = Dirty::new();
        match clock.wait(sources) {
            Ok(FrameEvent::Frame { skipped }) => {
                dirty.add(desktop.animate(skipped + 1));
                dirty.add(desktop.tick());
            }
            Ok(FrameEvent::Ready(_)) => {}
            Err(_) if sources & wait::MESSAGE != 0 => {
                sources &= !wait::MESSAGE;
                let _ = libwoven::print("woven-shell: no Ipc capability; applications disabled\n");
                continue;
            }
            Err(_) => fail(8),
        }
        while let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            log(&[b"woven-shell: application message received"]);
            for &area in desktop.handle_message(sender, &message[..length]).areas() {
                dirty.add(area);
            }
        }
        while let Ok(Some(event)) = libwoven::input_poll_event() {
            let area = desktop.handle_input(event);
            if !area.is_empty() {
                // Repainted per event, so pointer motion over several
                // events never leaves a trail.
                desktop.repaint(&mut painter, area);
            }
        }
        dirty.add(desktop.check_clients());
        for &area in dirty.areas() {
            desktop.repaint(&mut painter, area);
        }
        if libwoven::compositor_present().is_err() {
            fail(7);
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
