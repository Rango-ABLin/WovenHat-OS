//! Software display stack: geometry, an off-screen surface, damage tracking,
//! and a single presentation path to the framebuffer.
//!
//! Drawing previously went straight to the framebuffer one pixel at a time,
//! with a bounds check and a pixel-format branch per pixel. That is workable
//! for a static demo and wrong for anything that redraws: a visible frame is
//! modified while the display is scanning it, so changes tear, and a full
//! repaint costs width × height checked writes however little actually moved.
//!
//! Here, drawing goes to a [`Surface`] the display owns, each operation
//! records what it touched in a [`Damage`] list, and [`present`] copies only
//! those regions to the framebuffer, converting the pixel format once per run
//! of pixels rather than once per pixel.
//!
//! Pixels are stored as `0x00RRGGBB` so composition is format-independent;
//! conversion to the framebuffer's layout happens only at presentation.

use alloc::vec;
use alloc::vec::Vec;

use bootloader_api::info::{FrameBufferInfo, PixelFormat};

/// Rectangles the damage list tracks before collapsing to a bounding box.
///
/// A bounded list keeps presentation predictable. Beyond this, merging into one
/// region costs less than tracking more.
pub const MAX_DAMAGE_RECTS: usize = 16;

/// An axis-aligned rectangle in pixels.
///
/// Geometry lives here rather than in `gui` because the display stack is below
/// the widget layer; `gui` re-exports this type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub const EMPTY: Self = Self {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    pub const fn new(x: i32, y: i32, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Exclusive right edge, saturating rather than wrapping.
    pub fn right(self) -> i32 {
        self.x
            .saturating_add(i32::try_from(self.width).unwrap_or(i32::MAX))
    }

    /// Exclusive bottom edge, saturating rather than wrapping.
    pub fn bottom(self) -> i32 {
        self.y
            .saturating_add(i32::try_from(self.height).unwrap_or(i32::MAX))
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    /// The overlap of two rectangles, empty when they do not meet.
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            return Self::EMPTY;
        }
        Self {
            x,
            y,
            width: (right - x) as usize,
            height: (bottom - y) as usize,
        }
    }

    /// The smallest rectangle containing both, ignoring empty operands.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self {
            x,
            y,
            width: (right - x) as usize,
            height: (bottom - y) as usize,
        }
    }
}

/// The colour a keyed blit treats as transparent: magenta, which no WovenHat
/// chrome uses. A client draws an overlay (text, borders, a pointer) on a
/// surface filled with it, and a keyed blit copies everything else.
pub const TRANSPARENT_KEY: u32 = 0x00ff_00ff;

/// Pack a colour into the surface's `0x00RRGGBB` representation.
pub const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

