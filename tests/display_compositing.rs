//! Host tests for the software display stack.
//!
//! Compositing is pure computation over a pixel buffer, so it can be verified
//! exhaustively on the host without booting. These tests check the properties
//! the kernel relies on: clipping never writes outside a surface, damage
//! tracking stays bounded, and presentation touches only the regions that
//! actually changed.
#![allow(dead_code)]

extern crate alloc;

#[path = "../kernel/src/display.rs"]
mod display;
#[path = "../kernel/src/font.rs"]
mod font;

use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use display::{Damage, MAX_DAMAGE_RECTS, Rect, Surface, blend_pixel, present, present_scaled, rgb};

const WHITE: u32 = rgb(255, 255, 255);
const RED: u32 = rgb(255, 0, 0);
const BLUE: u32 = rgb(0, 0, 255);

fn info(width: usize, height: usize, format: PixelFormat, bytes: usize) -> FrameBufferInfo {
    FrameBufferInfo {
        byte_len: width * height * bytes,
        width,
        height,
        pixel_format: format,
        bytes_per_pixel: bytes,
        stride: width,
    }
}

// ----------------------------------------------------------------- geometry

#[test]
fn intersection_is_empty_when_rectangles_do_not_meet() {
    let left = Rect::new(0, 0, 10, 10);
    assert_eq!(left.intersect(Rect::new(20, 0, 5, 5)), Rect::EMPTY);
    assert_eq!(
        left.intersect(Rect::new(10, 0, 5, 5)),
        Rect::EMPTY,
        "touching edges do not overlap"
    );
    assert_eq!(
        left.intersect(Rect::new(5, 5, 10, 10)),
        Rect::new(5, 5, 5, 5)
    );
}

#[test]
fn intersection_clips_negative_origins() {
    let surface = Rect::new(0, 0, 10, 10);
    assert_eq!(
        Rect::new(-4, -4, 8, 8).intersect(surface),
        Rect::new(0, 0, 4, 4),
        "a rectangle starting off-surface must clip to the visible part"
    );
}

#[test]
fn union_ignores_empty_operands() {
    let rect = Rect::new(3, 4, 5, 6);
    assert_eq!(rect.union(Rect::EMPTY), rect);
    assert_eq!(Rect::EMPTY.union(rect), rect);
    assert_eq!(
        Rect::new(0, 0, 2, 2).union(Rect::new(8, 8, 2, 2)),
        Rect::new(0, 0, 10, 10)
    );
}

#[test]
fn edges_saturate_rather_than_wrapping() {
    let huge = Rect::new(i32::MAX - 1, 0, usize::MAX, 1);
    assert_eq!(huge.right(), i32::MAX, "a width beyond i32 must saturate");
}

// ------------------------------------------------------------------ surface

#[test]
fn fill_clips_to_the_surface_and_reports_what_it_touched() {
    let mut surface = Surface::new(8, 8);
    let touched = surface.fill_rect(Rect::new(-2, -2, 5, 5), WHITE);
    assert_eq!(touched, Rect::new(0, 0, 3, 3));
    assert_eq!(surface.pixel(0, 0), Some(WHITE));
    assert_eq!(surface.pixel(2, 2), Some(WHITE));
    assert_eq!(
        surface.pixel(3, 3),
        Some(0),
        "outside the clip stays untouched"
    );
}

#[test]
fn a_fill_entirely_off_surface_writes_nothing() {
    let mut surface = Surface::new(4, 4);
    surface.fill_rect(Rect::new(0, 0, 4, 4), WHITE);
    let touched = surface.fill_rect(Rect::new(10, 10, 4, 4), RED);
    assert_eq!(touched, Rect::EMPTY);
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(
                surface.pixel(x, y),
                Some(WHITE),
                "off-surface fill corrupted ({x},{y})"
            );
        }
    }
}

