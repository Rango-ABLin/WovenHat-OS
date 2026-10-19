use libwoven::{rgb, Canvas, GraphicsSurface};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 200;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 176;
const MAX_EVENTS: u64 = 16;

struct PixelBuffer(core::cell::UnsafeCell<[u32; (WIDTH * HEIGHT) as usize]>);

// SAFETY: the security GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; (WIDTH * HEIGHT) as usize]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenSecurity: failure\n");
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
) {
    canvas.draw_text(x, y, label, TEXT);
    let text = libwoven::format_u64(value, digits);
    if let Ok(text) = core::str::from_utf8(text) {
        canvas.draw_text(x + label.len() as i32 * 6, y, text, ACCENT);
    }
}

fn audit_count() -> u64 {
    let mut count = 0;
    for offset in 0..MAX_EVENTS {
        match libwoven::audit_read(offset) {
            Ok(Some(_)) => count += 1,
            Ok(None) | Err(_) => break,
        }
    }
    count
}

/// Open a read-only Security window through WovenShell's WOPN protocol.
pub fn run(shell: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_u64).map(libwoven::process::Id) else {
        fail(7)
    };
    let window = GraphicsSurface::create(WIDTH, HEIGHT).unwrap_or_else(|_| fail(1));
    // SAFETY: this process is single-threaded and takes the buffer once.
    let pixels = unsafe { &mut *CANVAS_PIXELS.0.get() };
    let mut canvas = Canvas::new(pixels, 0, 0, WIDTH as usize, HEIGHT as usize)
        .unwrap_or_else(|| fail(2));
    canvas.fill_rect(0, 0, WIDTH as usize, HEIGHT as usize, BACKGROUND);
    canvas.fill_rect(PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, PANEL);
    stroke_rect(&mut canvas, PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT, ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 8, "WovenSecurity", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 36, "Current identity", ACCENT);
    let mut digits = [0u8; 20];
    // SAFETY: argument-free identity query.
    draw_number(
        &mut canvas,
        PANEL_X + 10,
        PANEL_Y + 54,
        "UID ",
        unsafe { libwoven::syscall::getuid() },
        &mut digits,
    );
    // SAFETY: argument-free identity query.
    draw_number(
        &mut canvas,
        PANEL_X + 150,
        PANEL_Y + 54,
        "GID ",
        unsafe { libwoven::syscall::getgid() },
        &mut digits,
    );
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 88, "Audit trail", ACCENT);
    draw_number(
        &mut canvas,
        PANEL_X + 10,
        PANEL_Y + 106,
        "Entries ",
        audit_count(),
        &mut digits,
    );
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 140, "Bounded view: 16 events", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 156, "Read-only security summary", TEXT);
    if window.write_canvas(&canvas).is_err() {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Security";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(5)
    }
    let _ = libwoven::print("WovenSecurity: window opened\n");
    let mut message = [0u8; 64];
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            fail(6)
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