/// An off-screen pixel buffer.
pub struct Surface {
    pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl Surface {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: vec![0; width.saturating_mul(height)],
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Exercised by `tests/display_compositing.rs`; the kernel reaches the
    /// same value through [`Surface::bounds`].
    #[allow(dead_code)]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The whole surface as a rectangle, for clipping.
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// Read one pixel, or `None` outside the surface.
    ///
    /// Used by `tests/display_compositing.rs` to verify clipping. The kernel
    /// has no read path yet; a compositor reading back a client surface will.
    #[allow(dead_code)]
    pub fn pixel(&self, x: i32, y: i32) -> Option<u32> {
        if !self.bounds().contains(x, y) {
            return None;
        }
        self.pixels
            .get(y as usize * self.width + x as usize)
            .copied()
    }

    /// Fill `out` with physical row pixels starting at physical column `x`,
    /// each surface pixel of row `y` repeated `scale` times; pixels beyond
    /// the surface are black.
    #[allow(dead_code)]
    pub fn scaled_row(&self, y: usize, x: usize, scale: usize, out: &mut [u32]) {
        let scale = scale.max(1);
        let row = if y < self.height { &self.pixels[y * self.width..(y + 1) * self.width] } else { &[][..] };
        let (mut source, mut phase) = (x / scale, x % scale);
        for pixel in out {
            *pixel = row.get(source).copied().unwrap_or(0);
            phase += 1;
            if phase == scale {
                phase = 0;
                source += 1;
            }
        }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if !self.bounds().contains(x, y) {
            return;
        }
        let index = y as usize * self.width + x as usize;
        if let Some(slot) = self.pixels.get_mut(index) {
            *slot = color;
        }
    }

    /// Fill `rect`, clipped to the surface, and return the region touched.
    ///
    /// Rows are filled with a slice write rather than a loop of checked
    /// per-pixel stores: the clip is computed once, so the inner loop needs no
    /// bounds test at all.
    pub fn fill_rect(&mut self, rect: Rect, color: u32) -> Rect {
        let clipped = rect.intersect(self.bounds());
        if clipped.is_empty() {
            return Rect::EMPTY;
        }
        let left = clipped.x as usize;
        for row in clipped.y as usize..clipped.bottom() as usize {
            let start = row * self.width + left;
            self.pixels[start..start + clipped.width].fill(color);
        }
        clipped
    }

    /// Copy `source` so its top-left lands at `x`, `y`, and return the region
    /// touched.
    ///
    /// This is the path a compositor uses to place a client's surface on the
    /// screen. Nothing in the kernel owns a second surface yet, so it is
    /// currently exercised only by `tests/display_compositing.rs`.
    #[allow(dead_code)]
    pub fn blit(&mut self, source: &Surface, x: i32, y: i32) -> Rect {
        let target = Rect::new(x, y, source.width, source.height).intersect(self.bounds());
        if target.is_empty() {
            return Rect::EMPTY;
        }
        for row in 0..target.height {
            let destination_row = target.y as usize + row;
            let source_row = (target.y - y) as usize + row;
            let destination = destination_row * self.width + target.x as usize;
            let origin = source_row * source.width + (target.x - x) as usize;
            self.pixels[destination..destination + target.width]
                .copy_from_slice(&source.pixels[origin..origin + target.width]);
        }
        target
    }

    /// Copy the `region` of `source` (in source coordinates) so its top-left
    /// lands at `(x, y)`, clipped to both surfaces. Returns the region
    /// touched here.
    ///
    /// This is what lets a compositor redraw exactly an exposed rectangle: it
    /// copies only the part of a window inside the rectangle, rather than the
    /// whole window, which would paint over anything stacked above it
    /// outside the rectangle.
    pub fn blit_region(&mut self, source: &Surface, region: Rect, x: i32, y: i32) -> Rect {
        let clipped = region.intersect(source.bounds());
        if clipped.is_empty() {
            return Rect::EMPTY;
        }
        // Where the clipped region's top-left lands.
        let origin_x = x.saturating_add(clipped.x - region.x);
        let origin_y = y.saturating_add(clipped.y - region.y);
        let target =
            Rect::new(origin_x, origin_y, clipped.width, clipped.height).intersect(self.bounds());
        if target.is_empty() {
            return Rect::EMPTY;
        }
        for row in 0..target.height {
            let source_row = clipped.y as usize + (target.y - origin_y) as usize + row;
            let source_column = clipped.x as usize + (target.x - origin_x) as usize;
            let destination = (target.y as usize + row) * self.width + target.x as usize;
            let origin = source_row * source.width + source_column;
            self.pixels[destination..destination + target.width]
                .copy_from_slice(&source.pixels[origin..origin + target.width]);
        }
        target
    }

    /// [`blit_region`](Self::blit_region), skipping every source pixel equal
    /// to `key`: those leave the destination as it was. Returns the region
    /// the copy covered, whether or not each pixel in it changed.
    pub fn blit_region_keyed(
        &mut self,
        source: &Surface,
        region: Rect,
        x: i32,
        y: i32,
        key: u32,
    ) -> Rect {
        let clipped = region.intersect(source.bounds());
        if clipped.is_empty() {
            return Rect::EMPTY;
        }
        let origin_x = x.saturating_add(clipped.x - region.x);
        let origin_y = y.saturating_add(clipped.y - region.y);
        let target =
            Rect::new(origin_x, origin_y, clipped.width, clipped.height).intersect(self.bounds());
        if target.is_empty() {
            return Rect::EMPTY;
        }
        for row in 0..target.height {
            let source_row = clipped.y as usize + (target.y - origin_y) as usize + row;
            let source_column = clipped.x as usize + (target.x - origin_x) as usize;
            let destination = (target.y as usize + row) * self.width + target.x as usize;
            let origin = source_row * source.width + source_column;
            for column in 0..target.width {
                let pixel = source.pixels[origin + column];
                if pixel != key {
                    self.pixels[destination + column] = pixel;
                }
            }
        }
        target
    }

    /// Overwrite `rect` with pixels decoded from `bytes`: little-endian
    /// `u32`s, row by row, exactly `rect.width * rect.height` of them. The
    /// unused high byte is cleared so every stored pixel stays `0x00RRGGBB`.
    ///
    /// `rect` must lie wholly inside the surface; anything else, or a buffer
    /// of the wrong length, changes nothing and returns [`Rect::EMPTY`].
    /// Returns the region written.
    pub fn write_pixels(&mut self, rect: Rect, bytes: &[u8]) -> Rect {
        if rect.is_empty()
            || rect.intersect(self.bounds()) != rect
            || bytes.len() != rect.width.saturating_mul(rect.height).saturating_mul(4)
        {
            return Rect::EMPTY;
        }
        for (row, source) in bytes.chunks_exact(rect.width * 4).enumerate() {
            let start = (rect.y as usize + row) * self.width + rect.x as usize;
            for (slot, pixel) in self.pixels[start..start + rect.width]
                .iter_mut()
                .zip(source.as_chunks::<4>().0)
            {
                *slot = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
            }
        }
        rect
    }

    /// Blend `source` over this surface at `(x, y)` with uniform `opacity`:
    /// 0 leaves the surface unchanged, 255 is exactly [`blit`](Self::blit).
    /// Returns the region touched, which is empty when nothing changed.
    ///
    /// Pixels carry no alpha channel (`0x00RRGGBB`), so translucency is a
    /// property of the blit rather than of the pixels. That keeps every
    /// existing opaque surface opaque.
    pub fn blend(&mut self, source: &Surface, x: i32, y: i32, opacity: u8) -> Rect {
        if opacity == u8::MAX {
            return self.blit(source, x, y);
        }
        let target = Rect::new(x, y, source.width, source.height).intersect(self.bounds());
        if target.is_empty() || opacity == 0 {
            return Rect::EMPTY;
        }
        for row in 0..target.height {
            let destination_row = target.y as usize + row;
            let source_row = (target.y - y) as usize + row;
            let destination = destination_row * self.width + target.x as usize;
            let origin = source_row * source.width + (target.x - x) as usize;
            for column in 0..target.width {
                let under = self.pixels[destination + column];
                self.pixels[destination + column] =
                    blend_pixel(source.pixels[origin + column], under, opacity);
            }
        }
        target
    }

    /// Draw a one-pixel outline of `rect`, returning the region touched.
    pub fn stroke_rect(&mut self, rect: Rect, color: u32) -> Rect {
        if rect.is_empty() {
            return Rect::EMPTY;
        }
        let bottom = rect.bottom() - 1;
        let right = rect.right() - 1;
        let mut touched = self.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1), color);
        touched = touched.union(self.fill_rect(Rect::new(rect.x, bottom, rect.width, 1), color));
        touched = touched.union(self.fill_rect(Rect::new(rect.x, rect.y, 1, rect.height), color));
        touched.union(self.fill_rect(Rect::new(right, rect.y, 1, rect.height), color))
    }

    /// Draw `text` with its top-left at `x`, `y`, returning the region touched.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: u32) -> Rect {
        let mut touched = Rect::EMPTY;
        let mut cursor_x = x;
        let mut cursor_y = y;
        for character in text.chars() {
            if character == '\n' {
                cursor_x = x;
                cursor_y += crate::font::LINE_HEIGHT;
                continue;
            }
            let glyph = crate::font::glyph(character);
            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..crate::font::WIDTH as i32 {
                    if bits & (1 << (crate::font::WIDTH as i32 - 1 - column)) != 0 {
                        self.set_pixel(cursor_x + column, cursor_y + row as i32, color);
                    }
                }
            }
            touched = touched.union(
                Rect::new(cursor_x, cursor_y, crate::font::WIDTH, crate::font::HEIGHT)
                    .intersect(self.bounds()),
            );
            cursor_x += crate::font::ADVANCE;
        }
        touched
    }
}

