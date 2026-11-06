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
//! - `WCAP`, version (u8 = 1): opt the sender's open window into cancellation.
//! - `WCAN`: cancel an opted-in client's pointer actions before a synthetic
//!   `WPBT` release when locking or leaving a workspace. Legacy clients get
//!   only the established release message.
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
use core::sync::atomic::Ordering;

use libwoven::graphics::{
    Camera, Color, DepthBuffer, Rect, Renderer, TrueTypeFace, Vertex3, Vertex3Field, Vertex3Tex,
};
use libwoven::layout::{self, Frame};
use libwoven::{
    blend, font, process, rgb, wait, Args, Canvas, DateTime, FrameEvent, GraphicsSurface,
    InputEvent, Key, Spring,
};

libwoven::entry!(main);

mod tokens;
use tokens::*;
mod geometry;
use geometry::{Area, Dirty};
mod textures;
use textures::{CENTER_TEXTURE, QS_PANEL_TEXTURE, TILE, TILE_DEPTH_3D, TOAST_TEXTURE};

/// The first desktop release keeps a small, curated set of shortcuts visible
/// so the desktop feels useful before the launcher is opened. The catalog
/// remains the source of truth: these names are resolved against it at click
/// time, so a missing package simply leaves the shortcut inert.
const DESKTOP_ICON_NAMES: [&str; 3] = ["WovenFiles", "WovenTerminal", "WovenStore"];
const DESKTOP_ICON_LABELS: [&str; 3] = ["Files", "Terminal", "Store"];
const DESKTOP_ICON_X: i32 = 24;
const DESKTOP_ICON_Y: i32 = 52;
const DESKTOP_ICON_STEP: i32 = 76;
const DESKTOP_ICON_SIZE: i32 = 32;

mod icons;
use icons::{app_icon, draw_app_icon};

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
    let _ = libwoven::write(
        libwoven::syscall::STDOUT,
        libwoven::format_u64(code, &mut digits),
    );
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
    let base = if high_contrast() {
        0
    } else {
        blend(WALLPAPER_BOTTOM, WALLPAPER_TOP, t)
    };
    let band = ((x + y) / WEAVE_PITCH) % 2 == 0;
    let tint = if band { WEAVE } else { WEAVE_SHADE };
    blend(tint, base, WEAVE_TINT_OPACITY)
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
fn fill_shell_catalog(catalog: &mut libwoven::i18n::Catalog, language: libwoven::i18n::Language) {
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
            // M5: the Overview taskbar and launcher overview
            // (new strings since M2/M3, not part of the already-accepted
            // lock-screen gates above).
            ("start", "Start"),
            ("type_to_search", "Type to search"),
            ("no_open_windows", "No open windows on this workspace"),
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
            ("start", "Yambani"),
            ("type_to_search", "Lembani kufunafuna"),
            ("no_open_windows", "Palibe mawindo otseguka pa gawo ili"),
        ],
        Language::Arabic => &[
            ("sign_in", "تسجيل الدخول"),
            ("locked", "مقفل"),
            ("change_pin", "تغيير الرقم السري"),
            ("current_pin", "الرقم السري الحالي"),
            ("new_pin", "الرقم السري الجديد"),
            ("confirm_new_pin", "تأكيد الرقم السري الجديد"),
            ("enter_pin", "أدخل الرقم السري"),
            ("wrong_pin", "الرقم السري غير صحيح"),
            ("try_again_shortly", "حاول مرة أخرى قريباً"),
            ("start", "ابدأ"),
            ("type_to_search", "اكتب للبحث"),
            ("no_open_windows", "لا توجد نوافذ مفتوحة في مساحة العمل"),
        ],
    };
    for (key, value) in entries {
        let _ = catalog.insert(key, value);
    }
}

/// Both shell catalogs (lock screen, taskbar, launcher overview),
/// const-initialized to `Catalog::new()` (an all-zero value LLVM places in
/// `.bss`, not `.data`) and filled in place on first use, so no by-value
/// `Catalog` is ever constructed on the stack.
struct ShellCatalogs {
    english: UnsafeCell<libwoven::i18n::Catalog>,
    chichewa: UnsafeCell<libwoven::i18n::Catalog>,
    arabic: UnsafeCell<libwoven::i18n::Catalog>,
    built: UnsafeCell<bool>,
}

// SAFETY: the shell is single-threaded, and `shell_catalog` takes the only reference.
unsafe impl Sync for ShellCatalogs {}

static SHELL_CATALOGS: ShellCatalogs = ShellCatalogs {
    english: UnsafeCell::new(libwoven::i18n::Catalog::new()),
    chichewa: UnsafeCell::new(libwoven::i18n::Catalog::new()),
    arabic: UnsafeCell::new(libwoven::i18n::Catalog::new()),
    built: UnsafeCell::new(false),
};

/// The shell's translatable strings for `language` -- the lock screen,
/// and, since M5, the taskbar and launcher overview -- built
/// once (lazily, in place) through `libwoven::i18n::Catalog` (Stage
/// 26.5). English values are byte-for-byte the literals every
/// already-accepted lock-screen gate expects, so the default language
/// changes nothing; Chichewa is a best-effort demo translation, not a
/// reviewed localization.
fn shell_catalog(language: libwoven::i18n::Language) -> &'static libwoven::i18n::Catalog {
    use libwoven::i18n::Language;
    // SAFETY: single-threaded; build-once guarded by `built`, and every
    // reference handed out is immutable.
    unsafe {
        if !*SHELL_CATALOGS.built.get() {
            fill_shell_catalog(&mut *SHELL_CATALOGS.english.get(), Language::English);
            fill_shell_catalog(&mut *SHELL_CATALOGS.chichewa.get(), Language::Chichewa);
            fill_shell_catalog(&mut *SHELL_CATALOGS.arabic.get(), Language::Arabic);
            *SHELL_CATALOGS.built.get() = true;
        }
        match language {
            Language::English => &*SHELL_CATALOGS.english.get(),
            Language::Chichewa => &*SHELL_CATALOGS.chichewa.get(),
            Language::Arabic => &*SHELL_CATALOGS.arabic.get(),
        }
    }
}

