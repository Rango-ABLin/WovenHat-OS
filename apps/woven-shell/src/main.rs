//! WovenShell: the WovenHat desktop.
//!
//! A userspace program that takes the compositor lease and draws the desktop:
//! a procedural wallpaper, a translucent taskbar with the WovenStart button
//! and a clock read from the hardware clock.
//!
//! Everything is rendered in software into a [`Canvas`] one tile at a time
//! and uploaded into a single staging surface, which is blitted into place.
//! Client surfaces are capped at 320Ã—240, far smaller than a screen, so the
//! desktop is never one surface: any region is repainted by rendering just
//! the tiles that cover it. The scene is a pure function of position and
//! state, so a tile renders the same whatever order tiles are drawn in.
//!
//! On a display at least 1280Ã—800 the shell switches to display scale 2, so
//! the 5Ã—7 font stays legible.
//!
//! With `--seconds` the clock shows seconds, which lets the acceptance gate
//! watch it advance without waiting for a minute.
#![no_std]
#![no_main]

use core::cell::UnsafeCell;
use core::panic::PanicInfo;

use libwoven::{blend, font, rgb, Args, Canvas, DateTime, FrameEvent, GraphicsSurface};

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

const TILE_WIDTH: usize = 320;
const TILE_HEIGHT: usize = 40;
const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;

/// The clock is checked four times a second, so a new second shows within
/// a quarter of one.
const CLOCK_PERIOD: u64 = libwoven::time::TICKS_PER_SECOND / 4;

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

/// A rectangle in screen coordinates.
#[derive(Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

struct Desktop {
    width: i32,
    height: i32,
    seconds: bool,
    /// The time on screen, `None` if the clock could not be read.
    time: Option<DateTime>,
}

impl Desktop {
    fn taskbar_top(&self) -> i32 {
        self.height - TASKBAR_HEIGHT
    }

    /// Baseline row for taskbar text: seven glyph rows centred in the bar
    /// below its one-pixel accent line.
    fn text_y(&self) -> i32 {
        self.taskbar_top() + 9
    }


    /// The region the clock occupies, with a margin for a wider reading.
    fn clock_area(&self) -> Area {
        let width = 8 * font::ADVANCE + CLOCK_MARGIN;
        Area {
            x: self.width - width,
            y: self.taskbar_top(),
            width,
            height: TASKBAR_HEIGHT,
        }
    }

    /// Render the part of the desktop the canvas covers.
    fn render(&self, canvas: &mut Canvas<'_>) {
        let height = self.height;
        canvas.shade(|x, y| wallpaper(x, y, height));

        let top = self.taskbar_top();
        let width = self.width as usize;
        canvas.blend_rect(0, top, width, TASKBAR_HEIGHT as usize, TASKBAR, TASKBAR_OPACITY);
        canvas.fill_rect(0, top, width, 1, ACCENT);

        canvas.fill_rect(START_X, top + 4, START_WIDTH, START_HEIGHT, START_FILL);
        let label_x = START_X + (START_WIDTH - font::text_width(START_LABEL)) as i32 / 2;
        canvas.draw_text(label_x, self.text_y(), START_LABEL, TEXT);

        let (bytes, length) = clock_text(self.time, self.seconds);
        let text = core::str::from_utf8(&bytes[..length]).unwrap_or("");
        let clock_x = self.width - CLOCK_MARGIN - font::text_width(text) as i32;
        canvas.draw_text(clock_x, self.text_y(), text, TEXT);
    }

    /// Repaint `area` by rendering and blitting every tile that covers it.
    /// Tiles sit on a fixed grid, so a tile hanging off the screen's right
    /// or bottom edge is clipped by the compositor.
    fn paint(&self, staging: &GraphicsSurface, tile: &mut [u32], area: Area) {
        let (tile_width, tile_height) = (TILE_WIDTH as i32, TILE_HEIGHT as i32);
        let first_column = area.x.max(0) / tile_width;
        let first_row = area.y.max(0) / tile_height;
        let last_column = (area.x + area.width - 1).min(self.width - 1) / tile_width;
        let last_row = (area.y + area.height - 1).min(self.height - 1) / tile_height;
        for row in first_row..=last_row {
            for column in first_column..=last_column {
                let (x, y) = (column * tile_width, row * tile_height);
                let Some(mut canvas) = Canvas::new(tile, x, y, TILE_WIDTH, TILE_HEIGHT) else {
                    fail(10);
                };
                self.render(&mut canvas);
                if staging.write_canvas(&canvas).is_err() || staging.present(x, y).is_err() {
                    fail(11);
                }
            }
        }
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

    let mut desktop = Desktop {
        width: info.width as i32,
        height: info.height as i32,
        seconds,
        time: libwoven::clock().ok(),
    };
    let screen = Area {
        x: 0,
        y: 0,
        width: desktop.width,
        height: desktop.height,
    };
    desktop.paint(&staging, tile, screen);
    // The first present takes the display from the text terminal.
    if libwoven::compositor_present().is_err() {
        fail(6);
    }
    let _ = libwoven::print("woven-shell: desktop ready\n");

    let mut clock = libwoven::FrameClock::new(CLOCK_PERIOD);
    loop {
        match clock.wait(libwoven::wait::INPUT) {
            Ok(FrameEvent::Frame { .. }) => {
                // Repaint only when the text on screen would change.
                let time = libwoven::clock().ok();
                if clock_text(time, seconds) != clock_text(desktop.time, seconds) {
                    desktop.time = time;
                    desktop.paint(&staging, tile, desktop.clock_area());
                    if libwoven::compositor_present().is_err() {
                        fail(7);
                    }
                }
            }
            // Input belongs to the focused desktop, but nothing on it takes
            // input yet; drain it so the queue does not fill.
            Ok(FrameEvent::Ready(_)) => while let Ok(Some(_)) = libwoven::input_poll_event() {},
            Err(_) => fail(8),
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
