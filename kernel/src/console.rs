use bootloader_api::info::{FrameBufferInfo, PixelFormat};

use crate::terminal;
#[cfg(any(test, feature = "qemu-test"))]
use crate::{
    display::{self, Damage, Surface},
    gui::Desktop,
};

const BG_R: u8 = 4;
const BG_G: u8 = 12;
const BG_B: u8 = 28;

const FG_R: u8 = 40;
const FG_G: u8 = 220;
const FG_B: u8 = 230;

/// The boot and kernel-shell console.
///
/// It keeps only the framebuffer's geometry. Every drawing operation borrows
/// the buffer through `terminal::with_text_framebuffer`, so the console, the
/// userspace terminal and the compositor never write the framebuffer
/// concurrently, and console text is suppressed while the compositor owns the
/// display. Each operation takes the lock once, not once per pixel.
pub struct Console {
    info: FrameBufferInfo,

    cursor_x: usize,
    cursor_y: usize,

    start_x: usize,
    scale: usize,

    /// Production system-app boots show a branded splash while boot work is
    /// reported to serial, then stay quiet until the desktop owns scanout.
    quiet: bool,

    /// Retained compositing surface for the desktop.
    ///
    /// Allocated on first use rather than at construction, because the console
    /// is created during early boot before the heap can satisfy a
    /// screen-sized buffer. Retaining it across calls is what lets an
    /// unchanged region go uncopied.
    #[cfg(any(test, feature = "qemu-test"))]
    desktop_surface: Option<Surface>,
}

impl Console {
    /// Create the console. `terminal::init` must already have been given the
    /// framebuffer; until then every drawing operation is a no-op.
    pub fn new(info: FrameBufferInfo, x: usize, y: usize, scale: usize) -> Self {
        Self {
            info,
            cursor_x: x,
            cursor_y: y,
            start_x: x,
            scale,
            quiet: cfg!(feature = "system-apps"),
            #[cfg(any(test, feature = "qemu-test"))]
            desktop_surface: None,
        }
    }

    pub fn clear(&mut self) {
        terminal::with_text_framebuffer(|buffer, info| {
            fill(buffer, info, 0, 0, info.width, info.height);
        });

        self.cursor_x = self.start_x;
        self.cursor_y = 40;
    }

    /// Show the short boot-loader handoff screen without exposing the
    /// diagnostic boot transcript on the normal desktop path.
    #[cfg(feature = "system-apps")]
    pub fn show_splash(&mut self) {
        self.show_splash_frame(0);
    }

    /// Paint a branded boot frame. The spinner is deliberately composed from
    /// integer primitives so the loader works before the compositor exists.
    #[cfg(feature = "system-apps")]
    pub fn show_splash_frame(&mut self, frame: usize) {
        let quiet = self.quiet;
        self.quiet = false;
        self.clear();

        let centre_x = self.info.width / 2;
        let centre_y = self.info.height / 2;
        let ring = [
            (0isize, -34isize),
            (17, -29),
            (29, -17),
            (34, 0),
            (29, 17),
            (17, 29),
            (0, 34),
            (-17, 29),
            (-29, 17),
            (-34, 0),
            (-29, -17),
            (-17, -29),
        ];
        for (index, (dx, dy)) in ring.iter().copied().enumerate() {
            let distance = (index + frame) % ring.len();
            let (red, green, blue) = match distance {
                0 => (255, 205, 96),
                1 | 2 => (105, 232, 225),
                3 | 4 => (55, 164, 178),
                _ => (25, 74, 103),
            };
            let x = centre_x.saturating_add_signed(dx).saturating_sub(3);
            let y = centre_y.saturating_add_signed(dy).saturating_sub(3);
            terminal::with_framebuffer(|buffer, info| {
                fill_color(buffer, info, x, y, 7, 7, red, green, blue);
            });
        }

        // A small woven-hat mark above the wordmark: crown, band and brim.
        let mark_y = centre_y.saturating_sub(88);
        terminal::with_framebuffer(|buffer, info| {
            fill_color(
                buffer,
                info,
                centre_x.saturating_sub(24),
                mark_y,
                48,
                4,
                38,
                128,
                149,
            );
            fill_color(
                buffer,
                info,
                centre_x.saturating_sub(17),
                mark_y.saturating_sub(12),
                34,
                13,
                76,
                190,
                188,
            );
            fill_color(
                buffer,
                info,
                centre_x.saturating_sub(10),
                mark_y.saturating_sub(19),
                20,
                8,
                255,
                205,
                96,
            );
            fill_color(
                buffer,
                info,
                centre_x.saturating_sub(30),
                mark_y + 5,
                60,
                5,
                255,
                205,
                96,
            );
        });

        let title = "WOVENHAT";
        let title_width = title.chars().count() * 6 * self.scale;
        let x = self.info.width.saturating_sub(title_width) / 2;
        let y = centre_y.saturating_add(48);
        self.set_cursor_position(x, y);
        self.print(title);
        self.newline();
        self.set_cursor_position(
            (self.info.width / 2).saturating_sub(12 * self.scale),
            y + 18 * self.scale,
        );
        self.print("Starting desktop");
        self.quiet = quiet;
    }

    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    fn ensure_room(&mut self) {
        let line_height = 9 * self.scale;
        if self.cursor_y + 8 * self.scale < self.info.height {
            return;
        }
        self.scroll(line_height);
        self.cursor_y = self.cursor_y.saturating_sub(line_height);
    }