/// Whether `a` and `b` together cover exactly their bounding box.
fn exact_union(a: Rect, b: Rect) -> bool {
    let area = |rect: Rect| rect.width as u64 * rect.height as u64;
    area(a.union(b)) == area(a) + area(b) - area(a.intersect(b))
}

/// The regions changed since the last presentation.
///
/// Holds up to [`MAX_DAMAGE_RECTS`] regions and collapses to their bounding box
/// beyond that, so presentation never has unbounded work to do.
#[derive(Clone, Copy)]
pub struct Damage {
    rects: [Rect; MAX_DAMAGE_RECTS],
    len: usize,
    collapsed: bool,
}

impl Default for Damage {
    fn default() -> Self {
        Self::new()
    }
}

impl Damage {
    pub const fn new() -> Self {
        Self {
            rects: [Rect::EMPTY; MAX_DAMAGE_RECTS],
            len: 0,
            collapsed: false,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the list has merged into a single bounding region.
    /// Reported by tests to confirm the list stays bounded.
    #[allow(dead_code)]
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Reported by tests to confirm coalescing.
    #[allow(dead_code)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// For a retained damage list once frames are tracked across presentations.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.len = 0;
        self.collapsed = false;
    }

    /// Record a changed region.
    ///
    /// An empty region is ignored, a region already covered by a tracked one is
    /// dropped, and tracked regions the new one covers are dropped. A region
    /// that, together with a tracked one, forms exactly a rectangle merges
    /// with it, which costs no extra pixels: a frame drawn in adjacent tiles
    /// stays a few regions. Once the list is full everything merges into one
    /// bounding box. Merging loses precision but keeps presentation bounded,
    /// which matters more than redrawing a little extra.
    pub fn add(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        if self.collapsed {
            self.rects[0] = self.rects[0].union(rect);
            return;
        }
        let mut rect = rect;
        'merge: loop {
            let mut index = 0;
            while index < self.len {
                let existing = self.rects[index];
                if existing.intersect(rect) == rect {
                    return;
                }
                let covered = rect.intersect(existing) == existing;
                if covered || exact_union(existing, rect) {
                    self.len -= 1;
                    self.rects[index] = self.rects[self.len];
                    if !covered {
                        // The merged region may now cover or extend others.
                        rect = existing.union(rect);
                        continue 'merge;
                    }
                    continue;
                }
                index += 1;
            }
            break;
        }
        if self.len == MAX_DAMAGE_RECTS {
            let mut bounds = rect;
            for existing in &self.rects[..self.len] {
                bounds = bounds.union(*existing);
            }
            self.rects[0] = bounds;
            self.len = 1;
            self.collapsed = true;
            return;
        }
        self.rects[self.len] = rect;
        self.len += 1;
    }

