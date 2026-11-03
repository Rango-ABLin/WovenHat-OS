//! Offscreen `.bss` render-target buffers, extracted from `main.rs`
//! verbatim (M1, behavior preserving). Each one is flat-rendered once per
//! repaint and then sampled by the 3D desktop mode's perspective
//! projection, or (for `TILE`/`TILE_DEPTH_3D`) used directly as the one
//! shared tile-sized render target. None is safe to share across threads
//! in general; the per-buffer `SAFETY` comments record why `Sync` holds
//! here specifically (the shell is single-threaded, and each buffer's own
//! render function is the only caller that takes a reference to it).

use core::cell::UnsafeCell;

use crate::tokens::{MAX_APPS, PANEL_ROW, PANEL_WIDTH, TASKBAR_HEIGHT, TOAST_HEIGHT, TOAST_WIDTH};
use crate::{CENTER_WIDTH, MAX_NOTES, QS_ROWS, TILE_HEIGHT, TILE_WIDTH};

pub const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;

/// The one tile buffer, in .bss rather than on the stack.
pub struct TileBuffer(pub UnsafeCell<[u32; TILE_PIXELS]>);

// SAFETY: the shell is single-threaded, and `main` takes the only reference.
unsafe impl Sync for TileBuffer {}

pub static TILE: TileBuffer = TileBuffer(UnsafeCell::new([0; TILE_PIXELS]));

/// The Quick Settings panel's maximum pixel footprint, flat-rendered once
/// per repaint into this offscreen texture, then sampled by the 3D desktop
/// mode's perspective projection. `.bss`, not the stack, for the same reason
/// as `TILE`.
pub const QS_PANEL_MAX_HEIGHT: usize = (1 + QS_ROWS) * PANEL_ROW as usize + 2;
pub const QS_PANEL_PIXELS: usize = CENTER_WIDTH as usize * QS_PANEL_MAX_HEIGHT;

pub struct QsPanelTexture(pub UnsafeCell<[u32; QS_PANEL_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_qs_3d` takes the only reference.
unsafe impl Sync for QsPanelTexture {}

pub static QS_PANEL_TEXTURE: QsPanelTexture = QsPanelTexture(UnsafeCell::new([0; QS_PANEL_PIXELS]));

/// The launcher panel's maximum pixel footprint (every catalog entry
/// matching, `MAX_APPS` rows), flat-rendered once per repaint into this
/// offscreen texture, then sampled by the 3D desktop mode's perspective
/// projection.
pub const LAUNCHER_PANEL_MAX_HEIGHT: usize = (2 + MAX_APPS) * PANEL_ROW as usize + 2;
pub const LAUNCHER_PANEL_PIXELS: usize = PANEL_WIDTH as usize * LAUNCHER_PANEL_MAX_HEIGHT;

pub struct LauncherPanelTexture(pub UnsafeCell<[u32; LAUNCHER_PANEL_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_panel_3d` takes the
// only reference.
unsafe impl Sync for LauncherPanelTexture {}

pub static LAUNCHER_PANEL_TEXTURE: LauncherPanelTexture =
    LauncherPanelTexture(UnsafeCell::new([0; LAUNCHER_PANEL_PIXELS]));

/// The Notification Center's maximum pixel footprint (`MAX_NOTES` rows),
/// flat-rendered once per repaint into this offscreen texture, then
/// sampled by the 3D desktop mode's perspective projection.
pub const CENTER_MAX_HEIGHT: usize = (1 + MAX_NOTES) * PANEL_ROW as usize + 2;
pub const CENTER_PIXELS: usize = CENTER_WIDTH as usize * CENTER_MAX_HEIGHT;

pub struct CenterTexture(pub UnsafeCell<[u32; CENTER_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_center_3d` takes the
// only reference.
unsafe impl Sync for CenterTexture {}

pub static CENTER_TEXTURE: CenterTexture = CenterTexture(UnsafeCell::new([0; CENTER_PIXELS]));

/// The toast's fixed pixel footprint, flat-rendered once per repaint into
/// this offscreen texture, then sampled by the 3D desktop mode's
/// perspective projection.
pub const TOAST_PIXELS: usize = TOAST_WIDTH as usize * TOAST_HEIGHT as usize;

pub struct ToastTexture(pub UnsafeCell<[u32; TOAST_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_toast_3d` takes the
// only reference.
unsafe impl Sync for ToastTexture {}

pub static TOAST_TEXTURE: ToastTexture = ToastTexture(UnsafeCell::new([0; TOAST_PIXELS]));

/// The taskbar's maximum pixel footprint, flat-rendered once per repaint
/// into this offscreen texture, then sampled by the 3D desktop mode's
/// perspective projection. `MAX_TASKBAR_WIDTH` mirrors the kernel
/// compositor's own `MAX_WIDTH` bound (two 1280-pixel monitors side by
/// side), since the taskbar spans the primary monitor's full width.
pub const MAX_TASKBAR_WIDTH: usize = 2560;
pub const TASKBAR_PIXELS: usize = MAX_TASKBAR_WIDTH * TASKBAR_HEIGHT as usize;

pub struct TaskbarTexture(pub UnsafeCell<[u32; TASKBAR_PIXELS]>);

// SAFETY: the shell is single-threaded, and `render_taskbar_3d` takes the
// only reference.
unsafe impl Sync for TaskbarTexture {}

pub static TASKBAR_TEXTURE: TaskbarTexture = TaskbarTexture(UnsafeCell::new([0; TASKBAR_PIXELS]));

/// A depth buffer sized for one `TILE`-sized render pass, shared by every 3D
/// surface (the Quick Settings panel, title bars, the wallpaper, and so on):
/// each one clears it before use and none holds it across a call, so they
/// never need their own copy.
pub struct TileDepthBuffer(pub UnsafeCell<[u16; TILE_PIXELS]>);

// SAFETY: the shell is single-threaded, and each 3D render function takes
// the only reference, for the duration of that call only.
unsafe impl Sync for TileDepthBuffer {}

pub static TILE_DEPTH_3D: TileDepthBuffer = TileDepthBuffer(UnsafeCell::new([0; TILE_PIXELS]));