#[test]
fn pixels_outside_the_surface_read_as_none() {
    let surface = Surface::new(4, 4);
    assert_eq!(surface.pixel(-1, 0), None);
    assert_eq!(surface.pixel(0, -1), None);
    assert_eq!(surface.pixel(4, 0), None);
    assert_eq!(surface.pixel(0, 4), None);
}

#[test]
fn blit_places_a_surface_and_clips_at_the_edge() {
    let mut source = Surface::new(4, 4);
    source.fill_rect(source.bounds(), RED);
    let mut target = Surface::new(8, 8);
    target.fill_rect(target.bounds(), BLUE);

    let touched = target.blit(&source, 6, 6);
    assert_eq!(
        touched,
        Rect::new(6, 6, 2, 2),
        "only the on-surface part is copied"
    );
    assert_eq!(target.pixel(6, 6), Some(RED));
    assert_eq!(target.pixel(7, 7), Some(RED));
    assert_eq!(
        target.pixel(5, 5),
        Some(BLUE),
        "the blit must not bleed left"
    );
}

#[test]
fn blit_with_a_negative_origin_copies_the_correct_source_pixels() {
    // A source clipped at the top-left must copy from its interior, not from
    // its origin, or the image would shift instead of being cropped.
    let mut source = Surface::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            source.set_pixel(x, y, rgb(x as u8, y as u8, 0));
        }
    }
    let mut target = Surface::new(4, 4);
    target.blit(&source, -2, -2);
    assert_eq!(
        target.pixel(0, 0),
        Some(rgb(2, 2, 0)),
        "clipping must crop the source, not translate it"
    );
    assert_eq!(target.pixel(1, 1), Some(rgb(3, 3, 0)));
}

#[test]
fn text_reports_the_region_it_covered() {
    let mut surface = Surface::new(64, 32);
    let touched = surface.draw_text(2, 3, "Hi", WHITE);
    assert_eq!(touched.x, 2);
    assert_eq!(touched.y, 3);
    assert!(touched.width >= font::WIDTH, "at least one glyph wide");
    assert!(touched.height >= font::HEIGHT);
    // Some pixel of the text must have landed.
    let drawn = (0..32).any(|y| (0..64).any(|x| surface.pixel(x, y) == Some(WHITE)));
    assert!(drawn, "draw_text wrote nothing");
}

#[test]
fn a_newline_moves_down_a_line_rather_than_overdrawing() {
    let mut surface = Surface::new(64, 32);
    surface.draw_text(0, 0, "A\nA", WHITE);
    let first_line =
        (0..font::HEIGHT as i32).any(|y| (0..8).any(|x| surface.pixel(x, y) == Some(WHITE)));
    let second_line = (font::LINE_HEIGHT..font::LINE_HEIGHT + font::HEIGHT as i32)
        .any(|y| (0..8).any(|x| surface.pixel(x, y) == Some(WHITE)));
    assert!(
        first_line && second_line,
        "both lines must be drawn separately"
    );
}

// ------------------------------------------------------------------- damage

#[test]
fn damage_ignores_empty_regions() {
    let mut damage = Damage::new();
    damage.add(Rect::EMPTY);
    damage.add(Rect::new(0, 0, 0, 5));
    assert!(damage.is_empty());
}

#[test]
fn damage_drops_a_region_already_covered() {
    let mut damage = Damage::new();
    damage.add(Rect::new(0, 0, 10, 10));
    damage.add(Rect::new(2, 2, 4, 4));
    assert_eq!(damage.len(), 1, "a contained region adds no work");
}

#[test]
fn damage_merges_regions_that_form_exactly_a_rectangle() {
    // A window drawn in 320x40 tiles, five passes over the same area (as a
    // shell repaints wallpaper, shadow, title and body), plus a distant clock.
    let mut damage = Damage::new();
    for _pass in 0..5 {
        for row in 0..6 {
            damage.add(Rect::new(30, 20 + row * 40, 320, 40));
            damage.add(Rect::new(350, 20 + row * 40, 50, 40));
        }
    }
    damage.add(Rect::new(560, 380, 60, 20));
    assert!(!damage.is_collapsed(), "adjacent tiles must not overflow the list");
    assert_eq!(damage.regions(), &[Rect::new(30, 20, 370, 240), Rect::new(560, 380, 60, 20)]);
}

