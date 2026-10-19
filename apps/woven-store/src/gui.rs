use libwoven::{rgb, Canvas, GraphicsSurface};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 220;
const BACKGROUND: u32 = rgb(14, 18, 30);
const PANEL: u32 = rgb(35, 43, 66);
const ACCENT: u32 = rgb(100, 220, 180);
const TEXT: u32 = rgb(235, 240, 250);
const PANEL_X: i32 = 12;
const PANEL_Y: i32 = 12;
const PANEL_WIDTH: usize = 336;
const PANEL_HEIGHT: usize = 196;
const MAX_ENTRIES: usize = 128;

struct PixelBuffer(core::cell::UnsafeCell<[u32; (WIDTH * HEIGHT) as usize]>);

// SAFETY: the store GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; (WIDTH * HEIGHT) as usize]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenStore: failure\n");
    libwoven::exit(code)
}

fn stroke_rect(canvas: &mut Canvas<'_>, x: i32, y: i32, width: usize, height: usize, color: u32) {
    canvas.fill_rect(x, y, width, 1, color);
    canvas.fill_rect(x, y + height as i32 - 1, width, 1, color);
    canvas.fill_rect(x, y, 1, height, color);
    canvas.fill_rect(x + width as i32 - 1, y, 1, height, color);
}

fn descriptor_count() -> usize {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_ENTRIES {
        match libwoven::read_dir("/share/applications", index, &mut name) {
            Ok((_, directory)) if !directory => count += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    count
}

/// Open a read-only installed-catalog window through WovenShell.
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
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 8, "WovenStore", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 38, "Installed catalog", ACCENT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 58, "/share/applications", TEXT);
    let mut digits = [0u8; 20];
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 92, "Descriptors ", TEXT);
    let count_text = libwoven::format_u64(descriptor_count() as u64, &mut digits);
    if let Ok(text) = core::str::from_utf8(count_text) {
        canvas.draw_text(PANEL_X + 82, PANEL_Y + 92, text, ACCENT);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 126, "Signed catalog entries", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 150, "Read-only installed view", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 178, "No package mutation", TEXT);
    if window.write_canvas(&canvas).is_err() {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Store";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(5)
    }
    let _ = libwoven::print("WovenStore: window opened\n");
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
