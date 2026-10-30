#![no_std]
#![no_main]

use core::panic::PanicInfo;

use libwoven::layout::Frame;
use libwoven::ui::{Button, DesignTokens, Label, Theme};
use libwoven::{Args, Canvas, GraphicsSurface, rgb};
use wovenhat_calculator::evaluate;

libwoven::entry!(main);

const WIDTH: u32 = 240;
const HEIGHT: u32 = 150;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const BUTTON: u32 = rgb(45, 65, 90);

// Layout from shared DesignTokens (Stage 11).
const TOKENS: DesignTokens = DesignTokens::DEFAULT;
const PANEL_INSET: i32 = TOKENS.space_3; // 12
const BUTTON_X: i32 = 150;
const BUTTON_Y: i32 = 110;
const BUTTON_WIDTH: usize = 60;
const BUTTON_HEIGHT: usize = 18;
const PANEL_X: i32 = PANEL_INSET;
const PANEL_Y: i32 = PANEL_INSET;
const PANEL_WIDTH: usize = (WIDTH as i32 - PANEL_INSET * 2) as usize; // 216
const PANEL_HEIGHT: usize = (HEIGHT as i32 - PANEL_INSET * 2) as usize; // 126
const MAX_INPUT: usize = 24;
// Keep the static render target below the userspace image/stack window while
// still covering the common tiled-client geometry. Larger WSIZ requests are
// answered with this bounded surface size.
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

/// In .bss rather than the 8 KiB task stack: too large to carry as a stack
/// local. Sized to the whole window: `GraphicsSurface::write_canvas` always
/// uploads to the surface's (0, 0), regardless of the canvas's own x/y
/// origin (that's purely for the canvas's own coordinate addressing), so
/// everything must be drawn into one full-window canvas rather than a
/// separately-placed inset one.
struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: woven-calculator is single-threaded and run_gui takes the only reference.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

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

fn draw_gui(
    canvas: &mut Canvas<'_>,
    expression: &[u8],
    result: Option<i64>,
    active: bool,
    _pointer: Option<(u16, u16)>,
    _pressed: bool,
    evaluate_button: &Button<'_>,
) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.fill_rect(
        PANEL_X + 184,
        PANEL_Y + 8,
        18,
        8,
        if active { ACCENT } else { PANEL },
    );
    let theme = Theme {
        surface: PANEL,
        accent: ACCENT,
        text: TEXT,
        disabled: BUTTON,
        focus: ACCENT,
    };
    Label::new(Frame::new(PANEL_X - 2, PANEL_Y, 180, 16), "WovenCalculator").draw(canvas, theme);
    canvas.draw_text(PANEL_X - 2, PANEL_Y + 24, "Expression:", TEXT);
    if expression.is_empty() {
        canvas.draw_text(PANEL_X - 2, PANEL_Y + 40, "Type e.g. 12+3", ACCENT);
    } else if let Ok(text) = core::str::from_utf8(expression) {
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
    evaluate_button.draw(canvas, theme);
    canvas.draw_text(PANEL_X - 2, PANEL_Y + 104, "Enter evaluates", TEXT);
    // The desktop owns the cursor; clients draw only control hover state.
}

fn render(
    window: &GraphicsSurface,
    expression: &[u8],
    result: Option<i64>,
    active: bool,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    evaluate_button: &Button<'_>,
) -> bool {
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
    draw_gui(
        &mut canvas,
        expression,
        result,
        active,
        pointer,
        pressed,
        evaluate_button,
    );
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
    let mut window = match GraphicsSurface::create(WIDTH, HEIGHT) {
        Ok(window) => window,
        Err(_) => fail(3),
    };
    let mut evaluate_button = Button::new(
        Frame::new(
            BUTTON_X,
            BUTTON_Y,
            BUTTON_WIDTH as i32,
            BUTTON_HEIGHT as i32,
        ),
        "Evaluate",
    );
    let expression = [0u8; MAX_INPUT];
    if !render(
        &window,
        &expression[..0],
        None,
        false,
        None,
        false,
        &evaluate_button,
    ) {
        fail(5);
    }

    if window.share_with(shell).is_err() {
        fail(6);
    }
    const TITLE: &[u8] = b"Calculator";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err()
        || libwoven::message_send(shell, b"WCAP\x01").is_err()
    {
        fail(7);
    }
    let _ = libwoven::print("WovenCalculator: window opened\n");

    // Wait for the shell to close this window, or update the bounded
    // expression preview from forwarded ASCII/WKEY events.
    let mut message = [0u8; 64];
    let mut expression = [0u8; MAX_INPUT];
    let mut length = 0usize;
    let mut result = None;
    let mut pointer = None;
    let mut pointer_pressed = false;
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
            if message.get(..message_length) == Some(&b"WCAN"[..]) {
                evaluate_button.cancel_pointer();
                pointer_pressed = false;
                if !render(&window, &expression[..length], result, true, pointer, pointer_pressed, &evaluate_button) {
                    fail(10)
                }
                continue;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && message_length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]]))
                    .min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]]))
                    .min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    // Publish the replacement token before the potentially
                    // expensive pixel upload, so the shell's bounded resize
                    // grace window cannot expire while copying the canvas.
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(
                            &resized,
                            &expression[..length],
                            result,
                            true,
                            pointer,
                            pointer_pressed,
                            &evaluate_button,
                        ) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenCalculator: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
                continue;
            }
            if message.get(..4) == Some(&b"WPMV"[..]) && message_length == 8 {
                pointer = Some((
                    u16::from_le_bytes([message[4], message[5]]),
                    u16::from_le_bytes([message[6], message[7]]),
                ));
                let before = (evaluate_button.hovered, evaluate_button.pressed);
                if let Some((x, y)) = pointer {
                    evaluate_button.pointer_move(i32::from(x), i32::from(y));
                }
                if before == (evaluate_button.hovered, evaluate_button.pressed) {
                    continue;
                }
                if !render(
                    &window,
                    &expression[..length],
                    result,
                    true,
                    pointer,
                    pointer_pressed,
                    &evaluate_button,
                ) {
                    fail(10)
                }
                let _ = libwoven::print("WovenCalculator: pointer updated\n");
                continue;
            }
            if message.get(..4) == Some(&b"WPBT"[..]) && message_length == 10 && message[4] == 0 {
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer_pressed = message[5] != 0;
                pointer = Some((local_x, local_y));
                if evaluate_button.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed)
                {
                    result = evaluate(&expression[..length]).ok();
                    let _ = libwoven::print("WovenCalculator: pointer evaluated\n");
                }
                if !render(
                    &window,
                    &expression[..length],
                    result,
                    true,
                    pointer,
                    pointer_pressed,
                    &evaluate_button,
                ) {
                    fail(10)
                }
                let _ = libwoven::print("WovenCalculator: pointer updated\n");
                continue;
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
            if !render(
                &window,
                &expression[..length],
                result,
                active,
                pointer,
                pointer_pressed,
                &evaluate_button,
            ) {
                fail(10)
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
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))?;
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