#[test]
fn damage_keeps_regions_whose_union_would_add_pixels() {
    let mut damage = Damage::new();
    damage.add(Rect::new(0, 0, 10, 10));
    damage.add(Rect::new(10, 5, 10, 10)); // adjacent but offset: an L shape
    damage.add(Rect::new(30, 0, 5, 5)); // separate
    assert_eq!(damage.len(), 3);
    // A region covering tracked ones replaces them.
    damage.add(Rect::new(0, 0, 40, 20));
    assert_eq!(damage.regions(), &[Rect::new(0, 0, 40, 20)]);
}

#[test]
fn damage_collapses_to_a_bounding_box_when_full() {
    let mut damage = Damage::new();
    for index in 0..MAX_DAMAGE_RECTS {
        damage.add(Rect::new(index as i32 * 4, 0, 2, 2));
    }
    assert_eq!(damage.len(), MAX_DAMAGE_RECTS);
    assert!(!damage.is_collapsed());

    damage.add(Rect::new(200, 100, 2, 2));
    assert!(damage.is_collapsed(), "the list must stay bounded");
    assert_eq!(damage.len(), 1);
    let bounds = damage.regions()[0];
    assert!(
        bounds.contains(0, 0),
        "the bounding box must cover the first region"
    );
    assert!(bounds.contains(200, 100), "and the one that overflowed it");
}

#[test]
fn a_collapsed_list_keeps_growing_its_bounding_box() {
    let mut damage = Damage::new();
    for index in 0..=MAX_DAMAGE_RECTS {
        damage.add(Rect::new(index as i32 * 4, 0, 2, 2));
    }
    assert!(damage.is_collapsed());
    damage.add(Rect::new(500, 500, 1, 1));
    assert_eq!(damage.len(), 1);
    assert!(damage.regions()[0].contains(500, 500));
}

#[test]
fn clearing_damage_resets_collapse() {
    let mut damage = Damage::new();
    for index in 0..=MAX_DAMAGE_RECTS {
        damage.add(Rect::new(index as i32 * 4, 0, 2, 2));
    }
    damage.clear();
    assert!(damage.is_empty());
    assert!(!damage.is_collapsed());
}

// ------------------------------------------------------------- presentation

