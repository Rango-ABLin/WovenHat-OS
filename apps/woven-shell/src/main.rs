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

use libwoven::design_system::{SemanticPalette, TypeRole};
use libwoven::graphics::{
    Camera, Color, DepthBuffer, Rect, Renderer, Vertex3, Vertex3Field, Vertex3Tex,
};
use libwoven::layout::{self, Frame};
use libwoven::semantic_search;
use libwoven::ui::DesignTokens;
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
use icons::{app_icon, draw_app_icon, draw_app_icon_light};
mod status;
use status::{BatteryStatus, NetworkStatus, StatusSnapshot, VolumeStatus};
mod taskbar_prefs;
use taskbar_prefs::{Alignment, AutoHide, IconSize, TaskbarPrefs, AUTO_HIDE_HOLD};
mod pinned_apps;
use pinned_apps::{PinnedApps, PinError, MAX_PINNED as PIN_CAP};

/// One taskbar group entry: a window on the current workspace, or a pinned
/// catalog app that is not running on it. A pinned app with a running window
/// shows that window's entry at the pin's position.
#[derive(Clone, Copy)]
enum TaskbarEntry {
    /// Window slot.
    Window(usize),
    /// Catalog app index.
    PinnedApp(usize),
}

/// Bounded system-status indicators in the taskbar tray. The module only
/// reads existing status calls and never adds a capability; see `status.rs`.
/// Quick Settings presents the same sampled values in its own panel.
const STATUS_ICONS: usize = 3;
const STATUS_SAMPLE_PERIOD: u64 = 2 * libwoven::time::TICKS_PER_SECOND;

/// Notifications remembered for the Notification Center.
const MAX_NOTES: usize = 8;
const CENTER_WIDTH: i32 = 220;
const CENTER_TITLE: &str = "Notifications";
const CENTER_CLEAR: &str = "Clear";
const QS_TITLE: &str = "Quick Settings";
/// Quick Settings panel, Win11-style: action grid + sliders + footer.
const QS_WIDTH: i32 = 360;
const QS_PADDING: i32 = 12;
const QS_GRID_COLS: usize = 3;
const QS_GRID_GAP: i32 = 8;
const QS_BUTTON_HEIGHT: i32 = 48;
const QS_BUTTON_RADIUS: usize = 10;
const QS_SLIDER_HEIGHT: i32 = 34;
const QS_SLIDER_GAP: i32 = 10;
const QS_SECTION_GAP: i32 = 8;
const QS_FOOTER_HEIGHT: i32 = 40;
/// Grid rows shown in the quick-action panel. Five rows hold the combined
/// action set from both reconciled Phase B branches (one spare cell).
const QS_GRID_ROWS: usize = 5;
/// Both branches' controls are retained: honest radios/power/display plus the
/// taskbar layout controls. The final grid cell is intentionally unused.
const QS_ACTIONS: usize = 14;

#[derive(Clone, Copy, PartialEq, Eq)]
enum QsAction {
    Wifi,
    Bluetooth,
    Airplane,
    BatterySaver,
    TaskbarAlign,
    FocusAssist,
    NightLight,
    TaskbarIconSize,
    Accessibility,
    DisplayScale,
    Tiling,
    ThreeD,
    Language,
    ChangePin,
}

const QS_ACTION_ORDER: [QsAction; QS_ACTIONS] = [
    QsAction::Wifi,
    QsAction::Bluetooth,
    QsAction::Airplane,
    QsAction::BatterySaver,
    QsAction::TaskbarAlign,
    QsAction::FocusAssist,
    QsAction::NightLight,
    QsAction::TaskbarIconSize,
    QsAction::Accessibility,
    QsAction::DisplayScale,
    QsAction::Tiling,
    QsAction::ThreeD,
    QsAction::Language,
    QsAction::ChangePin,
];

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
            ("pinned", "Pinned"),
            ("all_apps", "All apps"),
            ("network", "Network"),
            ("volume", "Volume"),
            ("battery", "Battery"),
            ("online", "Connected"),
            ("offline", "Offline"),
            ("unavailable", "Unavailable"),
            ("muted", "Muted"),
            ("not_available", "Not available"),
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
            ("pinned", "Zokhazikika"),
            ("all_apps", "Mapulogalamu onse"),
            ("network", "Netiweki"),
            ("volume", "Voliyumu"),
            ("battery", "Batri"),
            ("online", "Yalumikizana"),
            ("offline", "Siyalumikizana"),
            ("unavailable", "Sikupezeka"),
            ("muted", "Yachetsetsedwa"),
            ("not_available", "Sikupezeka"),
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
            ("pinned", "مثبت"),
            ("all_apps", "كل التطبيقات"),
            ("network", "الشبكة"),
            ("volume", "الصوت"),
            ("battery", "البطارية"),
            ("online", "متصل"),
            ("offline", "غير متصل"),
            ("unavailable", "غير متاح"),
            ("muted", "مكتوم"),
            ("not_available", "غير متاح"),
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
fn label(bytes: &[u8], limit: usize, buffer: &mut [u8]) -> usize {
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

/// `prefix + name` for a pin/unpin toast, truncated to the caller's buffer.
/// Catalog names are bounded ASCII, so this never escapes the buffer.
fn pin_text<'a>(prefix: &[u8], name: &[u8], buffer: &'a mut [u8; 48]) -> &'a str {
    let length = (prefix.len() + name.len()).min(buffer.len());
    buffer[..prefix.len()].copy_from_slice(prefix);
    buffer[prefix.len()..length].copy_from_slice(&name[..length - prefix.len()]);
    core::str::from_utf8(&buffer[..length]).unwrap_or("Pinned")
}

