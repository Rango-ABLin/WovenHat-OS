//! Design tokens: colors, sizes, and the high-contrast theme resolution
//! (`theme_*`) that the rest of the shell draws through instead of the raw
//! constants directly. Extracted from `main.rs` verbatim (M1, behavior
//! preserving) as the first step of the Overview module split; the Overview
//! layout's own tokens (radii, elevations, motion durations, light/dark and
//! reduced-motion variants) land here too as they're introduced in M2+.

use core::sync::atomic::{AtomicU8, Ordering};

use libwoven::{font, rgb};

/// Wallpaper gradient, top to bottom.
pub const WALLPAPER_TOP: u32 = rgb(11, 29, 58);
pub const WALLPAPER_BOTTOM: u32 = rgb(16, 78, 82);
/// The weave: faint diagonal bands over the gradient, `WEAVE_PITCH` pixels
/// wide. Axis-aligned squares with a highlight seam at each boundary read
/// as a literal checkerboard at screen scale rather than a fabric texture,
/// so this runs along the diagonal instead, with no seam line, a wider
/// pitch and lower contrast.
pub const WEAVE: u32 = rgb(255, 255, 255);
pub const WEAVE_SHADE: u32 = rgb(0, 0, 0);
pub const WEAVE_TINT_OPACITY: u8 = 6;
pub const WEAVE_PITCH: i32 = 220;

pub const TASKBAR_HEIGHT: i32 = 24;
pub const TASKBAR: u32 = rgb(10, 18, 30);
pub const TASKBAR_OPACITY: u8 = 216;
pub const ACCENT: u32 = rgb(76, 190, 188);
pub const START_X: i32 = 4;
pub const START_WIDTH: usize = 44;
pub const START_HEIGHT: usize = 16;
pub const START_FILL: u32 = rgb(28, 115, 145);
pub const START_LABEL: &str = "Woven";
/// The WovenStart icon: a basket-weave glyph, two shades suggesting strands
/// crossing over and under. `A` is the strand that is on top, `D` the one
/// beneath it; `.` is transparent (the button fill shows through).
pub const START_ICON: [&[u8; 8]; 8] = [
    b"AA..AA..",
    b"AA..AA..",
    b"..DD..DD",
    b"..DD..DD",
    b"AA..AA..",
    b"AA..AA..",
    b"..DD..DD",
    b"..DD..DD",
];
pub const START_ICON_SIZE: i32 = 8;
pub const START_ICON_A: u32 = rgb(235, 225, 190);
pub const START_ICON_D: u32 = rgb(165, 125, 70);
/// Left margin from the button's edge to the icon, chosen so it clears the
/// background-fill sample point Stage 20.1's self-test reads at `(6, top+6)`.
pub const START_ICON_MARGIN: i32 = 3;
pub const START_ICON_GAP: i32 = 2;
pub const TEXT: u32 = rgb(255, 255, 255);
/// Clock text is right-aligned this far from the right edge.
pub const CLOCK_MARGIN: i32 = 6;
/// Room for the widest clock reading, `HH:MM:SS`, plus its margin.
pub const CLOCK_WIDTH: i32 = 8 * font::ADVANCE + CLOCK_MARGIN;

/// Taskbar window entries, left to right after the start button.
pub const ENTRY_X: i32 = START_X + START_WIDTH as i32 + 8;
pub const ENTRY_WIDTH: i32 = 80;
pub const ENTRY_GAP: i32 = 4;
pub const ENTRY_ACTIVE: u32 = rgb(70, 120, 210);
pub const ENTRY_INACTIVE: u32 = rgb(44, 52, 68);
/// Characters of a title that fit in an entry.
pub const ENTRY_CHARS: usize = ((ENTRY_WIDTH - 8) / font::ADVANCE) as usize;

/// Tray icons, right to left from the clock.
pub const TRAY_ICON: i32 = 16;
pub const TRAY_GAP: i32 = 4;
pub const MAX_TRAY: usize = 4;