#[test]
fn presentation_writes_only_the_damaged_region() {
    let mut surface = Surface::new(16, 16);
    surface.fill_rect(surface.bounds(), WHITE);

    let mut damage = Damage::new();
    damage.add(Rect::new(4, 4, 2, 2));

    let descriptor = info(16, 16, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    let written = present(&surface, &damage, &mut framebuffer, descriptor);

    assert_eq!(
        written, 4,
        "a 2x2 region is four pixels, not the whole screen"
    );
    // The damaged pixels carry the colour and the rest stay untouched.
    let at = |x: usize, y: usize| (y * descriptor.stride + x) * descriptor.bytes_per_pixel;
    assert_eq!(framebuffer[at(4, 4)], 255, "blue channel first under BGR");
    assert_eq!(
        framebuffer[at(0, 0)],
        0,
        "undamaged pixels must not be written"
    );
}

#[test]
fn presentation_honours_the_pixel_format() {
    let mut surface = Surface::new(2, 1);
    surface.set_pixel(0, 0, rgb(10, 20, 30));
    let mut damage = Damage::new();
    damage.add(Rect::new(0, 0, 1, 1));

    for (format, expected) in [
        (PixelFormat::Rgb, [10u8, 20, 30]),
        (PixelFormat::Bgr, [30u8, 20, 10]),
    ] {
        let descriptor = info(2, 1, format, 3);
        let mut framebuffer = vec![0u8; descriptor.byte_len];
        present(&surface, &damage, &mut framebuffer, descriptor);
        assert_eq!(&framebuffer[..3], &expected, "channel order for {format:?}");
    }
}

#[test]
fn presentation_clips_damage_to_the_screen() {
    let mut surface = Surface::new(8, 8);
    surface.fill_rect(surface.bounds(), WHITE);
    let mut damage = Damage::new();
    // Deliberately larger than both the surface and the screen.
    damage.add(Rect::new(-4, -4, 100, 100));

    let descriptor = info(4, 4, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    let written = present(&surface, &damage, &mut framebuffer, descriptor);
    assert_eq!(written, 16, "a 4x4 screen can only take sixteen pixels");
}

#[test]
fn presentation_with_no_damage_writes_nothing() {
    let mut surface = Surface::new(8, 8);
    surface.fill_rect(surface.bounds(), WHITE);
    let descriptor = info(8, 8, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    let written = present(&surface, &Damage::new(), &mut framebuffer, descriptor);
    assert_eq!(written, 0);
    assert!(framebuffer.iter().all(|byte| *byte == 0));
}

#[test]
fn presentation_respects_a_stride_wider_than_the_visible_width() {
    // A framebuffer row is often padded. Writing at width rather than stride
    // would skew the image progressively down the screen.
    let mut surface = Surface::new(2, 2);
    surface.fill_rect(surface.bounds(), WHITE);
    let mut damage = Damage::new();
    damage.add(surface.bounds());

    let mut descriptor = info(2, 2, PixelFormat::Bgr, 4);
    descriptor.stride = 8;
    descriptor.byte_len = descriptor.stride * descriptor.height * descriptor.bytes_per_pixel;
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    present(&surface, &damage, &mut framebuffer, descriptor);

    let row_one_start = descriptor.stride * descriptor.bytes_per_pixel;
    assert_eq!(
        framebuffer[row_one_start], 255,
        "second row must start at the stride"
    );
    assert_eq!(
        framebuffer[2 * descriptor.bytes_per_pixel],
        0,
        "padding past the visible width must stay untouched"
    );
}

#[test]
fn blend_pixel_is_exact_at_both_ends_and_rounds_between() {
    let (over, under) = (rgb(200, 100, 0), rgb(0, 50, 255));
    assert_eq!(blend_pixel(over, under, 0), under);
    assert_eq!(blend_pixel(over, under, 255), over);
    // Midpoint: (200*128 + 0*127 + 127) / 255 = 100.89 -> 100 per the formula.
    let mid = blend_pixel(over, under, 128);
    assert_eq!(mid >> 16 & 0xff, (200 * 128 + 127) / 255);
    assert_eq!(mid >> 8 & 0xff, (100 * 128 + 50 * 127 + 127) / 255);
    assert_eq!(mid & 0xff, (255 * 127 + 127) / 255);
    // Blending a colour over itself returns it at any opacity.
    for opacity in [1u8, 64, 127, 200, 254] {
        assert_eq!(blend_pixel(over, over, opacity), over);
    }
}

#[test]
fn blend_full_opacity_equals_blit_and_zero_changes_nothing() {
    let mut source = Surface::new(4, 3);
    source.fill_rect(Rect::new(0, 0, 4, 3), rgb(9, 99, 199));
    let mut blitted = Surface::new(10, 10);
    let mut blended = Surface::new(10, 10);
    blitted.fill_rect(blitted.bounds(), WHITE);
    blended.fill_rect(blended.bounds(), WHITE);
    assert_eq!(
        blitted.blit(&source, 2, 2),
        blended.blend(&source, 2, 2, 255)
    );
    for y in 0..10 {
        for x in 0..10 {
            assert_eq!(blitted.pixel(x, y), blended.pixel(x, y));
        }
    }
    assert_eq!(blended.blend(&source, 0, 0, 0), Rect::EMPTY);
    assert_eq!(blended.pixel(0, 0), Some(WHITE));
}

#[test]
fn blend_clips_like_blit_and_mixes_only_the_overlap() {
    let mut source = Surface::new(4, 4);
    source.fill_rect(source.bounds(), rgb(0, 0, 0));
    let mut target = Surface::new(6, 6);
    target.fill_rect(target.bounds(), rgb(200, 200, 200));
    // Hanging off the top-left corner: only the 2x2 overlap is touched.
    assert_eq!(target.blend(&source, -2, -2, 128), Rect::new(0, 0, 2, 2));
    let mixed = blend_pixel(rgb(0, 0, 0), rgb(200, 200, 200), 128);
    assert_eq!(target.pixel(1, 1), Some(mixed));
    assert_eq!(target.pixel(2, 2), Some(rgb(200, 200, 200)));
    // Entirely outside: nothing touched.
    assert_eq!(target.blend(&source, 10, 10, 128), Rect::EMPTY);
}

// ------------------------------------------------------------- display scale

/// Read a framebuffer pixel back as `0x00RRGGBB` from a 4-byte BGR buffer.
fn bgr_at(framebuffer: &[u8], descriptor: FrameBufferInfo, x: usize, y: usize) -> u32 {
    let offset = (y * descriptor.stride + x) * descriptor.bytes_per_pixel;
    rgb(
        framebuffer[offset + 2],
        framebuffer[offset + 1],
        framebuffer[offset],
    )
}

#[test]
fn scaled_presentation_turns_each_pixel_into_a_block() {
    let mut surface = Surface::new(4, 3);
    surface.fill_rect(surface.bounds(), WHITE);
    surface.set_pixel(1, 2, RED);
    let mut damage = Damage::new();
    damage.add(surface.bounds());

    let descriptor = info(8, 6, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    let written = present_scaled(&surface, &damage, &mut framebuffer, descriptor, 2);
    assert_eq!(
        written,
        8 * 6,
        "a 4x3 surface at scale 2 covers an 8x6 screen"
    );
    // Surface pixel (1, 2) is the 2x2 block at (2..4, 4..6), and only it.
    for (x, y) in [(2, 4), (3, 4), (2, 5), (3, 5)] {
        assert_eq!(
            bgr_at(&framebuffer, descriptor, x, y),
            RED,
            "block pixel ({x}, {y})"
        );
    }
    for (x, y) in [(1, 4), (4, 4), (2, 3), (0, 0), (7, 5)] {
        assert_eq!(
            bgr_at(&framebuffer, descriptor, x, y),
            WHITE,
            "outside pixel ({x}, {y})"
        );
    }
}

#[test]
fn scaled_presentation_scales_the_damage_too() {
    let mut surface = Surface::new(8, 8);
    surface.fill_rect(surface.bounds(), BLUE);
    let mut damage = Damage::new();
    damage.add(Rect::new(2, 1, 2, 1));

    let descriptor = info(24, 24, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    let written = present_scaled(&surface, &damage, &mut framebuffer, descriptor, 3);
    assert_eq!(written, 2 * 3 * 3, "a 2x1 region at scale 3 is a 6x3 block");
    assert_eq!(bgr_at(&framebuffer, descriptor, 6, 3), BLUE);
    assert_eq!(bgr_at(&framebuffer, descriptor, 11, 5), BLUE);
    assert_eq!(
        bgr_at(&framebuffer, descriptor, 5, 3),
        0,
        "left of the block untouched"
    );
    assert_eq!(
        bgr_at(&framebuffer, descriptor, 12, 3),
        0,
        "right of the block untouched"
    );
    assert_eq!(
        bgr_at(&framebuffer, descriptor, 6, 6),
        0,
        "below the block untouched"
    );
}

#[test]
fn scaled_presentation_clips_to_the_screen_and_scale_one_matches_present() {
    let mut surface = Surface::new(5, 5);
    surface.fill_rect(surface.bounds(), WHITE);
    surface.set_pixel(2, 2, RED);
    let mut damage = Damage::new();
    damage.add(surface.bounds());

    // A 5x5 surface at scale 2 is 10x10; a 7x9 screen takes only 7x9 of it.
    let descriptor = info(7, 9, PixelFormat::Bgr, 4);
    let mut framebuffer = vec![0u8; descriptor.byte_len];
    assert_eq!(
        present_scaled(&surface, &damage, &mut framebuffer, descriptor, 2),
        7 * 9
    );
    assert_eq!(bgr_at(&framebuffer, descriptor, 5, 5), RED);

    let mut scaled = vec![0u8; descriptor.byte_len];
    let mut plain = vec![0u8; descriptor.byte_len];
    assert_eq!(
        present_scaled(&surface, &damage, &mut scaled, descriptor, 1),
        present(&surface, &damage, &mut plain, descriptor)
    );
    assert_eq!(scaled, plain);
    assert_eq!(
        present_scaled(&surface, &damage, &mut scaled, descriptor, 0),
        0
    );
}

// ------------------------------------------------------------- pixel upload

fn le_bytes(pixels: &[u32]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|pixel| pixel.to_le_bytes())
        .collect()
}

#[test]
fn write_pixels_fills_exactly_the_rectangle_and_clears_the_high_byte() {
    let mut surface = Surface::new(6, 5);
    surface.fill_rect(surface.bounds(), WHITE);
    // A 3x2 block, one pixel carrying junk in its unused high byte.
    let block = [RED, BLUE, 0xff00_ff00, BLUE, RED, rgb(1, 2, 3)];
    assert_eq!(
        surface.write_pixels(Rect::new(2, 1, 3, 2), &le_bytes(&block)),
        Rect::new(2, 1, 3, 2)
    );
    assert_eq!(surface.pixel(2, 1), Some(RED));
    assert_eq!(surface.pixel(4, 1), Some(0x0000_ff00), "high byte cleared");
    assert_eq!(surface.pixel(4, 2), Some(rgb(1, 2, 3)));
    for (x, y) in [(1, 1), (5, 1), (2, 0), (2, 3)] {
        assert_eq!(
            surface.pixel(x, y),
            Some(WHITE),
            "outside ({x}, {y}) untouched"
        );
    }
}

#[test]
fn write_pixels_refuses_partial_rectangles_and_wrong_lengths() {
    let mut surface = Surface::new(4, 4);
    surface.fill_rect(surface.bounds(), WHITE);
    let four = le_bytes(&[RED; 4]);
    // Hanging off the edge, negative, empty, or a buffer of the wrong size:
    // nothing is written.
    assert_eq!(
        surface.write_pixels(Rect::new(3, 0, 2, 2), &four),
        Rect::EMPTY
    );
    assert_eq!(
        surface.write_pixels(Rect::new(-1, 0, 2, 2), &four),
        Rect::EMPTY
    );
    assert_eq!(
        surface.write_pixels(Rect::new(0, 0, 0, 2), &[]),
        Rect::EMPTY
    );
    assert_eq!(
        surface.write_pixels(Rect::new(0, 0, 2, 2), &four[..12]),
        Rect::EMPTY
    );
    assert_eq!(
        surface.write_pixels(Rect::new(0, 0, 1, 1), &four),
        Rect::EMPTY
    );
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(surface.pixel(x, y), Some(WHITE));
        }
    }
}

// ------------------------------------------------------------- region blit

#[test]
fn blit_region_copies_only_the_region_to_the_destination() {
    let mut source = Surface::new(6, 4);
    for y in 0..4 {
        for x in 0..6 {
            source.set_pixel(x, y, rgb(x as u8, y as u8, 7));
        }
    }
    let mut target = Surface::new(10, 10);
    target.fill_rect(target.bounds(), WHITE);
    // Source region (2, 1) 3x2 lands at (5, 6).
    assert_eq!(
        target.blit_region(&source, Rect::new(2, 1, 3, 2), 5, 6),
        Rect::new(5, 6, 3, 2)
    );
    assert_eq!(target.pixel(5, 6), Some(rgb(2, 1, 7)));
    assert_eq!(target.pixel(7, 7), Some(rgb(4, 2, 7)));
    for (x, y) in [(4, 6), (8, 6), (5, 5), (5, 8)] {
        assert_eq!(
            target.pixel(x, y),
            Some(WHITE),
            "outside ({x}, {y}) untouched"
        );
    }
}

#[test]
fn blit_region_clips_to_both_surfaces() {
    let mut source = Surface::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            source.set_pixel(x, y, rgb(x as u8, y as u8, 1));
        }
    }
    let mut target = Surface::new(5, 5);
    target.fill_rect(target.bounds(), WHITE);
    // A region hanging off the source's top-left keeps its offset: source
    // (0, 0) lands one pixel in from the requested origin.
    assert_eq!(
        target.blit_region(&source, Rect::new(-1, -1, 3, 3), 0, 0),
        Rect::new(1, 1, 2, 2)
    );
    assert_eq!(target.pixel(1, 1), Some(rgb(0, 0, 1)));
    assert_eq!(target.pixel(0, 0), Some(WHITE));
    // Off the target's bottom-right: only the overlap is written.
    assert_eq!(
        target.blit_region(&source, Rect::new(0, 0, 4, 4), 3, 3),
        Rect::new(3, 3, 2, 2)
    );
    assert_eq!(target.pixel(4, 4), Some(rgb(1, 1, 1)));
    // Entirely outside either surface: nothing.
    assert_eq!(
        target.blit_region(&source, Rect::new(4, 0, 2, 2), 0, 0),
        Rect::EMPTY
    );
    assert_eq!(
        target.blit_region(&source, Rect::new(0, 0, 2, 2), 9, 9),
        Rect::EMPTY
    );
}