/// `bytes` as text, cut to the longest prefix that fits `max_width` at the
/// body size, with the same printable-ASCII substitution as [`label`].
fn truncate_to_width(bytes: &[u8], max_width: i32, buffer: &mut [u8; 64]) -> usize {
    let mut length = bytes.len().min(buffer.len());
    while length > 0 {
        let Ok(candidate) = core::str::from_utf8(&bytes[..length]) else {
            length -= 1;
            continue;
        };
        if libwoven::ui_text_width(candidate, BODY_TEXT) <= max_width {
            break;
        }
        length -= 1;
    }
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

/// The shared type scale. Sizes come from `libwoven::ui::DesignTokens` so the
/// shell's sign-in, taskbar, and window chrome agree with every WovenUI app.
const TOKENS: DesignTokens = DesignTokens::DEFAULT;
const BODY_TEXT: u16 = TOKENS.type_body;

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
fn fill_rounded_blended(
    canvas: &mut Canvas<'_>,
    area: Area,
    radius: usize,
    color: u32,
    opacity: u8,
) {
    Renderer::new(canvas).rounded_rect_blended(rect_of(area), radius, color_of(color), opacity);
}

/// A filled circle of `diameter` centred at `(center_x, center_y)`. A circle
/// is a square rounded rectangle whose radius is half its side, so this
/// reuses `rounded_rect` rather than adding a second corner-distance
/// implementation.
fn fill_circle(canvas: &mut Canvas<'_>, center_x: i32, center_y: i32, diameter: i32, color: u32) {
    let area = Area::new(
        center_x - diameter / 2,
        center_y - diameter / 2,
        diameter,
        diameter,
    );
    fill_rounded(canvas, area, (diameter / 2).max(0) as usize, color);
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
use model::{load_catalog, matches, App, Child, Note, Toast, TrayIcon, Window};

mod painter;
use painter::Painter;

/// The desktop taskbar's launcher affordance. Opens the Start overview:
/// search, current-workspace windows, and the application grid.
const START_LABEL: &str = "Start";
const START_BUTTON_WIDTH: i32 = 58;
const START_ICON_SIZE: i32 = 14;
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
    /// Bottom taskbar placement and entry size, from the installed settings.
    taskbar: TaskbarPrefs,
    /// Taskbar auto-hide state: the settings-seeded preference plus its
    /// reveal/hide timing. In-session state like pins: the shell holds
    /// `FileRead` and no settings-write path exists, so it resets to the
    /// record's value on the next boot.
    auto_hide: AutoHide,
    /// Bounded pinned catalog names, in pin order. Seeded from the installed
    /// settings record's `pinned_apps` value (absent: the curated defaults;
    /// empty: nothing pinned) and changed in-session by right-clicking a
    /// taskbar entry. The same list drives the Overview Dash. In-session state
    /// only: the shell process holds `FileRead` and no settings-write path
    /// exists, so pins honestly reset to the record's list on the next boot.
    pinned: PinnedApps,
    /// Brightness percentage for the quick-settings slider (0-100).
    brightness: u8,
    /// The display scale the shell set.
    scale: u32,
    /// Last sampled network/volume/battery state behind the tray indicators.
    status: StatusSnapshot,
    /// Tick at which the next bounded status re-sample is due.
    status_next_sample: u64,
    /// Installed settings path supplied by `--settings`, used for bounded
    /// live re-reads. The shell has `FileRead` only and never writes it.
    settings_path: [u8; 256],
    settings_path_len: usize,
    /// Last observed settings digest and the next bounded re-read tick.
    settings_hash: u64,
    settings_next_sample: u64,
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
    /// The first overview frame publishes the panel and search field; the
    /// app grid follows on the next frame so a cold paint cannot stall input.
    overview_detail: bool,
    query: [u8; MAX_QUERY],
    query_length: usize,
    /// Bounded hit list from the semantic search service, mapped to catalog
    /// indices. Empty means the service was unavailable or the query is empty.
    semantic_indices: [usize; MAX_APPS],
    semantic_count: usize,
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
        taskbar: TaskbarPrefs::DEFAULT,
        auto_hide: AutoHide::off(),
        pinned: PinnedApps::curated(),
        brightness: 80,
        scale: 1,
        status: StatusSnapshot {
            network: NetworkStatus::Unavailable,
            volume: VolumeStatus::Unavailable,
            battery: BatteryStatus::Unavailable,
        },
        status_next_sample: 0,
        settings_path: [0; 256],
        settings_path_len: 0,
        settings_hash: 0,
        settings_next_sample: 0,
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
        overview_detail: false,
        query: [0; MAX_QUERY],
        query_length: 0,
        semantic_indices: [0; MAX_APPS],
        semantic_count: 0,
        selected: 0,
        children: [None; MAX_CHILDREN],
    };

    fn screen(&self) -> Area {
        Area::new(0, 0, self.desktop_width, self.height)
    }

    /// The sign-in card: one bounded region that owns every lock-screen
    /// element (avatar, title, account pills, prompt, entry dots). All lock
    /// damage is unioned with this rect because the content is re-laid-out
    /// per state change, not incrementally repainted.
    fn lock_card_area(&self) -> Area {
        let middle = self.height / 2;
        self.screen()
            .intersect(Area::new(self.width / 2 - 160, middle - 132, 320, 210))
    }

    /// The lock-screen account rows and PIN prompt are the only pixels that
    /// change while choosing an account. Keeping this damage bounded avoids
    /// repainting the full 1280x800 wallpaper for every key on SMP boots.
    fn lock_account_area(&self) -> Area {
        self.lock_card_area()
    }

    /// The prompt and masked PIN dots change independently of the wallpaper.
    fn lock_input_area(&self) -> Area {
        self.lock_card_area()
    }

    /// The taskbar sits flush at the bottom edge of the screen. Keep this as a
    /// function so every layout calculation shares one chrome origin.
    fn taskbar_top(&self) -> i32 {
        (self.height - TASKBAR_HEIGHT).max(0)
    }

    fn taskbar(&self) -> Area {
        Area::new(0, self.taskbar_top(), self.width, TASKBAR_HEIGHT)
    }

    /// The screen's usable band for windows: everything when the bar is
    /// hidden, everything above the bar when it is not. Shared by every
    /// window-placement/clamp site (`work_area`, `open`, `resized`,
    /// `toggle_scale`, `drag_to_pointer`); the revealed bar simply draws
    /// over whatever a window placed in the freed strip.
    fn usable_band(&self) -> (i32, i32) {
        let height = if self.auto_hide.is_hidden() {
            self.height
        } else {
            (self.height - TASKBAR_HEIGHT).max(0)
        };
        (0, height)
    }

    /// Row for taskbar text, centered vertically in the bar.
    fn text_y(&self) -> i32 {
        self.taskbar_top() + (TASKBAR_HEIGHT - font::WIDTH as i32) / 2
    }

    /// Right side of the taskbar (Win11-style).
    fn clock_area(&self) -> Area {
        let x = (self.width - CLOCK_WIDTH - 4).max(0);
        Area::new(x, self.taskbar_top(), CLOCK_WIDTH, TASKBAR_HEIGHT)
    }

    fn tray_area(&self, index: usize) -> Area {
        let x = self.clock_area().x - (index as i32 + 1) * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    /// Bottom-right, floating above the taskbar (Win11-style).
    fn toast_area(&self) -> Area {
        Area::new(
            (self.width - TOAST_WIDTH - TOAST_MARGIN).max(0),
            (self.taskbar_top() - TOAST_HEIGHT - TOAST_MARGIN).max(0),
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

    /// Ask the bounded `woven.search` broker for application hits and cache
    /// them as catalog indices. A missing service or malformed reply simply
    /// leaves the semantic list empty so `filtered` keeps the local fallback.
    fn refresh_semantic_search(&mut self) {
        self.semantic_count = 0;
        if self.query_length == 0 {
            return;
        }
        let Ok(query) = core::str::from_utf8(&self.query[..self.query_length]) else {
            return;
        };
        let Ok(reply) = libwoven::search(query, 1) else {
            return;
        };
        for hit in reply.hits[..reply.hit_count.min(semantic_search::MAX_HITS as u8) as usize].iter()
        {
            if hit.kind != semantic_search::Kind::Application
                || hit.id == 0
                || hit.id as usize > self.app_count
            {
                continue;
            }
            let index = hit.id as usize - 1;
            if self.semantic_count < MAX_APPS
                && !self.semantic_indices[..self.semantic_count].contains(&index)
            {
                self.semantic_indices[self.semantic_count] = index;
                self.semantic_count += 1;
            }
        }
        // Preserve the launcher's established catalog order: the service
        // ranks by score, but keyboard/pointer acceptance expects the same
        // grid position a substring filter would have produced.
        for outer in 1..self.semantic_count {
            let mut cursor = outer;
            while cursor > 0 && self.semantic_indices[cursor] < self.semantic_indices[cursor - 1] {
                self.semantic_indices.swap(cursor, cursor - 1);
                cursor -= 1;
            }
        }
        if self.semantic_count > 0 {
            log(&[b"woven-shell: semantic search hits"]);
        }
    }

    /// Catalog indices matching the query, in catalog order.
    fn filtered(&self) -> ([usize; MAX_APPS], usize) {
        let mut found = [0; MAX_APPS];
        let mut count = 0;
        if self.semantic_count > 0 {
            for &index in &self.semantic_indices[..self.semantic_count] {
                if self.apps[index].is_some() && count < MAX_APPS {
                    found[count] = index;
                    count += 1;
                }
            }
            return (found, count);
        }
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

    fn taskbar_entry_width(&self) -> i32 {
        self.taskbar.icon_size.entry_width()
    }

    /// Win11-style application group: Start + pinned/running entries. It is
    /// centred by default, or pinned to the left edge when the preference says
    /// so. The count is the same combined list the painter and hit tests use,
    /// so centering cannot disagree with what is actually on screen.
    fn taskbar_group_width(&self) -> i32 {
        let (_, count) = self.taskbar_entries();
        let entries = count as i32;
        let entries_width = if entries > 0 {
            8 + entries * self.taskbar_entry_width() + (entries - 1).max(0) * TASKBAR_ENTRY_GAP
        } else {
            0
        };
        START_BUTTON_WIDTH + entries_width
    }

    fn start_area(&self) -> Area {
        let width = self.taskbar_group_width();
        // The group may not run under the right-side controls (workspace
        // boxes, tray, status icons, clock). Centre it inside the free band
        // between the left margin and those controls -- not the whole screen,
        // which the pinned defaults already fill past their left edge. When
        // the group cannot fit even there, clamp it to the left margin so
        // only the excess, never the whole group, overlaps.
        let controls_left = self.workspace_area(0).x - 8;
        let left = match self.taskbar.alignment {
            Alignment::Center => {
                let free = (controls_left - 8).max(0);
                (8 + free / 2 - width / 2).clamp(8, (controls_left - width).max(8))
            }
            Alignment::Left => 8,
        };
        Area::new(
            left.max(4),
            self.taskbar_top() + 4,
            START_BUTTON_WIDTH,
            START_HEIGHT as i32,
        )
    }

    fn taskbar_entry_area(&self, index: usize) -> Area {
        let start = self.start_area();
        let width = self.taskbar_entry_width();
        let x = start.x + start.width + 8 + index as i32 * (width + TASKBAR_ENTRY_GAP);
        Area::new(x, self.taskbar_top() + 4, width, START_HEIGHT as i32)
    }

    /// The full-screen launcher has no projected halo; its bounded overview
    /// area is the complete damage region.
    fn panel_damage_area(&self) -> Area {
        self.overview_area()
    }

    /// Win11-style floating Start panel, centered above the taskbar.
    /// Empty unless the launcher is open.
    fn overview_area(&self) -> Area {
        if !self.launcher_open {
            return Area::new(0, 0, 0, 0);
        }
        let width = (self.width - 48).clamp(320, 700);
        let max_height = (self.taskbar_top() - 24).max(240);
        let height = max_height.clamp(240, 560);
        let x = (self.width - width) / 2;
        let y = (self.taskbar_top() - height - 8).max(8);
        Area::new(x, y, width, height)
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

    /// Top of the Dash row, below the "Pinned" label.
    fn overview_dash_row_top(&self) -> i32 {
        self.overview_search_area().y + OVERVIEW_SEARCH_HEIGHT + OVERVIEW_MARGIN + 14
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

    /// Pinned catalog names from the installed settings record first, then
    /// any other app with a running instance ([`Self::children`]) not already
    /// pinned, deduplicated and bounded to `DASH_MAX`. A pin whose package is
    /// absent is skipped rather than fabricated.
    fn dash_apps(&self) -> ([usize; DASH_MAX], usize) {
        let mut apps = [0usize; DASH_MAX];
        let mut count = 0;
        for index in 0..self.pinned.len() {
            if count >= DASH_MAX {
                break;
            }
            if let Some(name) = self.pinned.get(index) {
                if let Some(app_index) = self.app_by_name(name.as_bytes()) {
                    apps[count] = app_index;
                    count += 1;
                }
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
            overview.x
                + OVERVIEW_MARGIN
                + index as i32 * (OVERVIEW_THUMB_WIDTH + OVERVIEW_THUMB_GAP),
            self.overview_thumb_row_top(),
            OVERVIEW_THUMB_WIDTH,
            OVERVIEW_THUMB_HEIGHT,
        )
    }

    /// Top of the app grid, below the window cards (if any) and "All apps" label.
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
            + 14
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
        self.overview_detail = !self.launcher_open;
        self.query_length = 0;
        self.semantic_count = 0;
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
        self.toggle_launcher().union(self.start_app(index))
    }

    /// Start catalog entry `index` without touching the launcher: the
    /// taskbar's pinned entries call this directly, so clicking a pin does
    /// not open the Start panel. A failure is reported in a toast.
    /// Returns the area to repaint.
    fn start_app(&mut self, index: usize) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        let Some(app) = self.apps[index] else {
            return empty;
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
            let mut text = [0u8; TOAST_CHARS];
            let shown = label(&message[..length], TOAST_CHARS, &mut text);
            self.toast = Some(Toast {
                text,
                length: shown,
                until: libwoven::ticks() + TOAST_TICKS,
            });
            log(&[b"woven-shell: could not start ", app.name()]);
            return empty.union(self.toast_area());
        }
        empty
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
                self.refresh_semantic_search();
                self.log_search_latency();
            }
            Key::Backspace => {
                self.query_length = self.query_length.saturating_sub(1);
                self.selected = 0;
                self.refresh_semantic_search();
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

    /// Catalog index of the app named `name`, if the catalog lists it.
    fn app_index(&self, name: &[u8]) -> Option<usize> {
        self.apps[..self.app_count]
            .iter()
            .position(|app| app.is_some_and(|app| app.name() == name))
    }

    /// Catalog index of the app that owns window `slot`, when the shell
    /// launched it. Windows whose owner the shell did not spawn have no
    /// app, and are not pinnable.
    fn window_app(&self, slot: usize) -> Option<usize> {
        let owner = self.windows.get(slot)?.as_ref()?.owner;
        self.children
            .iter()
            .flatten()
            .find(|child| child.pid == owner)
            .map(|child| child.app)
    }

    /// The taskbar's application group, in display order: pinned apps in
    /// pin order first -- a pinned app with a window on this workspace
    /// shows that window's entry at the pin's position -- then every other
    /// window in slot order. A pin whose name is not in the catalog is
    /// not shown: a missing package leaves the pin inert, the same
    /// contract as the desktop shortcuts. Bounded by
    /// `MAX_PINNED` + `MAX_WINDOWS` entries.
    fn taskbar_entries(&self) -> ([TaskbarEntry; PIN_CAP + MAX_WINDOWS], usize) {
        let mut entries = [TaskbarEntry::Window(0); PIN_CAP + MAX_WINDOWS];
        let mut claimed = [false; MAX_WINDOWS];
        let mut count = 0;
        for pin in 0..self.pinned.len() {
            let Some(name) = self.pinned.get(pin) else {
                continue;
            };
            let Some(app) = self.app_index(name.as_bytes()) else {
                continue;
            };
            let claimed_slot = self.entries().find_map(|(slot, _)| {
                (!claimed[slot] && self.window_app(slot) == Some(app)).then_some(slot)
            });
            match claimed_slot {
                Some(slot) => {
                    claimed[slot] = true;
                    entries[count] = TaskbarEntry::Window(slot);
                }
                None => entries[count] = TaskbarEntry::PinnedApp(app),
            }
            count += 1;
        }
        for (slot, _) in self.entries() {
            if !claimed[slot] {
                entries[count] = TaskbarEntry::Window(slot);
                count += 1;
            }
        }
        (entries, count)
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
        self.app_by_name(name)
    }

    /// Resolve a catalog entry by exact name. Pinned names come from the
    /// installed settings record, so an unknown or removed package simply has
    /// no Dash entry instead of being fabricated.
    fn app_by_name(&self, name: &[u8]) -> Option<usize> {
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
        if !self.auto_hide.is_hidden() {
            self.paint_taskbar_content(canvas);
        }
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
        // Win11-like subtle top edge: a faint stroke separates chrome from the
        // desktop without the old heavy accent line.
        canvas.blend_rect(0, top, width, 1, theme_stroke(), 28);

        let start = self.start_area();
        let catalog = shell_catalog(self.language);
        let start_hover = !self.launcher_open && start.contains(self.pointer.0, self.pointer.1);
        let fill = if self.launcher_open {
            theme_accent()
        } else if start_hover {
            lighten(theme_start_fill(), 10)
        } else {
            theme_start_fill()
        };
        let fill_top = if self.launcher_open {
            lighten(fill, 18)
        } else {
            lighten(fill, 28)
        };
        Renderer::new(canvas).rounded_rect_gradient(
            rect_of(start),
            CHROME_RADIUS,
            color_of(fill_top),
            color_of(fill),
        );
        // Subtle highlight stroke gives the Start pill a Win11-like surface edge.
        outline(canvas, start, blend(theme_stroke(), fill, 48));
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
            fill_rounded(
                canvas,
                Area::new(icon_left + dx, icon_top + dy, 6, 6),
                1,
                color,
            );
        }

        let (taskbar_entries, entry_count) = self.taskbar_entries();
        for (index, entry) in taskbar_entries[..entry_count].iter().enumerate() {
            let area = self.taskbar_entry_area(index);
            match entry {
                TaskbarEntry::Window(slot) => {
                    let slot = *slot;
                    let active = self.focused() == Some(slot);
                    let minimized = self.is_minimized(slot);
                    let hovered =
                        !active && !minimized && area.contains(self.pointer.0, self.pointer.1);
                    let fill = if active {
                        lighten(theme_entry(true), 12)
                    } else if minimized {
                        blend(theme_entry(false), theme_taskbar(), 90)
                    } else if hovered {
                        lighten(theme_entry(false), 8)
                    } else {
                        theme_entry(false)
                    };
                    fill_rounded(canvas, area, CHROME_RADIUS_SMALL, fill);
                    // Win11-style running/active indicator: a wide accent bar for the
                    // focused window, a short muted bar for a running one, and a short
                    // dim bar for a minimized one.
                    let (indicator_w, indicator_color) = if active {
                        (16, theme_accent())
                    } else if minimized {
                        (6, blend(theme_muted(), theme_taskbar(), 100))
                    } else {
                        (6, theme_muted())
                    };
                    let indicator_x = area.x + (area.width - indicator_w) / 2;
                    let indicator_y = area.y + area.height - 3;
                    fill_rounded(
                        canvas,
                        Area::new(indicator_x, indicator_y, indicator_w, 3),
                        1,
                        indicator_color,
                    );
                    let Some(window) = self.windows[slot].as_ref() else {
                        continue;
                    };
                    draw_app_icon(
                        canvas,
                        area.x + 2,
                        area.y + 1,
                        app_icon(&window.title[..window.title_length]),
                        14,
                        minimized,
                    );
                    let mut title = [0u8; 32];
                    let length = label(&window.title[..window.title_length], 5, &mut title);
                    let text = core::str::from_utf8(&title[..length]).unwrap_or("");
                    let ink = if minimized {
                        blend(theme_text(), theme_taskbar(), 110)
                    } else {
                        theme_text()
                    };
                    canvas.draw_text_system(area.x + 18, self.text_y(), text, ink);
                }
                TaskbarEntry::PinnedApp(app) => {
                    // A pinned app with no window on this workspace. Same
                    // tile treatment as a running entry, but no running
                    // indicator: being in the group at all is the pin.
                    let Some(app) = self.apps[*app] else {
                        continue;
                    };
                    let hovered = area.contains(self.pointer.0, self.pointer.1);
                    fill_rounded(
                        canvas,
                        area,
                        CHROME_RADIUS_SMALL,
                        if hovered {
                            lighten(theme_entry(false), 8)
                        } else {
                            theme_entry(false)
                        },
                    );
                    draw_app_icon(canvas, area.x + 2, area.y + 1, app_icon(app.name()), 14, false);
                    let shown = app
                        .name()
                        .strip_prefix(b"Woven")
                        .unwrap_or_else(|| app.name());
                    let mut title = [0u8; 32];
                    let length = label(shown, 5, &mut title);
                    let text = core::str::from_utf8(&title[..length]).unwrap_or("");
                    canvas.draw_text_system(area.x + 18, self.text_y(), text, theme_text());
                }
            }
        }

        let (bytes, length) = clock_text(self.time, self.seconds);
        let text = core::str::from_utf8(&bytes[..length]).unwrap_or("");
        let clock = self.clock_area();
        let clock_active = self.center_open;
        let clock_hover = !clock_active && clock.contains(self.pointer.0, self.pointer.1);
        if clock_active || clock_hover {
            fill_rounded(
                canvas,
                clock,
                CHROME_RADIUS_SMALL,
                if clock_hover {
                    lighten(theme_entry(false), 8)
                } else {
                    theme_entry(true)
                },
            );
        }
        let clock_x = clock.x + (clock.width - font::text_width(text) as i32).max(0) / 2;
        canvas.draw_text_system(clock_x, self.text_y(), text, theme_text());

        // The workspace switcher sits left of the system menu button.
        for index in 0..WORKSPACES {
            let area = self.workspace_area(index);
            let active = index == self.workspace;
            let hovered = !active && area.contains(self.pointer.0, self.pointer.1);
            let fill = if hovered {
                lighten(theme_entry(false), 8)
            } else {
                theme_entry(active)
            };
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
        let qs_hover = !self.qs_open && button.contains(self.pointer.0, self.pointer.1);
        let fill = if qs_hover {
            lighten(theme_entry(false), 8)
        } else {
            theme_entry(self.qs_open)
        };
        fill_rounded(canvas, button, CHROME_RADIUS_SMALL, fill);
        for row in 0..3 {
            canvas.fill_rect(button.x + 4, button.y + 4 + row * 3, 8, 1, theme_text());
        }

        for index in 0..MAX_TRAY {
            if self.tray[index].is_some() {
                let place = self.tray_area(index);
                if place.contains(self.pointer.0, self.pointer.1) {
                    fill_rounded(
                        canvas,
                        place,
                        CHROME_RADIUS_SMALL,
                        lighten(theme_entry(false), 8),
                    );
                }
            }
        }

        let status = self.status_icons_area();
        if !self.qs_open && status.contains(self.pointer.0, self.pointer.1) {
            fill_rounded(
                canvas,
                status,
                CHROME_RADIUS_SMALL,
                lighten(theme_entry(false), 8),
            );
        }

        // System status glyphs: network, volume, battery. Drawn in the same
        // bounded canvas as the rest of the taskbar, never from a client
        // surface, so no application can spoof or own them.
        for index in 0..STATUS_ICONS {
            let place = self.status_area(index);
            let ink = theme_accent();
            let dim = theme_muted();
            match index {
                0 => status::draw_network(canvas, place.x, place.y, self.status.network, ink, dim),
                1 => status::draw_volume(canvas, place.x, place.y, self.status.volume, ink, dim),
                _ => status::draw_battery(
                    canvas,
                    place.x,
                    place.y,
                    self.status.battery,
                    theme_taskbar(),
                    ink,
                    dim,
                ),
            }
        }

        if self.unread > 0 {
            let badge = self.badge_area();
            let hovered = badge.contains(self.pointer.0, self.pointer.1);
            let fill = if hovered {
                lighten(theme_accent(), 14)
            } else {
                theme_accent()
            };
            fill_rounded(canvas, badge, CHROME_RADIUS_SMALL, fill);
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
        fill_rounded_blended(canvas, overview, CHROME_CARD_RADIUS, theme_panel(), 240);
        outline(canvas, overview, blend(theme_stroke(), theme_panel(), 36));
        // The shell paints in small tiles and calls this for every tile. Skip
        // items whose own footprint misses the tile: the canvas would clip
        // their pixels anyway, but redrawing every icon and label in all ~76
        // tiles made the first overview frame take several seconds on TCG.
        let visible = Area::new(
            canvas.x(),
            canvas.y(),
            canvas.width() as i32,
            canvas.height() as i32,
        );

        // Search field.
        let catalog = shell_catalog(self.language);
        let search = self.overview_search_area();
        if !visible.intersect(search).is_empty() {
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
        }

        // Pinned / favorites section.
        canvas.draw_text_system(
            overview.x + OVERVIEW_MARGIN,
            self.overview_search_area().y + OVERVIEW_SEARCH_HEIGHT + 4,
            catalog.lookup("pinned", "Pinned"),
            theme_muted(),
        );
        let (dash_apps, dash_count) = self.dash_apps();
        for (index, &app_index) in dash_apps[..dash_count].iter().enumerate() {
            let Some(app) = self.apps[app_index] else {
                continue;
            };
            let area = self.overview_dash_area(index);
            if visible.intersect(area).is_empty() {
                continue;
            }
            if area.contains(self.pointer.0, self.pointer.1) {
                fill_rounded(
                    canvas,
                    Area::new(area.x - 2, area.y - 2, area.width + 4, area.height + 4),
                    CHROME_RADIUS_SMALL,
                    lighten(theme_taskbar(), 10),
                );
            }
            draw_app_icon_light(
                canvas,
                area.x,
                area.y,
                app_icon(app.name()),
                OVERVIEW_DASH_ICON,
            );
        }

        // The first frame only publishes the panel and search field. The
        // window cards and app grid are filled on the following frame.
        if self.overview_detail {
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
                    if visible.intersect(card).is_empty() {
                        continue;
                    }
                    let hovered = card.contains(self.pointer.0, self.pointer.1);
                    fill_rounded(
                        canvas,
                        card,
                        CHROME_RADIUS_SMALL,
                        if hovered {
                            lighten(theme_entry(false), 10)
                        } else {
                            theme_entry(false)
                        },
                    );
                    outline(canvas, card, theme_accent());
                    let mut buffer = [0u8; 32];
                    let length = label(
                        &window.title[..window.title_length],
                        ((card.width - 8) / font::ADVANCE) as usize,
                        &mut buffer,
                    );
                    let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
                    canvas.draw_text_system(
                        card.x + 4,
                        card.y + card.height / 2 - 3,
                        text,
                        theme_text(),
                    );
                }
            }

            // All apps section.
            canvas.draw_text_system(
                overview.x + OVERVIEW_MARGIN,
                self.overview_grid_top() - 14,
                catalog.lookup("all_apps", "All apps"),
                theme_muted(),
            );
            let (found, count) = self.filtered();
            if count == 0 {
                canvas.draw_text_system(
                    overview.x + OVERVIEW_MARGIN,
                    self.overview_grid_top() + 4,
                    catalog.lookup("no_matches", "No matches"),
                    theme_muted(),
                );
            }
            for (index, &app_index) in found[..count].iter().enumerate() {
                let Some(app) = self.apps[app_index] else {
                    continue;
                };
                let cell = self.overview_cell_area(index);
                if visible.intersect(cell).is_empty() {
                    continue;
                }
                let cell_bg = Area::new(cell.x + 2, cell.y, cell.width - 4, cell.height - 4);
                if index == self.selected {
                    fill_rounded(canvas, cell_bg, CHROME_RADIUS, lighten(theme_taskbar(), 18));
                } else if cell.contains(self.pointer.0, self.pointer.1) {
                    fill_rounded(canvas, cell_bg, CHROME_RADIUS, lighten(theme_taskbar(), 10));
                }
                draw_app_icon_light(
                    canvas,
                    cell.x + (cell.width - OVERVIEW_APP_ICON_SIZE) / 2,
                    cell.y + 2,
                    app_icon(app.name()),
                    OVERVIEW_APP_ICON_SIZE,
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

            // Footer: user and power hint, Win11-style bottom bar.
            let footer_h = 28;
            let footer_y = overview.y + overview.height - footer_h;
            let footer = Area::new(overview.x, footer_y, overview.width, footer_h);
            canvas.fill_rect(
                footer.x,
                footer.y,
                footer.width as usize,
                1,
                blend(theme_stroke(), theme_panel(), 28),
            );
            let mut user_buf = [0u8; 32];
            let user_name = if let Some(account) = self
                .session
                .and_then(|index| self.accounts.get(index).copied().flatten())
            {
                let name = account.name();
                let len = name.len().min(user_buf.len());
                user_buf[..len].copy_from_slice(&name[..len]);
                core::str::from_utf8(&user_buf[..len]).unwrap_or("User")
            } else {
                "User"
            };
            canvas.draw_text_system(footer.x + 12, footer.y + 9, user_name, theme_text());
            let power = catalog.lookup("power", "Power");
            let power_w = font::text_width(power) as i32;
            canvas.draw_text_system(
                footer.x + footer.width - power_w - 12,
                footer.y + 9,
                power,
                theme_muted(),
            );
        }
    }

    /// One of the three bounded system-status icons, between the Quick Settings
    /// button and the application tray: network, volume, battery (left to
    /// right). Keeping them in their own slots leaves the existing `TRAY`
    /// protocol and its surface ownership untouched.
    fn status_area(&self, index: usize) -> Area {
        let x = self.clock_area().x - (MAX_TRAY + index + 1) as i32 * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    /// The union of the status icons' damage area.
    fn status_icons_area(&self) -> Area {
        let mut area = Area::new(0, 0, 0, 0);
        for index in 0..STATUS_ICONS {
            area = area.union(self.status_area(index));
        }
        area
    }

    /// The Quick Settings button, left of the status icons.
    fn qs_button(&self) -> Area {
        let x = self.clock_area().x - (MAX_TRAY + STATUS_ICONS + 1) as i32 * (TRAY_ICON + TRAY_GAP);
        Area::new(x, self.taskbar_top() + 4, TRAY_ICON, TRAY_ICON)
    }

    /// Quick Settings: a title row and one row per toggle, above the
    /// taskbar's right end; empty while closed.
    fn qs_action_active(&self, action: QsAction) -> bool {
        match action {
            QsAction::Wifi
            | QsAction::Bluetooth
            | QsAction::Airplane
            | QsAction::BatterySaver
            | QsAction::NightLight => false,
            QsAction::TaskbarAlign => self.taskbar.alignment == taskbar_prefs::Alignment::Left,
            QsAction::FocusAssist => self.dnd,
            QsAction::TaskbarIconSize => self.taskbar.icon_size != taskbar_prefs::IconSize::Medium,
            QsAction::Accessibility => self.reduced_motion,
            QsAction::DisplayScale => self.scale == 2,
            QsAction::Tiling => self.tiled,
            QsAction::ThreeD => self.three_d,
            QsAction::Language => true,
            QsAction::ChangePin => false,
        }
    }

    fn qs_action_label(&self, action: QsAction) -> &'static str {
        match action {
            QsAction::Wifi => "Wi-Fi",
            QsAction::Bluetooth => "Bluetooth",
            QsAction::Airplane => "Airplane mode",
            QsAction::BatterySaver => "Battery saver",
            QsAction::TaskbarAlign => "Taskbar align",
            QsAction::FocusAssist => "Focus assist",
            QsAction::NightLight => "Night light",
            QsAction::TaskbarIconSize => "Icon size",
            QsAction::Accessibility => "Accessibility",
            QsAction::DisplayScale => "Scale",
            QsAction::Tiling => "Tiling",
            QsAction::ThreeD => "3D Desktop",
            QsAction::Language => "Language",
            QsAction::ChangePin => "Change PIN",
        }
    }

    fn qs_action_subtitle(&self, action: QsAction) -> &'static str {
        match action {
            QsAction::Wifi
            | QsAction::Bluetooth
            | QsAction::Airplane
            | QsAction::BatterySaver
            | QsAction::NightLight => "Unavailable",
            QsAction::TaskbarAlign => match self.taskbar.alignment {
                taskbar_prefs::Alignment::Center => "Center",
                taskbar_prefs::Alignment::Left => "Left",
            },
            QsAction::FocusAssist => {
                if self.dnd {
                    "On"
                } else {
                    "Off"
                }
            }
            QsAction::TaskbarIconSize => match self.taskbar.icon_size {
                taskbar_prefs::IconSize::Small => "Small",
                taskbar_prefs::IconSize::Medium => "Medium",
                taskbar_prefs::IconSize::Large => "Large",
            },
            QsAction::Accessibility => {
                if self.reduced_motion {
                    "On"
                } else {
                    "Off"
                }
            }
            QsAction::DisplayScale => {
                if self.scale == 2 {
                    "200%"
                } else {
                    "100%"
                }
            }
            QsAction::Tiling => {
                if self.tiled {
                    "On"
                } else {
                    "Off"
                }
            }
            QsAction::ThreeD => {
                if self.three_d {
                    "On"
                } else {
                    "Off"
                }
            }
            QsAction::Language => match self.language {
                libwoven::i18n::Language::English => "English",
                libwoven::i18n::Language::Chichewa => "Chichewa",
                libwoven::i18n::Language::Arabic => "Arabic",
            },
            QsAction::ChangePin => "Security",
        }
    }

    fn qs_panel_height(&self) -> i32 {
        let grid_h =
            QS_GRID_ROWS as i32 * QS_BUTTON_HEIGHT + (QS_GRID_ROWS as i32 - 1) * QS_GRID_GAP;
        // header + grid + sliders(2) + footer + paddings/gaps
        16 + 24
            + QS_PADDING
            + grid_h
            + QS_SECTION_GAP
            + QS_SLIDER_HEIGHT * 2
            + QS_SLIDER_GAP
            + QS_SECTION_GAP
            + QS_FOOTER_HEIGHT
            + QS_PADDING
    }

    fn qs_area(&self) -> Area {
        if !self.qs_open {
            return Area::new(0, 0, 0, 0);
        }
        let height = self.qs_panel_height().min(self.taskbar_top() - 8).max(0);
        let y = (self.taskbar_top() - height - 8).max(0);
        Area::new(self.width - QS_WIDTH - 8, y, QS_WIDTH, height)
    }

    /// The lock screen: the wallpaper dimmed, "Locked", the time, the
    /// prompt, and a masked dot per digit typed, all centred.
    fn render_lock(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        self.render_floor_3d(canvas, |x, y| {
            blend(rgb(0, 0, 0), wallpaper(x, y, height), LOCK_DIM)
        });
        let middle = self.height / 2;
        let catalog = shell_catalog(self.language);
        // The sign-in pilot consumes the shared design system directly: one
        // palette for every role here and one type scale for every label, so
        // this surface cannot drift from the documented identity.
        let palette = SemanticPalette::resolve(high_contrast(), false);

        // A frosted card behind the sign-in/lock content, Windows-11-style,
        // instead of text floating directly on the dimmed wallpaper. Its fill
        // is the design-system surface role rather than a local color literal.
        let card = self.lock_card_area();
        fill_rounded_blended(canvas, card, CHROME_CARD_RADIUS, palette.surface, 170);

        if self.mode == LockMode::SignIn {
            // A circular avatar for the selected account: its initial inside
            // a filled circle, the same technique used for rounded chrome.
            let initial = self.accounts[self.chosen]
                .map(|account| account.name().first().copied().unwrap_or(b'?'))
                .unwrap_or(b'?')
                .to_ascii_uppercase();
            let avatar_y = middle - 92;
            fill_circle(
                canvas,
                self.width / 2,
                avatar_y,
                AVATAR_DIAMETER,
                palette.accent,
            );
            let glyph = [initial];
            let glyph = core::str::from_utf8(&glyph).unwrap_or("?");
            let avatar_size = (AVATAR_DIAMETER as u16 / 2).max(BODY_TEXT);
            canvas.draw_ui_text(
                self.width / 2 - libwoven::ui_text_width(glyph, avatar_size) / 2,
                avatar_y - libwoven::ui_line_height(avatar_size) / 2,
                glyph,
                palette.on_accent,
                avatar_size,
            );

            // The sign-in prompt, then the accounts, the chosen one marked
            // with an accent pill rather than an ASCII caret. The shared type
            // scale is used for both size and leading, and the selected row
            // uses `on_accent` ink so the label stays legible on the pill.
            let text = catalog.lookup("sign_in", "Sign in");
            let headline = TypeRole::Headline;
            canvas.draw_ui_text(
                self.width / 2 - libwoven::ui_text_width(text, headline.em_px()) / 2,
                middle - 56,
                text,
                palette.text,
                headline.em_px(),
            );
            let account_role = TypeRole::Subtitle;
            let account_line = account_role.line_height_px() as i32;
            for (index, account) in self.accounts.iter().enumerate() {
                let Some(account) = account else { continue };
                let name = core::str::from_utf8(account.name()).unwrap_or("");
                let width = libwoven::ui_text_width(name, account_role.em_px());
                let x = self.width / 2 - width / 2;
                let y = middle - 24 + (index as i32) * account_line;
                let chosen = index == self.chosen;
                if chosen {
                    let pill = Area::new(x - 10, y - 3, width + 20, account_line + 6);
                    fill_rounded(
                        canvas,
                        pill,
                        (pill.height / 2).max(0) as usize,
                        palette.accent,
                    );
                }
                let ink = if chosen {
                    palette.on_accent
                } else {
                    palette.text
                };
                canvas.draw_ui_text(x, y, name, ink, account_role.em_px());
            }
        } else if self.mode == LockMode::ChangePin {
            let text = catalog.lookup("change_pin", "Change PIN");
            let headline = TypeRole::Headline;
            canvas.draw_ui_text(
                self.width / 2 - libwoven::ui_text_width(text, headline.em_px()) / 2,
                middle - 44,
                text,
                palette.text,
                headline.em_px(),
            );
        } else {
            let muted = TypeRole::Subtitle;
            let text = catalog.lookup("locked", "Locked");
            canvas.draw_ui_text(
                self.width / 2 - libwoven::ui_text_width(text, muted.em_px()) / 2,
                middle - 60,
                text,
                palette.text_muted,
                muted.em_px(),
            );
            let (bytes, length) = clock_text(self.time, false);
            let clock = core::str::from_utf8(&bytes[..length]).unwrap_or("");
            let display = TypeRole::Display;
            canvas.draw_ui_text(
                self.width / 2 - libwoven::ui_text_width(clock, display.em_px()) / 2,
                middle - 48,
                clock,
                palette.text,
                display.em_px(),
            );
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
        let prompt_role = TypeRole::Subtitle;
        canvas.draw_ui_text(
            self.width / 2 - libwoven::ui_text_width(prompt, prompt_role.em_px()) / 2,
            middle + 26,
            prompt,
            palette.text_muted,
            prompt_role.em_px(),
        );
        let dots = self.entry_length as i32;
        if dots > 0 {
            let left = (self.width - (dots * 10 - 4)) / 2;
            // A rounded pill behind the dots, Windows-11-style, instead of
            // the digits floating bare on the card.
            let pill = Area::new(left - 10, middle + 48, dots * 10 - 4 + 20, 20);
            fill_rounded(canvas, pill, 10, palette.surface_raised);
            for index in 0..dots {
                fill_circle(canvas, left + index * 10 + 3, middle + 57, 6, palette.text);
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
            let mut text = [0u8; TOAST_CHARS];
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
                let mut text = [0u8; TOAST_CHARS];
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
    fn qs_activate_action(&mut self, action: QsAction) -> Area {
        let panel = self.qs_area();
        match action {
            QsAction::Wifi | QsAction::Bluetooth | QsAction::Airplane => {
                // No radio or airplane backend exists on this device. Say so
                // instead of toggling a state that changes nothing.
                log(&[b"woven-shell: radio unavailable"]);
                panel.union(self.toast_message("No wireless hardware is available"))
            }
            QsAction::BatterySaver => {
                // There is no ACPI battery driver and no power policy backend,
                // so this control cannot change anything. Say so honestly.
                log(&[b"woven-shell: battery saver unavailable"]);
                panel.union(self.toast_message("No battery or power policy is available"))
            }
            QsAction::FocusAssist => {
                self.dnd = !self.dnd;
                log(if self.dnd {
                    &[b"woven-shell: do not disturb on"]
                } else {
                    &[b"woven-shell: do not disturb off"]
                });
                panel
            }
            QsAction::NightLight => {
                // No display colour-temperature backend exists; do not toggle
                // a state that changes no pixels.
                log(&[b"woven-shell: night light unavailable"]);
                panel.union(self.toast_message("Night light needs a display colour policy backend"))
            }
            QsAction::Accessibility => {
                self.reduced_motion = !self.reduced_motion;
                log(if self.reduced_motion {
                    &[b"woven-shell: reduced motion on"]
                } else {
                    &[b"woven-shell: reduced motion off"]
                });
                panel
            }
            QsAction::DisplayScale => self.toggle_scale().union(self.qs_area()),
            QsAction::Tiling => self.toggle_tiling().union(self.qs_area()),
            QsAction::ThreeD => {
                self.three_d = !self.three_d;
                log(if self.three_d {
                    &[b"woven-shell: 3D desktop on"]
                } else {
                    &[b"woven-shell: 3D desktop off"]
                });
                panel.union(self.screen())
            }
            QsAction::Language => {
                self.language = match self.language {
                    libwoven::i18n::Language::English => libwoven::i18n::Language::Chichewa,
                    libwoven::i18n::Language::Chichewa => libwoven::i18n::Language::Arabic,
                    libwoven::i18n::Language::Arabic => libwoven::i18n::Language::English,
                };
                log(&[b"woven-shell: language toggled"]);
                panel
            }
            QsAction::ChangePin => self.start_change_pin(),
            QsAction::TaskbarAlign => {
                self.taskbar.alignment = match self.taskbar.alignment {
                    taskbar_prefs::Alignment::Center => taskbar_prefs::Alignment::Left,
                    taskbar_prefs::Alignment::Left => taskbar_prefs::Alignment::Center,
                };
                let label = match self.taskbar.alignment {
                    taskbar_prefs::Alignment::Center => "Taskbar alignment: Center (session)",
                    taskbar_prefs::Alignment::Left => "Taskbar alignment: Left (session)",
                };
                log(&[b"woven-shell: ", label.as_bytes()]);
                panel.union(self.taskbar()).union(self.toast_message(label))
            }
            QsAction::TaskbarIconSize => {
                self.taskbar.icon_size = match self.taskbar.icon_size {
                    taskbar_prefs::IconSize::Small => taskbar_prefs::IconSize::Medium,
                    taskbar_prefs::IconSize::Medium => taskbar_prefs::IconSize::Large,
                    taskbar_prefs::IconSize::Large => taskbar_prefs::IconSize::Small,
                };
                let label = match self.taskbar.icon_size {
                    taskbar_prefs::IconSize::Small => "Taskbar icon size: Small (session)",
                    taskbar_prefs::IconSize::Medium => "Taskbar icon size: Medium (session)",
                    taskbar_prefs::IconSize::Large => "Taskbar icon size: Large (session)",
                };
                log(&[b"woven-shell: ", label.as_bytes()]);
                panel.union(self.taskbar()).union(self.toast_message(label))
            }
        }
    }

    fn qs_grid_area_at(&self, panel: Area) -> Area {
        let top = panel.y + 16 + 24 + QS_PADDING;
        let grid_h =
            QS_GRID_ROWS as i32 * QS_BUTTON_HEIGHT + (QS_GRID_ROWS as i32 - 1) * QS_GRID_GAP;
        Area::new(
            panel.x + QS_PADDING,
            top,
            panel.width - QS_PADDING * 2,
            grid_h,
        )
    }

    fn qs_action_area_at(&self, panel: Area, index: usize) -> Area {
        let grid = self.qs_grid_area_at(panel);
        let col = (index % QS_GRID_COLS) as i32;
        let row = (index / QS_GRID_COLS) as i32;
        let cell_w = (grid.width - QS_GRID_GAP * (QS_GRID_COLS as i32 - 1)) / QS_GRID_COLS as i32;
        Area::new(
            grid.x + col * (cell_w + QS_GRID_GAP),
            grid.y + row * (QS_BUTTON_HEIGHT + QS_GRID_GAP),
            cell_w,
            QS_BUTTON_HEIGHT,
        )
    }

    fn qs_slider_area_at(&self, panel: Area, which: i32) -> Area {
        let grid = self.qs_grid_area_at(panel);
        let top =
            grid.y + grid.height + QS_SECTION_GAP + which * (QS_SLIDER_HEIGHT + QS_SLIDER_GAP);
        Area::new(
            panel.x + QS_PADDING,
            top,
            panel.width - QS_PADDING * 2,
            QS_SLIDER_HEIGHT,
        )
    }

    fn qs_footer_area_at(&self, panel: Area) -> Area {
        Area::new(
            panel.x,
            panel.y + panel.height - QS_FOOTER_HEIGHT - QS_PADDING,
            panel.width,
            QS_FOOTER_HEIGHT + QS_PADDING,
        )
    }

    fn qs_summary<'a>(&self, buf: &'a mut [u8; 32]) -> &'a str {
        let mut len = 0usize;
        let net: &[u8] = match self.status.network {
            NetworkStatus::Online { .. } => b"Online",
            NetworkStatus::Offline => b"Offline",
            NetworkStatus::Unavailable => b"No net",
        };
        for b in net.iter().take(buf.len()) {
            buf[len] = *b;
            len += 1;
        }
        if len < buf.len() {
            buf[len] = b' ';
            len += 1;
        }
        let (pct, muted) = match self.status.volume {
            VolumeStatus::Level { percent, muted } => (percent, muted),
            VolumeStatus::Unavailable => (0, true),
        };
        let mut pbuf = [0u8; 3];
        let plen = status::format_percent(pct, &mut pbuf);
        for b in pbuf[..plen].iter().take(buf.len().saturating_sub(len)) {
            buf[len] = *b;
            len += 1;
        }
        if len < buf.len() {
            buf[len] = b'%';
            len += 1;
        }
        if muted && len < buf.len() {
            buf[len] = b' ';
            len += 1;
            if len < buf.len() {
                buf[len] = b'M';
                len += 1;
            }
        }
        core::str::from_utf8(&buf[..len]).unwrap_or("")
    }

    fn qs_footer_status<'a>(&self, buf: &'a mut [u8; 32]) -> &'a str {
        let mut len = 0usize;
        let prefix: &[u8] = b"Battery: ";
        for b in prefix {
            if len < buf.len() {
                buf[len] = *b;
                len += 1;
            }
        }
        let text: &[u8] = match self.status.battery {
            BatteryStatus::Unavailable => b"N/A",
        };
        for b in text {
            if len < buf.len() {
                buf[len] = *b;
                len += 1;
            }
        }
        core::str::from_utf8(&buf[..len]).unwrap_or("Battery: N/A")
    }

    fn draw_qs_footer_button(&self, canvas: &mut Canvas<'_>, area: Area, label: &str) {
        fill_rounded(canvas, area, 6, blend(theme_stroke(), theme_panel(), 20));
        outline(canvas, area, blend(theme_stroke(), theme_panel(), 34));
        let tw = font::text_width(label) as i32;
        let tx = area.x + (area.width - tw).max(0) / 2;
        let ty = area.y + (area.height - font::LINE_HEIGHT).max(0) / 2;
        canvas.draw_text_system(tx, ty, label, theme_text());
    }

    fn draw_qs_slider(
        &self,
        canvas: &mut Canvas<'_>,
        area: Area,
        label: &str,
        value: u8,
        active: bool,
    ) {
        let value = value.min(100);
        fill_rounded(canvas, area, 8, blend(theme_stroke(), theme_panel(), 14));
        outline(canvas, area, blend(theme_stroke(), theme_panel(), 24));

        // Icon box
        let icon_area = Area::new(area.x + 8, area.y + 7, 20, 20);
        let icon_fg = if active { theme_text() } else { theme_muted() };
        if label.starts_with("Brightness") {
            // sun icon
            fill_rounded(
                canvas,
                Area::new(icon_area.x + 6, icon_area.y + 6, 8, 8),
                4,
                icon_fg,
            );
            for (dx, dy, w, h) in [(9, 0, 2, 3), (9, 17, 2, 3), (0, 9, 3, 2), (17, 9, 3, 2)] {
                canvas.fill_rect(
                    icon_area.x + dx,
                    icon_area.y + dy,
                    w as usize,
                    h as usize,
                    icon_fg,
                );
            }
        } else {
            // speaker icon
            let c = icon_fg;
            canvas.fill_rect(icon_area.x + 2, icon_area.y + 7, 3, 6, c);
            canvas.fill_rect(icon_area.x + 5, icon_area.y + 5, 2, 10, c);
            canvas.fill_rect(icon_area.x + 7, icon_area.y + 4, 2, 12, c);
            if !active {
                canvas.fill_rect(icon_area.x + 12, icon_area.y + 7, 1, 1, c);
                canvas.fill_rect(icon_area.x + 13, icon_area.y + 8, 1, 1, c);
                canvas.fill_rect(icon_area.x + 14, icon_area.y + 9, 1, 1, c);
                canvas.fill_rect(icon_area.x + 13, icon_area.y + 10, 1, 1, c);
                canvas.fill_rect(icon_area.x + 12, icon_area.y + 11, 1, 1, c);
            } else {
                canvas.fill_rect(icon_area.x + 11, icon_area.y + 7, 1, 6, c);
                canvas.fill_rect(icon_area.x + 13, icon_area.y + 5, 1, 10, c);
            }
        }

        let track_x = area.x + 36;
        let track_w = area.width - 44;
        let track_y = area.y + area.height / 2 - 2;
        let track_h = 4i32;
        fill_rounded(
            canvas,
            Area::new(track_x, track_y, track_w, track_h),
            2,
            blend(theme_stroke(), theme_panel(), 44),
        );
        let fill_w = (track_w * value as i32) / 100;
        if fill_w > 0 {
            let fill_color = if active {
                theme_accent()
            } else {
                theme_muted()
            };
            fill_rounded(
                canvas,
                Area::new(track_x, track_y, fill_w.max(4), track_h),
                2,
                fill_color,
            );
            let thumb_x = track_x + fill_w - 5;
            let thumb_y = area.y + area.height / 2 - 6;
            fill_rounded(canvas, Area::new(thumb_x, thumb_y, 10, 12), 5, theme_text());
            outline(
                canvas,
                Area::new(thumb_x, thumb_y, 10, 12),
                blend(theme_panel(), theme_text(), 60),
            );
        }
        let _ = label;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_qs_action_icon(
        &self,
        canvas: &mut Canvas<'_>,
        action: QsAction,
        x: i32,
        y: i32,
        fg: u32,
        bg: u32,
        active: bool,
    ) {
        let a = Area::new(x, y, 20, 20);
        fill_rounded(canvas, a, 6, bg);
        let c = fg;
        match action {
            QsAction::Wifi => {
                // simple wifi arcs as bars
                for (i, w) in [14i32, 10, 6].iter().enumerate() {
                    let by = y + 14 - i as i32 * 4;
                    let bx = x + (20 - *w) / 2;
                    canvas.fill_rect(bx, by, *w as usize, 2, c);
                }
                canvas.fill_rect(x + 9, y + 16, 2, 2, c);
            }
            QsAction::Bluetooth => {
                canvas.fill_rect(x + 9, y + 3, 2, 14, c);
                canvas.fill_rect(x + 11, y + 5, 2, 2, c);
                canvas.fill_rect(x + 13, y + 7, 2, 2, c);
                canvas.fill_rect(x + 11, y + 9, 2, 2, c);
                canvas.fill_rect(x + 13, y + 11, 2, 2, c);
                canvas.fill_rect(x + 11, y + 13, 2, 2, c);
                canvas.fill_rect(x + 7, y + 5, 2, 2, c);
                canvas.fill_rect(x + 5, y + 7, 2, 2, c);
                canvas.fill_rect(x + 7, y + 13, 2, 2, c);
                canvas.fill_rect(x + 5, y + 11, 2, 2, c);
            }
            QsAction::Airplane => {
                canvas.fill_rect(x + 9, y + 2, 2, 16, c);
                canvas.fill_rect(x + 4, y + 7, 12, 2, c);
                canvas.fill_rect(x + 6, y + 14, 8, 2, c);
            }
            QsAction::BatterySaver => {
                canvas.fill_rect(x + 4, y + 5, 10, 10, c);
                canvas.fill_rect(x + 14, y + 8, 2, 4, c);
                let inner = if active {
                    bg
                } else {
                    blend(theme_panel(), theme_stroke(), 8)
                };
                canvas.fill_rect(x + 5, y + 6, 8, 8, inner);
                canvas.fill_rect(x + 8, y + 7, 2, 6, c);
                canvas.fill_rect(x + 7, y + 9, 4, 2, c);
            }
            QsAction::FocusAssist => {
                // moon
                fill_rounded(canvas, Area::new(x + 4, y + 4, 12, 12), 6, c);
                let cut = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                fill_rounded(canvas, Area::new(x + 8, y + 2, 10, 10), 5, cut);
            }
            QsAction::NightLight => {
                fill_rounded(canvas, Area::new(x + 5, y + 5, 10, 10), 5, c);
                for (dx, dy) in [(2, 2), (16, 2), (2, 16), (16, 16)] {
                    canvas.fill_rect(x + dx, y + dy, 2, 2, c);
                }
            }
            QsAction::Accessibility => {
                canvas.fill_rect(x + 9, y + 3, 2, 2, c);
                canvas.fill_rect(x + 9, y + 6, 2, 6, c);
                canvas.fill_rect(x + 6, y + 7, 8, 2, c);
                canvas.fill_rect(x + 7, y + 12, 2, 5, c);
                canvas.fill_rect(x + 11, y + 12, 2, 5, c);
            }
            QsAction::DisplayScale => {
                canvas.fill_rect(x + 3, y + 4, 14, 10, c);
                let inner = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                canvas.fill_rect(x + 4, y + 5, 12, 8, inner);
                canvas.fill_rect(x + 8, y + 14, 4, 2, c);
                canvas.fill_rect(x + 6, y + 16, 8, 1, c);
            }
            QsAction::Tiling => {
                let inner = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                for (dx, dy, w, h) in [(3, 3, 6, 6), (11, 3, 6, 6), (3, 11, 6, 6), (11, 11, 6, 6)] {
                    canvas.fill_rect(x + dx, y + dy, w as usize, h as usize, c);
                    canvas.fill_rect(
                        x + dx + 1,
                        y + dy + 1,
                        (w - 2) as usize,
                        (h - 2) as usize,
                        inner,
                    );
                }
            }
            QsAction::ThreeD => {
                canvas.fill_rect(x + 4, y + 6, 8, 8, c);
                let inner = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                canvas.fill_rect(x + 5, y + 7, 6, 6, inner);
                canvas.fill_rect(x + 7, y + 3, 8, 1, c);
                canvas.fill_rect(x + 14, y + 4, 1, 8, c);
            }
            QsAction::Language => {
                canvas.fill_rect(x + 3, y + 9, 14, 2, c);
                canvas.fill_rect(x + 9, y + 3, 2, 14, c);
                fill_rounded(canvas, Area::new(x + 3, y + 3, 14, 14), 7, c);
                let inner = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                fill_rounded(canvas, Area::new(x + 4, y + 4, 12, 12), 6, inner);
                canvas.fill_rect(x + 4, y + 9, 12, 1, c);
                canvas.fill_rect(x + 9, y + 4, 1, 12, c);
            }
            QsAction::ChangePin => {
                fill_rounded(canvas, Area::new(x + 5, y + 9, 10, 8), 2, c);
                let inner = if active {
                    bg
                } else {
                    blend(theme_stroke(), theme_panel(), 18)
                };
                canvas.fill_rect(x + 7, y + 4, 6, 6, c);
                canvas.fill_rect(x + 8, y + 5, 4, 4, inner);
                canvas.fill_rect(x + 9, y + 12, 2, 3, inner);
            }
            QsAction::TaskbarAlign => {
                // bar with centered/left group indicator
                canvas.fill_rect(x + 2, y + 14, 16, 3, c);
                let inner = if active { bg } else { blend(theme_stroke(), theme_panel(), 18) };
                if self.taskbar.alignment == taskbar_prefs::Alignment::Left {
                    canvas.fill_rect(x + 3, y + 15, 3, 1, inner);
                    canvas.fill_rect(x + 7, y + 15, 3, 1, inner);
                } else {
                    canvas.fill_rect(x + 7, y + 15, 2, 1, inner);
                    canvas.fill_rect(x + 10, y + 15, 2, 1, inner);
                }
                canvas.fill_rect(x + 4, y + 5, 12, 7, c);
                canvas.fill_rect(x + 5, y + 6, 10, 5, inner);
            }
            QsAction::TaskbarIconSize => {
                let inner = if active { bg } else { blend(theme_stroke(), theme_panel(), 18) };
                for (dx, size) in [(3i32, 3i32), (8, 4), (14, 5)] {
                    let dy = y + 16 - size;
                    canvas.fill_rect(x + dx, dy, size as usize, size as usize, c);
                    if size > 2 {
                        canvas.fill_rect(x + dx + 1, dy + 1, (size - 2) as usize, (size - 2) as usize, inner);
                    }
                }
            }

        }
        let _ = (a, active);
    }

    fn paint_qs_content(&self, canvas: &mut Canvas<'_>, area: Area) {
        let (x, y) = (area.x, area.y);
        fill_rounded_blended(canvas, area, 12, theme_panel(), 238);
        outline(canvas, area, blend(theme_stroke(), theme_panel(), 30));

        // Header
        canvas.draw_text_system(x + QS_PADDING, y + 12, QS_TITLE, theme_text());
        let mut summary_buf = [0u8; 32];
        let summary = self.qs_summary(&mut summary_buf);
        let summary_w = font::text_width(summary) as i32;
        canvas.draw_text_system(
            x + area.width - QS_PADDING - summary_w,
            y + 12,
            summary,
            theme_muted(),
        );

        // Quick-action grid
        for (index, action) in QS_ACTION_ORDER.iter().enumerate() {
            let cell = self.qs_action_area_at(area, index);
            let active = self.qs_action_active(*action);
            let fill = if active {
                blend(theme_accent(), theme_panel(), 210)
            } else {
                blend(theme_stroke(), theme_panel(), 18)
            };
            let border = if active {
                blend(theme_accent(), theme_text(), 40)
            } else {
                blend(theme_stroke(), theme_panel(), 28)
            };
            fill_rounded(canvas, cell, QS_BUTTON_RADIUS, fill);
            outline(canvas, cell, border);

            let icon_x = cell.x + 10;
            let icon_y = cell.y + 8;
            let icon_fg = if active {
                rgb(20, 28, 38)
            } else {
                theme_text()
            };
            let icon_bg = if active {
                theme_accent()
            } else {
                blend(theme_stroke(), theme_panel(), 40)
            };
            self.draw_qs_action_icon(canvas, *action, icon_x, icon_y, icon_fg, icon_bg, active);

            let label = self.qs_action_label(*action);
            let sub = self.qs_action_subtitle(*action);
            let text_x = cell.x + 10;
            let label_y = cell.y + 30;
            let max_w = cell.width - 14;
            let mut lbuf = [0u8; 64];
            let llen = truncate_to_width(label.as_bytes(), max_w, &mut lbuf);
            let ltext = core::str::from_utf8(&lbuf[..llen]).unwrap_or(label);
            let label_color = if active {
                rgb(20, 28, 38)
            } else {
                theme_text()
            };
            let sub_color = if active {
                blend(rgb(20, 28, 38), theme_accent(), 140)
            } else {
                theme_muted()
            };
            canvas.draw_text_system(text_x, label_y, ltext, label_color);
            let mut sbuf = [0u8; 64];
            let slen = truncate_to_width(sub.as_bytes(), max_w, &mut sbuf);
            let stext = core::str::from_utf8(&sbuf[..slen]).unwrap_or(sub);
            canvas.draw_text_system(text_x, cell.y + cell.height - 12, stext, sub_color);
        }

        // Sliders
        let brightness_area = self.qs_slider_area_at(area, 0);
        let volume_area = self.qs_slider_area_at(area, 1);
        self.draw_qs_slider(canvas, brightness_area, "Brightness", self.brightness, true);
        let (vol_percent, muted) = match self.status.volume {
            VolumeStatus::Level { percent, muted } => (percent, muted),
            VolumeStatus::Unavailable => (0, true),
        };
        let vol_label = if muted { "Volume (muted)" } else { "Volume" };
        self.draw_qs_slider(canvas, volume_area, vol_label, vol_percent, !muted);

        // Footer
        let footer = self.qs_footer_area_at(area);
        canvas.fill_rect(
            x + QS_PADDING,
            footer.y,
            (area.width - QS_PADDING * 2).max(0) as usize,
            1,
            blend(theme_stroke(), theme_panel(), 24),
        );
        let mut bat_buf = [0u8; 32];
        let bat_text = self.qs_footer_status(&mut bat_buf);
        canvas.draw_text_system(x + QS_PADDING, footer.y + 14, bat_text, theme_muted());

        let btn_y = footer.y + 8;
        let btn_h = 24;
        let power_w = 54;
        let restart_w = 58;
        let lock_w = 40;
        let settings_w = 60;
        let gap = 6;
        let power_x = x + area.width - QS_PADDING - power_w;
        let restart_x = power_x - gap - restart_w;
        let lock_x = restart_x - gap - lock_w;
        let settings_x = lock_x - gap - settings_w;
        self.draw_qs_footer_button(
            canvas,
            Area::new(settings_x, btn_y, settings_w, btn_h),
            "Settings",
        );
        self.draw_qs_footer_button(canvas, Area::new(lock_x, btn_y, lock_w, btn_h), "Lock");
        self.draw_qs_footer_button(
            canvas,
            Area::new(restart_x, btn_y, restart_w, btn_h),
            "Restart",
        );
        self.draw_qs_footer_button(canvas, Area::new(power_x, btn_y, power_w, btn_h), "Power");
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

    /// Click one of the tray status glyphs. Network and battery open Quick
    /// Settings for their detail row; volume toggles the real output mute via
    /// the existing audio ABI. No new capability or client surface is involved.
    fn status_click(&mut self, index: usize) -> Area {
        let icon = self.status_area(index);
        if index != 1 {
            return self.toggle_qs().union(icon);
        }
        let Some(level) = self.status.toggle_mute() else {
            log(&[b"woven-shell: volume unavailable"]);
            return icon;
        };
        let mut digits = [0u8; 3];
        let length = status::format_percent(level, &mut digits);
        log(&[b"woven-shell: volume ", &digits[..length]]);
        icon.union(self.qs_area())
    }

    /// Show a short informational toast and return its damage area.
    fn toast_message(&mut self, text: &str) -> Area {
        let mut buffer = [0u8; TOAST_CHARS];
        let length = label(text.as_bytes(), TOAST_CHARS, &mut buffer);
        self.toast = Some(Toast {
            text: buffer,
            length,
            until: libwoven::ticks() + TOAST_TICKS,
        });
        self.toast_area()
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
        let mut text = [0u8; TOAST_CHARS];
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
        let clock = self.clock_area();
        Area::new(clock.x - 14, self.taskbar_top() + 7, 10, 11)
    }

    /// The Notification Center, sized to its rows, drops down from the
    /// clock, calendar/notification-popover style; empty while closed.
    fn center_area(&self) -> Area {
        if !self.center_open {
            return Area::new(0, 0, 0, 0);
        }
        let count = self.notes.iter().flatten().count();
        let height = (1 + count.max(1) as i32) * PANEL_ROW + 2;
        let y = (self.taskbar_top() - height - 4).max(0);
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
        outline(canvas, area, blend(theme_stroke(), theme_panel(), 34));
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
            let row_y = y + 1 + (1 + row as i32) * PANEL_ROW;
            let row_bg = Area::new(x + 4, row_y + 1, area.width - 8, PANEL_ROW - 2);
            fill_rounded(canvas, row_bg, 4, blend(theme_stroke(), theme_panel(), 14));
            let text = core::str::from_utf8(&note.text[..note.length]).unwrap_or("");
            canvas.draw_text_system(x + 8, row_y + 4, text, theme_text());
        }
        if !any {
            canvas.draw_text_system(
                x + 8,
                y + 1 + PANEL_ROW + 4,
                "No notifications",
                theme_muted(),
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
        close_hovered: bool,
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
            let close_fill = if close_hovered {
                lighten(CLOSE_FILL, 28)
            } else {
                CLOSE_FILL
            };
            fill_rounded(canvas, close, CHROME_RADIUS_SMALL, close_fill);
            canvas.draw_ui_text(close.x + 2, close.y - 1, "x", theme_text(), BODY_TEXT);
        }
        let reserved = if close.is_empty() {
            8
        } else {
            8 + CLOSE_SIZE + 4
        };
        let available = (window.width - reserved).max(0);
        let mut buffer = [0u8; 64];
        let length =
            truncate_to_width(&window.title[..window.title_length], available, &mut buffer);
        let text = core::str::from_utf8(&buffer[..length]).unwrap_or("");
        canvas.draw_ui_text(area.x + 8, area.y + 2, text, theme_text(), BODY_TEXT);
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
        // A single repaint can take many ticks in software; snapshot once so a
        // slow compose does not burn through a resize grace window by itself.
        let now = libwoven::ticks();
        // Locked, nothing but the lock screen is drawn: no window, no
        // notification, no pointer. Repaint the whole card, not just the
        // damaged sub-region: the sign-in content is re-laid-out on every
        // state change (prompt, account selection, entry dots), so a
        // smaller damage rect would leave pixels from the previous layout
        // behind.
        if self.locked {
            let area = area.union(self.lock_card_area()).intersect(self.screen());
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
            // Window shadows are temporarily disabled: painter.blend's
            // present_region_blended path can stall on the software compositor
            // during heavy QEMU repaints, which hung the shell after maximize.
            // TODO(phase-b): restore shadows once the compositor blend path is
            // proven non-blocking in this environment.
            let _ = (bands, frame, area, above_taskbar, SHADOW);
            let active = Some(slot) == focused;
            let title = window.title_area().intersect(area);
            if !title.is_empty() {
                let close_hovered = window.close_area().contains(self.pointer.0, self.pointer.1);
                if alpha == u8::MAX {
                    painter.draw(title, |canvas| {
                        Self::render_title(
                            window,
                            active,
                            canvas,
                            self.three_d,
                            self.height,
                            close_hovered,
                        )
                    });
                } else {
                    painter.draw_blended(title, alpha, |canvas| {
                        Self::render_title(
                            window,
                            active,
                            canvas,
                            self.three_d,
                            self.height,
                            close_hovered,
                        )
                    });
                }
            }
            let surface = Area::new(window.x, window.y, window.surface_w, window.surface_h);
            let body = window.body().intersect(surface).intersect(area);
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
                if result.is_err() && now >= window.resizing_until {
                    return Some(slot);
                }
            }
            // A client may clamp a maximize WSIZ to a smaller surface; fill the
            // uncovered body so desktop background never shows through the frame.
            let full_body = window.body().intersect(above_taskbar).intersect(area);
            if full_body.width > window.surface_w || full_body.height > window.surface_h {
                let sw = window.surface_w.max(0);
                let sh = window.surface_h.max(0);
                let right = Area::new(window.x + sw, window.y, window.width - sw, window.height);
                let bottom = Area::new(window.x, window.y + sh, sw, window.height - sh);
                for gap in [right, bottom] {
                    let part = gap.intersect(area).intersect(above_taskbar);
                    if !part.is_empty() {
                        painter.blend(part, theme_panel(), 255);
                    }
                }
            }
            // Subtle Win11-style window border: accent for focused, muted for inactive.
            let border_color = if active {
                blend(theme_accent(), theme_stroke(), 90)
            } else {
                blend(theme_stroke(), theme_panel(), 72)
            };
            let frame = window.frame().intersect(above_taskbar);
            for edge in [
                Area::new(frame.x, frame.y, frame.width, 1),
                Area::new(frame.x, frame.y + frame.height - 1, frame.width, 1),
                Area::new(frame.x, frame.y, 1, frame.height),
                Area::new(frame.x + frame.width - 1, frame.y, 1, frame.height),
            ] {
                let part = edge.intersect(area);
                if !part.is_empty() {
                    // Keyed overlay: only the border pixels land, so body blits
                    // already on screen are preserved.
                    painter.overlay(part, |canvas| {
                        canvas.fill_rect(
                            part.x,
                            part.y,
                            part.width as usize,
                            part.height as usize,
                            border_color,
                        );
                    });
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
            // Floating Start panel: draw desktop behind it, then the rounded
            // panel content so corners blend correctly over wallpaper/icons.
            painter.draw(overview, |canvas| {
                self.render_desktop(canvas);
                self.paint_overview_content(canvas);
            });
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
                higher.alpha() == u8::MAX && higher.frame().intersect(footprint) == footprint
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

    /// A window is minimized while its slot is outside the visual stack: it
    /// is then never painted, focused or hit-tested, but it keeps its taskbar
    /// entry so it can be restored.
    fn is_minimized(&self, slot: usize) -> bool {
        self.windows[slot].is_some() && !self.stack[..self.stacked].contains(&slot)
    }

    /// Minimize `slot`, handing focus to the next window down the stack.
    fn minimize(&mut self, slot: usize) -> Area {
        let Some(position) = self.stack[..self.stacked].iter().position(|&s| s == slot) else {
            return Area::new(0, 0, 0, 0);
        };
        self.cancel_pointer_gesture();
        let hidden = self.windows[slot].map(|w| w.footprint()).unwrap_or(Area::new(0, 0, 0, 0));
        self.stack[position..self.stacked].rotate_left(1);
        self.stacked -= 1;
        let next = self
            .focused()
            .and_then(|top| self.windows[top])
            .map(|w| w.footprint())
            .unwrap_or(Area::new(0, 0, 0, 0));
        log(&[b"woven-shell: window minimized"]);
        hidden.union(next).union(self.taskbar())
    }

    /// Restore a minimized window to the top of the stack and focus it.
    fn restore(&mut self, slot: usize) -> Area {
        if !self.is_minimized(slot) || self.stacked >= self.stack.len() {
            return Area::new(0, 0, 0, 0);
        }
        let previous = self
            .focused()
            .and_then(|top| self.windows[top])
            .map(|w| w.footprint())
            .unwrap_or(Area::new(0, 0, 0, 0));
        self.stack[self.stacked] = slot;
        self.stacked += 1;
        log(&[b"woven-shell: window restored"]);
        let current = self.windows[slot].map(|w| w.footprint()).unwrap_or(Area::new(0, 0, 0, 0));
        previous.union(current).union(self.taskbar())
    }

    /// Taskbar button policy: the focused window's button minimizes it, a
    /// minimized window's button restores it, and any other focuses it.
    fn taskbar_activate(&mut self, slot: usize) -> Area {
        if self.focused() == Some(slot) {
            self.minimize(slot)
        } else if self.is_minimized(slot) {
            self.restore(slot)
        } else {
            self.focus(slot)
        }
    }

    /// Maximize or restore window `slot`, from a right-click on its title
    /// bar. A maximized window's frame fills the usable band -- the title
    /// bar at the very top, the client area below it to the band's bottom;
    /// the exact pre-maximize geometry is remembered and returned verbatim
    /// on restore. Returns the areas to repaint.
    fn toggle_maximize(&mut self, slot: usize) -> Area {
        let (_, band_height) = self.usable_band();
        let Some(window) = self.windows[slot].as_mut() else {
            return Area::new(0, 0, 0, 0);
        };
        let before = window.footprint();
        let (next_x, next_y, next_w, next_h, next_maximized) = if window.maximized {
            let [x, y, width, height] = window.restore;
            (x, y, width, height, false)
        } else {
            (
                0,
                TITLE_HEIGHT,
                self.width,
                (band_height - TITLE_HEIGHT).max(0),
                true,
            )
        };
        if !window.maximized {
            window.restore = [window.x, window.y, window.width, window.height];
        }
        window.x = next_x;
        window.y = next_y;
        window.width = next_w;
        window.height = next_h;
        window.maximized = next_maximized;
        // Frame-only maximize: keep the client's current surface and blit it
        // inside the band-filling frame. Asking the client to reflow via WSIZ
        // can race with concurrent compositor work on slow 1-CPU QEMU runs;
        // client reflow on maximize is a follow-up once that path is proven.
        if next_maximized {
            log(&[
                b"woven-shell: window maximized: ",
                &window.title[..window.title_length],
            ]);
        } else {
            log(&[
                b"woven-shell: window restored: ",
                &window.title[..window.title_length],
            ]);
        }
        let after = self
            .windows
            .get(slot)
            .and_then(|window| window.as_ref())
            .map(Window::footprint)
            .unwrap_or(before);
        before.union(after)
    }

    /// Pin or unpin the app behind a taskbar entry, and say which happened
    /// in a toast. The whole strip repaints because the group re-flows
    /// around its pins. In-session state only: the shell process holds
    /// `FileRead` and no settings-write path exists, so pins reset to the
    /// installed record's list on the next boot, and no message here
    /// promises otherwise.
    fn toggle_pin(&mut self, entry: TaskbarEntry) -> Area {
        let Some((name, length)) = self.entry_app_name(entry) else {
            return self.toast_message("No app for this window");
        };
        let Ok(name) = core::str::from_utf8(&name[..length]) else {
            return self.toast_message("Cannot pin");
        };
        if self.pinned.contains(name) {
            self.pinned.unpin(name);
            let mut buffer = [0u8; 48];
            let text = pin_text(b"Unpinned ", name.as_bytes(), &mut buffer);
            log(&[b"woven-shell: ", text.as_bytes()]);
            let toast = self.toast_message(text);
            return self.taskbar().union(toast);
        }
        let mut buffer = [0u8; 48];
        let text = pin_text(b"Pinned ", name.as_bytes(), &mut buffer);
        log(&[b"woven-shell: ", text.as_bytes()]);
        let toast = self.toast_message(text);
        match self.pinned.pin(name) {
            Ok(()) => self.taskbar().union(toast),
            Err(PinError::Full) => self.taskbar().union(self.toast_message("Pin list full")),
            Err(PinError::Invalid) => self.taskbar().union(self.toast_message("Cannot pin")),
            // `contains` just said otherwise; report without changing.
            Err(PinError::Duplicate) => {
                self.taskbar().union(self.toast_message("Already pinned"))
            }
        }
    }

    /// The catalog name behind a taskbar entry, copied out so nothing
    /// borrows the catalog. Windows whose owner the shell did not launch
    /// have no app, so they have no name.
    fn entry_app_name(&self, entry: TaskbarEntry) -> Option<([u8; MAX_APP_NAME], usize)> {
        let app = match entry {
            TaskbarEntry::PinnedApp(app) => self.apps[app],
            TaskbarEntry::Window(slot) => self.window_app(slot).and_then(|app| self.apps[app]),
        }?;
        let name = app.name();
        if name.is_empty() || name.len() > MAX_APP_NAME {
            return None;
        }
        let mut buffer = [0u8; MAX_APP_NAME];
        buffer[..name.len()].copy_from_slice(name);
        Some((buffer, name.len()))
    }

    /// Right-click on an empty part of the strip: flip the auto-hide
    /// preference in-session, with a toast saying which way it went.
    /// Enabling hides the bar immediately; disabling reveals it. Like
    /// pins, the preference resets on the next boot to whatever the
    /// installed settings record says; no write path is pretended.
    fn toggle_auto_hide(&mut self) -> Area {
        let changed = self.auto_hide.toggle();
        let text = if self.auto_hide.enabled() {
            "Taskbar auto-hide on"
        } else {
            "Taskbar auto-hide off"
        };
        log(&[b"woven-shell: ", text.as_bytes()]);
        let toast = self.toast_message(text);
        let strip = if changed { self.taskbar() } else { Area::new(0, 0, 0, 0) };
        strip.union(toast)
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
            maximized: false,
            restore: [0; 4],
            surface_w: width,
            surface_h: height,
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
        // An explicit re-placement (snap, tiling) ends maximization: the
        // window is where the user put it, not filling the band.
        window.maximized = false;
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
        window.surface_w = width;
        window.surface_h = height;
        window.resizing_until = 0;
        let size_changed = (width, height) != (window.width, window.height);
        if window.maximized && size_changed {
            // A maximized client may clamp to a smaller surface than the
            // band-filling frame (e.g. a fixed max buffer). Keep the frame
            // maximized and blit the smaller surface inside it; the client
            // can still reflow later by sending another WRSZ.
            log(&[
                b"woven-shell: window resized (maximized, clamped): ",
                &window.title[..window.title_length],
            ]);
            return before.union(window.footprint());
        }
        window.width = width;
        window.height = height;
        // A client-driven resize ends maximization: the client owns its new
        // canvas, and the flag must never describe a stale band-filling
        // frame. Ignore the echo of a maximize/restore WSIZ we just sent,
        // where the client reports back the size the shell already asked for.
        if size_changed {
            window.maximized = false;
        }
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
                if let Some(window) = self
                    .windows
                    .iter_mut()
                    .flatten()
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
                let mut text = [0u8; TOAST_CHARS];
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
                // The strip's controls exist only while the bar is painted.
                // Hidden, none of them may capture: the reveal happens on
                // pointer motion, not on a press.
                if !self.auto_hide.is_hidden() {
                    if let Some(index) =
                        (0..WORKSPACES).find(|&index| self.workspace_area(index).contains(x, y))
                    {
                        return self.switch_workspace(index);
                    }
                    if self.start_area().contains(x, y) {
                        return self.toggle_launcher();
                    }
                    let (taskbar_entries, entry_count) = self.taskbar_entries();
                    let entry_hit = (0..entry_count)
                        .find(|&index| self.taskbar_entry_area(index).contains(x, y))
                        .map(|index| taskbar_entries[index]);
                    if let Some(entry) = entry_hit {
                        return match entry {
                            // A window entry behaves as before: focus, minimize
                            // when already focused, restore when minimized.
                            TaskbarEntry::Window(slot) => self.taskbar_activate(slot),
                            // A pinned app with no window on this workspace
                            // starts one, like a Dash icon. This does not open
                            // the launcher.
                            TaskbarEntry::PinnedApp(app) => self.start_app(app),
                        };
                    }
                    // The three status glyphs own their own slots; they are not
                    // client tray surfaces and cannot be hit-tested as one.
                    if let Some(index) =
                        (0..STATUS_ICONS).find(|&index| self.status_area(index).contains(x, y))
                    {
                        return self.status_click(index);
                    }
                    // The Quick Settings button, and its panel's rows.
                    if self.qs_button().contains(x, y) {
                        return self.toggle_qs();
                    }
                }
                if self.qs_open {
                    let panel = self.qs_area();
                    if !panel.contains(x, y) {
                        return self.close_panels();
                    }
                    // Grid actions
                    for (index, &action) in QS_ACTION_ORDER.iter().enumerate().take(QS_ACTIONS) {
                        let cell = self.qs_action_area_at(panel, index);
                        if cell.contains(x, y) {
                            return self.qs_activate_action(action);
                        }
                    }
                    // Footer buttons are painted over the lower rows on short
                    // screens, so their hit targets must also win.
                    let footer = self.qs_footer_area_at(panel);
                    let btn_y = footer.y + 8;
                    let btn_h = 24;
                    let power_w = 54;
                    let restart_w = 58;
                    let lock_w = 40;
                    let settings_w = 60;
                    let gap = 6;
                    let power_x = panel.x + panel.width - QS_PADDING - power_w;
                    let restart_x = power_x - gap - restart_w;
                    let lock_x = restart_x - gap - lock_w;
                    let settings_x = lock_x - gap - settings_w;
                    let settings = Area::new(settings_x, btn_y, settings_w, btn_h);
                    let lock = Area::new(lock_x, btn_y, lock_w, btn_h);
                    let restart = Area::new(restart_x, btn_y, restart_w, btn_h);
                    let power = Area::new(power_x, btn_y, power_w, btn_h);
                    if settings.contains(x, y) {
                        if let Some(index) = (0..self.app_count)
                            .find(|&i| self.apps[i].is_some_and(|a| a.name() == b"WovenSettings"))
                        {
                            return panel.union(self.launch(index));
                        }
                        return panel.union(self.toast_message("Settings unavailable"));
                    }
                    if lock.contains(x, y) {
                        return self.lock();
                    }
                    if restart.contains(x, y) {
                        return self.power(true);
                    }
                    if power.contains(x, y) {
                        return self.power(false);
                    }
                    // Brightness slider
                    let bslider = self.qs_slider_area_at(panel, 0);
                    if bslider.contains(x, y) {
                        let track_x = bslider.x + 36;
                        let track_w = (bslider.width - 44).max(1);
                        let pct =
                            ((x - track_x).clamp(0, track_w) * 100 / track_w).clamp(0, 100) as u8;
                        self.brightness = pct;
                        return panel;
                    }
                    // Volume slider
                    let vslider = self.qs_slider_area_at(panel, 1);
                    if vslider.contains(x, y) {
                        let track_x = vslider.x + 36;
                        let track_w = (vslider.width - 44).max(1);
                        let pct =
                            ((x - track_x).clamp(0, track_w) * 100 / track_w).clamp(0, 100) as u8;
                        let _ = self.status.set_volume(pct);
                        return panel.union(self.status_icons_area());
                    }
                    return empty;
                }
                // The clock opens and closes the Notification Center; it is a
                // strip control, so it exists only while the bar is painted.
                if !self.auto_hide.is_hidden()
                    && (self.clock_area().contains(x, y)
                        || (self.unread > 0 && self.badge_area().contains(x, y)))
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
                    // Dragging a maximized window restores it first, so the
                    // drag moves a normally-shaped window rather than a
                    // full-band frame under the pointer's original offset.
                    let restored = if window.maximized {
                        self.toggle_maximize(slot)
                    } else {
                        Area::new(0, 0, 0, 0)
                    };
                    if let Some(current) = self.windows[slot] {
                        self.drag = Some((slot, x - current.x, y - current.y));
                    }
                    return restored.union(self.focus(slot));
                } else if window.body().contains(x, y) {
                    self.pointer_capture = Some(slot);
                    self.forward_pointer_button(slot, true);
                }
                self.focus(slot)
            }
            // Right-click a taskbar entry to pin or unpin its app for this
            // session. The PS/2 decoder and the HID ABI both deliver
            // button 1; no other desktop control consumes it yet.
            InputEvent::PointerButton {
                button: 1,
                pressed: true,
            } => {
                let (x, y) = self.pointer;
                // A right-click on a window's title bar maximizes or
                // restores that window, Win11-style. The title bar is
                // shell-owned chrome (left-click drags it), so this never
                // reaches the client.
                let title_hit = self.stack[..self.stacked]
                    .iter()
                    .rev()
                    .copied()
                    .find(|&slot| {
                        self.windows[slot]
                            .is_some_and(|window| window.frame().contains(x, y))
                    })
                    .filter(|&slot| {
                        self.windows[slot]
                            .is_some_and(|window| window.title_area().contains(x, y))
                    });
                if let Some(slot) = title_hit {
                    return self.toggle_maximize(slot);
                }
                let (taskbar_entries, entry_count) = self.taskbar_entries();
                let Some(entry) = (0..entry_count)
                    .find(|&index| self.taskbar_entry_area(index).contains(x, y))
                    .map(|index| taskbar_entries[index])
                else {
                    // An empty part of the strip flips the auto-hide
                    // preference; the toast says which way it went.
                    if self.taskbar().contains(x, y) && !self.auto_hide.is_hidden() {
                        return self.toggle_auto_hide();
                    }
                    return empty;
                };
                self.toggle_pin(entry)
            }
            InputEvent::Key(key) if self.launcher_open => self.launcher_key(key),
            InputEvent::Key(Key::F1) => {
                log(&[b"woven-shell: F1 received"]);
                self.toggle_launcher()
            }
            InputEvent::Key(Key::Escape) => {
                self.cancel_pointer_gesture();
                let Some(slot) = self.focused() else {
                    return empty;
                };
                let Some(window) = self.windows[slot] else {
                    return empty;
                };
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
        let mut moved = self.drag_to_pointer();
        if self.drag.is_none() {
            self.forward_pointer_move();
        }
        // Auto-hide: the screen's bottom row reveals a hidden bar, and any
        // position inside the strip refreshes the hold on a revealed one.
        let (_, pointer_y) = self.pointer;
        if self.auto_hide.pointer(
            pointer_y,
            self.height - 1,
            self.taskbar_top(),
            libwoven::ticks(),
            AUTO_HIDE_HOLD,
        ) {
            log(&[b"woven-shell: taskbar revealed"]);
            moved = moved.union(self.taskbar());
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
            self.stack[..self.stacked]
                .iter()
                .rev()
                .copied()
                .find(|&slot| {
                    self.windows[slot].is_some_and(|window| window.frame().contains(x, y))
                })
                .filter(|&slot| {
                    self.windows[slot].is_some_and(|window| window.body().contains(x, y))
                })
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
        // Re-sample on a slow, bounded cadence. Only the status glyphs and an
        // open Quick Settings panel are dirtied, so this never triggers a
        // full-screen recompose just to update a tray icon.
        let now = libwoven::ticks();
        if now >= self.status_next_sample {
            self.status_next_sample = now.saturating_add(STATUS_SAMPLE_PERIOD);
            let before = self.status;
            self.status.refresh();
            if before != self.status {
                dirty = dirty.union(self.status_icons_area()).union(self.qs_area());
            }
        }
        // Live settings propagation: re-read the bounded installed record on
        // the same slow cadence and apply it only when its digest changes.
        // The shell has FileRead, never FileWrite, so this is observation
        // only; the Settings app remains the sole writer.
        if self.settings_path_len > 0 && now >= self.settings_next_sample {
            self.settings_next_sample = now.saturating_add(STATUS_SAMPLE_PERIOD);
            let mut path_buf = [0u8; 256];
            let path_len = self.settings_path_len;
            path_buf[..path_len].copy_from_slice(&self.settings_path[..path_len]);
            if let Ok(path) = core::str::from_utf8(&path_buf[..path_len]) {
                if let Some(hash) = current_settings_digest(path) {
                    if hash != self.settings_hash {
                        self.settings_hash = hash;
                        load_accessibility_settings(path, self);
                        let _ = libwoven::print("woven-shell: settings reloaded\n");
                        dirty = dirty.union(self.screen());
                    }
                }
            }
        }
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
        // Auto-hide: past the hold, the bar goes back down. Panels that hang
        // off the strip (Quick Settings, Notification Center, the overview)
        // hold it up until they close.
        if self.auto_hide.tick(now)
            && !self.qs_open
            && !self.center_open
            && !self.launcher_open
        {
            log(&[b"woven-shell: taskbar hidden"]);
            dirty = dirty.union(self.taskbar());
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
    if let Some(locale) = json_string(data, b"locale").and_then(libwoven::i18n::Locale::parse_tag) {
        desktop.language = locale.language;
    }
    desktop.taskbar = TaskbarPrefs::from_values(
        json_string(data, b"taskbar_alignment"),
        json_string(data, b"taskbar_icon_size"),
    );
    desktop.auto_hide = AutoHide::from_setting(json_string(data, b"taskbar_auto_hide"));
    desktop.pinned = PinnedApps::from_setting(json_string(data, b"pinned_apps"));
    let mut settings = libwoven::accessibility::Settings::new();
    settings.apply_json(data);
    // Both fields are applied absolutely so a later record that turns them
    // off takes effect immediately, not just on the next boot.
    THEME_MODE.store(u8::from(settings.high_contrast), Ordering::Relaxed);
    desktop.reduced_motion = settings.reduced_motion;
}

/// FNV-1a digest over a bounded settings record, used only to detect changes
/// between the shell's periodic re-reads.
fn settings_digest(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn current_settings_digest(path: &str) -> Option<u64> {
    let descriptor = libwoven::open(path).ok()?;
    let mut bytes = [0u8; 512];
    let read = libwoven::read(descriptor, &mut bytes);
    let _ = libwoven::close(descriptor);
    let length = read.ok()?;
    Some(settings_digest(&bytes[..length.min(bytes.len())]))
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
    let start = data
        .windows(length)
        .position(|window| window == &needle[..length])?;
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
                    let length = path.len().min(desktop.settings_path.len());
                    desktop.settings_path[..length].copy_from_slice(&path.as_bytes()[..length]);
                    desktop.settings_path_len = length;
                    desktop.settings_hash = current_settings_digest(path).unwrap_or(0);
                    desktop.settings_next_sample = libwoven::ticks().saturating_add(STATUS_SAMPLE_PERIOD);
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
            b"--taskbar-alignment" => {
                index += 1;
                let Some(value) = args
                    .get(index)
                    .and_then(|value| core::str::from_utf8(value).ok())
                    .and_then(Alignment::parse)
                else {
                    fail(1);
                };
                desktop.taskbar.alignment = value;
            }
            b"--taskbar-icon-size" => {
                index += 1;
                let Some(value) = args
                    .get(index)
                    .and_then(|value| core::str::from_utf8(value).ok())
                    .and_then(IconSize::parse)
                else {
                    fail(1);
                };
                desktop.taskbar.icon_size = value;
            }
            b"--taskbar-auto-hide" => {
                index += 1;
                let Some(value) = args
                    .get(index)
                    .and_then(|value| core::str::from_utf8(value).ok())
                else {
                    fail(1);
                };
                desktop.auto_hide = AutoHide::from_setting(Some(value));
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
    // Sample the indicators before the first frame so no boot paints a
    // placeholder "Unavailable" set of glyphs. `refresh` only calls existing
    // read-only status syscalls and copies their results into local storage.
    desktop.status = StatusSnapshot::new();
    desktop.status_next_sample = libwoven::ticks().saturating_add(STATUS_SAMPLE_PERIOD);
    // A real observation, not a guess: the read-only status syscall either
    // returned a version-1 link record or it did not. Serial evidence can
    // distinguish "offline" from "shell cannot observe the link".
    match desktop.status.network {
        NetworkStatus::Online { ipv4 } => {
            let mut ip = [0u8; 15];
            let len = status::format_ipv4(ipv4, &mut ip);
            log(&[b"woven-shell: network online ", &ip[..len]]);
        }
        NetworkStatus::Offline => log(&[b"woven-shell: network offline"]),
        NetworkStatus::Unavailable => log(&[b"woven-shell: network unavailable"]),
    }
    log(&[b"woven-shell: status indicators initialized"]);
    // With accounts, the desktop starts at the sign-in screen.
    desktop.locked = desktop.accounts.iter().any(Option::is_some);
    desktop.pointer = (width / 2, height / 2);
    desktop.app_count = load_catalog(&mut desktop.apps);
    let mut catalog_digits = [0u8; 20];
    let catalog_count = libwoven::format_u64(desktop.app_count as u64, &mut catalog_digits);
    log(&[b"woven-shell: catalog entries=", catalog_count]);
    // Typography diagnostics: report whether the staged system face is
    // reachable from this process, so a bitmap fallback is visible in serial
    // evidence instead of only in screenshots.
    match libwoven::graphics::load_system_font_mapping() {
        Ok(bytes) => {
            let mut length_digits = [0u8; 20];
            let length = libwoven::format_u64(bytes.len() as u64, &mut length_digits);
            match libwoven::graphics::TrueTypeFace::from_bytes(bytes) {
                Ok(face) => {
                    let mut unit_digits = [0u8; 20];
                    let units =
                        libwoven::format_u64(u64::from(face.units_per_em()), &mut unit_digits);
                    log(&[
                        b"woven-shell: system font parsed bytes=",
                        length,
                        b" units_per_em=",
                        units,
                    ]);
                }
                Err(_) => log(&[b"woven-shell: system font failed to parse"]),
            }
        }
        Err(_) => log(&[b"woven-shell: system font mapping unavailable"]),
    }
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
    log(&[b"woven-shell: full recompose ticks=", recompose_text]);
    // The first present takes the display from the text terminal.
    if libwoven::compositor_present().is_err() {
        fail(6);
    }
    let mut pid = [0u8; 20];
    let pid = libwoven::format_u64(libwoven::current_pid().0, &mut pid);
    log(&[b"woven-shell: desktop ready pid=", pid]);
    let alignment = match desktop.taskbar.alignment {
        taskbar_prefs::Alignment::Center => b"center" as &[u8],
        taskbar_prefs::Alignment::Left => b"left",
    };
    let icon_size = match desktop.taskbar.icon_size {
        taskbar_prefs::IconSize::Small => b"small" as &[u8],
        taskbar_prefs::IconSize::Medium => b"medium",
        taskbar_prefs::IconSize::Large => b"large",
    };
    let auto_hide = if desktop.auto_hide.enabled() {
        b"on" as &[u8]
    } else {
        b"off"
    };
    log(&[
        b"woven-shell: taskbar layout alignment=",
        alignment,
        b" icon_size=",
        icon_size,
        b" auto_hide=",
        auto_hide,
    ]);

    // The overview draws every app label on its first F1 paint. Warm the fixed
    // glyph cache with that bounded label set now so cold outline rasterisation
    // cannot consume the input-to-frame budget.
    let mut warm: [&[u8]; MAX_APPS + 2] = [b""; MAX_APPS + 2];
    let mut warm_count = 0;
    for app in desktop.apps[..desktop.app_count].iter().flatten() {
        let name = app.name();
        warm[warm_count] = name.strip_prefix(b"Woven").unwrap_or(name);
        warm_count += 1;
    }
    warm[warm_count] = b"Type to search";
    warm[warm_count + 1] = b"No open windows on this workspace";
    libwoven::warm_system_glyphs(&warm[..warm_count + 2], BODY_TEXT);

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
    // Published once the first unlocked frame is actually on the scanout, so
    // harnesses can stop guessing when the sign-in transition has settled.
    let mut desktop_frame_ready = false;
    let mut overview_frame_ready = false;
    let mut overview_refresh_pending = false;
    loop {
        // Switch frame rate as animations start and finish.
        if desktop.animating() != fast {
            fast = !fast;
            clock = libwoven::FrameClock::new(if fast { ANIMATION_PERIOD } else { FRAME_PERIOD });
        }
        let mut dirty = Dirty::new();
        if overview_refresh_pending {
            overview_refresh_pending = false;
            dirty.add(desktop.overview_area());
        }
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
                let (px, py) = desktop.pointer;
                let taskbar = desktop.taskbar();
                if taskbar.contains(px, py)
                    || taskbar.contains(pointer_before.0 .0, pointer_before.0 .1)
                {
                    dirty.add(taskbar);
                }
                if desktop.launcher_open {
                    dirty.add(desktop.overview_area());
                }
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
        if !desktop_frame_ready && !desktop.locked {
            desktop_frame_ready = true;
            log(&[b"woven-shell: desktop frame ready"]);
        }
        // Publish the first presented overview frame so harnesses can wait for
        // the real state instead of racing a slow cold-cache paint.
        if desktop.launcher_open && !overview_frame_ready {
            overview_frame_ready = true;
            log(&[b"woven-shell: overview frame ready"]);
            desktop.overview_detail = true;
            overview_refresh_pending = true;
        } else if !desktop.launcher_open {
            overview_frame_ready = false;
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
fn panic(info: &PanicInfo) -> ! {
    // A silent `exit(101)` left a crashed shell indistinguishable from an
    // idle one: the compositor keeps scanning out the last frame, so the
    // desktop simply freezes with no serial evidence. Report the location
    // and payload first; the code is still 101 so existing gates that look
    // for an abnormal exit keep working.
    let _ = libwoven::print("woven-shell: panic: ");
    if let Some(message) = info.message().as_str() {
        let _ = libwoven::print(message);
    } else {
        let _ = libwoven::print("<non-literal panic message>");
    }
    if let Some(location) = info.location() {
        let _ = libwoven::print(" at ");
        let _ = libwoven::print(location.file());
        let _ = libwoven::print(":");
        let mut digits = [0u8; 20];
        let _ = libwoven::write(
            libwoven::syscall::STDOUT,
            libwoven::format_u64(u64::from(location.line()), &mut digits),
        );
    }
    let _ = libwoven::print("\n");
    libwoven::exit(101)
}
