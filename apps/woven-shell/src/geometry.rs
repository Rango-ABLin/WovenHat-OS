//! Screen-space rectangles and damage tracking, extracted from `main.rs`
//! verbatim (M1, behavior preserving) as the next step of the module split.

use libwoven::Region;

#[derive(Clone, Copy, PartialEq, Eq)]
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
