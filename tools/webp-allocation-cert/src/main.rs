use core::alloc::{GlobalAlloc, Layout};
use std::alloc::System;
use std::sync::atomic::{AtomicUsize, Ordering};

use libwoven::codecs::{ImageStream, StreamError, decode_webp_rgba_stream_bounded};
use libwoven::media::parse_image_header;

const WEBP: [u8; 38] = [
    0x52, 0x49, 0x46, 0x46, 0x1e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x4c,
    0x11, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x07, 0x50, 0x94, 0x52, 0x94, 0xa7, 0xff,
    0x81, 0x88, 0xe8, 0x7f, 0x00, 0x00,
];
const EXTENDED_WEBP: [u8; 92] = [
    0x52, 0x49, 0x46, 0x46, 0x54, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58,
    0x0a, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x41, 0x4c,
    0x50, 0x48, 0x05, 0x00, 0x00, 0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x56, 0x50, 0x38, 0x20,
    0x28, 0x00, 0x00, 0x00, 0x90, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x02, 0x00, 0x02, 0x00, 0x00, 0xc0,
    0x12, 0x25, 0x00, 0x4e, 0x97, 0x40, 0x00, 0x8c, 0xd0, 0x00, 0xfe, 0xfe, 0x22, 0x72, 0xfd, 0x5d,
    0x89, 0xf0, 0x9f, 0xa0, 0xfe, 0xe6, 0xc7, 0x38, 0xb6, 0x43, 0x00, 0x00,
];
// Strict process-wide budget for the pinned backend's exercised VP8L and
// VP8X/ALPH paths. The measured extended path is ~703 KiB.
const HARD_BUDGET: usize = 1024 * 1024;

struct CountingAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static VIOLATION: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwards the caller's valid allocation layout to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_alloc(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forwards the pointer/layout pair supplied by the allocator contract.
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwards the pointer/layout/new-size triple supplied by the allocator contract.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                record_alloc(new_size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        replacement
    }
}

fn record_alloc(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed).saturating_add(size);
    TOTAL.fetch_add(size, Ordering::Relaxed);
    if live > HARD_BUDGET {
        VIOLATION.store(1, Ordering::Relaxed);
    }
    let mut peak = PEAK.load(Ordering::Relaxed);
    while live > peak {
        match PEAK.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

struct SliceStream<'a> {
    bytes: &'a [u8],
}

impl ImageStream for SliceStream<'_> {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, StreamError> {
        let start = usize::try_from(offset).map_err(|_| StreamError::OutOfRange)?;
        if start > self.bytes.len() {
            return Err(StreamError::OutOfRange);
        }
        let count = destination.len().min(self.bytes.len() - start);
        destination[..count].copy_from_slice(&self.bytes[start..start + count]);
        Ok(count)
    }
}

fn main() {
    // Warm the standard library, decoder tables, and error formatting before
    // the measured region. The measured region contains only the decode call.
    let metadata = parse_image_header(&WEBP).expect("fixture header");
    let mut warm_workspace = WEBP;
    let mut warm_pixels = [0u32; 4];
    let mut warm_stream = SliceStream { bytes: &WEBP };
    decode_webp_rgba_stream_bounded(
        &mut warm_stream,
        metadata,
        &mut warm_workspace,
        &mut warm_pixels,
    )
    .expect("warm decode");
    let extended_metadata = parse_image_header(&EXTENDED_WEBP).expect("extended fixture header");
    let mut extended_workspace = EXTENDED_WEBP;
    let mut extended_pixels = [0u32; 4];
    let mut extended_stream = SliceStream {
        bytes: &EXTENDED_WEBP,
    };
    decode_webp_rgba_stream_bounded(
        &mut extended_stream,
        extended_metadata,
        &mut extended_workspace,
        &mut extended_pixels,
    )
    .expect("warm extended decode");

    let mut peak = 0usize;
    let mut total = 0usize;
    for bytes in [&WEBP[..], &EXTENDED_WEBP[..]] {
        let metadata = parse_image_header(bytes).expect("valid fixture header");
        LIVE.store(0, Ordering::Relaxed);
        PEAK.store(0, Ordering::Relaxed);
        TOTAL.store(0, Ordering::Relaxed);
        VIOLATION.store(0, Ordering::Relaxed);
        let mut workspace = [0u8; 128];
        let mut pixels = [0u32; 4];
        let mut stream = SliceStream { bytes };
        decode_webp_rgba_stream_bounded(&mut stream, metadata, &mut workspace, &mut pixels)
            .expect("valid decode");
        assert_eq!(LIVE.load(Ordering::Relaxed), 0, "decoder leaked live bytes");
        peak = peak.max(PEAK.load(Ordering::Relaxed));
        total = total.max(TOTAL.load(Ordering::Relaxed));
    }
    let malformed = malformed_fixture();
    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    VIOLATION.store(0, Ordering::Relaxed);
    let mut malformed_workspace = [0u8; 128];
    let mut malformed_pixels = [0u32; 4];
    let mut malformed_stream = SliceStream { bytes: &malformed };
    assert!(
        decode_webp_rgba_stream_bounded(
            &mut malformed_stream,
            metadata,
            &mut malformed_workspace,
            &mut malformed_pixels,
        )
        .is_err()
    );
    assert_eq!(
        LIVE.load(Ordering::Relaxed),
        0,
        "malformed decode leaked live bytes"
    );
    peak = peak.max(PEAK.load(Ordering::Relaxed));
    total = total.max(TOTAL.load(Ordering::Relaxed));
    assert_eq!(
        VIOLATION.load(Ordering::Relaxed),
        0,
        "allocation budget violation"
    );
    assert!(peak <= HARD_BUDGET, "peak allocation exceeded hard budget");
    println!(
        "webp allocation certification: PASS peak_live={peak} total_allocated={total} budget={HARD_BUDGET}"
    );
}

fn malformed_fixture() -> [u8; 38] {
    let mut fixture = WEBP;
    fixture[0] = b'X';
    fixture
}
