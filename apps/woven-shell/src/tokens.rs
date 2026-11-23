//! Design tokens: colors, sizes, and high-contrast theme resolution.
//!
//! The shell draws through semantic `theme_*` helpers and named constants rather
//! than ad-hoc colors. This module is the Stage 1 Windows 11-inspired visual
//! foundation: calmer surfaces, clearer elevation, consistent accent/stroke
//! roles, and preserved high-contrast behavior. Layout geometry changes land in
//! later stages; token names remain compatible where possible.

use core::sync::atomic::{AtomicU8, Ordering};

use libwoven::{font, rgb};

// ---------------------------------------------------------------------------
// Brand / wallpaper
// ---------------------------------------------------------------------------

/// Wallpaper gradient, top to bottom. A calm Win11-like blue-teal depth field,
/// keeping WovenHat's teal identity while reducing harsh contrast.
pub const WALLPAPER_TOP: u32 = rgb(16, 36, 74);
pub const WALLPAPER_BOTTOM: u32 = rgb(18, 84, 96);
/// Faint diagonal weave over the gradient. Kept subtle so app chrome reads as
/// foreground, not texture noise.
pub const WEAVE: u32 = rgb(255, 255, 255);
pub const WEAVE_SHADE: u32 = rgb(0, 0, 0);
pub const WEAVE_TINT_OPACITY: u8 = 4;
pub const WEAVE_PITCH: i32 = 220;

// ---------------------------------------------------------------------------
// Semantic surfaces
// ---------------------------------------------------------------------------

/// WovenHat accent: teal brand aligned with `libwoven::ui::Theme::DARK.accent`.
pub const ACCENT: u32 = rgb(76, 190, 188);
/// A softer accent fill for large surfaces (Start button, selected pills).
pub const ACCENT_SOFT: u32 = rgb(34, 118, 132);
pub const TEXT: u32 = rgb(243, 246, 251);
pub const TEXT_MUTED: u32 = rgb(166, 178, 196);
pub const STROKE: u32 = rgb(255, 255, 255);

/// Desktop/taskbar shell surface ("mica-like" approximation in software).
pub const TASKBAR: u32 = rgb(24, 32, 46);
pub const TASKBAR_OPACITY: u8 = 188;
pub const TASKBAR_HEIGHT: i32 = 24;

/// Floating panels/cards (Start, Quick Settings, Notification Center).
pub const PANEL_FILL: u32 = rgb(32, 40, 56);
pub const PANEL_OPACITY: u8 = 232;

/// Toasts are slightly more opaque for readability.
pub const TOAST_FILL: u32 = rgb(36, 44, 60);
pub const TOAST_OPACITY: u8 = 236;

// ---------------------------------------------------------------------------
// Taskbar / chrome geometry
// ---------------------------------------------------------------------------

pub const START_HEIGHT: usize = 16;
pub const START_FILL: u32 = ACCENT_SOFT;
/// Clock text is right-aligned this far from the right edge.
pub const CLOCK_MARGIN: i32 = 6;
/// Room for the widest clock reading, `HH:MM:SS`, plus its margin.
pub const CLOCK_WIDTH: i32 = 8 * font::ADVANCE + CLOCK_MARGIN;

pub const ENTRY_ACTIVE: u32 = rgb(58, 96, 132);
pub const ENTRY_INACTIVE: u32 = rgb(48, 58, 74);

/// Tray icons, right to left from the clock.
pub const TRAY_ICON: i32 = 16;
pub const TRAY_GAP: i32 = 4;
pub const MAX_TRAY: usize = 4;

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

pub const TITLE_HEIGHT: i32 = 14;
pub const TITLE_ACTIVE: u32 = rgb(30, 94, 120);
pub const TITLE_INACTIVE: u32 = rgb(52, 60, 74);
pub const MAX_TITLE: usize = 24;

/// Shadow bands, outermost first, as (offset, opacity) of black.
pub const FOCUSED_SHADOW: [(i32, u8); 3] = [(10, 18), (6, 34), (2, 52)];
pub const UNFOCUSED_SHADOW: [(i32, u8); 3] = [(8, 12), (4, 24), (1, 36)];
/// The furthest any shadow band reaches beyond its window.
pub const SHADOW_REACH: i32 = 10;
pub const SHADOW: u32 = rgb(0, 0, 0);