// ------------------------------------------------------------- keyed blit

#[test]
fn keyed_blit_skips_the_key_and_copies_everything_else() {
    use display::TRANSPARENT_KEY;
    let mut overlay = Surface::new(4, 2);
    overlay.fill_rect(overlay.bounds(), TRANSPARENT_KEY);
    overlay.set_pixel(1, 0, WHITE);
    overlay.set_pixel(3, 1, RED);
    let mut target = Surface::new(6, 4);
    target.fill_rect(target.bounds(), BLUE);
    assert_eq!(
        target.blit_region_keyed(&overlay, overlay.bounds(), 2, 1, TRANSPARENT_KEY),
        Rect::new(2, 1, 4, 2),
        "the covered region is reported"
    );
    assert_eq!(target.pixel(3, 1), Some(WHITE));
    assert_eq!(target.pixel(5, 2), Some(RED));
    // Keyed pixels leave the destination untouched, and nothing outside the
    // region changes.
    for (x, y) in [(2, 1), (4, 1), (2, 2), (4, 2), (1, 1), (3, 3)] {
        assert_eq!(target.pixel(x, y), Some(BLUE), "({x}, {y})");
    }
    for y in 0..4 {
        for x in 0..6 {
            assert_ne!(
                target.pixel(x, y),
                Some(TRANSPARENT_KEY),
                "the key itself never lands"
            );
        }
    }
}

