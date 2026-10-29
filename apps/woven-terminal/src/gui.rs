use libwoven::layout::Frame;
use libwoven::ui::{Button, Label, Theme};
use libwoven::{Canvas, GraphicsSurface, rgb};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 220;
const BACKGROUND: u32 = rgb(10, 14, 22);
const PANEL: u32 = rgb(25, 34, 48);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const MUTED: u32 = rgb(150, 170, 190);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const BUTTON: u32 = rgb(45, 65, 90);
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 170;
const BUTTON_WIDTH: usize = 80;
const BUTTON_HEIGHT: usize = 20;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 196;
// Keep the static render target below the userspace image/stack window while
// still covering the common tiled-client geometry. Larger WSIZ requests are
// answered with this bounded surface size.
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the terminal GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenTerminal: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

const MAX_INPUT: usize = 48;

fn draw(
    canvas: &mut Canvas<'_>,
    line: &[u8],
    status: &[u8],
    pointer: Option<(u16, u16)>,
    pressed: bool,
    clear: &Button<'_>,
) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    let theme = Theme {
        surface: PANEL,
        accent: ACCENT,
        text: TEXT,
        disabled: BUTTON,
        focus: ACCENT,
    };
    Label::new(
        Frame::new(PANEL_X + 10, PANEL_Y + 8, 180, 16),
        "WovenTerminal",
    )
    .draw(canvas, theme);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 38, "> help", ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 58, "WovenTerminal commands:", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 78, "ls PATH", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 94, "cat PATH", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 110, "calc EXPR", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 142, ">", ACCENT);
    if let Ok(text) = core::str::from_utf8(line) {
        canvas.draw_text(PANEL_X + 20, PANEL_Y + 142, text, ACCENT);
    }
    if let Ok(text) = core::str::from_utf8(status) {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 158, text, TEXT);
    }
    clear.draw(canvas, theme);
    canvas.draw_text(
        PANEL_X + 10,
        PANEL_Y + 174,
        "Enter submits; Backspace edits",
        MUTED,
    );
    if let Some((x, y)) = pointer {
        let color = if pressed { TEXT } else { POINTER_MARKER };
        canvas.fill_rect(
            i32::from(x).saturating_sub(2),
            i32::from(y).saturating_sub(2),
            5,
            5,
            color,
        );
    }
}

fn render(
    window: &GraphicsSurface,
    line: &[u8],
    status: &[u8],
    pointer: Option<(u16, u16)>,
    pressed: bool,
    clear: &Button<'_>,
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
    draw(&mut canvas, line, status, pointer, pressed, clear);
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

/// Open a terminal preview through WovenShell. ASCII input, Enter, and
/// Backspace are accepted through the bounded shell key protocol; the
/// existing `--interactive` CLI remains available for full command I/O.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    let mut clear = Button::new(
        Frame::new(
            BUTTON_X,
            BUTTON_Y,
            BUTTON_WIDTH as i32,
            BUTTON_HEIGHT as i32,
        ),
        "Clear",
    );
    let line = [0u8; MAX_INPUT];
    let status: &[u8] = b"Ready for a bounded command preview";
    if !render(&window, &line, status, None, false, &clear) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Terminal";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err()
        || libwoven::message_send(shell, b"WCAP\x01").is_err()
    {
        fail(5)
    }
    let _ = libwoven::print("WovenTerminal: window opened\n");
    let mut message = [0u8; 64];
    let mut line = [0u8; MAX_INPUT];
    let mut length = 0usize;
    let mut status: &[u8] = b"Ready for a bounded command preview";
    let mut pointer = None;
    let mut pointer_pressed = false;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            fail(6)
        }
        if let Ok((message_length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..message_length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..message_length) == Some(&b"WCAN"[..]) {
                clear.cancel_pointer();
                pointer_pressed = false;
                if !render(&window, &line[..length], status, pointer, pointer_pressed, &clear) {
                    fail(9)
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
                            &line[..length],
                            status,
                            pointer,
                            pointer_pressed,
                            &clear,
                        ) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenTerminal: window resized\n");
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
                if let Some((x, y)) = pointer {
                    clear.pointer_move(i32::from(x), i32::from(y));
                }
                if !render(
                    &window,
                    &line[..length],
                    status,
                    pointer,
                    pointer_pressed,
                    &clear,
                ) {
                    fail(9)
                }
                let _ = libwoven::print("WovenTerminal: pointer updated\n");
                continue;
            }
            if message.get(..4) == Some(&b"WPBT"[..]) && message_length == 10 && message[4] == 0 {
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer_pressed = message[5] != 0;
                pointer = Some((local_x, local_y));
                if clear.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    length = 0;
                    status = b"Input cleared by pointer control";
                    let _ = libwoven::print("WovenTerminal: pointer cleared\n");
                }
                if !render(
                    &window,
                    &line[..length],
                    status,
                    pointer,
                    pointer_pressed,
                    &clear,
                ) {
                    fail(9)
                }
                let _ = libwoven::print("WovenTerminal: pointer updated\n");
                continue;
            }
            if message.get(..4) == Some(&b"WKEY"[..]) && message_length >= 5 {
                match message[4] {
                    1 => {
                        status = b"Command submitted to terminal preview";
                        length = 0;
                    }
                    2 => {
                        length = length.saturating_sub(1);
                        status = b"Editing command preview";
                    }
                    _ => {}
                }
                // Redraw after every bounded edit; the shared surface keeps
                // the compositor's ownership and lifecycle unchanged.
                if !render(
                    &window,
                    &line[..length],
                    status,
                    pointer,
                    pointer_pressed,
                    &clear,
                ) {
                    fail(9)
                }
            } else if message_length == 1 && message[0].is_ascii_graphic() && length < MAX_INPUT {
                line[length] = message[0];
                length += 1;
                status = b"Editing command preview";
                if !render(
                    &window,
                    &line[..length],
                    status,
                    pointer,
                    pointer_pressed,
                    &clear,
                ) {
                    fail(11)
                }
                let _ = libwoven::print("WovenTerminal: input updated\n");
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
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))?;
    }
    Some(value)
}
