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
//!
//! A surface is accepted only if the sender owns it: the token names its
//! owner, and the kernel enforces that name on every operation. A window or
//! icon goes away when its surface does, normally because its owner exited.
//! Keystrokes go to the focused window's owner as one-byte messages.
//!
//! Every window casts a soft shadow of two translucent bands, offset down and
//! right; the focused window sits higher, so its shadow falls further and
//! darker. Shadows stay off the taskbar.
//!
//! A window is moved by dragging its title bar. Its close control sends the
//! owner `WCLS`, a request: the window goes when the owner releases its
//! surface, so an application can finish first, or decline.
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

use libwoven::{
    blend, font, process, rgb, wait, Args, Canvas, DateTime, FrameEvent, GraphicsSurface,
    InputEvent, Key, Region,
};

libwoven::entry!(main);

/// Wallpaper gradient, top to bottom.
const WALLPAPER_TOP: u32 = rgb(18, 42, 74);
const WALLPAPER_BOTTOM: u32 = rgb(64, 28, 88);
/// The weave: a faint light grid every `WEAVE_PITCH` pixels.
const WEAVE: u32 = rgb(255, 255, 255);
const WEAVE_OPACITY: u8 = 28;
const WEAVE_PITCH: i32 = 32;

const TASKBAR_HEIGHT: i32 = 24;
const TASKBAR: u32 = rgb(20, 24, 32);
const TASKBAR_OPACITY: u8 = 216;
const ACCENT: u32 = rgb(90, 140, 220);
const START_X: i32 = 4;
const START_WIDTH: usize = 44;
const START_HEIGHT: usize = 16;
const START_FILL: u32 = rgb(52, 96, 180);
const START_LABEL: &str = "Woven";
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
/// New windows cascade from here, this far apart.
const CASCADE_ORIGIN: i32 = 40;
const CASCADE_STEP: i32 = 24;

const TOAST_WIDTH: i32 = 180;
const TOAST_HEIGHT: i32 = 22;
const TOAST_MARGIN: i32 = 8;
const TOAST_FILL: u32 = rgb(30, 34, 46);
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
const MAX_APPS: usize = 8;
const MAX_APP_NAME: usize = 20;
const MAX_EXEC: usize = 96;
const MAX_QUERY: usize = 16;
const MAX_CHILDREN: usize = 8;

const PANEL_X: i32 = 4;
const PANEL_WIDTH: i32 = 160;
const PANEL_ROW: i32 = 14;
const PANEL_FILL: u32 = rgb(28, 32, 44);
const PANEL_TITLE: &str = "WovenStart";

const TILE_WIDTH: usize = 320;
const TILE_HEIGHT: usize = 40;
const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;

/// The shell wakes at least four times a second for the clock, the toast
/// timer and window liveness.
const FRAME_PERIOD: u64 = libwoven::time::TICKS_PER_SECOND / 4;

/// The one tile buffer, in .bss rather than on the stack.
struct TileBuffer(UnsafeCell<[u32; TILE_PIXELS]>);

// SAFETY: the shell is single-threaded, and `main` takes the only reference.
unsafe impl Sync for TileBuffer {}

