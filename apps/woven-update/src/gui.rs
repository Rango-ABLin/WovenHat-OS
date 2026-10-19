use libwoven::{rgb, Canvas, GraphicsSurface};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 200;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const POINTER_MARKER: u32 = rgb(255, 80, 80);
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 176;
const MAX_ENTRIES: usize = 64;
const MAX_SURFACE_WIDTH: usize = 480;
const MAX_SURFACE_HEIGHT: usize = 320;

struct PixelBuffer(core::cell::UnsafeCell<[u32; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]>);

// SAFETY: the update GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; MAX_SURFACE_WIDTH * MAX_SURFACE_HEIGHT]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenUpdate: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn update_count() -> usize {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir("/updates", index, &mut name) {
            Ok((_, directory)) if !directory => count += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    count
}

fn draw_update(canvas: &mut Canvas<'_>, pointer: Option<(u16, u16)>, pressed: bool) {
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 8, "WovenUpdate", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 38, "Update preflight", ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 58, "Candidate packages", TEXT);
    let mut digits = [0u8; 20];
    let count_text = libwoven::format_u64(update_count() as u64, &mut digits);
    if let Ok(text) = core::str::from_utf8(count_text) {
        canvas.draw_text(PANEL_X + 130, PANEL_Y + 58, text, ACCENT);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 92, "/updates", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 124, "No installation performed", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 148, "Bounded scan: 64 entries", TEXT);
    if let Some((x, y)) = pointer {
        canvas.fill_rect(i32::from(x).saturating_sub(2), i32::from(y).saturating_sub(2), 5, 5, if pressed { TEXT } else { POINTER_MARKER });
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
    draw_update(&mut canvas, pointer, pressed);
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

/// Open a read-only update preflight window through WovenShell.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let mut window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    if !render(&window, None, false) {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Update";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(5)
    }
    let _ = libwoven::print("WovenUpdate: window opened\n");
    let mut message = [0u8; 64];
    let mut pointer = None;
    let mut pointer_pressed = false;
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
            if message.get(..4) == Some(&b"WSIZ"[..]) && length == 8 {
                let width = u32::from(u16::from_le_bytes([message[4], message[5]])).min(MAX_SURFACE_WIDTH as u32);
                let height = u32::from(u16::from_le_bytes([message[6], message[7]])).min(MAX_SURFACE_HEIGHT as u32);
                if let Ok(resized) = GraphicsSurface::create(width, height) {
                    if resized.share_with(shell).is_ok() && send_resize(&resized, shell) {
                        if render(&resized, pointer, pointer_pressed) {
                            let previous = core::mem::replace(&mut window, resized);
                            drop(previous);
                            let _ = libwoven::print("WovenUpdate: window resized\n");
                        } else {
                            drop(resized);
                        }
                    } else {
                        drop(resized);
                    }
                }
            } else if message.get(..4) == Some(&b"WPMV"[..]) && length == 8 {
                pointer = Some((u16::from_le_bytes([message[4], message[5]]), u16::from_le_bytes([message[6], message[7]])));
                if !render(&window, pointer, pointer_pressed) { fail(8) }
                let _ = libwoven::print("WovenUpdate: pointer updated\n");
            } else if message.get(..4) == Some(&b"WPBT"[..]) && length == 10 && message[4] == 0 {
                pointer_pressed = message[5] != 0;
                pointer = Some((u16::from_le_bytes([message[6], message[7]]), u16::from_le_bytes([message[8], message[9]])));
                if !render(&window, pointer, pointer_pressed) { fail(8) }
                let _ = libwoven::print("WovenUpdate: pointer updated\n");
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
