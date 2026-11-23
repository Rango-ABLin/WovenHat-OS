//! Host coverage for the proportional typography contract added for the
//! desktop modernization pass: `hhea` vertical metrics, scaled advances,
//! glyph ink bounds, and measurement that matches drawing.
//!
//! The fixture is a synthetic TrueType face, not the production font: it
//! makes the arithmetic exact (1000 units/em, 600-unit 'A') so a regression
//! in the scaling or rounding rules fails here deterministically.

use libwoven::Canvas;
use libwoven::graphics::TrueTypeFace;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

/// Build a face with `unitsPerEm = 1000`, one mapped character ('A' -> glyph
/// 1), `hhea` metrics 800/-200/+100, and one glyph with ink bounds
/// (-20, 500).
fn fixture() -> [u8; 404] {
    let mut bytes = [0u8; 404];
    put32(&mut bytes, 0, 0x0001_0000);
    put16(&mut bytes, 4, 7);

    for (record, (tag, offset, length)) in [
        (*b"head", 200u32, 52u32),
        (*b"maxp", 252u32, 6u32),
        (*b"cmap", 258u32, 44u32),
        (*b"hhea", 302u32, 36u32),
        (*b"hmtx", 338u32, 8u32),
        (*b"loca", 346u32, 12u32),
        (*b"glyf", 358u32, 46u32),
    ]
    .into_iter()
    .enumerate()
    {
        let base = 12 + record * 16;
        bytes[base..base + 4].copy_from_slice(&tag);
        put32(&mut bytes, base + 8, offset);
        put32(&mut bytes, base + 12, length);
    }

    // head: unitsPerEm at +18, indexToLocFormat at +50 (long offsets).
    put32(&mut bytes, 200, 0x0001_0000);
    put16(&mut bytes, 218, 1000);
    put16(&mut bytes, 250, 1);

    // maxp: numGlyphs = 2.
    put32(&mut bytes, 252, 0x0001_0000);
    put16(&mut bytes, 256, 2);

    // cmap: one format-4 subtable mapping 'A' (0x41) to glyph 1.
    put16(&mut bytes, 258, 0);
    put16(&mut bytes, 260, 1);
    put16(&mut bytes, 262, 3);
    put16(&mut bytes, 264, 1);
    put32(&mut bytes, 266, 12);
    put16(&mut bytes, 270, 4);
    put16(&mut bytes, 272, 32);
    put16(&mut bytes, 274, 0);
    put16(&mut bytes, 276, 4);
    put16(&mut bytes, 278, 0);
    put16(&mut bytes, 280, 0);
    put16(&mut bytes, 284, 0x0041);
    put16(&mut bytes, 286, 0xffff);
    put16(&mut bytes, 288, 0);
    put16(&mut bytes, 290, 0x0041);
    put16(&mut bytes, 292, 0xffff);
    put16(&mut bytes, 294, 0xffc0); // 'A' (0x41) + (-64) = glyph 1
    put16(&mut bytes, 296, 1);
    put16(&mut bytes, 298, 0);
    put16(&mut bytes, 300, 0);

    // hhea: ascender +4, descender +6, lineGap +8, numberOfHMetrics +34.
    put32(&mut bytes, 302, 0x0001_0000);
    put16(&mut bytes, 306, 800);
    put16(&mut bytes, 308, (-200i16) as u16);
    put16(&mut bytes, 310, 100);
    put16(&mut bytes, 336, 2);

    // hmtx: glyph 0 advance 500/lsb 0; glyph 1 ('A') advance 600/lsb 10.
    put16(&mut bytes, 338, 500);
    put16(&mut bytes, 340, 0);
    put16(&mut bytes, 342, 600);
    put16(&mut bytes, 344, 10);

    // loca: [0, 0, 46] -- glyph 0 is empty, glyph 1 spans the whole `glyf`.
    put32(&mut bytes, 346, 0);
    put32(&mut bytes, 350, 0);
    put32(&mut bytes, 354, 46);

    // glyf: one contour, xMin -20, xMax 500.
    put16(&mut bytes, 358, 1);
    put16(&mut bytes, 360, (-20i16) as u16);
    put16(&mut bytes, 362, (-10i16) as u16);
    put16(&mut bytes, 364, 500);
    put16(&mut bytes, 366, 700);

    bytes
}