static TILE: TileBuffer = TileBuffer(UnsafeCell::new([0; TILE_PIXELS]));

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
/// gradient with a faint grid woven over it.
fn wallpaper(x: i32, y: i32, height: i32) -> u32 {
    let span = (height - 1).max(1);
    let t = (y.clamp(0, span) * 255 / span) as u8;
    let base = blend(WALLPAPER_BOTTOM, WALLPAPER_TOP, t);
    if x % WEAVE_PITCH == 0 || y % WEAVE_PITCH == 0 {
        blend(WEAVE, base, WEAVE_OPACITY)
    } else {
        base
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

/// `bytes` as text, cut to `limit` characters; anything not printable ASCII
/// is shown as `?`, so the font's replacement box never appears.
fn label(bytes: &[u8], limit: usize, buffer: &mut [u8; 32]) -> usize {
    let length = bytes.len().min(limit).min(buffer.len());
    for (slot, byte) in buffer.iter_mut().zip(&bytes[..length]) {
        *slot = if (b' '..=b'~').contains(byte) { *byte } else { b'?' };
    }
    length
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
}

impl Window {
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
fn load_catalog() -> ([Option<App>; MAX_APPS], usize) {
    let mut apps = [None; MAX_APPS];
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
    (apps, count)
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
    width: i32,
    height: i32,
    seconds: bool,
    /// The time on screen, `None` if the clock could not be read.
    time: Option<DateTime>,
    windows: [Option<Window>; MAX_WINDOWS],
    /// Window slots bottom to top; the last is focused.
    stack: [usize; MAX_WINDOWS],
    stacked: usize,
    /// Windows opened so far, for the cascade.
    opened: i32,
    tray: [Option<TrayIcon>; MAX_TRAY],
    toast: Option<Toast>,
    pointer: (i32, i32),
    pointer_visible: bool,
    /// The window being dragged, and where in it the pointer took hold.
    drag: Option<(usize, i32, i32)>,
    apps: [Option<App>; MAX_APPS],
    app_count: usize,
    launcher_open: bool,
    query: [u8; MAX_QUERY],
    query_length: usize,
    selected: usize,
    children: [Option<Child>; MAX_CHILDREN],
}

impl Desktop {
    fn screen(&self) -> Area {
        Area::new(0, 0, self.width, self.height)
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

    fn render_panel(&self, canvas: &mut Canvas<'_>) {
        let area = self.panel_area();
        let (x, y) = (area.x, area.y);
        let inner = (area.width - 2) as usize;
        canvas.fill_rect(x, y, area.width as usize, area.height as usize, ACCENT);
        canvas.fill_rect(x + 1, y + 1, inner, (area.height - 2) as usize, PANEL_FILL);
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

    /// Open or close the launcher. Returns the area to repaint.
    fn toggle_launcher(&mut self) -> Area {
        let before = self.panel_area();
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
            Key::Char(_) => {}
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
    fn entries(&self) -> impl Iterator<Item = (usize, &Window)> {
        self.windows
            .iter()
            .enumerate()
            .filter_map(|(slot, window)| window.as_ref().map(|window| (slot, window)))
    }

    /// The desktop layer: wallpaper and taskbar, with its start button,
    /// window entries and clock.
    fn render_desktop(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        canvas.shade(|x, y| wallpaper(x, y, height));

        let top = self.taskbar_top();
        let width = self.width as usize;
        canvas.blend_rect(0, top, width, TASKBAR_HEIGHT as usize, TASKBAR, TASKBAR_OPACITY);
        canvas.fill_rect(0, top, width, 1, ACCENT);

        canvas.fill_rect(START_X, top + 4, START_WIDTH, START_HEIGHT, START_FILL);
        let label_x = START_X + (START_WIDTH - font::text_width(START_LABEL)) as i32 / 2;
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
    }

    fn render_title(window: &Window, focused: bool, canvas: &mut Canvas<'_>) {
        let area = window.title_area();
        let fill = if focused { TITLE_ACTIVE } else { TITLE_INACTIVE };
        canvas.fill_rect(area.x, area.y, area.width as usize, area.height as usize, fill);
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

    fn render_toast(&self, toast: &Toast, canvas: &mut Canvas<'_>) {
        let area = self.toast_area();
        let (width, height) = (area.width as usize, area.height as usize);
        canvas.fill_rect(area.x, area.y, width, height, ACCENT);
        canvas.fill_rect(area.x + 1, area.y + 1, width - 2, height - 2, TOAST_FILL);
        let text = core::str::from_utf8(&toast.text[..toast.length]).unwrap_or("");
        canvas.draw_text(area.x + 6, area.y + 8, text, TEXT);
    }

    /// The pointer over whatever is already on screen beneath it.
    fn render_cursor(&self, canvas: &mut Canvas<'_>) {
        canvas.shade(|x, y| libwoven::screen_pixel(x, y).unwrap_or(0));
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
        let above_taskbar = Area::new(0, 0, self.width, self.taskbar_top());
        for &slot in &self.stack[..self.stacked] {
            let Some(window) = &self.windows[slot] else { continue };
            let bands = if Some(slot) == focused { FOCUSED_SHADOW } else { UNFOCUSED_SHADOW };
            let frame = window.frame();
            for (offset, opacity) in bands {
                let band = Area::new(frame.x + offset, frame.y + offset, frame.width, frame.height);
                let part = band.intersect(area).intersect(above_taskbar);
                if !part.is_empty() {
                    painter.blend(part, SHADOW, opacity);
                }
            }
            let title = window.title_area().intersect(area);
            if !title.is_empty() {
                let active = Some(slot) == focused;
                painter.draw(title, |canvas| Self::render_title(window, active, canvas));
            }
            let body = window.body().intersect(area);
            if !body.is_empty() {
                let region = body.relative_to(window.x, window.y);
                let _ = libwoven::compositor_blit_region_shared(window.token, region, body.x, body.y);
            }
        }

        if let Some(toast) = &self.toast {
            let part = self.toast_area().intersect(area);
            if !part.is_empty() {
                painter.draw(part, |canvas| self.render_toast(toast, canvas));
            }
        }

        let panel = self.panel_area().intersect(area);
        if !panel.is_empty() {
            painter.draw(panel, |canvas| self.render_panel(canvas));
        }

        let cursor = self.cursor_area().intersect(area);
        if !cursor.is_empty() {
            painter.draw(cursor, |canvas| self.render_cursor(canvas));
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

    /// Open a window for `owner`. Returns the area to repaint.
    fn open(&mut self, owner: process::Id, message: &[u8]) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        if message.len() < 16 {
            return empty;
        }
        let token = u64::from_le_bytes(message[4..12].try_into().unwrap_or([0; 8]));
        let width = i32::from(u16::from_le_bytes([message[12], message[13]]));
        let height = i32::from(u16::from_le_bytes([message[14], message[15]]));
        if libwoven::surface_owner(token) != owner
            || !(1..=self.width).contains(&width)
            || !(1..=self.taskbar_top() - TITLE_HEIGHT).contains(&height)
        {
            return empty;
        }
        let Some(slot) = self.windows.iter().position(Option::is_none) else {
            return empty;
        };
        let offset = CASCADE_ORIGIN + (self.opened % 6) * CASCADE_STEP;
        self.opened += 1;
        // Keep the whole frame above the taskbar and on screen.
        let x = offset.min((self.width - width).max(0));
        let y = offset.min((self.taskbar_top() - height).max(TITLE_HEIGHT));
        let mut title = [0u8; MAX_TITLE];
        let title_length = (message.len() - 16).min(MAX_TITLE);
        title[..title_length].copy_from_slice(&message[16..16 + title_length]);
        let window = Window { owner, token, x, y, width, height, title, title_length };
        self.windows[slot] = Some(window);
        let previous = self.focused().and_then(|old| self.windows[old]).map(|w| w.footprint());
        self.stack[self.stacked] = slot;
        self.stacked += 1;
        log(&[b"woven-shell: window opened: ", &title[..title_length]]);
        window.footprint().union(previous.unwrap_or(empty)).union(self.taskbar())
    }

    /// Remove window `slot`. Returns the area to repaint.
    fn close(&mut self, slot: usize) -> Area {
        let Some(window) = self.windows[slot].take() else {
            return Area::new(0, 0, 0, 0);
        };
        if let Some(position) = self.stack[..self.stacked].iter().position(|&s| s == slot) {
            self.stack[position..self.stacked].rotate_left(1);
            self.stacked -= 1;
        }
        if self.drag.is_some_and(|(dragged, _, _)| dragged == slot) {
            self.drag = None;
        }
        log(&[b"woven-shell: window closed: ", &window.title[..window.title_length]]);
        // The newly focused window's title and shadow change too.
        let next = self.focused().and_then(|top| self.windows[top]).map(|w| w.footprint());
        window.footprint().union(next.unwrap_or(Area::new(0, 0, 0, 0))).union(self.taskbar())
    }

    fn handle_message(&mut self, sender: process::Id, message: &[u8]) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        match message.get(..4) {
            Some(b"WOPN") => self.open(sender, message),
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
                self.tray_area(index)
            }
            Some(b"NOTE") => {
                let mut text = [0u8; 32];
                let length = label(&message[4..], TOAST_CHARS, &mut text);
                self.toast = Some(Toast {
                    text,
                    length,
                    until: libwoven::ticks() + TOAST_TICKS,
                });
                log(&[b"woven-shell: notification: ", &text[..length]]);
                self.toast_area()
            }
            _ => empty,
        }
    }

    fn handle_input(&mut self, event: InputEvent) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        match event {
            InputEvent::PointerMove { dx, dy } => {
                let before = self.cursor_area();
                self.pointer = (
                    (self.pointer.0 + i32::from(dx)).clamp(0, self.width - 1),
                    (self.pointer.1 + i32::from(dy)).clamp(0, self.height - 1),
                );
                self.pointer_visible = true;
                let moved = self.drag_to_pointer();
                before.union(self.cursor_area()).union(moved)
            }
            InputEvent::PointerButton { button: 0, pressed: false } => {
                self.drag = None;
                empty
            }
            InputEvent::PointerButton { button: 0, pressed: true } => {
                let (x, y) = self.pointer;
                if self.start_area().contains(x, y) {
                    return self.toggle_launcher();
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
                }
                self.focus(slot)
            }
            InputEvent::Key(key) if self.launcher_open => self.launcher_key(key),
            InputEvent::Key(Key::F1) => self.toggle_launcher(),
            InputEvent::Key(Key::Char(character)) if character.is_ascii() => {
                if let Some(window) = self.focused().and_then(|slot| self.windows[slot]) {
                    let _ = libwoven::message_send(window.owner, &[character as u8]);
                }
                empty
            }
            _ => empty,
        }
    }

    /// Move the dragged window under the pointer, keeping its frame on screen
    /// and above the taskbar. Returns the area to repaint.
    fn drag_to_pointer(&mut self) -> Area {
        let empty = Area::new(0, 0, 0, 0);
        let Some((slot, grab_x, grab_y)) = self.drag else { return empty };
        let Some(window) = self.windows[slot].as_mut() else { return empty };
        let before = window.footprint();
        let top = self.height - TASKBAR_HEIGHT;
        window.x = (self.pointer.0 - grab_x).clamp(0, (self.width - window.width).max(0));
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
                    dirty = dirty.union(changed);
                }
                Ok(None) => {}
                Err(_) => dirty = dirty.union(self.close(slot)),
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

    /// Advance the clock and the toast timer, and reap exited
    /// applications. Returns the area to repaint.
    fn tick(&mut self) -> Area {
        self.reap();
        let mut dirty = Area::new(0, 0, 0, 0);
        let time = libwoven::clock().ok();
        if clock_text(time, self.seconds) != clock_text(self.time, self.seconds) {
            self.time = time;
            dirty = dirty.union(self.clock_area());
        }
        if self.toast.as_ref().is_some_and(|toast| libwoven::ticks() >= toast.until) {
            self.toast = None;
            dirty = dirty.union(self.toast_area());
        }
        dirty
    }
}

fn main(args: Args) -> ! {
    let seconds = match args.get(1) {
        None => false,
        Some(b"--seconds") => true,
        Some(_) => fail(1),
    };
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
    let (apps, app_count) = load_catalog();
    let mut desktop = Desktop {
        width,
        height,
        seconds,
        time: libwoven::clock().ok(),
        windows: [None; MAX_WINDOWS],
        stack: [0; MAX_WINDOWS],
        stacked: 0,
        opened: 0,
        tray: [None; MAX_TRAY],
        toast: None,
        pointer: (width / 2, height / 2),
        pointer_visible: false,
        drag: None,
        apps,
        app_count,
        launcher_open: false,
        query: [0; MAX_QUERY],
        query_length: 0,
        selected: 0,
        children: [None; MAX_CHILDREN],
    };
    desktop.repaint(&mut painter, desktop.screen());
    // The first present takes the display from the text terminal.
    if libwoven::compositor_present().is_err() {
        fail(6);
    }
    let _ = libwoven::print("woven-shell: desktop ready\n");

    let mut clock = libwoven::FrameClock::new(FRAME_PERIOD);
    // Applications reach the shell over IPC. Without the Ipc capability the
    // kernel refuses a wait on messages; the desktop then runs without
    // applications rather than not at all.
    let mut sources = wait::INPUT | wait::MESSAGE | wait::DAMAGE;
    // Large enough for any message: one that did not fit would stay queued
    // and keep the mailbox ready forever.
    let mut message = [0u8; 256];
    loop {
        let mut dirty = match clock.wait(sources) {
            Ok(FrameEvent::Frame { .. }) => desktop.tick(),
            Ok(FrameEvent::Ready(_)) => Area::new(0, 0, 0, 0),
            Err(_) if sources & wait::MESSAGE != 0 => {
                sources &= !wait::MESSAGE;
                let _ = libwoven::print("woven-shell: no Ipc capability; applications disabled\n");
                continue;
            }
            Err(_) => fail(8),
        };
        while let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            dirty = dirty.union(desktop.handle_message(sender, &message[..length]));
        }
        while let Ok(Some(event)) = libwoven::input_poll_event() {
            let area = desktop.handle_input(event);
            if !area.is_empty() {
                // Repainted per event, so pointer motion over several
                // events never leaves a trail.
                desktop.repaint(&mut painter, area);
            }
        }
        dirty = dirty.union(desktop.check_clients());
        if !dirty.is_empty() {
            desktop.repaint(&mut painter, dirty);
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