    pub fn regions(&self) -> &[Rect] {
        &self.rects[..self.len]
    }

    /// Total area tracked, counting overlaps once per region.
    /// For measuring how much a frame actually redraws.
    #[allow(dead_code)]
    pub fn area(&self) -> usize {
        self.regions()
            .iter()
            .map(|rect| rect.width * rect.height)
            .sum()
    }
}

/// Blend `over` onto `under` with `opacity`, per channel, rounding to
/// nearest: `(over * a + under * (255 - a) + 127) / 255`. Exact at both ends:
/// opacity 0 yields `under`, 255 yields `over`.
pub const fn blend_pixel(over: u32, under: u32, opacity: u8) -> u32 {
    blend_channel(over, under, opacity, 16)
        | blend_channel(over, under, opacity, 8)
        | blend_channel(over, under, opacity, 0)
}

const fn blend_channel(over: u32, under: u32, opacity: u8, shift: u32) -> u32 {
    let a = opacity as u32;
    let top = (over >> shift) & 0xff;
    let bottom = (under >> shift) & 0xff;
    ((top * a + bottom * (255 - a) + 127) / 255) << shift
}

/// Copy the damaged regions of `surface` into a framebuffer.
///
/// The pixel format is resolved once per run of pixels instead of once per
/// pixel, and only damaged rows are touched. Returns framebuffer pixels
/// written.
pub fn present(
    surface: &Surface,
    damage: &Damage,
    framebuffer: &mut [u8],
    info: FrameBufferInfo,
) -> usize {
    present_scaled(surface, damage, framebuffer, info, 1)
}

