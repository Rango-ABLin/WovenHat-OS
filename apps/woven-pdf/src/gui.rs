//! WovenShell-hosted graphical PDF text-preview window.
//!
//! This is a text preview for the currently supported PDF subset, not a page
//! rasterizer. Do not register it as a general PDF handler.
use core::cell::UnsafeCell;
use libwoven::{rgb, Canvas, GraphicsSurface};
use libwoven::process::Id;
use wovenhat_pdf::{extract_plain_text, inspect, MAX_DOCUMENT_BYTES, MAX_TEXT_BYTES};
use wovenhat_pdf::xref::{read_table, Entry};
use wovenhat_pdf::objects::Reference;
use wovenhat_pdf::streams::decoded_stream;
use wovenhat_pdf::paint::paint_text_preview;
use wovenhat_pdf::document::page_content;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 320;
const BG: u32 = rgb(22, 29, 44);
const PAGE: u32 = rgb(248, 249, 253);
const INK: u32 = rgb(28, 38, 57);
const ACCENT: u32 = rgb(88, 210, 176);
const WIDTH_MAX: usize = 480;
const HEIGHT_MAX: usize = 320;
const DECODE_CAPACITY: usize = 64 * 1024;
const MAX_XREF: usize = 128;
struct DecodeBytes(UnsafeCell<[u8; DECODE_CAPACITY]>);
unsafe impl Sync for DecodeBytes {}
static DECODED: DecodeBytes = DecodeBytes(UnsafeCell::new([0; DECODE_CAPACITY]));
struct XrefBuffer(UnsafeCell<[Entry; MAX_XREF]>);
unsafe impl Sync for XrefBuffer {}
static XREF: XrefBuffer = XrefBuffer(UnsafeCell::new([Entry {
    object: 0, generation: 0, offset: 0, in_use: false
}; MAX_XREF]));
// SAFETY: single-threaded GUI event loop exclusively updates selection state.
static mut DECODED_LENGTH: usize = 0;
static mut DOCUMENT_LENGTH: usize = 0;
static mut SELECTED_PAGE: usize = 0;
static mut PAGE_COUNT: usize = 0;

struct Pixels(UnsafeCell<[u32; WIDTH_MAX * HEIGHT_MAX]>);
// SAFETY: this GUI is a single-threaded process and owns its static buffer.
unsafe impl Sync for Pixels {}
static PIXELS: Pixels = Pixels(UnsafeCell::new([0; WIDTH_MAX * HEIGHT_MAX]));

struct PdfBytes(UnsafeCell<[u8; MAX_DOCUMENT_BYTES]>);
// SAFETY: the event loop accesses this buffer exclusively.
unsafe impl Sync for PdfBytes {}
static PDF: PdfBytes = PdfBytes(UnsafeCell::new([0; MAX_DOCUMENT_BYTES]));

struct Preview(UnsafeCell<[u8; MAX_TEXT_BYTES]>);
// SAFETY: the event loop accesses this buffer exclusively.
unsafe impl Sync for Preview {}
static TEXT: Preview = Preview(UnsafeCell::new([0; MAX_TEXT_BYTES]));

fn exit_error(msg: &str) -> ! {
    let _ = libwoven::print(msg);
    libwoven::exit(4)
}