/// Close control: a square at the right of the title bar.
pub const CLOSE_SIZE: i32 = 10;
pub const CLOSE_FILL: u32 = rgb(196, 72, 72);
pub const MAX_WINDOWS: usize = 4;

// ---------------------------------------------------------------------------
// Workspaces / layout
// ---------------------------------------------------------------------------

pub const WORKSPACES: usize = 4;
pub const WORKSPACE_BOX: i32 = 12;
pub const WORKSPACE_GAP: i32 = 2;
/// New windows cascade from here, this far apart.
pub const CASCADE_ORIGIN: i32 = 40;
pub const CASCADE_STEP: i32 = 24;

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

pub const TOAST_WIDTH: i32 = 240;
pub const TOAST_HEIGHT: i32 = 24;
pub const TOAST_MARGIN: i32 = 10;
pub const TOAST_CHARS: usize = ((TOAST_WIDTH - 12) / font::ADVANCE) as usize;
pub const TOAST_TICKS: u64 = 4 * libwoven::time::TICKS_PER_SECOND;

// ---------------------------------------------------------------------------
// Cursor / catalog / launcher
// ---------------------------------------------------------------------------

/// The pointer: `B` black outline, `W` white fill, `.` transparent. The tip
/// is the top-left pixel.
pub const CURSOR: [&[u8; 7]; 10] = [
    b"B......", b"BB.....", b"BWB....", b"BWWB...", b"BWWWB..", b"BWWWWB.", b"BWWWWWB", b"BWWBBBB",
    b"BWB....", b"BB.....",
];
pub const CURSOR_WIDTH: i32 = 7;
pub const CURSOR_HEIGHT: i32 = 10;

/// The application catalog: one `*.app` file per application.
pub const CATALOG: &str = "/share/applications";
pub const MAX_APPS: usize = 64;
pub const MAX_APP_NAME: usize = 20;
pub const MAX_EXEC: usize = 96;
pub const MAX_QUERY: usize = 16;
pub const MAX_CHILDREN: usize = 32;

pub const PANEL_ROW: i32 = 14;

// ---------------------------------------------------------------------------
// Theme resolution
// ---------------------------------------------------------------------------

pub static THEME_MODE: AtomicU8 = AtomicU8::new(0);

pub fn high_contrast() -> bool {
    THEME_MODE.load(Ordering::Relaxed) != 0
}

pub fn theme_accent() -> u32 {
    if high_contrast() { rgb(255, 255, 0) } else { ACCENT }
}

pub fn theme_text() -> u32 {
    TEXT
}

/// Secondary/placeholder text. High-contrast keeps maximum legibility.
pub fn theme_muted() -> u32 {
    if high_contrast() { rgb(255, 255, 255) } else { TEXT_MUTED }
}

pub fn theme_panel() -> u32 {
    if high_contrast() { rgb(0, 0, 0) } else { PANEL_FILL }
}

pub fn theme_taskbar() -> u32 {
    if high_contrast() { rgb(0, 0, 0) } else { TASKBAR }
}

pub fn theme_toast() -> u32 {
    if high_contrast() { rgb(0, 0, 0) } else { TOAST_FILL }
}

pub fn theme_stroke() -> u32 {
    if high_contrast() { rgb(255, 255, 255) } else { STROKE }
}

pub fn theme_title(focused: bool) -> u32 {
    if high_contrast() {
        if focused { theme_accent() } else { rgb(80, 80, 80) }
    } else if focused {
        TITLE_ACTIVE
    } else {
        TITLE_INACTIVE
    }
}

pub fn theme_start_fill() -> u32 {
    if high_contrast() { rgb(0, 0, 0) } else { START_FILL }
}

pub fn theme_entry(active: bool) -> u32 {
    if high_contrast() {
        if active { theme_accent() } else { rgb(0, 0, 0) }
    } else if active {
        ENTRY_ACTIVE
    } else {
        ENTRY_INACTIVE
    }
}
