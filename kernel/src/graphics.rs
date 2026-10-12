//! Colour definitions shared by the display stack and the widget layer.
//!
//! This module previously held a `Graphics` type that drew directly to the
//! framebuffer, one checked and format-branched write per pixel. The display
//! stack in `crate::display` supersedes it: drawing targets a `Surface`,
//! operations report the region they touched, and the pixel format is applied
//! once per run at presentation. Only the colours remain here.

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const CYAN: Self = Self {
        r: 40,
        g: 220,
        b: 230,
    };
    pub const DARK_BLUE: Self = Self { r: 4, g: 12, b: 28 };

    /// Pack into the display stack's `0x00RRGGBB` representation.
    ///
    /// Composition is format-independent; the framebuffer's channel order is
    /// applied once at presentation rather than per drawing call.
    pub const fn packed(self) -> u32 {
        crate::display::rgb(self.r, self.g, self.b)
    }
}