/// `bytes` as text, cut to `limit` characters; anything not printable ASCII
/// is shown as `?`, so the font's replacement box never appears.
fn label(bytes: &[u8], limit: usize, buffer: &mut [u8; 32]) -> usize {
    let length = bytes.len().min(limit).min(buffer.len());
    for (slot, byte) in buffer.iter_mut().zip(&bytes[..length]) {
        *slot = if (b' '..=b'~').contains(byte) {
            *byte
        } else {
            b'?'
        };
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

/// Corner radius shared by window chrome, the taskbar, the Start button, and
/// toasts, so the desktop reads as one consistent style rather than a
/// patchwork of ad hoc roundings.
const CHROME_RADIUS: usize = 6;
/// A smaller radius for compact controls (the window close button) where the
/// full `CHROME_RADIUS` would look disproportionate.
const CHROME_RADIUS_SMALL: usize = 3;
/// Corner radius for the larger sign-in/lock-screen card.
const CHROME_CARD_RADIUS: usize = 14;
/// Diameter of the sign-in screen's account avatar circle.
const AVATAR_DIAMETER: i32 = 48;

fn rect_of(area: Area) -> Rect {
    Rect {
        x: area.x,
        y: area.y,
        width: area.width.max(0) as usize,
        height: area.height.max(0) as usize,
    }
}

fn color_of(packed: u32) -> Color {
    Color::rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

/// A filled rounded rectangle in `area`, replacing a flat `fill_rect` call
/// with WovenGraphics's anti-aliased corner blending.
fn fill_rounded(canvas: &mut Canvas<'_>, area: Area, radius: usize, color: u32) {
    Renderer::new(canvas).rounded_rect(rect_of(area), radius, color_of(color));
}

/// A filled rounded rectangle with explicit opacity, for translucent chrome
/// (the sign-in/lock card) rather than a fully opaque fill.
fn fill_rounded_blended(canvas: &mut Canvas<'_>, area: Area, radius: usize, color: u32, opacity: u8) {
    Renderer::new(canvas).rounded_rect_blended(rect_of(area), radius, color_of(color), opacity);
}

/// A filled circle of `diameter` centred at `(center_x, center_y)`. A circle
/// is a square rounded rectangle whose radius is half its side, so this
/// reuses `rounded_rect` rather than adding a second corner-distance
/// implementation.
fn fill_circle(canvas: &mut Canvas<'_>, center_x: i32, center_y: i32, diameter: i32, color: u32) {
    let area = Area::new(center_x - diameter / 2, center_y - diameter / 2, diameter, diameter);
    fill_rounded(canvas, area, (diameter / 2).max(0) as usize, color);
}

/// Borrow the staged system face through its read-only lazy file mapping.
/// Parsing the bounded table directory is cheap; glyph outlines remain
/// demand-paged from the shared mapping and never inflate this ELF's `.bss`.
fn system_font_face() -> Option<TrueTypeFace<'static>> {
    libwoven::graphics::load_system_font_mapping()
        .ok()
        .and_then(|bytes| TrueTypeFace::from_bytes(bytes).ok())
}

/// `color`, lightened toward white by `amount` (0..=255) -- used to derive a
/// gradient's lighter top stop from a single existing theme colour, so theme
/// swaps (e.g. high-contrast mode) only need to change one value.
fn lighten(color: u32, amount: u8) -> u32 {
    let mix = |channel: u8| {
        let headroom = 255 - channel;
        channel + ((u16::from(headroom) * u16::from(amount)) / 255) as u8
    };
    rgb(
        mix(((color >> 16) & 0xff) as u8),
        mix(((color >> 8) & 0xff) as u8),
        mix((color & 0xff) as u8),
    )
}

mod model;
use model::{load_catalog, matches, App, Child, Note, TrayIcon, Toast, Window};

mod painter;
use painter::Painter;

/// The desktop taskbar's launcher affordance. Opens the Start overview:
/// search, current-workspace windows, and the application grid.
const START_LABEL: &str = "Start";
const START_BUTTON_WIDTH: i32 = 58;
const START_ICON_SIZE: i32 = 14;
const TASKBAR_ENTRY_WIDTH: i32 = 48;
const TASKBAR_ENTRY_GAP: i32 = 4;

/// Launcher overview layout (M3). Reuses `launcher_open`/`query`/
/// `selected`/`filtered()` as its shared search/selection state.
const OVERVIEW_MARGIN: i32 = 16;
const OVERVIEW_SEARCH_HEIGHT: i32 = PANEL_ROW + 16;
const OVERVIEW_THUMB_ROW_HEIGHT: i32 = 90;
const OVERVIEW_THUMB_WIDTH: i32 = 140;
const OVERVIEW_THUMB_HEIGHT: i32 = 74;
const OVERVIEW_THUMB_GAP: i32 = 10;
const OVERVIEW_CELL_WIDTH: i32 = 96;
const OVERVIEW_CELL_HEIGHT: i32 = 92;
const OVERVIEW_CELL_GAP: i32 = 12;
const OVERVIEW_APP_ICON_SIZE: i32 = 48;
/// The Dash (M4): favorites and running apps, one row below the search
/// field. Icon-only, no label -- GNOME-dash-style, not the labeled app
/// grid below it.
const OVERVIEW_DASH_ICON: i32 = 40;
const OVERVIEW_DASH_GAP: i32 = 10;
const OVERVIEW_DASH_ROW_HEIGHT: i32 = OVERVIEW_DASH_ICON;
/// Favorites (the same curated set as the desktop shortcuts) plus running
/// apps, deduplicated, bounded to this many Dash entries.
const DASH_MAX: usize = 8;

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
        self.screen()
            .intersect(Area::new(0, middle - 50, self.desktop_width, 80))
    }

    /// The prompt and masked PIN dots change independently of the wallpaper.
    fn lock_input_area(&self) -> Area {
        let middle = self.height / 2;
        self.screen()
            .intersect(Area::new(0, middle - 8, self.desktop_width, 32))
    }

    /// The taskbar sits flush at the top of the screen. Keep this as a
    /// function so every layout calculation shares one chrome origin.
    fn taskbar_top(&self) -> i32 {
        0
    }

    fn taskbar(&self) -> Area {
        Area::new(0, self.taskbar_top(), self.width, TASKBAR_HEIGHT)
    }

    /// The screen's usable band, excluding the taskbar: `(top, height)`.
    /// Shared by every window-placement/clamp site that needs to keep
    /// windows clear of the bar (`work_area`, `open`, `resized`,
    /// `toggle_scale`, `drag_to_pointer`).
    fn usable_band(&self) -> (i32, i32) {
        (
            self.taskbar_top() + TASKBAR_HEIGHT,
            self.height - TASKBAR_HEIGHT,
        )
    }

    /// Row for taskbar text: seven glyph rows centred in the bar below its
    /// one-pixel accent line.
    fn text_y(&self) -> i32 {
        self.taskbar_top() + 9
    }

    /// Centered in the taskbar, not right-aligned.
    fn clock_area(&self) -> Area {
        let x = (self.width - CLOCK_WIDTH) / 2;
        Area::new(x, self.taskbar_top(), CLOCK_WIDTH, TASKBAR_HEIGHT)
    }

    fn tray_area(&self, index: usize) -> Area {
        let x = self.width - CLOCK_WIDTH - (index as i32 + 1) * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    /// Centered, dropping down from the taskbar (M4): the fixed
    /// TOAST_MARGIN from the screen's own top edge alone would land the
    /// toast under/behind the bar, since the bar occupies that band.
    fn toast_area(&self) -> Area {
        Area::new(
            (self.width - TOAST_WIDTH) / 2,
            self.taskbar_top() + TASKBAR_HEIGHT + TOAST_MARGIN,
            TOAST_WIDTH,
            TOAST_HEIGHT,
        )
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

    /// M6: a real, measured search-filter latency number (not assumed),
    /// logged each time the query actually changes. Uses only the
    /// existing `libwoven::ticks()` clock and `log()` -- no new
    /// capability. `self.app_count` is the catalog size actually loaded
    /// this boot; report it alongside the latency, since the design
    /// brief's "over 64 apps" budget assumes `MAX_APPS` (64) entries and
    /// the demo catalog fixture currently has far fewer.
    fn log_search_latency(&self) {
        let start = libwoven::ticks();
        let (_, count) = self.filtered();
        let elapsed = libwoven::ticks().saturating_sub(start);
        let mut digits = [0u8; 20];
        let elapsed_text = libwoven::format_u64(elapsed, &mut digits);
        let mut count_digits = [0u8; 20];
        let count_text = libwoven::format_u64(count as u64, &mut count_digits);
        let mut catalog_digits = [0u8; 20];
        let catalog_text = libwoven::format_u64(self.app_count as u64, &mut catalog_digits);
        log(&[
            b"woven-shell: search latency ticks=",
            elapsed_text,
            b" matches=",
            count_text,
            b" catalog=",
            catalog_text,
        ]);
    }

    /// Fixed launcher button geometry keeps the taskbar stable across
    /// languages and avoids shifting the centered clock.
    fn start_area(&self) -> Area {
        Area::new(
            START_X,
            self.taskbar_top() + 4,
            START_BUTTON_WIDTH,
            START_HEIGHT as i32,
        )
    }

    fn taskbar_entry_area(&self, index: usize) -> Area {
        let x = self.start_area().x
            + self.start_area().width
            + 8
            + index as i32 * (TASKBAR_ENTRY_WIDTH + TASKBAR_ENTRY_GAP);
        Area::new(
            x,
            self.taskbar_top() + 4,
            TASKBAR_ENTRY_WIDTH,
            START_HEIGHT as i32,
        )
    }

    /// The full-screen launcher has no projected halo; its bounded overview
    /// area is the complete damage region.
    fn panel_damage_area(&self) -> Area {
        self.overview_area()
    }

    /// The launcher overview (M3): search, this workspace's window cards,
    /// and the app grid, filling the screen below the taskbar.
    /// Empty unless the launcher is open.
    fn overview_area(&self) -> Area {
        if !self.launcher_open {
            return Area::new(0, 0, 0, 0);
        }
        Area::new(
            0,
            self.taskbar_top() + TASKBAR_HEIGHT,
            self.width,
            self.height - TASKBAR_HEIGHT,
        )
    }

    /// The search field, top of the overview.
    fn overview_search_area(&self) -> Area {
        let overview = self.overview_area();
        Area::new(
            overview.x + OVERVIEW_MARGIN,
            overview.y + OVERVIEW_MARGIN,
            overview.width - 2 * OVERVIEW_MARGIN,
            OVERVIEW_SEARCH_HEIGHT,
        )
    }

    /// Top of the Dash row, below the search field.
    fn overview_dash_row_top(&self) -> i32 {
        self.overview_search_area().y + OVERVIEW_SEARCH_HEIGHT + OVERVIEW_MARGIN
    }

    /// Dash entry `index`'s area, left to right. See [`Self::dash_apps`].
    fn overview_dash_area(&self, index: usize) -> Area {
        let overview = self.overview_area();
        Area::new(
            overview.x + OVERVIEW_MARGIN + index as i32 * (OVERVIEW_DASH_ICON + OVERVIEW_DASH_GAP),
            self.overview_dash_row_top(),
            OVERVIEW_DASH_ICON,
            OVERVIEW_DASH_ICON,
        )
    }

    /// Favorites (the same curated set as the desktop shortcuts,
    /// `DESKTOP_ICON_NAMES`) first, then any other app with a running
    /// instance ([`Self::children`]) not already a favorite, deduplicated,
    /// bounded to `DASH_MAX`.
    fn dash_apps(&self) -> ([usize; DASH_MAX], usize) {
        let mut apps = [0usize; DASH_MAX];
        let mut count = 0;
        for index in 0..DESKTOP_ICON_NAMES.len() {
            if count >= DASH_MAX {
                break;
            }
            if let Some(app_index) = self.desktop_icon_app(index) {
                apps[count] = app_index;
                count += 1;
            }
        }
        for child in self.children.iter().flatten() {
            if count >= DASH_MAX {
                break;
            }
            if !apps[..count].contains(&child.app) {
                apps[count] = child.app;
                count += 1;
            }
        }
        (apps, count)
    }

    /// Top of the window-thumbnails row, below the Dash.
    fn overview_thumb_row_top(&self) -> i32 {
        self.overview_dash_row_top() + OVERVIEW_DASH_ROW_HEIGHT + OVERVIEW_MARGIN
    }

    /// Window-card `index`'s area, left to right, one per current-workspace
    /// window ([`Self::entries`]); a title-card placeholder, not a live
    /// thumbnail, since the compositor ABI has no scaled blit (see
    /// `docs/graphics/overview-desktop-2026-11-03.md`).
    fn overview_thumb_area(&self, index: usize) -> Area {
        let overview = self.overview_area();
        Area::new(
            overview.x + OVERVIEW_MARGIN + index as i32 * (OVERVIEW_THUMB_WIDTH + OVERVIEW_THUMB_GAP),
            self.overview_thumb_row_top(),
            OVERVIEW_THUMB_WIDTH,
            OVERVIEW_THUMB_HEIGHT,
        )
    }

    /// Top of the app grid, below the (always reserved) thumbnails row.
    fn overview_grid_top(&self) -> i32 {
        let has_windows = self.entries().next().is_some();
        self.overview_dash_row_top()
            + OVERVIEW_DASH_ROW_HEIGHT
            + OVERVIEW_MARGIN
            + if has_windows {
                OVERVIEW_THUMB_ROW_HEIGHT + OVERVIEW_MARGIN
            } else {
                0
            }
    }

    /// Grid columns that fit the overview's width.
    fn overview_cols(&self) -> usize {
        let overview = self.overview_area();
        let available = overview.width - 2 * OVERVIEW_MARGIN + OVERVIEW_CELL_GAP;
        (available / (OVERVIEW_CELL_WIDTH + OVERVIEW_CELL_GAP)).max(1) as usize
    }

    /// App grid cell `index`'s area (row-major). `MAX_APPS` (64) always
    /// fits within the rows a typical desktop height offers at this cell
    /// size without paging; a future larger catalog would need real paging
    /// here, not assumed by this layout.
    fn overview_cell_area(&self, index: usize) -> Area {
        let overview = self.overview_area();
        let cols = self.overview_cols();
        let (col, row) = (index % cols, index / cols);
        Area::new(
            overview.x + OVERVIEW_MARGIN + col as i32 * (OVERVIEW_CELL_WIDTH + OVERVIEW_CELL_GAP),
            self.overview_grid_top() + row as i32 * (OVERVIEW_CELL_HEIGHT + OVERVIEW_CELL_GAP),
            OVERVIEW_CELL_WIDTH,
            OVERVIEW_CELL_HEIGHT,
        )
    }

    /// Open or close the launcher. Returns the area to repaint.
    fn toggle_launcher(&mut self) -> Area {
        let closing = self.launcher_open;
        let mut before = self.panel_damage_area();
        if !self.launcher_open && (self.center_open || self.qs_open) {
            // Only one panel at a time.
            before = before
                .union(self.center_area())
                .union(self.qs_area())
                .union(self.qs_button());
            self.center_open = false;
            self.qs_open = false;
        }
        self.launcher_open = !self.launcher_open;
        self.query_length = 0;
        self.selected = 0;
        // A close must restore every pixel under the projected card. The
        // launcher is a foreground mode switch, so a full recomposition is
        // bounded and prevents stale projected corners or icon glyphs.
        if closing {
            before.union(self.screen())
        } else {
            before.union(self.panel_damage_area())
        }
    }

    /// Launch catalog entry `index` and close the launcher. A failure is
    /// reported in a toast. Returns the area to repaint.
    fn launch(&mut self, index: usize) -> Area {
        let mut dirty = self.toggle_launcher();
        let Some(app) = self.apps[index] else {
            return dirty;
        };
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
            let length = (prefix.len() + app.name().len()).min(message.len());
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
        let before = self.overview_area();
        match key {
            Key::Char(character) if (' '..='~').contains(&character) => {
                if self.query_length < MAX_QUERY {
                    self.query[self.query_length] = character as u8;
                    self.query_length += 1;
                }
                self.selected = 0;
                self.log_search_latency();
            }
            Key::Backspace => {
                self.query_length = self.query_length.saturating_sub(1);
                self.selected = 0;
                self.log_search_latency();
            }
            Key::Tab => {
                let count = self.filtered().1;
                if count > 0 {
                    self.selected = (self.selected + 1) % count;
                }
            }
            Key::Escape => return before.union(self.toggle_launcher()),
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
        before.union(self.overview_area())
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
        Area::new(
            x,
            self.taskbar_top() + 4,
            WORKSPACE_BOX,
            START_HEIGHT as i32,
        )
    }

    /// Show workspace `target`: save the current stack, restore the target's.
    fn switch_workspace(&mut self, target: usize) -> Area {
        if target >= WORKSPACES || target == self.workspace {
            return Area::new(0, 0, 0, 0);
        }
        self.cancel_pointer_gesture();
        let _ = self.close_panels();
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
        let Some(slot) = self.focused() else {
            return Area::new(0, 0, 0, 0);
        };
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
        let Some(mut depth) = DepthBuffer::new(
            depth_values,
            canvas.x(),
            canvas.y(),
            canvas.width(),
            canvas.height(),
        ) else {
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

    fn desktop_icon_area(&self, index: usize) -> Area {
        Area::new(
            DESKTOP_ICON_X,
            DESKTOP_ICON_Y + index as i32 * DESKTOP_ICON_STEP,
            64,
            62,
        )
    }

    fn desktop_icon_app(&self, index: usize) -> Option<usize> {
        let name = DESKTOP_ICON_NAMES.get(index)?.as_bytes();
        self.apps[..self.app_count]
            .iter()
            .position(|app| app.is_some_and(|app| app.name() == name))
    }

    /// A small desktop dock: familiar shortcuts with a consistent tile and
    /// label treatment. It is painted above the wallpaper but below windows,
    /// so open applications retain normal z-order and the shortcuts remain
    /// useful on an otherwise empty desktop.
    fn paint_desktop_icons(&self, canvas: &mut Canvas<'_>) {
        for (index, name) in DESKTOP_ICON_LABELS.iter().enumerate() {
            let area = self.desktop_icon_area(index);
            let Some(app_index) = self.desktop_icon_app(index) else {
                continue;
            };
            let Some(app) = self.apps[app_index] else {
                continue;
            };
            let icon_x = area.x + (area.width - DESKTOP_ICON_SIZE) / 2;
            draw_app_icon(
                canvas,
                icon_x,
                area.y,
                app_icon(app.name()),
                DESKTOP_ICON_SIZE,
                false,
            );
            let width = font::text_width(name) as i32;
            let text_x = area.x + ((area.width - width) / 2).max(0);
            canvas.draw_text_system(text_x, area.y + 40, name, theme_text());
        }
    }

    /// The desktop layer: wallpaper and taskbar, with its start button,
    /// window entries and clock.
    fn render_desktop(&self, canvas: &mut Canvas<'_>) {
        self.render_desktop_background(canvas);
        self.paint_desktop_icons(canvas);
        self.paint_taskbar_content(canvas);
    }

    /// The taskbar content: its translucent fill and bottom accent line, the
    /// Start launcher, current-workspace window entries, the centered clock,
    /// workspace switcher, the system menu button, and the unread badge --
    /// drawn at its own real screen position in `canvas`.
    fn paint_taskbar_content(&self, canvas: &mut Canvas<'_>) {
        let top = self.taskbar_top();
        let width = self.width as usize;
        let height = self.height;
        let left = canvas.x().max(0);
        let right = (canvas.x() + canvas.width() as i32).min(self.width);
        let first_row = canvas.y().max(top);
        let last_row = (canvas.y() + canvas.height() as i32).min(height);
        for y in first_row..last_row {
            for x in left..right {
                canvas.set(x, y, wallpaper(x, y, height));
            }
        }
        canvas.blend_rect(
            0,
            top,
            width,
            TASKBAR_HEIGHT as usize,
            theme_taskbar(),
            TASKBAR_OPACITY,
        );
        // The accent line marks the taskbar's boundary with the desktop.
        canvas.fill_rect(0, top + TASKBAR_HEIGHT - 1, width, 1, theme_accent());

        let start = self.start_area();
        let catalog = shell_catalog(self.language);
        let fill = theme_start_fill();
        Renderer::new(canvas).rounded_rect_gradient(
            rect_of(start),
            CHROME_RADIUS,
            color_of(lighten(fill, 24)),
            color_of(fill),
        );
        canvas.draw_text_system(
            start.x + 24,
            self.text_y(),
            catalog.lookup("start", START_LABEL),
            theme_text(),
        );

        let icon_left = start.x + 7;
        let icon_top = start.y + (start.height - START_ICON_SIZE) / 2;
        for (dx, dy, color) in [
            (0, 0, theme_accent()),
            (8, 0, theme_text()),
            (0, 8, theme_text()),
            (8, 8, theme_accent()),
        ] {
            fill_rounded(canvas, Area::new(icon_left + dx, icon_top + dy, 6, 6), 1, color);
        }

        for (index, (slot, window)) in self.entries().enumerate() {
            let area = self.taskbar_entry_area(index);
            let active = self.focused() == Some(slot);
            fill_rounded(
                canvas,
                area,
                CHROME_RADIUS_SMALL,
                if active {
                    lighten(theme_entry(true), 12)
                } else {
                    theme_entry(false)
                },
            );
            draw_app_icon(
                canvas,
                area.x + 2,
                area.y + 1,
                app_icon(&window.title[..window.title_length]),
                14,
                false,
            );
            let mut title = [0u8; 32];
            let length = label(&window.title[..window.title_length], 5, &mut title);
            let text = core::str::from_utf8(&title[..length]).unwrap_or("");
            canvas.draw_text_system(area.x + 18, self.text_y(), text, theme_text());
        }

        let (bytes, length) = clock_text(self.time, self.seconds);
        let text = core::str::from_utf8(&bytes[..length]).unwrap_or("");
        let clock_x = (self.width - font::text_width(text) as i32) / 2;
        canvas.draw_text_system(clock_x, self.text_y(), text, theme_text());

        // The workspace switcher sits left of the system menu button.
        for index in 0..WORKSPACES {
            let area = self.workspace_area(index);
            let fill = theme_entry(index == self.workspace);
            fill_rounded(canvas, area, CHROME_RADIUS_SMALL, fill);
            let digit = [b'1' + index as u8];
            canvas.draw_text_system(
                area.x + 3,
                self.text_y(),
                core::str::from_utf8(&digit).unwrap_or("?"),
                theme_text(),
            );
        }

        // The system menu button opens the existing Quick Settings panel.
        let button = self.qs_button();
        let fill = theme_entry(self.qs_open);
        fill_rounded(canvas, button, CHROME_RADIUS_SMALL, fill);
        for row in 0..3 {
            canvas.fill_rect(button.x + 4, button.y + 4 + row * 3, 8, 1, theme_text());
        }

        if self.unread > 0 {
            let badge = self.badge_area();
            fill_rounded(canvas, badge, CHROME_RADIUS_SMALL, theme_accent());
            let digit = [b'0' + self.unread.min(9) as u8];
            canvas.draw_text_system(
                badge.x + 3,
                badge.y + 2,
                core::str::from_utf8(&digit).unwrap_or("?"),
                theme_text(),
            );
        }
    }

    /// The launcher overview's content (M3): search field, this
    /// workspace's window cards, and the filtered app grid, drawn at its
    /// own real screen position in `canvas`; no alternate legacy chrome is
    /// rendered behind this overview.
    fn paint_overview_content(&self, canvas: &mut Canvas<'_>) {
        let overview = self.overview_area();
        fill_rounded_blended(canvas, overview, CHROME_CARD_RADIUS, theme_panel(), 248);

        // Search field.
        let catalog = shell_catalog(self.language);
        let search = self.overview_search_area();
        fill_rounded(canvas, search, CHROME_RADIUS, theme_taskbar());
        let query = &self.query[..self.query_length];
        if query.is_empty() {
            canvas.draw_text_system(
                search.x + 6,
                search.y + (OVERVIEW_SEARCH_HEIGHT - font::WIDTH as i32) / 2 - 2,
                catalog.lookup("type_to_search", "Type to search"),
                theme_muted(),
            );
        } else {
            let mut buffer = [0u8; 32];
            let length = label(query, MAX_QUERY, &mut buffer);
            let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
            canvas.draw_text_system(
                search.x + 6,
                search.y + (OVERVIEW_SEARCH_HEIGHT - font::WIDTH as i32) / 2 - 2,
                text,
                theme_text(),
            );
        }

        // The Dash: favorites and running apps, icon-only.
        let (dash_apps, dash_count) = self.dash_apps();
        for (index, &app_index) in dash_apps[..dash_count].iter().enumerate() {
            let Some(app) = self.apps[app_index] else { continue };
            let area = self.overview_dash_area(index);
            draw_app_icon(canvas, area.x, area.y, app_icon(app.name()), OVERVIEW_DASH_ICON, false);
        }

        // This workspace's window cards: a title-card placeholder per
        // window, not a live thumbnail -- the compositor ABI has no scaled
        // blit (see `docs/graphics/overview-desktop-2026-11-03.md`).
        let entries: [Option<&Window>; MAX_WINDOWS] = {
            let mut cards = [None; MAX_WINDOWS];
            for (slot, (_, window)) in self.entries().enumerate() {
                if slot < MAX_WINDOWS {
                    cards[slot] = Some(window);
                }
            }
            cards
        };
        if entries.iter().all(Option::is_none) {
            canvas.draw_text_system(
                overview.x + OVERVIEW_MARGIN,
                self.overview_thumb_row_top() + OVERVIEW_THUMB_HEIGHT / 2 - 3,
                catalog.lookup("no_open_windows", "No open windows on this workspace"),
                theme_muted(),
            );
        } else {
            for (index, window) in entries.iter().enumerate() {
                let Some(window) = window else { continue };
                let card = self.overview_thumb_area(index);
                fill_rounded(canvas, card, CHROME_RADIUS_SMALL, theme_entry(false));
                outline(canvas, card, theme_accent());
                let mut buffer = [0u8; 32];
                let length = label(
                    &window.title[..window.title_length],
                    ((card.width - 8) / font::ADVANCE) as usize,
                    &mut buffer,
                );
                let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
                canvas.draw_text_system(card.x + 4, card.y + card.height / 2 - 3, text, theme_text());
            }
        }

        // The app grid, filtered by the search field.
        let (found, count) = self.filtered();
        for (index, &app_index) in found[..count].iter().enumerate() {
            let Some(app) = self.apps[app_index] else { continue };
            let cell = self.overview_cell_area(index);
            if index == self.selected {
                fill_rounded(
                    canvas,
                    Area::new(cell.x + 2, cell.y, cell.width - 4, cell.height - 4),
                    CHROME_RADIUS,
                    lighten(theme_taskbar(), 18),
                );
            }
            draw_app_icon(
                canvas,
                cell.x + (cell.width - OVERVIEW_APP_ICON_SIZE) / 2,
                cell.y + 2,
                app_icon(app.name()),
                OVERVIEW_APP_ICON_SIZE,
                index == self.selected,
            );
            // Catalog names are "Woven<App>"; the grid already shows the
            // WovenHat-branded icon beside each one, so the repeated
            // prefix only costs width better spent on the rest of the
            // name before truncation.
            let display_name = app.name().strip_prefix(b"Woven").unwrap_or(app.name());
            let mut buffer = [0u8; 32];
            let length = label(
                display_name,
                (cell.width / font::ADVANCE) as usize,
                &mut buffer,
            );
            let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
            let text_x = cell.x + (cell.width - font::text_width(text) as i32) / 2;
            canvas.draw_text_system(
                text_x.max(cell.x),
                cell.y + OVERVIEW_APP_ICON_SIZE + 12,
                text,
                theme_text(),
            );
        }
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
        let y = self.taskbar_top() + TASKBAR_HEIGHT + 2;
        Area::new(self.width - 4 - CENTER_WIDTH, y, CENTER_WIDTH, height)
    }

    /// The toggles as they currently read.
    fn qs_rows(&self) -> [&'static str; QS_ROWS] {
        [
            "Power off",
            "Restart",
            if self.scale == 2 {
                "Display scale: 2x"
            } else {
                "Display scale: 1x"
            },
            if self.dnd {
                "Do not disturb: On"
            } else {
                "Do not disturb: Off"
            },
            if self.tiled {
                "Tiling: On"
            } else {
                "Tiling: Off"
            },
            "Lock screen",
            "Change PIN",
            match self.language {
                libwoven::i18n::Language::English => "Language: English",
                libwoven::i18n::Language::Chichewa => "Language: Chichewa",
                libwoven::i18n::Language::Arabic => "Language: Arabic",
            },
            if self.three_d {
                "3D Desktop: On"
            } else {
                "3D Desktop: Off"
            },
            if self.reduced_motion {
                "Reduced Motion: On"
            } else {
                "Reduced Motion: Off"
            },
        ]
    }

    /// The lock screen: the wallpaper dimmed, "Locked", the time, the
    /// prompt, and a masked dot per digit typed, all centred.
    fn render_lock(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        self.render_floor_3d(canvas, |x, y| {
            blend(rgb(0, 0, 0), wallpaper(x, y, height), LOCK_DIM)
        });
        let middle = self.height / 2;
        let centred = |text: &str| (self.width - font::text_width(text) as i32) / 2;
        let catalog = shell_catalog(self.language);

        // A frosted card behind the sign-in/lock content, Windows-11-style,
        // instead of text floating directly on the dimmed wallpaper.
        let card = Area::new(self.width / 2 - 140, middle - 120, 280, 170);
        fill_rounded_blended(canvas, card, CHROME_CARD_RADIUS, rgb(10, 14, 24), 170);

        if self.mode == LockMode::SignIn {
            // A circular avatar for the selected account: its initial inside
            // a filled circle, the same technique used for rounded chrome.
            let initial = self.accounts[self.chosen]
                .map(|account| account.name().first().copied().unwrap_or(b'?'))
                .unwrap_or(b'?')
                .to_ascii_uppercase();
            let avatar_y = middle - 85;
            fill_circle(canvas, self.width / 2, avatar_y, AVATAR_DIAMETER, theme_accent());
            let glyph = [initial];
            let glyph = core::str::from_utf8(&glyph).unwrap_or("?");
            canvas.draw_text_system(
                self.width / 2 - font::text_width(glyph) as i32 / 2,
                avatar_y - 3,
                glyph,
                rgb(255, 255, 255),
            );

            // The sign-in prompt, then the accounts, the chosen one marked.
            let text = catalog.lookup("sign_in", "Sign in");
            if let Some(face) = system_font_face() {
                canvas.draw_text_ttf(
                    centred(text),
                    middle - 52,
                    text,
                    theme_text(),
                    &face,
                    12,
                );
            } else {
                canvas.draw_text_system(centred(text), middle - 52, text, theme_text());
            }
            for (index, account) in self.accounts.iter().flatten().enumerate() {
                let mut line = [b' '; 2 + libwoven::accounts::MAX_NAME];
                if index == self.chosen {
                    line[0] = b'>';
                }
                line[2..2 + account.name().len()].copy_from_slice(account.name());
                let text = core::str::from_utf8(&line[..2 + account.name().len()]).unwrap_or("");
                canvas.draw_text_system(
                    centred(text),
                    middle - 38 + index as i32 * 10,
                    text,
                    theme_text(),
                );
            }
        } else if self.mode == LockMode::ChangePin {
            let text = catalog.lookup("change_pin", "Change PIN");
            canvas.draw_text_system(centred(text), middle - 34, text, theme_text());
        } else {
            let text = catalog.lookup("locked", "Locked");
            canvas.draw_text_system(centred(text), middle - 34, text, theme_text());
            let (bytes, length) = clock_text(self.time, false);
            let clock = core::str::from_utf8(&bytes[..length]).unwrap_or("");
            canvas.draw_text_system(centred(clock), middle - 20, clock, theme_text());
        }
        let prompt = if self.mode == LockMode::ChangePin && self.prompt == LockPrompt::Enter {
            match self.change_pin {
                Some(ChangePinStep::Current) => catalog.lookup("current_pin", "Current PIN"),
                Some(ChangePinStep::New) => catalog.lookup("new_pin", "New PIN"),
                Some(ChangePinStep::Confirm) => {
                    catalog.lookup("confirm_new_pin", "Confirm new PIN")
                }
                None => catalog.lookup("enter_pin", "Enter PIN"),
            }
        } else {
            match self.prompt {
                LockPrompt::Enter => catalog.lookup("enter_pin", "Enter PIN"),
                LockPrompt::Wrong => catalog.lookup("wrong_pin", "Wrong PIN"),
                LockPrompt::LockedOut => catalog.lookup("try_again_shortly", "Try again shortly"),
            }
        };
        canvas.draw_text_system(centred(prompt), middle - 4, prompt, theme_text());
        let dots = self.entry_length as i32;
        if dots > 0 {
            let left = (self.width - (dots * 10 - 4)) / 2;
            // A rounded pill behind the dots, Windows-11-style, instead of
            // the digits floating bare on the card.
            let pill = Area::new(left - 10, middle + 6, dots * 10 - 4 + 20, 20);
            fill_rounded(canvas, pill, 10, rgb(24, 30, 44));
            for index in 0..dots {
                fill_circle(canvas, left + index * 10 + 3, middle + 15, 6, theme_text());
            }
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
        self.cancel_pointer_gesture();
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
            self.toast = Some(Toast {
                text,
                length,
                until: libwoven::ticks() + TOAST_TICKS,
            });
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
        let Some((stored, length)) = &self.pin else {
            return true;
        };
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
                        if self.accounts.iter().any(Option::is_some) =>
                    {
                        true
                    }
                    _ => self.pin_matches(),
                } {
                    let kernel_authenticated = match self.mode {
                        LockMode::SignIn => match self.accounts[self.chosen] {
                            Some(account) => {
                                let digest =
                                    account.password_digest(&self.entry[..self.entry_length]);
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
                        },
                        LockMode::Lock => match (
                            self.kernel_session,
                            self.session.and_then(|index| self.accounts[index]),
                        ) {
                            (Some(session), Some(account)) => {
                                let digest =
                                    account.password_digest(&self.entry[..self.entry_length]);
                                libwoven::identity::session_unlock_verified(session, &digest)
                                    .is_ok()
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
                        log(&[
                            b"woven-shell: signed in: ",
                            name.as_ref().map_or(&b"?"[..], |account| account.name()),
                        ]);
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
                    && self.entry[..self.entry_length]
                        == self.change_pin_new[..self.change_pin_new_length];
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
                self.toast = Some(Toast {
                    text,
                    length,
                    until: libwoven::ticks() + TOAST_TICKS,
                });
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
        // A rounded, near-opaque card instead of a hard-edged square outline
        // with a flat header block, Windows-11-style like the lock screen's
        // card. `render()`'s keyed overlay leaves the handful of corner
        // pixels outside the rounding untouched, so the square backdrop tint
        // `blend()` already laid down under this shows through there as a
        // soft drop shadow rather than a visible seam.
        fill_rounded_blended(canvas, area, CHROME_RADIUS, theme_panel(), 235);
        canvas.draw_text_system(x + 6, y + 5, QS_TITLE, theme_accent());
        canvas.fill_rect(
            x + CHROME_RADIUS as i32,
            y + PANEL_ROW,
            (area.width - 2 * CHROME_RADIUS as i32).max(0) as usize,
            1,
            theme_accent(),
        );
        for (row, text) in self.qs_rows().iter().enumerate() {
            canvas.draw_text_system(
                x + 6,
                y + 1 + (1 + row as i32) * PANEL_ROW + 4,
                text,
                theme_text(),
            );
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
        let Some(mut texture) = Canvas::new(
            texture_pixels,
            0,
            0,
            area.width as usize,
            area.height as usize,
        ) else {
            return self.paint_qs_content(canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                theme_panel(),
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
        let vertex =
            |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) = DepthBuffer::new(
            depth_values,
            canvas.x(),
            canvas.y(),
            canvas.width(),
            canvas.height(),
        ) else {
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
        log(if restart {
            &[b"woven-shell: restart"]
        } else {
            &[b"woven-shell: power off"]
        });
        let result = if restart {
            libwoven::restart()
        } else {
            libwoven::power_off()
        };
        let reason: &[u8] = match result {
            Err(_) => b"Not allowed to turn the machine off",
            Ok(1) => b"This machine has no ACPI power-off",
            Ok(2) => b"The firmware refused ACPI power-off",
            Ok(_) => b"The machine ignored the request",
        };
        let mut text = [0u8; 32];
        let length = label(reason, TOAST_CHARS, &mut text);
        self.toast = Some(Toast {
            text,
            length,
            until: libwoven::ticks() + TOAST_TICKS,
        });
        self.close_panels().union(self.toast_area())
    }

    fn toggle_tiling(&mut self) -> Area {
        self.tiled = !self.tiled;
        log(if self.tiled {
            &[b"woven-shell: tiling on"]
        } else {
            &[b"woven-shell: tiling off"]
        });
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
        let (outputs, monitors) =
            libwoven::compositor_outputs().unwrap_or(([libwoven::Output::default(); 4], 0));
        self.desktop_width = info.width as i32;
        self.width = if monitors > 0 {
            (outputs[0].width as i32).clamp(1, self.desktop_width)
        } else {
            self.desktop_width
        };
        self.height = info.height as i32;
        let (area_top, area_height) = self.usable_band();
        for window in self.windows.iter_mut().flatten() {
            window.x = window
                .x
                .clamp(0, (self.desktop_width - window.width).max(0));
            window.y = window.y.clamp(
                area_top + TITLE_HEIGHT,
                (area_top + area_height - window.height).max(area_top + TITLE_HEIGHT),
            );
        }
        self.pointer = (
            self.pointer.0.min(self.desktop_width - 1),
            self.pointer.1.min(self.height - 1),
        );
        log(if target == 1 {
            &[b"woven-shell: display scale 1x"]
        } else {
            &[b"woven-shell: display scale 2x"]
        });
        let _ = self.retile();
        self.screen()
    }

    /// Where the unread badge sits: a 10x11 box just left of the clock text.
    fn badge_area(&self) -> Area {
        let (_, length) = clock_text(self.time, self.seconds);
        let clock_width = length as i32 * font::ADVANCE - (font::ADVANCE - font::WIDTH as i32);
        let clock_x = (self.width - clock_width) / 2;
        Area::new(clock_x - 13, self.taskbar_top() + 7, 10, 11)
    }

    /// The Notification Center, sized to its rows, drops down from the
    /// clock, calendar/notification-popover style; empty while closed.
    fn center_area(&self) -> Area {
        if !self.center_open {
            return Area::new(0, 0, 0, 0);
        }
        let count = self.notes.iter().flatten().count();
        let height = (1 + count.max(1) as i32) * PANEL_ROW + 2;
        let y = self.taskbar_top() + TASKBAR_HEIGHT + 2;
        Area::new(self.width - 4 - CENTER_WIDTH, y, CENTER_WIDTH, height)
    }

    /// The Notification Center's overlay: border, title row with Clear, and
    /// one row per remembered notification, drawn at `area`'s own position
    /// in `canvas`. Used both for the ordinary flat 2D panel and, rendered
    /// once at local origin into an offscreen texture, as the source the 3D
    /// mode's perspective projection samples.
    fn paint_center_content(&self, canvas: &mut Canvas<'_>, area: Area) {
        let (x, y) = (area.x, area.y);
        // Same rounded, near-opaque card treatment as the Quick Settings
        // panel and the lock screen, instead of a hard-edged square outline
        // with a flat header block.
        fill_rounded_blended(canvas, area, CHROME_RADIUS, theme_panel(), 235);
        canvas.draw_text_system(x + 6, y + 5, CENTER_TITLE, theme_accent());
        let clear_x = x + area.width - 6 - font::text_width(CENTER_CLEAR) as i32;
        canvas.draw_text_system(clear_x, y + 5, CENTER_CLEAR, theme_text());
        canvas.fill_rect(
            x + CHROME_RADIUS as i32,
            y + PANEL_ROW,
            (area.width - 2 * CHROME_RADIUS as i32).max(0) as usize,
            1,
            theme_accent(),
        );
        let mut any = false;
        for (row, note) in self.notes.iter().flatten().enumerate() {
            any = true;
            let text = core::str::from_utf8(&note.text[..note.length]).unwrap_or("");
            canvas.draw_text_system(
                x + 6,
                y + 1 + (1 + row as i32) * PANEL_ROW + 4,
                text,
                theme_text(),
            );
        }
        if !any {
            canvas.draw_text_system(
                x + 6,
                y + 1 + PANEL_ROW + 4,
                "No notifications",
                theme_text(),
            );
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
        let Some(mut texture) = Canvas::new(
            texture_pixels,
            0,
            0,
            area.width as usize,
            area.height as usize,
        ) else {
            return self.paint_center_content(canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                theme_panel(),
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
        let vertex =
            |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) = DepthBuffer::new(
            depth_values,
            canvas.x(),
            canvas.y(),
            canvas.width(),
            canvas.height(),
        ) else {
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
        let window = self
            .windows
            .iter()
            .flatten()
            .find(|window| window.owner == sender);
        let source = window.map_or(&b"app"[..], |window| &window.title[..window.title_length]);
        let mut row = [0u8; 64];
        let mut length = 0;
        for part in [source, b": ", text] {
            let take = part.len().min(row.len() - length);
            row[length..length + take].copy_from_slice(&part[..take]);
            length += take;
        }
        let mut note = Note {
            text: [0; 32],
            length: 0,
        };
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

    fn render_title(
        window: &Window,
        focused: bool,
        canvas: &mut Canvas<'_>,
        three_d: bool,
        height: i32,
    ) {
        let area = window.title_area();
        let fill = theme_title(focused);
        if three_d {
            Self::render_title_3d(area, fill, canvas);
        } else {
            // Rounded top corners, square bottom: the standard modern
            // window shape (Windows, macOS, GNOME all use it) -- the title
            // bar's bottom edge sits flush against the square body below
            // it, while its top corners read as a floating card rather
            // than a flat slab. The rounded fill below leaves the handful
            // of corner pixels outside the curve untouched, and `draw`
            // presents this canvas opaquely from a reused scratch buffer
            // (no transparent-key compositing here, unlike the floating
            // panels), so those pixels must be given real content -- the
            // desktop wallpaper, the common case for what is behind a
            // window's top corners -- rather than left holding whatever an
            // earlier, unrelated draw call put in the shared scratch tile.
            canvas.shade(|x, y| wallpaper(x, y, height));
            Renderer::new(canvas).rounded_rect_top_gradient(
                rect_of(area),
                CHROME_RADIUS_SMALL,
                color_of(lighten(fill, 40)),
                color_of(fill),
            );
        }
        let close = window.close_area();
        if !close.is_empty() {
            fill_rounded(canvas, close, CHROME_RADIUS_SMALL, CLOSE_FILL);
            canvas.draw_text_system(close.x + 3, close.y + 2, "x", theme_text());
        }
        let mut buffer = [0u8; 32];
        let reserved = if close.is_empty() {
            8
        } else {
            8 + CLOSE_SIZE + 4
        };
        let chars = ((window.width - reserved) / font::ADVANCE).max(0) as usize;
        let length = label(&window.title[..window.title_length], chars, &mut buffer);
        let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
        canvas.draw_text_system(area.x + 4, area.y + 4, text, theme_text());
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
        canvas.fill_rect(
            area.x,
            area.y,
            area.width as usize,
            area.height as usize,
            fill,
        );
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
        let color =
            libwoven::graphics::Color::rgb((fill >> 16) as u8, (fill >> 8) as u8, fill as u8);
        let vertex = |(x, y, depth): (i32, i32, u16)| Vertex3::new(x, y, depth, color);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) = DepthBuffer::new(
            depth_values,
            canvas.x(),
            canvas.y(),
            canvas.width(),
            canvas.height(),
        ) else {
            return;
        };
        depth.clear(u16::MAX);
        let mut renderer = Renderer::new(canvas);
        renderer.triangle_3d(
            &mut depth,
            vertex(top_left),
            vertex(top_right),
            vertex(bottom_left),
        );
        renderer.triangle_3d(
            &mut depth,
            vertex(top_right),
            vertex(bottom_right),
            vertex(bottom_left),
        );
    }

    /// The toast's overlay: border and text over its blended background,
    /// drawn at `area`'s own position in `canvas`. Used both for the
    /// ordinary flat 2D toast and, rendered once at local origin into an
    /// offscreen texture, as the source the 3D mode's perspective
    /// projection samples.
    fn paint_toast_content(&self, toast: &Toast, canvas: &mut Canvas<'_>, area: Area) {
        // Rounded, near-opaque card, matching the rest of the floating
        // chrome, instead of a hard-edged square outline.
        fill_rounded_blended(canvas, area, CHROME_RADIUS_SMALL, theme_toast(), 235);
        let text = core::str::from_utf8(&toast.text[..toast.length]).unwrap_or("");
        canvas.draw_text_system(area.x + 6, area.y + 8, text, theme_text());
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
        let Some(mut texture) = Canvas::new(
            texture_pixels,
            0,
            0,
            area.width as usize,
            area.height as usize,
        ) else {
            return self.paint_toast_content(toast, canvas, area);
        };
        texture.shade(|x, y| {
            blend(
                theme_toast(),
                wallpaper(area.x + x, area.y + y, self.height),
                TOAST_OPACITY,
            )
        });
        self.paint_toast_content(
            toast,
            &mut texture,
            Area::new(0, 0, area.width, area.height),
        );
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
        let vertex =
            |(x, y, depth): (i32, i32, u16), u: u8, v: u8| Vertex3Tex::new(x, y, depth, u, v);

        // SAFETY: single-threaded; this is the only live reference, and it
        // does not outlive this call.
        let depth_values = unsafe { &mut *TILE_DEPTH_3D.0.get() };
        let Some(mut depth) = DepthBuffer::new(
            depth_values,
            canvas.x(),
            canvas.y(),
            canvas.width(),
            canvas.height(),
        ) else {
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
                    b'W' => theme_text(),
                    _ => continue,
                };
                canvas.set(left + column as i32, top + row as i32, color);
            }
        }
    }

    /// Recompose `area` completely, bottom to top.
    fn repaint(&mut self, painter: &mut Painter<'_>, mut area: Area) {
        // A client can release its surface between check_clients and a blit.
        // Recover before presenting the frame, using the same teardown as the
        // liveness path. Each retry removes one window, so work is bounded.
        for _ in 0..=MAX_WINDOWS {
            let Some(slot) = self.repaint_once(painter, area) else {
                return;
            };
            let Some(window) = self.windows[slot] else {
                fail(8);
            };
            if libwoven::compositor_blit_damage(window.token, window.x, window.y).is_ok() {
                // The surface is still valid: retain the unexpected failure
                // diagnostic rather than classifying any blit error as exit.
                log(&[
                    b"woven-shell: shared surface compose failed: ",
                    &window.title[..window.title_length],
                ]);
                fail(8);
            }
            log(&[b"woven-shell: released surface removed before present"]);
            self.close(slot);
            // Closing can change focus, tiling and the taskbar outside area.
            // Rebuild the complete frame after this rare lifetime race.
            area = self.screen();
        }
        fail(8);
    }

    /// Return a client that vanished while composing this frame.
    fn repaint_once(&self, painter: &mut Painter<'_>, area: Area) -> Option<usize> {
        let area = area.intersect(self.screen());
        if area.is_empty() {
            return None;
        }
        // Locked, nothing but the lock screen is drawn: no window, no
        // notification, no pointer.
        if self.locked {
            painter.draw(area, |canvas| self.render_lock(canvas));
            return None;
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
        // Windows may sit on any monitor; only the bar's own band is excluded.
        let (band_top, band_height) = self.usable_band();
        let above_taskbar = Area::new(0, band_top, self.desktop_width, band_height);
        for (stack_index, &slot) in self.stack[..self.stacked].iter().enumerate() {
            let Some(window) = &self.windows[slot] else {
                continue;
            };
            // Window bodies are submitted with an opaque blit. If a fully
            // opaque higher window contains this entire footprint, neither
            // the lower title/body nor its shadow can affect final pixels.
            // Skip it before issuing any client blits. The stack is capped by
            // MAX_WINDOWS, so this bounded scan cannot turn repaint into an
            // unbounded scene walk.
            if self.frame_is_fully_occluded(stack_index, window.footprint()) {
                continue;
            }
            let bands = if Some(slot) == focused {
                FOCUSED_SHADOW
            } else {
                UNFOCUSED_SHADOW
            };
            let frame = window.frame();
            // Everything about a fading window, its shadow included, is
            // composed at its current opacity.
            let alpha = window.alpha();
            if alpha == 0 {
                continue;
            }
            for (offset, opacity) in bands {
                let band = Area::new(
                    frame.x + offset,
                    frame.y + offset,
                    frame.width,
                    frame.height,
                );
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
                    painter.draw(title, |canvas| {
                        Self::render_title(window, active, canvas, self.three_d, self.height)
                    });
                } else {
                    painter.draw_blended(title, alpha, |canvas| {
                        Self::render_title(window, active, canvas, self.three_d, self.height)
                    });
                }
            }
            let body = window.body().intersect(area);
            if !body.is_empty() {
                let region = body.relative_to(window.x, window.y);
                let result = if alpha == u8::MAX {
                    libwoven::compositor_blit_region_shared(window.token, region, body.x, body.y)
                } else {
                    libwoven::compositor_blend_region_shared(
                        window.token,
                        region,
                        body.x,
                        body.y,
                        alpha,
                    )
                };
                // A resize deliberately permits a bounded gap between the
                // old surface's release and the replacement WRSZ message.
                if result.is_err() && libwoven::ticks() >= window.resizing_until {
                    return Some(slot);
                }
            }
        }

        if let Some(toast) = &self.toast {
            let part = self.toast_area().intersect(area);
            if !part.is_empty() {
                painter.blend(part, theme_toast(), TOAST_OPACITY);
                painter.overlay(part, |canvas| self.render_toast(toast, canvas));
            }
        }

        let overview = self.overview_area().intersect(area);
        if !overview.is_empty() {
            // The M5 fade-in attempt (a single draw_blended pass at the
            // overview_fade spring's value) produced a ghosted, tile-
            // offset duplicate of the panel's content once actually seen
            // on screen via QEMU -- reverted to the plain blend+keyed-
            // overlay two-step the other panels already use and that M3
            // verified correct, rather than ship something visibly
            // broken. See the M5 writeup in
            // docs/graphics/overview-desktop-2026-11-03.md.
            painter.blend(overview, theme_panel(), PANEL_OPACITY);
            painter.overlay(overview, |canvas| self.paint_overview_content(canvas));
        }

        let qs = self.qs_area().intersect(area);
        if !qs.is_empty() {
            painter.blend(qs, theme_panel(), PANEL_OPACITY);
            painter.overlay(qs, |canvas| self.render_qs(canvas));
        }

        let center = self.center_area().intersect(area);
        if !center.is_empty() {
            painter.blend(center, theme_panel(), PANEL_OPACITY);
            painter.overlay(center, |canvas| self.render_center(canvas));
        }

        let cursor = self.cursor_area().intersect(area);
        if !cursor.is_empty() {
            painter.overlay(cursor, |canvas| self.render_cursor(canvas));
        }
        None
    }

    fn frame_is_fully_occluded(&self, stack_index: usize, footprint: Area) -> bool {
        self.stack[stack_index + 1..self.stacked]
            .iter()
            .filter_map(|&higher_slot| self.windows[higher_slot].as_ref())
            .any(|higher| {
                higher.alpha() == u8::MAX
                    && higher.frame().intersect(footprint) == footprint
            })
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
        let previous = self
            .focused()
            .and_then(|old| self.windows[old])
            .map(|w| w.footprint());
        self.raise(slot);
        let current = self.windows[slot]
            .map(|w| w.footprint())
            .unwrap_or(Area::new(0, 0, 0, 0));
        current
            .union(previous.unwrap_or(Area::new(0, 0, 0, 0)))
            .union(self.taskbar())
    }

    /// Open a window for `owner`. Returns the areas to repaint.
    fn open(&mut self, owner: process::Id, message: &[u8]) -> Dirty {
        if message.len() < 16 {
            return Dirty::new();
        }
        let token = u64::from_le_bytes(message[4..12].try_into().unwrap_or([0; 8]));
        let width = i32::from(u16::from_le_bytes([message[12], message[13]]));
        let height = i32::from(u16::from_le_bytes([message[14], message[15]]));
        // `work_area()` expresses the same usable band as a `Frame`;
        // `usable_band()` gives it as plain bounds, what this validation
        // and placement arithmetic need.
        let (area_top, area_height) = self.usable_band();
        if libwoven::surface_owner(token) != owner
            || !(1..=self.width).contains(&width)
            || !(1..=area_height - TITLE_HEIGHT).contains(&height)
        {
            log(&[b"woven-shell: window open rejected"]);
            return Dirty::new();
        }
        let Some(slot) = self.windows.iter().position(Option::is_none) else {
            return Dirty::new();
        };
        let offset = CASCADE_ORIGIN + (self.opened % 6) * CASCADE_STEP;
        self.opened += 1;
        // Keep the whole frame inside the usable band and on screen.
        let x = offset.min((self.width - width).max(0));
        let y = offset.min((area_top + area_height - height).max(area_top + TITLE_HEIGHT));
        let mut title = [0u8; MAX_TITLE];
        let title_length = (message.len() - 16).min(MAX_TITLE);
        title[..title_length].copy_from_slice(&message[16..16 + title_length]);
        // Reduced motion: appear at full opacity immediately rather than
        // animate in, matching libwoven::accessibility::Settings's own
        // "duration 0" convention for this preference.
        let mut fade = Spring::new(if self.reduced_motion {
            i32::from(u8::MAX)
        } else {
            0
        });
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
            cancellation_protocol: false,
        };
        self.windows[slot] = Some(window);
        let previous = self
            .focused()
            .and_then(|old| self.windows[old])
            .map(|w| w.footprint());
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
        let (top, height) = self.usable_band();
        Frame::new(0, top, self.width, height)
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
        let (_, area_height) = self.usable_band();
        if libwoven::surface_owner(token) != owner
            || !(1..=self.width).contains(&width)
            || !(1..=area_height - TITLE_HEIGHT).contains(&height)
        {
            log(&[b"woven-shell: WRSZ rejected: token or geometry"]);
            return empty;
        }
        let Some(window) = self
            .windows
            .iter_mut()
            .flatten()
            .find(|window| window.owner == owner)
        else {
            log(&[b"woven-shell: WRSZ rejected: owner window missing"]);
            return empty;
        };
        let before = window.footprint();
        window.token = token;
        window.width = width;
        window.height = height;
        window.resizing_until = 0;
        log(&[
            b"woven-shell: window resized: ",
            &window.title[..window.title_length],
        ]);
        before.union(window.footprint())
    }

    /// Remove window `slot`. Returns the area to repaint.
    fn close(&mut self, slot: usize) -> Dirty {
        let Some(window) = self.windows[slot].take() else {
            return Dirty::new();
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
        // A reused window slot must never inherit a dead client's capture.
        if self.pointer_capture == Some(slot) {
            self.pointer_capture = None;
        }
        log(&[
            b"woven-shell: window closed: ",
            &window.title[..window.title_length],
        ]);
        // The newly focused window's title and shadow change too.
        let next = self
            .focused()
            .and_then(|top| self.windows[top])
            .map(|w| w.footprint());
        let mut dirty = Dirty::of(window.footprint());
        dirty.add(next.unwrap_or(Area::new(0, 0, 0, 0)));
        dirty.add(self.taskbar());
        dirty.add(self.retile());
        dirty
    }

    fn handle_message(&mut self, sender: process::Id, message: &[u8]) -> Dirty {
        let empty = Dirty::new();
        match message.get(..4) {
            Some(b"WCAP") if message == b"WCAP\x01" => {
                if let Some(window) = self.windows.iter_mut().flatten()
                    .find(|window| window.owner == sender)
                {
                    window.cancellation_protocol = true;
                }
                empty
            }
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
        // Motion changes state immediately, but painting is deferred until the
        // bounded input batch completes. Absolute devices use the same hit tests
        // and pointer capture as the relative PS/2 path.
        match event {
            InputEvent::PointerMove { dx, dy } => {
                return self.move_pointer_to(
                    self.pointer.0 + i32::from(dx),
                    self.pointer.1 + i32::from(dy),
                );
            }
            InputEvent::PointerAbsolute { x, y } => {
                return self.move_pointer_to(
                    absolute_pointer_axis(x, self.desktop_width),
                    absolute_pointer_axis(y, self.height),
                );
            }
            _ => {}
        }
        if self.locked {
            return match event {
                InputEvent::Key(key) => self.lock_key(key),
                _ => empty,
            };
        }
        match event {
            InputEvent::PointerButton {
                button: 0,
                pressed: false,
            } => {
                if let Some(slot) = self.pointer_capture.take() {
                    self.forward_pointer_button(slot, false);
                }
                // Dropped on the left or right edge: snap to that half.
                let Some((slot, _, _)) = self.drag.take() else {
                    return empty;
                };
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
            InputEvent::PointerButton {
                button: 0,
                pressed: true,
            } => {
                let (x, y) = self.pointer;
                if let Some(index) =
                    (0..WORKSPACES).find(|&index| self.workspace_area(index).contains(x, y))
                {
                    return self.switch_workspace(index);
                }
                if self.start_area().contains(x, y) {
                    return self.toggle_launcher();
                }
                let taskbar_slot = self.entries().enumerate().find_map(|(index, (slot, _))| {
                    self.taskbar_entry_area(index).contains(x, y).then_some(slot)
                });
                if let Some(slot) = taskbar_slot {
                    return self.focus(slot);
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
                            log(if self.dnd {
                                &[b"woven-shell: do not disturb on"]
                            } else {
                                &[b"woven-shell: do not disturb off"]
                            });
                            panel
                        }
                        4 => self.toggle_tiling(),
                        5 => self.lock(),
                        6 => self.start_change_pin(),
                        7 => {
                            self.language = match self.language {
                                libwoven::i18n::Language::English => {
                                    libwoven::i18n::Language::Chichewa
                                }
                                libwoven::i18n::Language::Chichewa => {
                                    libwoven::i18n::Language::Arabic
                                }
                                libwoven::i18n::Language::Arabic => {
                                    libwoven::i18n::Language::English
                                }
                            };
                            log(&[b"woven-shell: language toggled"]);
                            panel
                        }
                        8 => {
                            self.three_d = !self.three_d;
                            log(if self.three_d {
                                &[b"woven-shell: 3D desktop on"]
                            } else {
                                &[b"woven-shell: 3D desktop off"]
                            });
                            // Every open window's title bar also renders
                            // differently in 3D mode, not just this panel.
                            panel.union(self.screen())
                        }
                        9 => {
                            self.reduced_motion = !self.reduced_motion;
                            log(if self.reduced_motion {
                                &[b"woven-shell: reduced motion on"]
                            } else {
                                &[b"woven-shell: reduced motion off"]
                            });
                            panel
                        }
                        _ => empty,
                    };
                }
                // The clock opens and closes the Notification Center.
                if self.clock_area().contains(x, y)
                    || (self.unread > 0 && self.badge_area().contains(x, y))
                {
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
                    // Anywhere outside the overview closes it.
                    let overview = self.overview_area();
                    if !overview.contains(x, y) {
                        return self.toggle_launcher();
                    }
                    // A window card focuses that window and closes the
                    // overview -- "open, search, launch, close; focus
                    // restored" also covers picking an existing window.
                    let card = self
                        .entries()
                        .enumerate()
                        .find(|(index, _)| self.overview_thumb_area(*index).contains(x, y))
                        .map(|(_, (slot, _))| slot);
                    if let Some(slot) = card {
                        let closed = self.toggle_launcher();
                        return closed.union(self.focus(slot));
                    }
                    // A Dash icon focuses a running instance, or launches
                    // one if none is running.
                    let (dash_apps, dash_count) = self.dash_apps();
                    let dash_hit = (0..dash_count)
                        .find(|&index| self.overview_dash_area(index).contains(x, y))
                        .map(|index| dash_apps[index]);
                    if let Some(app_index) = dash_hit {
                        let running = self
                            .children
                            .iter()
                            .flatten()
                            .find(|child| child.app == app_index)
                            .and_then(|child| {
                                self.windows
                                    .iter()
                                    .position(|window| window.is_some_and(|w| w.owner == child.pid))
                            });
                        if let Some(slot) = running {
                            let closed = self.toggle_launcher();
                            return closed.union(self.focus(slot));
                        }
                        return self.launch(app_index);
                    }
                    // An app grid cell launches it.
                    let (found, count) = self.filtered();
                    if let Some(app_index) = (0..count)
                        .find(|&index| self.overview_cell_area(index).contains(x, y))
                        .map(|index| found[index])
                    {
                        return self.launch(app_index);
                    }
                    return empty;
                }
                // Desktop shortcuts are below windows in the visual stack;
                // only activate one when no window frame covers the click.
                if let Some(index) = (0..DESKTOP_ICON_NAMES.len()).find(|&index| {
                    self.desktop_icon_area(index).contains(x, y)
                        && !self.stack[..self.stacked].iter().rev().any(|&slot| {
                            self.windows[slot].is_some_and(|window| window.frame().contains(x, y))
                        })
                }) {
                    if let Some(app) = self.desktop_icon_app(index) {
                        return self.launch(app);
                    }
                }
                // Otherwise the topmost window under the pointer: its close
                // control asks the owner to close; anywhere else focuses it,
                // and its title bar also starts a drag.
                let mut pointer_x = [0u8; 20];
                let mut pointer_y = [0u8; 20];
                let pointer_x = libwoven::format_u64(self.pointer.0.max(0) as u64, &mut pointer_x);
                let pointer_y = libwoven::format_u64(self.pointer.1.max(0) as u64, &mut pointer_y);
                log(&[
                    b"woven-shell: pointer press x=",
                    pointer_x,
                    b" y=",
                    pointer_y,
                ]);
                let hit = self.stack[..self.stacked]
                    .iter()
                    .rev()
                    .copied()
                    .find(|&slot| {
                        self.windows[slot].is_some_and(|window| window.frame().contains(x, y))
                    });
                let Some(slot) = hit else { return empty };
                let Some(window) = self.windows[slot] else {
                    return empty;
                };
                if window.close_area().contains(x, y) {
                    let _ = libwoven::message_send(window.owner, b"WCLS");
                    log(&[
                        b"woven-shell: close requested: ",
                        &window.title[..window.title_length],
                    ]);
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
            InputEvent::Key(Key::Escape) => {
                self.cancel_pointer_gesture();
                let Some(slot) = self.focused() else { return empty };
                let Some(window) = self.windows[slot] else { return empty };
                let _ = libwoven::message_send(window.owner, b"WCLS");
                log(&[b"woven-shell: keyboard close requested"]);
                empty
            }
            // Tab is shell-owned outside the sign-in/launcher paths. Cycle
            // the bounded workspace stack without requiring a pointer.
            InputEvent::Key(Key::Tab) => {
                self.cancel_pointer_gesture();
                let panels = self.close_panels();
                let focused = self.stack[..self.stacked]
                    .first()
                    .copied()
                    .map_or(empty, |slot| self.focus(slot));
                panels.union(focused)
            }
            InputEvent::Key(Key::F2) => self.toggle_tiling(),
            InputEvent::Key(Key::F3) => self.switch_workspace((self.workspace + 1) % WORKSPACES),
            InputEvent::Key(Key::F4) => self.move_focused_to_next_workspace(),
            InputEvent::Key(Key::Enter) => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ =
                        libwoven::message_send(window.owner, &[b'W', b'K', b'E', b'Y', KEY_ENTER]);
                }
                empty
            }
            InputEvent::Key(Key::Backspace) => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ = libwoven::message_send(
                        window.owner,
                        &[b'W', b'K', b'E', b'Y', KEY_BACKSPACE],
                    );
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

    /// Update pointer state without painting a bounding box across its path.
    /// The event loop damages the old/new cursor rectangles separately.
    fn move_pointer_to(&mut self, x: i32, y: i32) -> Area {
        self.pointer = (
            x.clamp(0, self.desktop_width - 1),
            y.clamp(0, self.height - 1),
        );
        if self.locked {
            return Area::new(0, 0, 0, 0);
        }
        self.pointer_visible = true;
        let moved = self.drag_to_pointer();
        if self.drag.is_none() {
            self.forward_pointer_move();
        }
        moved
    }

    /// End the current gesture before hiding its workspace or locking.
    /// Deliver exactly one release while the owner is still visible; a later
    /// physical release must not reach a hidden client or a reused slot.
    fn cancel_pointer_gesture(&mut self) {
        if let Some(slot) = self.pointer_capture.take() {
            if let Some(window) = self.windows[slot] {
                // Both messages use the same FIFO. If cancellation cannot be
                // queued, do not turn an aborted gesture into a client action.
                if !window.cancellation_protocol
                    || libwoven::message_send(window.owner, b"WCAN").is_ok()
                {
                    self.forward_pointer_button(slot, false);
                }
            }
        }
        self.drag = None;
    }

    /// Captured motion stays with its owner even outside the window. Without
    /// capture, the topmost frame (including chrome) blocks windows below.
    fn forward_pointer_move(&self) {
        let (x, y) = self.pointer;
        let captured = self.pointer_capture.filter(|&slot| {
            self.windows[slot].is_some_and(|window| window.workspace == self.workspace)
        });
        let slot = if let Some(slot) = captured {
            Some(slot)
        } else if self.overview_area().contains(x, y)
            || self.qs_area().contains(x, y)
            || self.center_area().contains(x, y)
        {
            None
        } else {
            self.stack[..self.stacked].iter().rev().copied()
                .find(|&slot| self.windows[slot].is_some_and(|window| window.frame().contains(x, y)))
                .filter(|&slot| self.windows[slot].is_some_and(|window| window.body().contains(x, y)))
        };
        let Some(slot) = slot else { return };
        let Some(window) = self.windows[slot] else {
            return;
        };
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
        let Some(window) = self.windows[slot] else {
            return;
        };
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
        let Some((slot, grab_x, grab_y)) = self.drag else {
            return empty;
        };
        // Computed before borrowing `window` mutably below.
        let (area_top, area_height) = self.usable_band();
        let desktop_width = self.desktop_width;
        let pointer = self.pointer;
        let Some(window) = self.windows[slot].as_mut() else {
            return empty;
        };
        let before = window.footprint();
        window.x = (pointer.0 - grab_x).clamp(0, (desktop_width - window.width).max(0));
        window.y = (pointer.1 - grab_y).clamp(
            area_top + TITLE_HEIGHT,
            (area_top + area_height - window.height).max(area_top + TITLE_HEIGHT),
        );
        let after = window.footprint();
        if after == before {
            empty
        } else {
            before.union(after)
        }
    }

    /// Composite client damage, and drop windows and icons whose surface is
    /// gone. Returns the area to repaint.
    fn check_clients(&mut self) -> Dirty {
        let mut dirty = Dirty::new();
        for slot in 0..MAX_WINDOWS {
            let Some(window) = self.windows[slot] else {
                continue;
            };
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
                    dirty.add(changed.union(window.body()));
                }
                Ok(None) => {}
                // While a resize is outstanding the owner may have released
                // the old surface before its new one arrives.
                Err(_) if libwoven::ticks() < window.resizing_until => {}
                Err(_) => {
                    // Recompose the released footprint, newly focused window,
                    // taskbar and tiling changes as separate regions. Painting
                    // an enclosing full-screen box delays close feedback.
                    for &area in self.close(slot).areas() {
                        dirty.add(area);
                    }
                }
            }
        }
        for index in 0..MAX_TRAY {
            let Some(icon) = self.tray[index] else {
                continue;
            };
            let place = self.tray_area(index);
            match libwoven::compositor_blit_damage(icon.token, place.x, place.y) {
                Ok(Some(_)) => dirty.add(place),
                Ok(None) => {}
                Err(_) => {
                    self.tray[index] = None;
                    log(&[b"woven-shell: tray icon removed"]);
                    dirty.add(place);
                }
            }
        }
        dirty
    }

    /// Whether any window is still fading.
    fn animating(&self) -> bool {
        self.windows
            .iter()
            .flatten()
            .any(|window| !window.fade.is_settled())
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
            dirty = dirty.union(if self.locked {
                self.screen()
            } else {
                self.clock_area()
            });
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| libwoven::ticks() >= toast.until)
        {
            self.toast = None;
            dirty = dirty.union(self.toast_area());
        }
        dirty
    }
}

/// Read up to [`MAX_ACCOUNTS`] accounts from an accounts file into
/// `accounts`, which starts empty; malformed lines are skipped.
fn load_accounts(path: &str, accounts: &mut [Option<libwoven::accounts::Account>; MAX_ACCOUNTS]) {
    let Ok(descriptor) = libwoven::open(path) else {
        return;
    };
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

fn load_accessibility_settings(path: &str, desktop: &mut Desktop) {
    let Ok(descriptor) = libwoven::open(path) else {
        return;
    };
    let mut bytes = [0u8; 512];
    let read = libwoven::read(descriptor, &mut bytes);
    let _ = libwoven::close(descriptor);
    let Ok(length) = read else { return };
    let data = &bytes[..length.min(bytes.len())];
    if let Some(locale) = json_string(data, b"locale")
        .and_then(libwoven::i18n::Locale::parse_tag)
    {
        desktop.language = locale.language;
    }
    let mut settings = libwoven::accessibility::Settings::new();
    settings.apply_json(data);
    if settings.high_contrast {
        THEME_MODE.store(1, Ordering::Relaxed);
    }
    if settings.reduced_motion {
        desktop.reduced_motion = true;
    }
}

/// Read one bounded JSON string field from the installed settings envelope.
/// This is intentionally narrower than a general JSON parser: settings are a
/// kernel-provisioned record and malformed or escaped values are ignored.
fn json_string<'a>(data: &'a [u8], key: &[u8]) -> Option<&'a str> {
    let mut needle = [0u8; 32];
    let length = 2usize.checked_add(key.len())?;
    if length > needle.len() {
        return None;
    }
    needle[0] = b'\"';
    needle[1..1 + key.len()].copy_from_slice(key);
    needle[1 + key.len()] = b'\"';
    let start = data.windows(length).position(|window| window == &needle[..length])?;
    let rest = &data[start + length..];
    let colon = rest.iter().position(|byte| *byte == b':')?;
    let quoted = &rest[colon + 1..];
    if quoted.first().copied()? != b'\"' {
        return None;
    }
    let end = quoted[1..].iter().position(|byte| *byte == b'\"')? + 1;
    core::str::from_utf8(&quoted[1..end]).ok()
}

mod input;
use input::{absolute_pointer_axis, EVENT_BATCH};

fn main(args: Args) -> ! {
    // SAFETY: the only reference to the desktop's state, taken once.
    let desktop = unsafe { &mut *DESKTOP.0.get() };
    let mut index = 1;
    while let Some(argument) = args.get(index) {
        match argument {
            b"--seconds" => desktop.seconds = true,
            b"--reduced-motion" => desktop.reduced_motion = true,
            // The Stage 20.5 contract isolates the flat compositor's window
            // depth/shadow behavior. Production remains 3D by default; this
            // explicit switch keeps that acceptance independent of the later
            // 3D presentation stages.
            b"--no-3d" => desktop.three_d = false,
            b"--high-contrast" => THEME_MODE.store(1, Ordering::Relaxed),
            b"--settings" => {
                index += 1;
                if let Some(path) = args
                    .get(index)
                    .and_then(|path| core::str::from_utf8(path).ok())
                {
                    load_accessibility_settings(path, desktop);
                } else {
                    fail(1);
                }
            }
            b"--pin" => {
                index += 1;
                match args.get(index) {
                    Some(digits)
                        if (1..=MAX_PIN).contains(&digits.len())
                            && digits.iter().all(u8::is_ascii_digit) =>
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
                let path = args
                    .get(index)
                    .and_then(|path| core::str::from_utf8(path).ok());
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
    let (outputs, monitors) =
        libwoven::compositor_outputs().unwrap_or(([libwoven::Output::default(); 4], 0));
    desktop.width = if monitors > 0 {
        (outputs[0].width as i32).clamp(1, width)
    } else {
        width
    };
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
    let mut catalog_digits = [0u8; 20];
    let catalog_count = libwoven::format_u64(desktop.app_count as u64, &mut catalog_digits);
    log(&[b"woven-shell: catalog entries=", catalog_count]);
    // M6: a real, measured full-recompose frame time (not assumed). This
    // first paint is a genuine full-screen recompose by construction, so
    // no extra bookkeeping is needed to pick that case out, unlike the
    // general repaint() entry point this session chose not to instrument
    // internally (multiple early-return paths; measuring from outside a
    // single known-full-screen call avoided touching that control flow).
    let recompose_start = libwoven::ticks();
    desktop.repaint(&mut painter, desktop.screen());
    let recompose_ticks = libwoven::ticks().saturating_sub(recompose_start);
    let mut recompose_digits = [0u8; 20];
    let recompose_text = libwoven::format_u64(recompose_ticks, &mut recompose_digits);
    log(&[
        b"woven-shell: full recompose ticks=",
        recompose_text,
    ]);
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
        // Bound each source so an application flooding its mailbox cannot
        // indefinitely postpone keyboard input, pointer edges or presentation.
        for _ in 0..EVENT_BATCH {
            let Ok((length, sender)) = libwoven::message_receive(&mut message) else {
                break;
            };
            log(&[b"woven-shell: application message received"]);
            for &area in desktop.handle_message(sender, &message[..length]).areas() {
                dirty.add(area);
            }
        }
        for _ in 0..EVENT_BATCH {
            let Ok(Some(event)) = libwoven::input_poll_event() else {
                break;
            };
            let pointer_before = (desktop.pointer, desktop.pointer_visible);
            let cursor_before = desktop.cursor_area();
            dirty.add(desktop.handle_input(event));
            if pointer_before != (desktop.pointer, desktop.pointer_visible) && !desktop.locked {
                dirty.add(cursor_before);
                dirty.add(desktop.cursor_area());
            }
        }
        for &area in desktop.check_clients().areas() {
            dirty.add(area);
        }
        // M7 follow-up (flagged in docs/graphics/overview-desktop-2026-11-03.md's
        // M6 section as a known gap): a real, measured steady-state interactive
        // recompose time, not just the one-off first-paint number M6 captured.
        // Every non-empty dirty batch this loop produces is a genuine
        // input-driven recompose (a keystroke, pointer move, window drag,
        // workspace switch, animation tick, ...), so timing the repaint+present
        // step here gives a real distribution across actual interaction types
        // rather than one cherry-picked scenario, with no new instrumentation
        // point needed per interaction kind.
        let timed = !dirty.areas().is_empty();
        let recompose_start = if timed { libwoven::ticks() } else { 0 };
        for &area in dirty.areas() {
            desktop.repaint(&mut painter, area);
        }
        let repaint_ticks = if timed {
            libwoven::ticks().saturating_sub(recompose_start)
        } else {
            0
        };
        let present_start = if timed { libwoven::ticks() } else { 0 };
        if libwoven::compositor_present().is_err() {
            fail(7);
        }
        if timed {
            let present_ticks = libwoven::ticks().saturating_sub(present_start);
            let elapsed = repaint_ticks.saturating_add(present_ticks);
            let mut digits = [0u8; 20];
            log(&[
                b"woven-shell: interactive recompose ticks=",
                libwoven::format_u64(elapsed, &mut digits),
            ]);
            let mut repaint_digits = [0u8; 20];
            log(&[
                b"woven-shell: repaint ticks=",
                libwoven::format_u64(repaint_ticks, &mut repaint_digits),
            ]);
            let mut present_digits = [0u8; 20];
            log(&[
                b"woven-shell: present ticks=",
                libwoven::format_u64(present_ticks, &mut present_digits),
            ]);
            if let Ok(metrics) = libwoven::process_metrics() {
                let mut abi_digits = [0u8; 20];
                let mut cpu_digits = [0u8; 20];
                let mut cpus_digits = [0u8; 20];
                let mut runtime_digits = [0u8; 20];
                let mut mapped_digits = [0u8; 20];
                log(&[
                    b"woven-shell: process metrics abi=",
                    libwoven::format_u64(metrics.abi_version as u64, &mut abi_digits),
                    b" cpu=",
                    libwoven::format_u64(metrics.cpu as u64, &mut cpu_digits),
                    b" cpus=",
                    libwoven::format_u64(metrics.cpu_count as u64, &mut cpus_digits),
                    b" runtime_ticks=",
                    libwoven::format_u64(metrics.runtime_ticks, &mut runtime_digits),
                    b" mapped_bytes=",
                    libwoven::format_u64(metrics.mapped_bytes, &mut mapped_digits),
                ]);
            }
            if let Ok(metrics) = libwoven::system_metrics() {
                let mut abi_digits = [0u8; 20];
                let mut cpus_digits = [0u8; 20];
                let mut timer_digits = [0u8; 20];
                let mut runtime_digits = [0u8; 20];
                let mut frames_digits = [0u8; 20];
                let mut remaining_digits = [0u8; 20];
                let mut heap_digits = [0u8; 20];
                log(&[
                    b"woven-shell: system metrics abi=",
                    libwoven::format_u64(metrics.abi_version as u64, &mut abi_digits),
                    b" cpus=",
                    libwoven::format_u64(metrics.cpu_count as u64, &mut cpus_digits),
                    b" timer_ticks=",
                    libwoven::format_u64(metrics.timer_ticks, &mut timer_digits),
                    b" cpu_runtime_ticks=",
                    libwoven::format_u64(metrics.cpu_runtime_ticks, &mut runtime_digits),
                    b" allocated_frames=",
                    libwoven::format_u64(metrics.allocated_frames, &mut frames_digits),
                    b" remaining_frames=",
                    libwoven::format_u64(metrics.remaining_frames, &mut remaining_digits),
                    b" heap_bytes=",
                    libwoven::format_u64(metrics.heap_allocated_bytes, &mut heap_digits),
                ]);
            }
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