/// [`present`] with an integer display scale: each surface pixel becomes a
/// `scale`×`scale` block of framebuffer pixels, so a surface drawn at
/// 640×400 fills a 1280×800 screen at scale 2. Damage is in surface
/// coordinates and is scaled with it. Returns framebuffer pixels written.
/// A scale of 0 writes nothing.
/// Bytes of one physical row staged at a time by [`present_scaled`].
const PRESENT_STAGING: usize = 2048;

pub fn present_scaled(
    surface: &Surface,
    damage: &Damage,
    framebuffer: &mut [u8],
    info: FrameBufferInfo,
    scale: usize,
) -> usize {
    let bytes = info.bytes_per_pixel;
    // Framebuffers are 1 to 4 bytes per pixel; anything else is not drawn.
    if damage.is_empty() || scale == 0 || bytes == 0 || bytes > 4 {
        return 0;
    }
    let screen = Rect::new(0, 0, info.width, info.height);
    let chunk = PRESENT_STAGING / bytes;
    let mut written = 0usize;

    for region in damage.regions() {
        let logical = region.intersect(surface.bounds());
        if logical.is_empty() {
            continue;
        }
        // Surface coordinates are non-negative after clipping, and the
        // surface is at most a few thousand pixels wide, so the products fit.
        let physical = Rect::new(
            logical.x * scale as i32,
            logical.y * scale as i32,
            logical.width * scale,
            logical.height * scale,
        )
        .intersect(screen);
        if physical.is_empty() {
            continue;
        }
        // Each logical row is converted once, a chunk at a time, into the
        // framebuffer's format, then copied into each physical row it covers:
        // no per-pixel division or format match, and no framebuffer reads
        // (write-combined memory is slow to read back).
        let order = match info.pixel_format {
            PixelFormat::Rgb => Some([16u32, 8, 0]),
            PixelFormat::Bgr => Some([0u32, 8, 16]),
            _ => None,
        };
        let mut staged = [0u8; PRESENT_STAGING];
        let (left, right) = (physical.x as usize, physical.right() as usize);
        let mut y = physical.y as usize;
        while y < physical.bottom() as usize {
            let row = (y / scale) * surface.width();
            let rows = (scale - y % scale).min(physical.bottom() as usize - y);
            let mut x = left;
            while x < right {
                let count = (right - x).min(chunk);
                if let Some(order) = order {
                    let (mut source, mut phase) = (row + x / scale, x % scale);
                    let width = bytes.min(4);
                    for slot in staged[..count * bytes].chunks_exact_mut(bytes) {
                        let color = surface.pixels[source];
                        let pixel = [(color >> order[0]) as u8, (color >> order[1]) as u8, (color >> order[2]) as u8, 0];
                        slot[..width].copy_from_slice(&pixel[..width]);
                        phase += 1;
                        if phase == scale {
                            phase = 0;
                            source += 1;
                        }
                    }
                }
                for physical_y in y..y + rows {
                    let offset = (physical_y * info.stride + x) * bytes;
                    // Whole pixels that fit in the mapping.
                    let fit = count.min(framebuffer.len().saturating_sub(offset) / bytes);
                    if fit > 0 && order.is_some() {
                        framebuffer[offset..offset + fit * bytes].copy_from_slice(&staged[..fit * bytes]);
                    }
                    written += fit;
                }
                x += count;
            }
            y += rows;
        }
    }
    written
}
