//! Shared-window surface protocol: bounded geometry and damage validation.
//!
//! Pure policy layer. The compositor and IPC service must separately enforce
//! WovenGuard capability checks, generation-tagged handles and mapping rights.
//! This module does not allocate or map physical memory.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    InvalidDimensions,
    InvalidStride,
    TooLarge,
    OutOfBounds,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLayout {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bytes: usize,
}

pub const BYTES_PER_PIXEL: u32 = 4;
pub const MAX_WIDTH: u32 = 4096;
pub const MAX_HEIGHT: u32 = 4096;
pub const MAX_SURFACE_BYTES: usize = 64 * 1024 * 1024;

impl SurfaceLayout {
    pub fn new(width: u32, height: u32, stride: u32) -> Result<Self, SurfaceError> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(SurfaceError::InvalidDimensions);
        }
        let row = width.checked_mul(BYTES_PER_PIXEL).ok_or(SurfaceError::Overflow)?;
        if stride < row || stride % BYTES_PER_PIXEL != 0 {
            return Err(SurfaceError::InvalidStride);
        }
        let bytes = (stride as usize)
            .checked_mul(height as usize)
            .ok_or(SurfaceError::Overflow)?;
        if bytes > MAX_SURFACE_BYTES {
            return Err(SurfaceError::TooLarge);
        }
        Ok(Self { width, height, stride, bytes })
    }

    pub fn validate_damage(&self, rect: DamageRect) -> Result<(), SurfaceError> {
        if rect.width == 0 || rect.height == 0 {
            return Err(SurfaceError::OutOfBounds);
        }
        let right = rect.x.checked_add(rect.width).ok_or(SurfaceError::Overflow)?;
        let bottom = rect.y.checked_add(rect.height).ok_or(SurfaceError::Overflow)?;
        if right > self.width || bottom > self.height {
            return Err(SurfaceError::OutOfBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Sequence-tagged publication metadata. This is not a synchronization
/// primitive: the service must enforce ordering and ownership externally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePublication {
    pub surface_generation: u64,
    pub frame_sequence: u64,
    pub damage: DamageRect,
}

impl FramePublication {
    pub fn validate(self, layout: SurfaceLayout, generation: u64, previous: u64) -> Result<(), SurfaceError> {
        if self.surface_generation != generation || self.frame_sequence <= previous {
            return Err(SurfaceError::OutOfBounds);
        }
        layout.validate_damage(self.damage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_layouts() {
        assert_eq!(SurfaceLayout::new(0, 1, 4), Err(SurfaceError::InvalidDimensions));
        assert_eq!(SurfaceLayout::new(10, 10, 39), Err(SurfaceError::InvalidStride));
        assert_eq!(SurfaceLayout::new(4096, 4096, 16388), Err(SurfaceError::TooLarge));
    }

    #[test]
    fn validates_damage_and_generation() {
        let layout = SurfaceLayout::new(100, 50, 400).unwrap();
        let rect = DamageRect { x: 90, y: 40, width: 10, height: 10 };
        assert_eq!(layout.validate_damage(rect), Ok(()));
        assert_eq!(layout.validate_damage(DamageRect { width: 11, ..rect }), Err(SurfaceError::OutOfBounds));
        let publication = FramePublication { surface_generation: 3, frame_sequence: 2, damage: rect };
        assert!(publication.validate(layout, 3, 1).is_ok());
        assert!(publication.validate(layout, 4, 1).is_err());
        assert!(publication.validate(layout, 3, 2).is_err());
    }
}
