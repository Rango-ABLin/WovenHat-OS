use libwoven::layout::Frame;
use libwoven::ui::{Button, Label, Theme};
use libwoven::{rgb, Canvas, GraphicsSurface};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    HighContrast,
}

impl ThemeMode {
    fn shared(self) -> Theme {
        match self {
            Self::Dark => Theme::DARK,
            Self::Light => Theme::LIGHT,
            Self::HighContrast => Theme::HIGH_CONTRAST,
        }
    }

    fn background(self) -> u32 {
        match self {
            Self::Dark => BACKGROUND,
            Self::Light => rgb(248, 250, 253),
            Self::HighContrast => rgb(0, 0, 0),
        }
    }
}

const WIDTH: u32 = 360;
const HEIGHT: u32 = 240;
const BACKGROUND: u32 = rgb(14, 18, 30);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const BUTTON_X: i32 = 250;
const BUTTON_Y: i32 = 190;
const BUTTON_WIDTH: usize = 90;
const BUTTON_HEIGHT: usize = 24;
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 216;
// Keep the static render target below the current Ring-3 image/stack window.
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the GUI process is single-threaded and takes this buffer once.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer = PixelBuffer(core::cell::UnsafeCell::new(
    [0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT],
));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenSettings: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn draw_number(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    label: &str,
    value: u64,
    digits: &mut [u8; 20],
    theme: Theme,
) {
    canvas.draw_text(x, y, label, theme.text);
    let text = libwoven::format_u64(value, digits);
    if let Ok(text) = core::str::from_utf8(text) {
        canvas.draw_text(x + label.len() as i32 * 6, y, text, theme.accent);
    }
}

fn draw_clock(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    time: libwoven::DateTime,
    digits: &mut [u8; 20],
    theme: Theme,
) {
    draw_number(canvas, x, y, "Year ", u64::from(time.year), digits, theme);
    draw_number(
        canvas,
        x + 100,
        y,
        "Month ",
        u64::from(time.month),
        digits,
        theme,
    );
    draw_number(
        canvas,
        x + 210,
        y,
        "Day ",
        u64::from(time.day),
        digits,
        theme,
    );
    draw_number(
        canvas,
        x,
        y + 16,
        "Hour ",
        u64::from(time.hour),
        digits,
        theme,
    );
    draw_number(
        canvas,
        x + 100,
        y + 16,
        "Min ",
        u64::from(time.minute),
        digits,
        theme,
    );
    draw_number(
        canvas,
        x + 210,
        y + 16,
        "Sec ",
        u64::from(time.second),
        digits,
        theme,
    );
}

fn draw_settings(
    canvas: &mut Canvas<'_>,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    mode: ThemeMode,
    refresh: &Button<'_>,
) {
    let shared = mode.shared();
    let panel = shared.surface;
    let accent = shared.accent;
    let text = shared.text;
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), mode.background());
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, panel);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, accent);
    let title_theme = shared;
    Label::new(
        Frame::new(PANEL_X + 10, PANEL_Y + 8, 180, 16),
        "WovenSettings",
    )
    .draw(canvas, title_theme);
    Label::new(Frame::new(PANEL_X + 10, PANEL_Y + 32, 120, 16), "Identity").draw(
        canvas,
        Theme {
            text: accent,
            ..title_theme
        },
    );
    let mut digits = [0u8; 20];
    // SAFETY: argument-free identity queries.
    draw_number(
        canvas,
        PANEL_X + 10,
        PANEL_Y + 48,
        "UID ",
        unsafe { libwoven::syscall::getuid() },
        &mut digits,
        shared,
    );
    // SAFETY: argument-free identity queries.
    draw_number(
        canvas,
        PANEL_X + 150,
        PANEL_Y + 48,
        "GID ",
        unsafe { libwoven::syscall::getgid() },
        &mut digits,
        shared,
    );
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 80, "Clock (UTC)", accent);
    if let Ok(time) = libwoven::clock() {
        draw_clock(
            canvas,
            PANEL_X + 10,
            PANEL_Y + 96,
            time,
            &mut digits,
            shared,
        );
    } else {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 96, "unavailable", text);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 136, "Display", accent);
    if let Ok(info) = libwoven::compositor_info() {
        draw_number(
            canvas,
            PANEL_X + 10,
            PANEL_Y + 152,
            "W ",
            u64::from(info.width),
            &mut digits,
            shared,
        );
        draw_number(
            canvas,
            PANEL_X + 110,
            PANEL_Y + 152,
            "H ",
            u64::from(info.height),
            &mut digits,
            shared,
        );
        draw_number(
            canvas,
            PANEL_X + 210,
            PANEL_Y + 152,
            "Scale ",
            u64::from(info.scale),
            &mut digits,
            shared,
        );
    } else {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 152, "unavailable", text);
    }
    let mut refresh_digits = [0u8; 20];
    draw_number(
        canvas,
        PANEL_X + 10,
        PANEL_Y + 184,
        "Refreshes ",
        u64::from(refresh_count),
        &mut refresh_digits,
        shared,
    );
    refresh.draw(canvas, shared);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(
            i32::from(x).saturating_sub(2),
            i32::from(y).saturating_sub(2),
            5,
            5,
            if pressed { text } else { POINTER_MARKER },
        );
    }
}

