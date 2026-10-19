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

const CANVAS_WIDTH: usize = 296;
const CANVAS_HEIGHT: usize = 216;
const LINE_HEIGHT: i32 = 10;
const MAX_VISIBLE_LINES: usize = (CANVAS_HEIGHT as i32 / LINE_HEIGHT) as usize;

/// In .bss rather than the 8 KiB task stack: too large to carry as a stack
/// local (see woven-calculator for the same pattern and the bug it avoids).
struct PixelBuffer(core::cell::UnsafeCell<[u32; CANVAS_WIDTH * CANVAS_HEIGHT]>);

// SAFETY: woven-files is single-threaded and run_gui takes the only reference.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; CANVAS_WIDTH * CANVAS_HEIGHT]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenFiles: failure\n");
    libwoven::exit(code)
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

    let window = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(window) => window,
        Err(_) => fail(1),
    };
    if window.fill_rect(0, 0, WIDTH as u16, HEIGHT as u16, BACKGROUND).is_err()
        || window
            .fill_rect(12, 12, CANVAS_WIDTH as u16, CANVAS_HEIGHT as u16, PANEL)
            .is_err()
        || window
            .stroke_rect(12, 12, CANVAS_WIDTH as u16, CANVAS_HEIGHT as u16, ACCENT)
            .is_err()
    {
        fail(2);
    }

    // SAFETY: the only reference to the buffer, taken once.
    let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
    let mut canvas = match Canvas::new(pixels, 12, 12, CANVAS_WIDTH, CANVAS_HEIGHT) {
        Some(canvas) => canvas,
        None => fail(3),
    };
    canvas.draw_text(6, 6, "/mnt", ACCENT);
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
                canvas.draw_text(6, 6 + (shown as i32 + 1) * LINE_HEIGHT, text, TEXT);
                shown += 1;
            }
            Err(_) => break,
        }
    }
    if window.write_canvas(&canvas).is_err() {
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
    loop {
        match libwoven::wait_any(libwoven::wait::MESSAGE, None) {
            Ok(_) => {}
            Err(_) => fail(8),
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            if sender == shell && message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
                break;
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
