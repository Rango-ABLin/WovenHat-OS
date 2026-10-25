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
