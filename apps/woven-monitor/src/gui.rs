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
const MAX_STORAGE_ENTRIES: usize = 128;

struct PixelBuffer(core::cell::UnsafeCell<[u32; (WIDTH * HEIGHT) as usize]>);

// SAFETY: the monitor GUI is single-threaded and owns this buffer exclusively.
unsafe impl Sync for PixelBuffer {}

static CANVAS_PIXELS: PixelBuffer =
    PixelBuffer(core::cell::UnsafeCell::new([0; (WIDTH * HEIGHT) as usize]));

fn fail(code: u64) -> ! {
    let _ = libwoven::print("WovenMonitor: failure\n");
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

fn storage_count() -> usize {
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut count = 0usize;
    for index in 0..MAX_STORAGE_ENTRIES {
        if libwoven::read_dir("/mnt", index, &mut name).is_err() {
            break;
        }
        count += 1;
    }
    count
}

/// Open a read-only Monitor window through WovenShell's WOPN protocol.
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
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 8, "WovenMonitor", TEXT);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 34, "System snapshot", ACCENT);
    let mut digits = [0u8; 20];
    if let Ok(time) = libwoven::clock() {
        draw_number(&mut canvas, PANEL_X + 10, PANEL_Y + 52, "Hour ", u64::from(time.hour), &mut digits);
        draw_number(&mut canvas, PANEL_X + 120, PANEL_Y + 52, "Min ", u64::from(time.minute), &mut digits);
        draw_number(&mut canvas, PANEL_X + 220, PANEL_Y + 52, "Sec ", u64::from(time.second), &mut digits);
    } else {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 52, "Clock unavailable", TEXT);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 84, "Display", ACCENT);
    if let Ok(info) = libwoven::compositor_info() {
        draw_number(&mut canvas, PANEL_X + 10, PANEL_Y + 100, "W ", u64::from(info.width), &mut digits);
        draw_number(&mut canvas, PANEL_X + 110, PANEL_Y + 100, "H ", u64::from(info.height), &mut digits);
        draw_number(&mut canvas, PANEL_X + 210, PANEL_Y + 100, "Scale ", u64::from(info.scale), &mut digits);
    } else {
        canvas.draw_text(PANEL_X + 10, PANEL_Y + 100, "Display unavailable", TEXT);
    }
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 132, "Storage", ACCENT);
    draw_number(&mut canvas, PANEL_X + 10, PANEL_Y + 148, "Entries ", storage_count() as u64, &mut digits);
    canvas.draw_text(PANEL_X + 10, PANEL_Y + 172, "Bounded scan: 128 entries", TEXT);
    if window.write_canvas(&canvas).is_err() {
        fail(3)
    }
    if window.share_with(shell).is_err() {
        fail(4)
    }
    const TITLE: &[u8] = b"Monitor";
    let mut open = [0u8; 16 + TITLE.len()];
    open[..4].copy_from_slice(b"WOPN");
    open[4..12].copy_from_slice(&window.handle().to_le_bytes());
    open[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    open[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    open[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &open).is_err() {
        fail(5)
    }
    let _ = libwoven::print("WovenMonitor: window opened\n");
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