fn render(
    window: &GraphicsSurface,
    pointer: Option<(u16, u16)>,
    pressed: bool,
    refresh_count: u32,
    mode: ThemeMode,
    refresh: &Button<'_>,
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
    draw_settings(&mut canvas, pointer, pressed, refresh_count, mode, refresh);
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

/// Open a read-only Settings window through WovenShell's WOPN protocol.
/// The shell PID is supplied by the catalog's `%p` token.
pub fn run(shell: Option<&[u8]>, mode: ThemeMode) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    let mut refresh = Button::new(
        Frame::new(
            BUTTON_X,
            BUTTON_Y,
            BUTTON_WIDTH as i32,
            BUTTON_HEIGHT as i32,
        ),
        "Refresh",
    );
    if !render(&window, None, false, 0, mode, &refresh) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Settings";
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
    let _ = libwoven::print("WovenSettings: window opened\n");
    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pointer_pressed = false;
    let mut refresh_count = 0u32;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            fail(6)
        }
        if let Ok((length, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell {
                continue;
            }
            if message.get(..length.min(4)) == Some(&b"WCLS"[..]) {
                break;
            }
            if message.get(..length) == Some(&b"WCAN"[..]) {
                refresh.cancel_pointer();
                pointer_pressed = false;
                if !render(&window, pointer, pointer_pressed, refresh_count, mode, &refresh) {
                    fail(8)
                }
                continue;
            }
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]]))
                    .min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]]))
                    .min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(&resized, pointer, pointer_pressed, refresh_count, mode, &refresh) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenSettings: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if (message.get(..length) == Some(&b"WKEY\x01"[..])
                || message.get(..length) == Some(&b"r"[..])
                || message.get(..length) == Some(&b" "[..]))
                && refresh.activate()
            {
                refresh.focused = true;
                refresh_count = refresh_count.saturating_add(1);
                if !render(&window, pointer, pointer_pressed, refresh_count, mode, &refresh) {
                    fail(8)
                }
                let _ = libwoven::print("WovenSettings: keyboard refreshed\n");
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                pointer = Some((
                    u16::from_le_bytes([message[4], message[5]]),
                    u16::from_le_bytes([message[6], message[7]]),
                ));
                if let Some((x, y)) = pointer {
                    refresh.pointer_move(i32::from(x), i32::from(y));
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, mode, &refresh) {
                    fail(8)
                }
                let _ = libwoven::print("WovenSettings: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                let local_x = u16::from_le_bytes([message[6], message[7]]);
                let local_y = u16::from_le_bytes([message[8], message[9]]);
                pointer = Some((local_x, local_y));
                if refresh.pointer(i32::from(local_x), i32::from(local_y), pointer_pressed) {
                    refresh_count = refresh_count.saturating_add(1);
                    let _ = libwoven::print("WovenSettings: pointer refreshed\n");
                }
                if !render(&window, pointer, pointer_pressed, refresh_count, mode, &refresh) {
                    fail(8)
                }
                let _ = libwoven::print("WovenSettings: pointer updated\n");
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
