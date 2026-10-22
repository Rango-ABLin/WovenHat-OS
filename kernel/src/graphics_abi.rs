//! Versioned, backend-neutral graphics contracts.
//!
//! GFX-01 deliberately defines the seam without migrating the compositor.
//! The existing [`crate::display::Surface`], [`crate::display::Damage`],
//! [`crate::compositor`] lease, and [`crate::virtio_gpu`] implementation
//! remain authoritative. Later stages may implement these contracts through
//! those modules without creating a second surface or presentation stack.

use crate::display::Rect;

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
}