pub const TITLE_HEIGHT: i32 = 14;
// Teal-tinted, matching ACCENT/START_FILL, instead of an unrelated
// saturated blue that clashed with the rest of the desktop's palette.
pub const TITLE_ACTIVE: u32 = rgb(24, 98, 128);
pub const TITLE_INACTIVE: u32 = rgb(38, 44, 56);
pub const MAX_TITLE: usize = 24;
/// Shadow bands, outermost first, as (offset, opacity) of black over what
/// lies beneath. The focused window's shadow is deeper: its elevation.
pub const FOCUSED_SHADOW: [(i32, u8); 2] = [(8, 24), (4, 48)];
pub const UNFOCUSED_SHADOW: [(i32, u8); 2] = [(4, 20), (2, 40)];
/// The furthest any shadow band reaches beyond its window.
pub const SHADOW_REACH: i32 = 8;
pub const SHADOW: u32 = rgb(0, 0, 0);

/// The close control: a square at the right of the title bar.
pub const CLOSE_SIZE: i32 = 10;
pub const CLOSE_FILL: u32 = rgb(170, 60, 60);
pub const MAX_WINDOWS: usize = 4;
/// Workspaces, and the switcher boxes that show them.
pub const WORKSPACES: usize = 4;
pub const WORKSPACE_BOX: i32 = 12;
pub const WORKSPACE_GAP: i32 = 2;
/// New windows cascade from here, this far apart.
pub const CASCADE_ORIGIN: i32 = 40;
pub const CASCADE_STEP: i32 = 24;

pub const TOAST_WIDTH: i32 = 180;
pub const TOAST_HEIGHT: i32 = 22;
pub const TOAST_MARGIN: i32 = 8;
pub const TOAST_OPACITY: u8 = 216;
pub const TOAST_CHARS: usize = ((TOAST_WIDTH - 12) / font::ADVANCE) as usize;
pub const TOAST_TICKS: u64 = 4 * libwoven::time::TICKS_PER_SECOND;

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
// The launcher stores catalog metadata in process-owned memory rather than
// truncating the production catalog at its old ten-entry fixture size. This
// is a generous staging capacity; the kernel still validates every catalog
// entry and rejects malformed or resource-exhausting inputs.
pub const MAX_APPS: usize = 64;
pub const MAX_APP_NAME: usize = 20;
pub const MAX_EXEC: usize = 96;
pub const MAX_QUERY: usize = 16;
pub const MAX_CHILDREN: usize = 32;

pub const PANEL_X: i32 = 4;
pub const PANEL_WIDTH: i32 = 160;
pub const PANEL_ROW: i32 = 14;
pub const PANEL_FILL: u32 = rgb(28, 32, 44);
pub const PANEL_OPACITY: u8 = 216;
pub const PANEL_TITLE: &str = "WovenStart";

pub static THEME_MODE: AtomicU8 = AtomicU8::new(0);

pub fn high_contrast() -> bool {
    THEME_MODE.load(Ordering::Relaxed) != 0
}

pub fn theme_accent() -> u32 {
    if high_contrast() {
        rgb(255, 255, 0)
    } else {
        ACCENT
    }
}

pub fn theme_text() -> u32 {
    TEXT
}

/// Secondary/placeholder text (M5): a muted gray normally, but
/// high-contrast mode wants maximum contrast against its own panel
/// color (black, see `theme_panel`), not a muted tone, so this is a
/// separate color rather than `theme_text` at reduced opacity.
pub fn theme_muted() -> u32 {
    if high_contrast() {
        rgb(255, 255, 255)
    } else {
        rgb(150, 160, 175)
    }
}

pub fn theme_panel() -> u32 {
    if high_contrast() {
        rgb(0, 0, 0)
    } else {
        PANEL_FILL
    }
}

pub fn theme_taskbar() -> u32 {
    if high_contrast() {
        rgb(0, 0, 0)
    } else {
        TASKBAR
    }
}

pub fn theme_toast() -> u32 {
    theme_panel()
}

pub fn theme_title(focused: bool) -> u32 {
    if high_contrast() {
        if focused {
            theme_accent()
        } else {
            rgb(80, 80, 80)
        }
    } else if focused {
        TITLE_ACTIVE
    } else {
        TITLE_INACTIVE
    }
}

pub fn theme_start_fill() -> u32 {
    if high_contrast() {
        rgb(0, 0, 0)
    } else {
        START_FILL
    }
}

pub fn theme_entry(active: bool) -> u32 {
    if high_contrast() {
        if active {
            theme_accent()
        } else {
            rgb(0, 0, 0)
        }
    } else if active {
        ENTRY_ACTIVE
    } else {
        ENTRY_INACTIVE
    }
}
