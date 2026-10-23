//! Versioned, backend-neutral graphics contracts.
//!
//! GFX-01 deliberately defines the seam without migrating the compositor.
//! The existing [`crate::display::Surface`], [`crate::display::Damage`],
//! [`crate::compositor`] lease, and [`crate::virtio_gpu`] implementation
//! remain authoritative. Later stages may implement these contracts through
//! those modules without creating a second surface or presentation stack.

use alloc::vec::Vec;

use crate::display::{blend_pixel, Rect, Surface};

/// ABI revision for the backend-neutral graphics boundary.
pub const ABI_VERSION: u16 = 1;

/// Pixel encodings accepted at a renderer boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Xrgb8888,
    Argb8888,
    Bgrx8888,
    Rgb565,
}

/// Generation-checked surface identity. The registry that owns the identity
/// remains responsible for rejecting stale or foreign values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceId(pub u64);

/// Completion identity for a submitted frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FenceId(pub u64);

/// Physical output identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputId(pub u32);

/// Validated output mode. Refresh is in millihertz to avoid floating point in
/// the kernel ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_millihz: u32,
}

/// A bounded presentation request. Damage rectangles are borrowed for the
/// call and must already be clipped to the target output.
#[derive(Clone, Copy, Debug)]
pub struct PresentRequest<'a> {
    pub output: OutputId,
    pub surface: SurfaceId,
    pub damage: &'a [Rect],
    pub wait_for: Option<FenceId>,
}

/// Errors shared by CPU and accelerated implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsError {
    InvalidSurface,
    UnsupportedFormat,
    OutOfMemory,
    DeviceLost,
    InvalidGeometry,
    PermissionDenied,
    Timeout,
    InvalidFence,
}

pub type GraphicsResult<T> = Result<T, GraphicsError>;

/// Renderer operations. Implementations must validate surface ownership,
/// format, geometry, and damage before touching memory or submitting work.
pub trait Renderer {
    fn clear(&mut self, target: SurfaceId, color: u32) -> GraphicsResult<()>;
    fn fill_rect(&mut self, target: SurfaceId, rect: Rect, color: u32) -> GraphicsResult<()>;
    fn composite(
        &mut self,
        source: SurfaceId,
        target: SurfaceId,
        source_rect: Rect,
        destination_rect: Rect,
        opacity: u8,
    ) -> GraphicsResult<()>;
    fn submit(&mut self) -> GraphicsResult<FenceId>;
    fn wait(&mut self, fence: FenceId, timeout_ns: u64) -> GraphicsResult<()>;
}

/// Display backend operations. A backend owns output discovery and recovery;
/// the compositor owns scene policy and never receives raw MMIO or DMA.
pub trait DisplayDriver {
    fn output_count(&self) -> usize;
    fn current_mode(&self, output: OutputId) -> GraphicsResult<DisplayMode>;
    fn present(&mut self, request: PresentRequest<'_>) -> GraphicsResult<FenceId>;
    fn recover(&mut self) -> GraphicsResult<()>;
}

/// Bounded software renderer used by the GFX-02 CPU path.
///
/// Surface identities carry a slot and generation. A destroyed surface can
/// therefore never be addressed through a stale handle after its slot is
/// reused. The renderer is deliberately independent of the compositor's
/// client-surface registry: it is an implementation of the graphics seam,
/// not a second presentation owner.
pub struct CpuRenderer {
    slots: [SurfaceSlot; MAX_CPU_SURFACES],
    submitted_fence: u64,
}

const MAX_CPU_SURFACES: usize = 8;
const MAX_CPU_SURFACE_EDGE: usize = 2048;

struct SurfaceSlot {
    generation: u32,
    format: PixelFormat,
    surface: Option<Surface>,
}

impl SurfaceSlot {
    const fn empty() -> Self {
        Self {
            generation: 1,
            format: PixelFormat::Xrgb8888,
            surface: None,
        }
    }
}

impl CpuRenderer {
    pub fn new() -> Self {
        Self {
            slots: [const { SurfaceSlot::empty() }; MAX_CPU_SURFACES],
            submitted_fence: 0,
        }
    }