#[test]
fn face_reports_hhea_vertical_metrics() {
    let bytes = fixture();
    let face = TrueTypeFace::from_bytes(&bytes).expect("fixture face");
    assert_eq!(face.units_per_em(), 1000);
    assert_eq!(face.ascender(), 800);
    assert_eq!(face.descender(), -200);
    assert_eq!(face.line_gap(), 100);
    // (800 - -200 + 100) * 20 / 1000 = 22
    assert_eq!(face.line_height(20), 22);
    // 800 * 20 / 1000 = 16
    assert_eq!(face.ascent_px(20), 16);
}

#[test]
fn face_scales_advances_by_em_size() {
    let bytes = fixture();
    let face = TrueTypeFace::from_bytes(&bytes).expect("fixture face");
    assert_eq!(face.advance_px('A', 20), Some(12));
    assert_eq!(face.advance_px('A', 10), Some(6));
    assert_eq!(face.advance_px('A', 14), Some(8));
    assert_eq!(face.advance_px('B', 14), None);
}

#[test]
fn face_reports_glyph_ink_bounds() {
    let bytes = fixture();
    let face = TrueTypeFace::from_bytes(&bytes).expect("fixture face");
    assert_eq!(face.glyph_x_bounds('A'), Some((-20, 500)));
    assert_eq!(face.glyph_x_bounds('B'), None);
}

#[test]
fn measurement_matches_proportional_advances() {
    let bytes = fixture();
    let face = TrueTypeFace::from_bytes(&bytes).expect("fixture face");
    assert_eq!(Canvas::measure_text_proportional(&face, "AA", 20), 24);
    assert_eq!(Canvas::measure_text_proportional(&face, "A\nAA", 20), 24);
    assert_eq!(Canvas::measure_text_proportional(&face, "", 20), 0);
}

#[test]
fn production_font_parses_and_reports_metrics() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../assets/fonts/NotoSans-Regular.ttf"
    );
    let bytes = std::fs::read(path).expect("production font asset");
    let face = TrueTypeFace::from_bytes(&bytes).expect("production font must validate");
    assert!(face.units_per_em() >= 1000);
    assert!(face.ascender() > 0);
    assert!(face.descender() < 0);
    let advance = face.advance_px('M', 12).expect("'M' must be mapped");
    assert!(
        (6..=12).contains(&advance),
        "12px 'M' advance was {advance}"
    );
    assert!(face.line_height(12) >= 14);
}

#[test]
fn unsupported_real_glyph_uses_bitmap_fallback() {
    // A 12 px 'S' has more contour points than the fixed path budget. It must
    // fall back to the bounded bitmap glyph rather than render as a blank.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../assets/fonts/NotoSans-Regular.ttf"
    );
    let bytes = std::fs::read(path).expect("production font asset");
    let face = TrueTypeFace::from_bytes(&bytes).expect("production font must validate");
    let mut pixels = [0u32; 64 * 16];
    let mut canvas = Canvas::new(&mut pixels, 0, 0, 64, 16).expect("text canvas");
    canvas.draw_text_ttf(2, 2, "S", 0x00ff_ffff, &face, 12);
    assert!(
        pixels.iter().any(|pixel| *pixel != 0),
        "a glyph that cannot fit the real-face path budget must still be drawn"
    );
}

#[test]
fn drawing_advances_by_real_glyph_width_not_a_fixed_cell() {
    let bytes = fixture();
    let face = TrueTypeFace::from_bytes(&bytes).expect("fixture face");
    let mut pixels = [0u32; 200 * 40];
    let mut canvas = Canvas::new(&mut pixels, 0, 0, 200, 40).expect("canvas");
    let pen = canvas.draw_text_proportional(10, 4, "AAA", 0x00ff_ffff, &face, 20);
    assert_eq!(pen, 46, "three 12px advances from x=10");
}
