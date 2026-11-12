//! The tile-chunked renderer the shell draws through, extracted from
//! `main.rs` verbatim (M1, behavior preserving).

use libwoven::{Canvas, GraphicsSurface, Region, TRANSPARENT};

use crate::geometry::Area;
use crate::fail;
use crate::{TILE_HEIGHT, TILE_WIDTH};

/// The staging surface and the tile buffer the shell renders into.
pub struct Painter<'a> {
    pub staging: GraphicsSurface,
    pub tile: &'a mut [u32],
}

impl Painter<'_> {
    /// [`draw`](Self::draw), blended at `opacity` over what is composed.
    pub fn draw_blended(&mut self, area: Area, opacity: u8, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(16);
                };
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self
                        .staging
                        .present_region_blended(region, x, y, opacity)
                        .is_err()
                {
                    fail(17);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Render an overlay over `area` with `render`, on a canvas pre-filled
    /// with the transparent key, and copy it with a keyed blit: only what
    /// `render` drew lands, over what is already composed.
    pub fn overlay(&mut self, area: Area, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(14);
                };
                canvas.fill_rect(x, y, width as usize, height as usize, TRANSPARENT);
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region_keyed(region, x, y).is_err()
                {
                    fail(15);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Blend `color` over `area` with uniform `opacity`, in chunks no larger
    /// than the staging surface.
    pub fn blend(&mut self, area: Area, color: u32, opacity: u8) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(12);
                };
                canvas.fill_rect(x, y, width as usize, height as usize, color);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self
                        .staging
                        .present_region_blended(region, x, y, opacity)
                        .is_err()
                {
                    fail(13);
                }
                x += width;
            }
            y += height;
        }
    }

    /// Blend a rounded rectangle directly through the compositor.
    ///
    /// A keyed scratch overlay cannot represent partial alpha: blending a
    /// rounded fill into the transparent key first produces opaque pixels
    /// that overwrite, rather than blend with, the desktop. Emit one bounded
    /// horizontal span per row instead. The span calculation matches
    /// WovenGraphics' rounded-rectangle circle mask.
    pub fn blend_rounded(&mut self, area: Area, color: u32, opacity: u8, radius: usize) {
        if area.is_empty() {
            return;
        }
        let radius = (radius as i32).min(area.width.min(area.height) / 2);
        let radius_squared = i64::from(radius) * i64::from(radius);
        for row in 0..area.height {
            let corner_y = if row < radius {
                radius
            } else if row > area.height - radius {
                area.height - radius
            } else {
                row
            };
            let dy = i64::from(row - corner_y);
            let mut inset = 0;
            while inset < radius {
                let dx = i64::from(inset - radius);
                if dx * dx + dy * dy <= radius_squared {
                    break;
                }
                inset += 1;
            }
            let width = area.width - 2 * inset;
            if width > 0 {
                self.blend(Area::new(area.x + inset, area.y + row, width, 1), color, opacity);
            }
        }
    }

    /// Render `area` with `render` in chunks no larger than the staging
    /// surface, uploading and blitting each.
    pub fn draw(&mut self, area: Area, render: impl Fn(&mut Canvas<'_>)) {
        let mut y = area.y;
        while y < area.y + area.height {
            let height = (area.y + area.height - y).min(TILE_HEIGHT as i32);
            let mut x = area.x;
            while x < area.x + area.width {
                let width = (area.x + area.width - x).min(TILE_WIDTH as i32);
                let Some(mut canvas) =
                    Canvas::new(self.tile, x, y, width as usize, height as usize)
                else {
                    fail(10);
                };
                render(&mut canvas);
                let region = Region {
                    x: 0,
                    y: 0,
                    width: width as u16,
                    height: height as u16,
                };
                if self.staging.write_canvas(&canvas).is_err()
                    || self.staging.present_region(region, x, y).is_err()
                {
                    fail(11);
                }
                x += width;
            }
            y += height;
        }
    }
}