    pub fn create_surface(
        &mut self,
        width: usize,
        height: usize,
        format: PixelFormat,
    ) -> GraphicsResult<SurfaceId> {
        if width == 0
            || height == 0
            || width > MAX_CPU_SURFACE_EDGE
            || height > MAX_CPU_SURFACE_EDGE
        {
            return Err(GraphicsError::InvalidGeometry);
        }
        let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.surface.is_none())
        else {
            return Err(GraphicsError::OutOfMemory);
        };
        let Some(surface) = Surface::try_new(width, height) else {
            return Err(GraphicsError::OutOfMemory);
        };
        slot.format = format;
        slot.surface = Some(surface);
        Ok(Self::id(index, slot.generation))
    }

    pub fn destroy_surface(&mut self, id: SurfaceId) -> GraphicsResult<()> {
        let index = self.resolve_index(id)?;
        let slot = &mut self.slots[index];
        slot.surface = None;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        Ok(())
    }

    pub fn surface_pixel(&self, id: SurfaceId, x: i32, y: i32) -> GraphicsResult<u32> {
        self.surface(id)?
            .pixel(x, y)
            .ok_or(GraphicsError::InvalidGeometry)
    }

    fn id(index: usize, generation: u32) -> SurfaceId {
        SurfaceId((u64::from(generation) << 32) | (index as u64 + 1))
    }

    fn resolve_index(&self, id: SurfaceId) -> GraphicsResult<usize> {
        let raw = id.0;
        let slot = (raw & u32::MAX as u64) as usize;
        let generation = (raw >> 32) as u32;
        if slot == 0 || slot > self.slots.len() || generation == 0 {
            return Err(GraphicsError::InvalidSurface);
        }
        let index = slot - 1;
        let entry = &self.slots[index];
        if entry.generation != generation || entry.surface.is_none() {
            return Err(GraphicsError::InvalidSurface);
        }
        Ok(index)
    }

    fn surface(&self, id: SurfaceId) -> GraphicsResult<&Surface> {
        let index = self.resolve_index(id)?;
        self.slots[index]
            .surface
            .as_ref()
            .ok_or(GraphicsError::InvalidSurface)
    }

    fn surface_mut(&mut self, id: SurfaceId) -> GraphicsResult<&mut Surface> {
        let index = self.resolve_index(id)?;
        self.slots[index]
            .surface
            .as_mut()
            .ok_or(GraphicsError::InvalidSurface)
    }

    fn format(&self, id: SurfaceId) -> GraphicsResult<PixelFormat> {
        let index = self.resolve_index(id)?;
        Ok(self.slots[index].format)
    }

    fn contains_rect(outer: Rect, inner: Rect) -> bool {
        !inner.is_empty()
            && inner.x >= outer.x
            && inner.y >= outer.y
            && inner.right() <= outer.right()
            && inner.bottom() <= outer.bottom()
    }
}

impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for CpuRenderer {
    fn clear(&mut self, target: SurfaceId, color: u32) -> GraphicsResult<()> {
        let bounds = self.surface(target)?.bounds();
        self.surface_mut(target)?.clear();
        self.surface_mut(target)?.fill_rect(bounds, color);
        Ok(())
    }

    fn fill_rect(&mut self, target: SurfaceId, rect: Rect, color: u32) -> GraphicsResult<()> {
        if !Self::contains_rect(self.surface(target)?.bounds(), rect) {
            return Err(GraphicsError::InvalidGeometry);
        }
        self.surface_mut(target)?.fill_rect(rect, color);
        Ok(())
    }

    fn composite(
        &mut self,
        source: SurfaceId,
        target: SurfaceId,
        source_rect: Rect,
        destination_rect: Rect,
        opacity: u8,
    ) -> GraphicsResult<()> {
        if source_rect.is_empty()
            || destination_rect.is_empty()
            || !Self::contains_rect(self.surface(source)?.bounds(), source_rect)
            || !Self::contains_rect(self.surface(target)?.bounds(), destination_rect)
            || self.format(source)? != self.format(target)?
        {
            return Err(GraphicsError::InvalidGeometry);
        }
        let count = destination_rect
            .width
            .checked_mul(destination_rect.height)
            .ok_or(GraphicsError::OutOfMemory)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(count)
            .map_err(|_| GraphicsError::OutOfMemory)?;
        for y in 0..destination_rect.height {
            let source_y = source_rect.y
                + (y.saturating_mul(source_rect.height) / destination_rect.height) as i32;
            for x in 0..destination_rect.width {
                let source_x = source_rect.x
                    + (x.saturating_mul(source_rect.width) / destination_rect.width) as i32;
                samples.push(
                    self.surface(source)?
                        .pixel(source_x, source_y)
                        .ok_or(GraphicsError::InvalidGeometry)?,
                );
            }
        }
        let target_surface = self.surface_mut(target)?;
        for y in 0..destination_rect.height {
            for x in 0..destination_rect.width {
                let index = y * destination_rect.width + x;
                let color = samples[index];
                let output = if opacity == u8::MAX {
                    color
                } else {
                    let under = target_surface
                        .pixel(destination_rect.x + x as i32, destination_rect.y + y as i32)
                        .ok_or(GraphicsError::InvalidGeometry)?;
                    blend_pixel(color, under, opacity)
                };
                target_surface.set_pixel(
                    destination_rect.x + x as i32,
                    destination_rect.y + y as i32,
                    output,
                );
            }
        }
        Ok(())
    }

    fn submit(&mut self) -> GraphicsResult<FenceId> {
        self.submitted_fence = self.submitted_fence.checked_add(1).unwrap_or(1);
        Ok(FenceId(self.submitted_fence))
    }

    fn wait(&mut self, fence: FenceId, _timeout_ns: u64) -> GraphicsResult<()> {
        if fence.0 == 0 || fence.0 > self.submitted_fence {
            return Err(GraphicsError::InvalidFence);
        }
        Ok(())
    }
}

/// GFX-02's bounded kernel acceptance probe.
pub fn cpu_renderer_self_test() -> bool {
    let mut renderer = CpuRenderer::new();
    let source = match renderer.create_surface(4, 4, PixelFormat::Xrgb8888) {
        Ok(id) => id,
        Err(_) => return false,
    };
    let target = match renderer.create_surface(8, 8, PixelFormat::Xrgb8888) {
        Ok(id) => id,
        Err(_) => return false,
    };
    let source_color = 0x0012_3456;
    let target_color = 0x0060_7080;
    renderer.clear(source, source_color).is_ok()
        && renderer.clear(target, target_color).is_ok()
        && renderer
            .composite(
                source,
                target,
                Rect::new(0, 0, 4, 4),
                Rect::new(2, 2, 4, 4),
                u8::MAX,
            )
            .is_ok()
        && renderer.surface_pixel(target, 2, 2) == Ok(source_color)
        && renderer.surface_pixel(target, 1, 1) == Ok(target_color)
        && renderer.fill_rect(target, Rect::new(0, 0, 2, 2), 0x00aa_bbcc).is_ok()
        && renderer.surface_pixel(target, 1, 1) == Ok(0x00aa_bbcc)
        && renderer.submit().is_ok_and(|fence| renderer.wait(fence, 0).is_ok())
        && renderer.destroy_surface(source).is_ok()
        && renderer.surface_pixel(source, 0, 0) == Err(GraphicsError::InvalidSurface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_types_are_stable_and_copyable() {
        assert_eq!(ABI_VERSION, 1);
        let rect = Rect::new(0, 0, 32, 24);
        let request = PresentRequest {
            output: OutputId(0),
            surface: SurfaceId(7),
            damage: core::slice::from_ref(&rect),
            wait_for: Some(FenceId(9)),
        };
        assert_eq!(request.damage[0], rect);
        assert_eq!(
            DisplayMode {
                width: 1920,
                height: 1080,
                refresh_millihz: 60_000
            },
            DisplayMode {
                width: 1920,
                height: 1080,
                refresh_millihz: 60_000
            }
        );
    }

    #[test]
    fn cpu_renderer_rejects_stale_surfaces_and_composites() {
        assert!(cpu_renderer_self_test());
    }
}
