#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::{rgb, Args, Canvas, GraphicsSurface};
use wovenhat_calculator::evaluate;

libwoven::entry!(main);

const WIDTH: u32 = 240;
const HEIGHT: u32 = 150;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);

const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 216;
const PANEL_HEIGHT: usize = 126;
const MAX_INPUT: usize = 24;

/// In .bss rather than the 8 KiB task stack: too large to carry as a stack
/// local. Sized to the whole window: `GraphicsSurface::write_canvas` always
/// uploads to the surface's (0, 0), regardless of the canvas's own x/y
/// origin (that's purely for the canvas's own coordinate addressing), so
/// everything must be drawn into one full-window canvas rather than a
/// separately-placed inset one.
struct PixelBuffer(core::cell::UnsafeCell<[u32; (WIDTH * HEIGHT) as usize]>);

// SAFETY: woven-calculator is single-threaded and run_gui takes the only reference.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; (WIDTH * HEIGHT) as usize]));

/// A one-pixel-thick rectangle outline, since `Canvas` has no stroke
/// primitive of its own.
fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenCalculator: failure\n");
    libwoven::exit(code)
}

fn main(args: Args) -> ! {
    if args.get(1) == Some(b"--gui") {
        run_gui(args.get(2))
    }
    let expression = args.get(1).unwrap_or(b"2 + 2");
    let result = match evaluate(expression) {
        Ok(value) => value,
        Err(_) => fail(2),
    };
    run_console(expression, result)
}

fn run_console(expression: &[u8], result: i64) -> ! {
    let _ = libwoven::print("WovenCalculator: ");
    let _ = libwoven::write(libwoven::syscall::STDOUT, expression);
    let _ = libwoven::print(" = ");
    let mut digits = [0u8; 24];
    let result_text = format_signed(result, &mut digits);
    let _ = libwoven::write(libwoven::syscall::STDOUT, result_text.as_bytes());
    let _ = libwoven::print("\n");
    libwoven::exit(0)
}

fn draw_gui(canvas: &mut Canvas<'_>, expression: &[u8], result: Option<i64>, active: bool) {
    canvas.fill_rect(0, 0, WIDTH as usize, HEIGHT as usize, BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.fill_rect(PANEL_X + 184, PANEL_Y + 8, 18, 8, if active { ACCENT } else { PANEL });
    canvas.draw_text(PANEL_X - 2, PANEL_Y, "WovenCalculator", TEXT);
    canvas.draw_text(PANEL_X - 2, PANEL_Y + 24, "Expression:", TEXT);
    if let Ok(text) = core::str::from_utf8(expression) {
        canvas.draw_text(PANEL_X - 2, PANEL_Y + 40, text, TEXT);
    }
    canvas.draw_text(PANEL_X - 2, PANEL_Y + 64, "Result:", ACCENT);
    if let Some(value) = result {
        let mut digits = [0u8; 24];
        let result_text = format_signed(value, &mut digits);
        canvas.draw_text(PANEL_X - 2, PANEL_Y + 80, result_text, ACCENT);
    } else {
        canvas.draw_text(PANEL_X - 2, PANEL_Y + 80, "Enter to evaluate", ACCENT);
    }
    canvas.draw_text(PANEL_X - 2, PANEL_Y + 104, "ASCII input; Enter evaluates", TEXT);
}

/// Opens a window on the WovenShell desktop: shares a drawn surface and asks
/// the shell (the sole compositor-authority holder) to place it, following
/// the same WOPN/WCLS protocol `hello-gui --desktop-client` uses. `shell` is
/// the shell's process id, passed as `%p` from the app catalog's `Exec=`
/// line; an ordinary launched process holds no DisplayIo capability of its
/// own, so it cannot claim the compositor or present directly.
fn run_gui(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let window = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(window) => window,
        Err(_) => fail(3),
    };
    // SAFETY: the only reference to the buffer, taken once.
    let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
    let mut canvas = match Canvas::new(pixels, 0, 0, WIDTH as usize, HEIGHT as usize) {
        Some(canvas) => canvas,
        None => fail(5),
    };
    let expression = [0u8; MAX_INPUT];
    draw_gui(&mut canvas, &expression, None, false);
    if window.write_canvas(&canvas).is_err() {
        fail(6);
    }

    if window.share_with(shell).is_err() {
        fail(8);
    }
    const TITLE: &[u8] = b"Calculator";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(9);
    }
    let _ = libwoven::print("WovenCalculator: window opened\n");

    // Wait for the shell to close this window, or update the bounded
    // expression preview from forwarded ASCII/WKEY events.
    let mut message = [0u8; 64];
    let mut expression = [0u8; MAX_INPUT];
    let mut length = 0usize;
    let mut result = None;
    let mut active: bool;
    loop {
        match libwoven::wait_any(libwoven::wait::MESSAGE, None) {
            Ok(_) => {}
            Err(_) => fail(10),
        }
        if let Ok((message_length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..message_length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..4) == Some(&b"WKEY"[..]) && message_length >= 5 {
                active = true;
                match message[4] {
                    1 => result = evaluate(&expression[..length]).ok(),
                    2 => {
                        length = length.saturating_sub(1);
                        result = None;
                    }
                    _ => {}
                }
            } else if message_length == 1 && message[0].is_ascii_graphic() && length < MAX_INPUT {
                active = true;
                expression[length] = message[0];
                length += 1;
                result = None;
            } else {
                continue;
            }
            let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
            let Some(mut canvas) = Canvas::new(pixels, 0, 0, WIDTH as usize, HEIGHT as usize) else {
                fail(10)
            };
            draw_gui(&mut canvas, &expression[..length], result, active);
            if window.write_canvas(&canvas).is_err() {
                fail(11)
            }
            let _ = libwoven::print("WovenCalculator: input updated\n");
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

fn format_signed(value: i64, output: &mut [u8; 24]) -> &str {
    let negative = value < 0;
    let mut number = value.unsigned_abs();
    let mut position = output.len();
    if number == 0 {
        position -= 1;
        output[position] = b'0';
    } else {
        while number != 0 {
            position -= 1;
            output[position] = b'0' + (number % 10) as u8;
            number /= 10;
        }
    }
    if negative {
        position -= 1;
        output[position] = b'-';
    }
    core::str::from_utf8(&output[position..]).unwrap_or("?")
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    libwoven::exit(101)
}