fn parse_id(input: &[u8]) -> Option<Id> {
    if input.is_empty() { return None; }
    let mut number = 0u64;
    for b in input {
        if !b.is_ascii_digit() { return None; }
        number = number.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(Id(number))
}

fn read_preview(path: Option<&[u8]>) -> usize {
    let Some(path) = path.and_then(|p| core::str::from_utf8(p).ok()) else {
        return 0;
    };
    let Ok(fd) = libwoven::open(path) else { return 0 };
    let buffer = unsafe { &mut *PDF.0.get() };
    let mut n = 0;
    let mut good = true;
    loop {
        if n == buffer.len() {
            let mut one = [0];
            if !matches!(libwoven::read(fd, &mut one), Ok(0)) { good = false; }
            break;
        }
        match libwoven::read(fd, &mut buffer[n..]) {
            Ok(0) => break,
            Ok(k) if k <= buffer.len() - n => n += k,
            _ => { good = false; break; }
        }
    }
    let _ = libwoven::close(fd);
    if !good || inspect(&buffer[..n]).is_err() { return 0; }
    unsafe { DOCUMENT_LENGTH = n; }
    let output = unsafe { &mut *TEXT.0.get() };
    if select_page(0) {
        return 0;
    }
    extract_plain_text(&buffer[..n], output).unwrap_or_default()
}

/// Select one actual page, refusing documents whose page contents cannot be
/// decoded by this bounded preview. No arbitrary stream selection is allowed.
fn select_page(index: usize) -> bool {
    let length = unsafe { DOCUMENT_LENGTH };
    let bytes = unsafe { &*PDF.0.get() };
    let entries = unsafe { &mut *XREF.0.get() };
    let Ok(count) = read_table(&bytes[..length], entries) else { return false };
    let mut pages = [Reference { number: 0, generation: 0 }; 64];
    let Ok((contents, page_count)) =
        page_content(&bytes[..length], &entries[..count], index, &mut pages)
    else { return false };
    let decoded = unsafe { &mut *DECODED.0.get() };
    let Ok(size) = decoded_stream(&bytes[..length], &entries[..count], contents, decoded)
    else { return false };
    if paint_text_preview(&decoded[..size], 280.0, 1.0, 456, 250, |_| Ok(())).is_err() {
        return false;
    }
    unsafe {
        DECODED_LENGTH = size;
        SELECTED_PAGE = index;
        PAGE_COUNT = page_count;
    }
    true
}

fn paint(window: &GraphicsSurface, content_len: usize, first_line: usize) -> bool {
    let pixels = unsafe { &mut *PIXELS.0.get() };
    let Some(mut canvas) = Canvas::new(
        pixels, 0, 0, window.width() as usize, window.height() as usize
    ) else { return false };
    canvas.fill_rect(0, 0, canvas.width(), canvas.height(), BG);
    canvas.draw_text(14, 12, "WovenPDF  |  Text Preview", ACCENT);
    canvas.draw_text(14, 32, "Left: previous   Right: next", ACCENT);
    canvas.fill_rect(12, 56, 456, 250, PAGE);
    let decoded_len = unsafe { DECODED_LENGTH };
    if decoded_len != 0 {
        let decoded = unsafe { &*DECODED.0.get() };
        let page_height = 280.0;
        let result = paint_text_preview(&decoded[..decoded_len], page_height, 1.0, 456, 250, |item| {
            canvas.draw_text(25 + item.x, 56 + item.y, item.text, INK);
            Ok(())
        });
        if result.is_err() {
            canvas.draw_text(25, 75, "Unsupported page content", INK);
        }
    } else if content_len == 0 {
        canvas.draw_text(25, 75, "No supported PDF text to display.", INK);
        canvas.draw_text(25, 94, "Uncompressed Tj strings only.", INK);
    } else {
        let data = unsafe { &*TEXT.0.get() };
        let mut line = 0usize;
        let mut start = 0usize;
        let mut y = 74i32;
        for i in 0..=content_len {
            if i == content_len || data[i] == b'\n' {
                if line >= first_line && y < 285 {
                    // The existing Canvas text API requires UTF-8. Truncate
                    // invalid input rather than interpreting arbitrary bytes.
                    let end = i.min(start + 68);
                    if let Ok(s) = core::str::from_utf8(&data[start..end]) {
                        canvas.draw_text(25, y, s, INK);
                    }
                    y += 15;
                }
                line += 1;
                start = i.saturating_add(1);
            }
        }
    }
    window.write_canvas(&canvas).is_ok()
}

pub fn run(shell: Option<&[u8]>, path: Option<&[u8]>) -> ! {
    let Some(shell) = shell.and_then(parse_id) else {
        exit_error("WovenPDF: missing WovenShell process id\n")
    };
    let content_len = read_preview(path);
    let window = GraphicsSurface::create(WIDTH, HEIGHT)
        .unwrap_or_else(|_| exit_error("WovenPDF: surface create failed\n"));
    if !paint(&window, content_len, 0) || window.share_with(shell).is_err() {
        exit_error("WovenPDF: surface initialization failed\n")
    }
    const TITLE: &[u8] = b"WovenPDF Preview";
    let mut packet = [0u8; 16 + TITLE.len()];
    packet[..4].copy_from_slice(b"WOPN");
    packet[4..12].copy_from_slice(&window.handle().to_le_bytes());
    packet[12..14].copy_from_slice(&(WIDTH as u16).to_le_bytes());
    packet[14..16].copy_from_slice(&(HEIGHT as u16).to_le_bytes());
    packet[16..].copy_from_slice(TITLE);
    if libwoven::message_send(shell, &packet).is_err() {
        exit_error("WovenPDF: shell registration failed\n")
    }
    let mut message = [0u8; 64];
    let mut first_line = 0usize;
    let mut pressed = false;
    loop {
        if libwoven::wait_any(libwoven::wait::MESSAGE, None).is_err() {
            exit_error("WovenPDF: event wait failed\n")
        }
        if let Ok((n, sender)) = libwoven::message_receive(&mut message) {
            if sender != shell { continue; }
            if n >= 4 && &message[..4] == b"WCLS" { break; }
            if n == 10 && &message[..4] == b"WPBT" && message[4] == 0 {
                let now = message[5] != 0;
                let y = u16::from_le_bytes([message[8], message[9]]);
                if pressed && !now {\n                    let x = u16::from_le_bytes([message[6], message[7]]);\n                    if y < 54 && unsafe { PAGE_COUNT } > 0 {\n                        let old = unsafe { SELECTED_PAGE };\n                        let next = if x < 240 { old.saturating_sub(1) } else { old.saturating_add(1) };\n                        if next != old { let _ = select_page(next); }\n                    }
                    if y < 54 {
                        first_line = first_line.saturating_sub(8);
                    } else if y > 275 {
                        first_line = first_line.saturating_add(8).min(1024);
                    }
                    if !paint(&window, content_len, first_line) {
                        exit_error("WovenPDF: redraw failed\n")
                    }
                }
                pressed = now;
            }
        }
    }
    drop(window);
    libwoven::exit(0)
}
