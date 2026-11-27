//! Screen-space rectangles and damage tracking, extracted from `main.rs`
//! verbatim (M1, behavior preserving) as the next step of the module split.

use libwoven::Region;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Area {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        Self::new(x, y, right - x, bottom - y)
    }

    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self::new(x, y, right - x, bottom - y)
    }

    /// The part of `self` outside `inner`, as up to four disjoint rectangles
    /// (empty ones omitted). Used to clip a window's shadow band to the ring
    /// the window does not immediately overwrite, so a band-filling window
    /// does not issue full-screen blends for pixels it then hides.
    pub fn subtract(self, inner: Self) -> [Self; 4] {
        let clipped = inner.intersect(self);
        if clipped.is_empty() {
            return [self, Self::new(0, 0, 0, 0), Self::new(0, 0, 0, 0), Self::new(0, 0, 0, 0)];
        }
        let right = (self.x + self.width).min(clipped.x + clipped.width);
        let bottom = (self.y + self.height).min(clipped.y + clipped.height);
        let empty = Self::new(0, 0, 0, 0);
        [
            Area::new(self.x, self.y, clipped.x - self.x, self.height),
            Area::new(right, self.y, self.x + self.width - right, self.height),
            Area::new(clipped.x, self.y, right - clipped.x, clipped.y - self.y),
            Area::new(clipped.x, bottom, right - clipped.x, self.y + self.height - bottom),
        ]
        .map(|piece| if piece.is_empty() { empty } else { piece })
    }

    /// This area as a region of a layer whose top-left is `(x, y)`.
    pub fn relative_to(self, x: i32, y: i32) -> Region {
        Region {
            x: (self.x - x) as u16,
            y: (self.y - y) as u16,
            width: self.width as u16,
            height: self.height as u16,
        }
    }
}

/// Most separate areas [`Dirty`] keeps.
const DIRTY_AREAS: usize = 4;

/// The areas to repaint this frame, kept apart while merging them would
/// repaint pixels that did not change: one bounding box over a window at the
/// top left and the tray at the bottom right covers most of the screen.
pub struct Dirty {
    areas: [Area; DIRTY_AREAS],
    len: usize,
}

impl Dirty {
    pub const fn new() -> Self {
        Self {
            areas: [Area::new(0, 0, 0, 0); DIRTY_AREAS],
            len: 0,
        }
    }

    pub fn of(area: Area) -> Self {
        let mut dirty = Self::new();
        dirty.add(area);
        dirty
    }

    pub fn add(&mut self, area: Area) {
        if area.is_empty() {
            return;
        }
        let size = |area: Area| i64::from(area.width) * i64::from(area.height);
        // Absorb every area whose merge costs no more than the two apart.
        let mut area = area;
        let mut index = 0;
        while index < self.len {
            let other = self.areas[index];
            if size(other.union(area)) <= size(other) + size(area) {
                area = area.union(other);
                self.len -= 1;
                self.areas[index] = self.areas[self.len];
                index = 0;
            } else {
                index += 1;
            }
        }
        if self.len < DIRTY_AREAS {
            self.areas[self.len] = area;
            self.len += 1;
            return;
        }
        // Full: merge into the area that grows least.
        let mut best = 0;
        for index in 1..self.len {
            let growth = |i: usize| size(self.areas[i].union(area)) - size(self.areas[i]);
            if growth(index) < growth(best) {
                best = index;
            }
        }
        self.areas[best] = self.areas[best].union(area);
    }

    pub fn areas(&self) -> &[Area] {
        &self.areas[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::Area;

    /// The returned pieces must be disjoint and cover exactly `outer - inner`.
    fn assert_subtraction(outer: Area, inner: Area) {
        let pieces: Vec<Area> = outer
            .subtract(inner)
            .into_iter()
            .filter(|piece| !piece.is_empty())
            .collect();
        let mut overlaps = 0;
        let mut missing = 0;
        let mut extra = 0;
        for y in -4..16 {
            for x in -4..16 {
                let in_outer = outer.contains(x, y);
                let in_inner = inner.contains(x, y);
                let covered = pieces.iter().any(|piece| piece.contains(x, y));
                if covered && in_inner {
                    extra += 1;
                }
                if covered && !in_outer {
                    extra += 1;
                }
                if in_outer && !in_inner && !covered {
                    missing += 1;
                }
                if pieces.iter().filter(|piece| piece.contains(x, y)).count() > 1 {
                    overlaps += 1;
                }
            }
        }
        assert_eq!(missing, 0, "subtract left holes: {outer:?} - {inner:?}");
        assert_eq!(extra, 0, "subtract covered the inner rect: {outer:?}");
        assert_eq!(overlaps, 0, "subtract pieces overlap: {outer:?}");
    }

    #[test]
    fn subtract_removes_exactly_the_inner_rectangle() {
        assert_subtraction(Area::new(0, 0, 10, 10), Area::new(3, 3, 4, 5));
        assert_subtraction(Area::new(-2, 1, 7, 9), Area::new(0, 0, 4, 4));
        assert_subtraction(Area::new(0, 0, 5, 5), Area::new(0, 0, 5, 5));
        assert_subtraction(Area::new(0, 0, 5, 5), Area::new(20, 20, 2, 2));
    }

    #[test]
    fn offset_shadow_band_subtracts_to_a_two_strip_ring() {
        // Shadow bands start `offset` pixels down/right of the frame and keep
        // its size, so clipping to what the frame does not cover must leave a
        // right strip and a bottom strip, never a full-frame blend.
        let frame = Area::new(40, 20, 100, 60);
        let band = Area::new(42, 22, 100, 60);
        let pieces: Vec<Area> = band
            .subtract(frame)
            .into_iter()
            .filter(|piece| !piece.is_empty())
            .collect();
        assert_eq!(pieces.len(), 2);
        let ring: i64 = pieces
            .iter()
            .map(|piece| piece.width as i64 * piece.height as i64)
            .sum();
        // Two strips two pixels wide around a 100x60 frame: 316 pixels, versus
        // the 6000 the old full-frame blend paid per band.
        assert_eq!(ring, 316);
        assert!(ring * 10 < band.width as i64 * band.height as i64);
        assert_subtraction(band, frame);
    }
}