    fn scroll(&mut self, rows: usize) {
        if rows == 0 || rows >= self.info.height || self.info.bytes_per_pixel == 0 {
            self.clear();
            return;
        }
        terminal::with_text_framebuffer(|buffer, info| {
            let row_bytes = info.stride.saturating_mul(info.bytes_per_pixel);
            let move_bytes = (info.height - rows).saturating_mul(row_bytes);
            let src_offset = rows.saturating_mul(row_bytes);
            if src_offset + move_bytes > buffer.len() {
                return;
            }
            buffer.copy_within(src_offset..src_offset + move_bytes, 0);
            fill(buffer, info, 0, info.height - rows, info.width, rows);
        });
    }

    pub fn print(&mut self, text: &str) {
        if self.quiet {
            return;
        }
        for c in text.chars() {
            self.put_char(c);
        }
    }

    pub fn println(&mut self, text: &str) {
        self.print(text);
        self.newline();
    }

    pub fn put_char(&mut self, character: char) {
        if self.quiet {
            return;
        }
        if character == '\n' {
            self.newline();
            return;
        }
        self.ensure_room();

        let (x, y, scale) = (self.cursor_x, self.cursor_y, self.scale);
        terminal::with_text_framebuffer(|buffer, info| {
            draw_char(buffer, info, x, y, scale, character);
        });

        self.cursor_x += 6 * self.scale;

        if self.cursor_x + 6 * self.scale >= self.info.width {
            self.newline();
        }
    }

    pub fn newline(&mut self) {
        self.cursor_x = self.start_x;
        self.cursor_y = self.cursor_y.saturating_add(9 * self.scale);
        self.ensure_room();
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn set_cursor_position(&mut self, x: usize, y: usize) {
        self.cursor_x = core::cmp::min(x, self.info.width.saturating_sub(1));
        self.cursor_y = core::cmp::min(y, self.info.height.saturating_sub(1));
    }

    pub fn backspace(&mut self) {
        if self.quiet {
            return;
        }
        let char_width = 6 * self.scale;

        if self.cursor_x <= self.start_x {
            return;
        }

        self.cursor_x -= char_width;

        let (x, y, height) = (self.cursor_x, self.cursor_y, 8 * self.scale);
        terminal::with_text_framebuffer(|buffer, info| {
            fill(buffer, info, x, y, char_width, height);
        });
    }

    /// Composite the desktop off-screen, then copy only what changed.
    ///
    /// Rendering straight to the framebuffer modified a frame the display was
    /// scanning, so every redraw could tear, and it repainted the whole screen
    /// however little moved. The surface is retained between calls so an
    /// unchanged region is never recopied.
    #[cfg(any(test, feature = "qemu-test"))]
    pub fn render_desktop(&mut self, desktop: &Desktop) {
        let width = self.info.width;
        let height = self.info.height;
        let surface = self
            .desktop_surface
            .get_or_insert_with(|| Surface::new(width, height));
        let mut damage = Damage::new();
        desktop.render(surface, &mut damage);
        let surface = &*surface;
        terminal::with_text_framebuffer(|buffer, info| {
            display::present(surface, &damage, buffer, info);
        });
    }
}

pub(crate) fn draw_char(
    buffer: &mut [u8],
    info: FrameBufferInfo,
    x: usize,
    y: usize,
    scale: usize,
    character: char,
) {
    let glyph = glyph(character);

    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        pixel(
                            buffer,
                            info,
                            x + col * scale + sx,
                            y + row * scale + sy,
                            FG_R,
                            FG_G,
                            FG_B,
                        );
                    }
                }
            }
        }
    }
}

/// Paint a rectangle in the console background colour.
pub(crate) fn fill(
    buffer: &mut [u8],
    info: FrameBufferInfo,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    for row in y..y.saturating_add(height) {
        for column in x..x.saturating_add(width) {
            pixel(buffer, info, column, row, BG_R, BG_G, BG_B);
        }
    }
}

#[cfg(feature = "system-apps")]
#[allow(clippy::too_many_arguments)]
fn fill_color(
    buffer: &mut [u8],
    info: FrameBufferInfo,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    red: u8,
    green: u8,
    blue: u8,
) {
    for row in y..y.saturating_add(height) {
        for column in x..x.saturating_add(width) {
            pixel(buffer, info, column, row, red, green, blue);
        }
    }
}

fn pixel(buffer: &mut [u8], info: FrameBufferInfo, x: usize, y: usize, r: u8, g: u8, b: u8) {
    if x >= info.width || y >= info.height {
        return;
    }

    let offset = y
        .checked_mul(info.stride)
        .and_then(|v| v.checked_add(x))
        .and_then(|v| v.checked_mul(info.bytes_per_pixel))
        .unwrap_or(usize::MAX);

    if offset.saturating_add(info.bytes_per_pixel) > buffer.len() {
        return;
    }

    match info.pixel_format {
        PixelFormat::Rgb => {
            buffer[offset] = r;

            if info.bytes_per_pixel > 1 {
                buffer[offset + 1] = g;
            }

            if info.bytes_per_pixel > 2 {
                buffer[offset + 2] = b;
            }
        }

        PixelFormat::Bgr => {
            buffer[offset] = b;

            if info.bytes_per_pixel > 1 {
                buffer[offset + 1] = g;
            }

            if info.bytes_per_pixel > 2 {
                buffer[offset + 2] = r;
            }
        }

        _ => {}
    }
}

/// Console glyphs come from the shared font.
///
/// The console previously carried its own 49-entry table and folded every
/// character to uppercase, so all console output was capitalised regardless of
/// what was written. The shared table covers all 95 printable characters with
/// real lowercase forms.
pub(crate) fn glyph(c: char) -> [u8; crate::font::HEIGHT] {
    crate::font::glyph(c)
}
