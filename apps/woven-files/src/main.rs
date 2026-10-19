#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, Args, Canvas, GraphicsSurface};

libwoven::entry!(main);

const MAX_ENTRIES: usize = 128;
const MAX_PREVIEW: usize = 2048;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const POINTER_MARKER: u32 = rgb(255, 80, 80);

const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 296;
const PANEL_HEIGHT: usize = 216;
const LINE_HEIGHT: i32 = 10;
const MAX_VISIBLE_LINES: usize = (PANEL_HEIGHT as i32 / LINE_HEIGHT) as usize;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

/// In .bss rather than the 8 KiB task stack: too large to carry as a stack
/// local. Sized to the whole window: `GraphicsSurface::write_canvas` always
/// uploads to the surface's (0, 0), regardless of the canvas's own x/y
/// origin (that's purely for the canvas's own coordinate addressing), so
/// everything must be drawn into one full-window canvas rather than a
/// separately-placed inset one. See woven-calculator for the same pattern.
struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: woven-files is single-threaded and run_gui takes the only reference.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]));

/// A one-pixel-thick rectangle outline, since `Canvas` has no stroke
/// primitive of its own.
fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenFiles: failure\n");
    libwoven::exit(code)
}

fn draw_files(canvas: &mut Canvas<'_>, pointer: Option<(u16, u16)>, pressed: bool) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.draw_text(PANEL_X + 6, PANEL_Y + 6, "/mnt", ACCENT);
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut shown = 0usize;
    for index in 0..MAX_ENTRIES {
        if shown >= MAX_VISIBLE_LINES.saturating_sub(2) {
            break;
        }
        match libwoven::read_dir("/mnt", index, &mut name) {
            Ok((length, directory)) => {
                let mut line = [0u8; libwoven::MAX_NAME + 1];
                let text_length = length.min(libwoven::MAX_NAME);
                line[..text_length].copy_from_slice(&name[..text_length]);
                if directory && text_length < line.len() {
                    line[text_length] = b'/';
                }
                let shown_length = text_length + usize::from(directory && text_length < line.len());
                let text = core::str::from_utf8(&line[..shown_length]).unwrap_or("?");
                canvas.draw_text(
                    PANEL_X + 6,
                    PANEL_Y + 6 + (shown as i32 + 1) * LINE_HEIGHT,
                    text,
                    TEXT,
                );
                shown += 1;
            }
            Err(_) => break,
        }
    }
    if shown == 0 {
        canvas.draw_text(
            PANEL_X + 6,
            PANEL_Y + 6 + LINE_HEIGHT,
            "(empty or unavailable)",
            TEXT,
        );
    }
    if let Some((x, y)) = pointer {
        let color = if pressed { TEXT } else { POINTER_MARKER };
        canvas.fill_rect(i32::from(x).saturating_sub(2), i32::from(y).saturating_sub(2), 5, 5, color);
    }
}

fn render(window: &GraphicsSurface, pointer: Option<(u16, u16)>, pressed: bool) -> bool {
    let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
    let Some(mut canvas) = Canvas::new(
        pixels,
        0,
        0,
        window.width() as usize,
        window.height() as usize,
    ) else {
        return false;
    };
    draw_files(&mut canvas, pointer, pressed);
    window.write_canvas(&canvas).is_ok()
}

fn send_resize(window: &GraphicsSurface, shell: libwoven::process::Id) -> bool {
    let mut response = [0u8; 16];
    response[..4].copy_from_slice(b"WRSZ");
    response[4..12].copy_from_slice(&window.handle().to_le_bytes());
    response[12..14].copy_from_slice(&(window.width() as u16).to_le_bytes());
    response[14..].copy_from_slice(&(window.height() as u16).to_le_bytes());
    libwoven::message_send(shell, &response).is_ok()
}

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        run_gui(args.get(2))
    }
    let status = match args.get(1) {
        Some(b"--view") => match args.get(2) {
            Some(path) => view(path),
            None => 2,
        },
        Some(b"--help") => {
            let _ = libwoven::print("WovenFiles: --path PATH or --view PATH\n");
            0
        }
        Some(b"--path") => match args.get(2) {
            Some(path) => list(path),
            None => 2,
        },
        None => list(b"/mnt"),
        Some(_) => 2,
    };
    libwoven::exit(status)
}

/// Opens a window on the WovenShell desktop showing the `/mnt` listing.
/// `shell` is the shell's process id, passed as `%p` from the app catalog's
/// `Exec=` line; see woven-calculator's `run_gui` for the protocol details.
fn run_gui(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };

    let mut window = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(window) => window,
        Err(_) => fail(1),
    };

    if !render(&window, None, false) {
        fail(4);
    }

    if window.share_with(shell).is_err() {
        fail(5);
    }
    const TITLE: &[u8] = b"Files";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(6);
    }
    let _ = libwoven::print("WovenFiles: window opened\n");

    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pointer_pressed = false;
    loop {
        match libwoven::wait_any(libwoven::wait::MESSAGE, None) {
            Ok(_) => {}
            Err(_) => fail(8),
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]])).min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]])).min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(&resized, pointer, pointer_pressed) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenFiles: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                pointer = Some((
                    u16::from_le_bytes([message[4], message[5]]),
                    u16::from_le_bytes([message[6], message[7]]),
                ));
                if !render(&window, pointer, pointer_pressed) {
                    fail(8)
                }
                let _ = libwoven::print("WovenFiles: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                pointer = Some((
                    u16::from_le_bytes([message[6], message[7]]),
                    u16::from_le_bytes([message[8], message[9]]),
                ));
                if !render(&window, pointer, pointer_pressed) {
                    fail(8)
                }
                let _ = libwoven::print("WovenFiles: pointer updated\n");
            }
        }
    }
    drop(window);
    libwoven::exit(0)
}

fn parse_u64(text: &[u8]) -> Option<u64> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u64;
    for digit in text {
        if !digit.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}

fn list(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let _ = libwoven::print("WovenFiles: ");
    let _ = libwoven::print(path);
    let _ = libwoven::print("\n");
    let mut name = [0u8; libwoven::MAX_NAME];
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir(path, index, &mut name) {
            Ok((length, directory)) => {
                let _ = libwoven::write(libwoven::syscall::STDOUT, &name[..length]);
                let _ = libwoven::print(if directory { "/\n" } else { "\n" });
            }
            Err(_) => return 0,
        }
    }
    0
}

fn view(raw_path: &[u8]) -> u64 {
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return 2;
    };
    let Ok(descriptor) = libwoven::open(path) else {
        let _ = libwoven::print("WovenFiles: cannot open file\n");
        return 2;
    };
    let mut bytes = [0u8; MAX_PREVIEW];
    let result = match libwoven::read(descriptor, &mut bytes) {
        Ok(length) => {
            let _ = libwoven::write(libwoven::syscall::STDOUT, &bytes[..length]);
            if length == MAX_PREVIEW {
                let _ = libwoven::print("\n[preview truncated]\n");
            }
            0
        }
        Err(_) => 2,
    };
    let _ = libwoven::close(descriptor);
    result
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