#[test]
fn keyed_blit_clips_like_the_region_blit() {
    use display::TRANSPARENT_KEY;
    let mut overlay = Surface::new(3, 3);
    overlay.fill_rect(overlay.bounds(), WHITE);
    let mut keyed = Surface::new(4, 4);
    let mut plain = Surface::new(4, 4);
    // With no keyed pixels it is exactly the region blit, clipping included.
    assert_eq!(
        keyed.blit_region_keyed(&overlay, Rect::new(-1, 0, 3, 3), 2, 2, TRANSPARENT_KEY),
        plain.blit_region(&overlay, Rect::new(-1, 0, 3, 3), 2, 2)
    );
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(keyed.pixel(x, y), plain.pixel(x, y));
        }
    }
    assert_eq!(
        keyed.blit_region_keyed(&overlay, overlay.bounds(), 9, 9, TRANSPARENT_KEY),
        Rect::EMPTY
    );
}

/// The per-pixel definition of scaled presentation, written for clarity:
/// physical pixel (x, y) shows surface pixel (x / scale, y / scale), and only
/// whole pixels inside the mapping are written.
fn present_reference(
    surface: &Surface,
    damage: &Damage,
    framebuffer: &mut [u8],
    descriptor: FrameBufferInfo,
    scale: usize,
) -> usize {
    let bytes = descriptor.bytes_per_pixel;
    let mut written = 0;
    for region in damage.regions() {
        let logical = region.intersect(surface.bounds());
        for y in 0..descriptor.height {
            for x in 0..descriptor.width {
                let (lx, ly) = ((x / scale) as i32, (y / scale) as i32);
                if lx < logical.x || lx >= logical.right() || ly < logical.y || ly >= logical.bottom() {
                    continue;
                }
                let offset = (y * descriptor.stride + x) * bytes;
                if offset + bytes > framebuffer.len() {
                    continue;
                }
                let color = surface.pixel(lx, ly).unwrap();
                let (red, green, blue) = ((color >> 16) as u8, (color >> 8) as u8, color as u8);
                let channels = match descriptor.pixel_format {
                    PixelFormat::Rgb => [red, green, blue],
                    _ => [blue, green, red],
                };
                framebuffer[offset..offset + bytes.min(3)].copy_from_slice(&channels[..bytes.min(3)]);
                written += 1;
            }
        }
    }
    written
}

