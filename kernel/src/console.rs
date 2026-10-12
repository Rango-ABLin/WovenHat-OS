use bootloader_api::info::{FrameBufferInfo, PixelFormat};

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

pub struct Console<'a> {
    buffer: &'a mut [u8],
    info: FrameBufferInfo,

    cursor_x: usize,
    cursor_y: usize,

    start_x: usize,
    scale: usize,

    /// Retained compositing surface for the desktop.
    ///
    /// Allocated on first use rather than at construction, because the console
    /// is created during early boot before the heap can satisfy a
    /// screen-sized buffer. Retaining it across calls is what lets an
    /// unchanged region go uncopied.
    desktop_surface: Option<Surface>,
}

impl<'a> Console<'a> {
    pub fn new(
        buffer: &'a mut [u8],
        info: FrameBufferInfo,
        x: usize,
        y: usize,
        scale: usize,
    ) -> Self {
        Self {
            buffer,
            info,
            cursor_x: x,
            cursor_y: y,
            start_x: x,
            scale,
            desktop_surface: None,
        }
    }

    pub fn clear(&mut self) {
        for y in 0..self.info.height {
            for x in 0..self.info.width {
                self.pixel(x, y, BG_R, BG_G, BG_B);
            }
        }

        self.cursor_x = self.start_x;
        self.cursor_y = 40;
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
        let row_bytes = self.info.stride.saturating_mul(self.info.bytes_per_pixel);
        let move_bytes = (self.info.height - rows).saturating_mul(row_bytes);
        let src_offset = rows.saturating_mul(row_bytes);
        if src_offset + move_bytes > self.buffer.len() {
            return;
        }
        unsafe {
            core::ptr::copy(
                self.buffer[src_offset..].as_ptr(),
                self.buffer[..move_bytes].as_mut_ptr(),
                move_bytes,
            );
        }
        for y in self.info.height - rows..self.info.height {
            for x in 0..self.info.width {
                self.pixel(x, y, BG_R, BG_G, BG_B);
            }
        }
    }

    pub fn print(&mut self, text: &str) {
        for c in text.chars() {
            self.put_char(c);
        }
    }

    pub fn println(&mut self, text: &str) {
        self.print(text);
        self.newline();
    }

    pub fn put_char(&mut self, character: char) {
        if character == '\n' {
            self.newline();
            return;
        }
        self.ensure_room();

        self.draw_char(self.cursor_x, self.cursor_y, character);

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
        let char_width = 6 * self.scale;

        if self.cursor_x <= self.start_x {
            return;
        }

        self.cursor_x -= char_width;

        let width = char_width;
        let height = 8 * self.scale;

        for y in self.cursor_y..self.cursor_y + height {
            for x in self.cursor_x..self.cursor_x + width {
                self.pixel(x, y, BG_R, BG_G, BG_B);
            }
        }
    }

    /// Composite the desktop off-screen, then copy only what changed.
    ///
    /// Rendering straight to the framebuffer modified a frame the display was
    /// scanning, so every redraw could tear, and it repainted the whole screen
    /// however little moved. The surface is retained between calls so an
    /// unchanged region is never recopied.
    pub fn render_desktop(&mut self, desktop: &Desktop) {
        let width = self.info.width;
        let height = self.info.height;
        let surface = self
            .desktop_surface
            .get_or_insert_with(|| Surface::new(width, height));
        let mut damage = Damage::new();
        desktop.render(surface, &mut damage);
        display::present(surface, &damage, self.buffer, self.info);
    }

    fn draw_char(&mut self, x: usize, y: usize, character: char) {
        let glyph = glyph(character);

        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    for sy in 0..self.scale {
                        for sx in 0..self.scale {
                            self.pixel(
                                x + col * self.scale + sx,
                                y + row * self.scale + sy,
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

    fn pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }

        let offset = y
            .checked_mul(self.info.stride)
            .and_then(|v| v.checked_add(x))
            .and_then(|v| v.checked_mul(self.info.bytes_per_pixel))
            .unwrap_or(usize::MAX);

        if offset + self.info.bytes_per_pixel > self.buffer.len() {
            return;
        }

        match self.info.pixel_format {
            PixelFormat::Rgb => {
                self.buffer[offset] = r;

                if self.info.bytes_per_pixel > 1 {
                    self.buffer[offset + 1] = g;
                }

                if self.info.bytes_per_pixel > 2 {
                    self.buffer[offset + 2] = b;
                }
            }

            PixelFormat::Bgr => {
                self.buffer[offset] = b;

                if self.info.bytes_per_pixel > 1 {
                    self.buffer[offset + 1] = g;
                }

                if self.info.bytes_per_pixel > 2 {
                    self.buffer[offset + 2] = r;
                }
            }

            _ => {}
        }
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