#[test]
fn scaled_presentation_matches_the_per_pixel_definition() {
    // A surface wide enough that a scaled row spans several staging chunks.
    let mut surface = Surface::new(700, 9);
    for y in 0..9 {
        for x in 0..700 {
            surface.set_pixel(x, y, rgb((x * 7) as u8, (y * 31) as u8, (x / 3 + y) as u8));
        }
    }
    for format in [PixelFormat::Rgb, PixelFormat::Bgr] {
        for bytes in [3, 4] {
            for scale in 1..=3 {
                let mut damage = Damage::new();
                damage.add(Rect::new(3, 1, 650, 5));
                damage.add(Rect::new(690, 7, 40, 4)); // clipped by the surface
                let mut descriptor = info(1500, 20, format, bytes);
                descriptor.stride = 1504;
                // The mapping ends partway through a row.
                let length = (17 * descriptor.stride + 333) * bytes + 1;
                descriptor.byte_len = length;
                let (mut fast, mut slow) = (vec![0xa5u8; length], vec![0xa5u8; length]);
                let written = present_scaled(&surface, &damage, &mut fast, descriptor, scale);
                let expected = present_reference(&surface, &damage, &mut slow, descriptor, scale);
                assert_eq!(written, expected, "{format:?} {bytes} bytes, scale {scale}: pixel count");
                // Colour bytes must match; a 4-byte pixel's padding byte is
                // not part of the colour.
                for (index, (a, b)) in fast.iter().zip(&slow).enumerate() {
                    if bytes == 4 && index % 4 == 3 {
                        continue;
                    }
                    assert_eq!(a, b, "{format:?} {bytes} bytes, scale {scale}: byte {index}");
                }
            }
        }
    }
}

#[test]
fn a_scaled_row_repeats_each_pixel_and_pads_with_black() {
    let mut surface = Surface::new(5, 2);
    for x in 0..5 {
        surface.set_pixel(x, 1, rgb(x as u8 + 1, 0, 0));
    }
    for scale in 1..=3usize {
        for start in 0..4usize {
            let mut out = [0xffff_ffffu32; 20];
            surface.scaled_row(1, start, scale, &mut out);
            for (index, pixel) in out.iter().enumerate() {
                let x = ((start + index) / scale) as i32;
                assert_eq!(Some(*pixel), Some(surface.pixel(x, 1).unwrap_or(0)), "scale {scale} start {start} index {index}");
            }
        }
    }
    let mut out = [7u32; 4];
    surface.scaled_row(2, 0, 2, &mut out);
    assert_eq!(out, [0; 4], "a row below the surface is black");
}
