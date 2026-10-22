//! Owner-bound, generation-tagged client surfaces for userspace graphics.
//!
//! A userspace program allocates a [`Surface`] through this registry, fills,
//! strokes, and draws text into it through syscalls, and reads individual
//! pixels back for its own verification. Composition onto the framebuffer is
//! left to the compositor layer (Phase 5): what this module provides is the
//! kernel-side primitive a client needs to own a drawing surface at all.
//!
//! # Bounded by construction
//!
//! There are at most [`MAX_SURFACES`] surfaces system-wide and
//! [`MAX_PER_OWNER`] per process. Each is at most [`MAX_WIDTH`] by
//! [`MAX_HEIGHT`], the compositor's own bound, and pixels are budgeted: one
//! process may hold at most [`OWNER_PIXEL_BUDGET`] pixels across its
//! surfaces, and all processes together [`SYSTEM_PIXEL_BUDGET`]. Until Stage
//! 20.4 every surface was capped at 320×240, too small for a real window.
//!
//! # Locking
//!
//! The registry lock disables preemption, not interrupts. A surface is up to
//! 9 MB, and copying one for the compositor must not hold interrupts off.
//! Nothing takes the lock from interrupt context. A new surface is allocated
//! with the lock released, between a budget check before and a re-check
//! after, and a destroyed one is freed after the lock is released.
//!
//! # Owner binding and generation tags
//!
//! Each slot records the owning process and a monotonic generation counter.
//! The token a syscall receives packs both: a stale handle whose generation
//! does not match the slot's current generation is refused rather than treated
//! as the slot's current occupant. [`release_owner`] runs on process exit and
//! frees every slot the process held, mirroring `async_network::release_owner`.

use alloc::vec::Vec;

use crate::display::{Rect, Surface};
use crate::irq_lock::PreemptMutex as Mutex;

pub const MAX_SURFACES: usize = 16;
pub const MAX_PER_OWNER: usize = 4;
pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1200;
/// Pixels one process may hold across its surfaces: 4 MB, enough for a
/// window filling a 1280×800 screen.
pub const OWNER_PIXEL_BUDGET: usize = 1 << 20;
/// Pixels all processes may hold together: 16 MB.
pub const SYSTEM_PIXEL_BUDGET: usize = 1 << 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The handle names no live surface, or its generation is stale, or its
    /// owner does not match the token's.
    InvalidHandle,
    /// This process already owns [`MAX_PER_OWNER`] surfaces, or the new
    /// surface would take it past [`OWNER_PIXEL_BUDGET`].
    OwnerFull,
    /// The system-wide table is full, or the new surface would take the
    /// system past [`SYSTEM_PIXEL_BUDGET`].
    SystemFull,
    /// Requested dimensions exceed the caps.
    OutOfRange,
    /// The copy this call makes could not be allocated.
    OutOfMemory,
}

/// A handle packing slot, generation and owner.
///
/// The token's owner field is supplied by userspace and therefore proves
/// nothing on its own: the registry compares it with the slot's owner, and the
/// syscall layer must also bind it to the calling process with
/// [`Handle::is_owned_by`]. Without that second check a process can forge a
/// handle naming another process as owner and drive that process's surface.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(u64);

impl Handle {
    const SLOT_BITS: u32 = 8;
    const GENERATION_BITS: u32 = 24;

    fn pack(slot: usize, generation: u32, owner: u64) -> Self {
        let slot = (slot as u64) & ((1 << Self::SLOT_BITS) - 1);
        let generation = u64::from(generation) & ((1u64 << Self::GENERATION_BITS) - 1);
        let owner = owner & 0xffff_ffff;
        Self(
            slot | (generation << Self::SLOT_BITS)
                | (owner << (Self::SLOT_BITS + Self::GENERATION_BITS)),
        )
    }

    pub const fn to_u64(self) -> u64 {
        self.0
    }

    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    const fn slot(self) -> usize {
        (self.0 & ((1 << Self::SLOT_BITS) - 1)) as usize
    }

    const fn generation(self) -> u32 {
        ((self.0 >> Self::SLOT_BITS) & ((1u64 << Self::GENERATION_BITS) - 1)) as u32
    }

    const fn owner(self) -> u64 {
        (self.0 >> (Self::SLOT_BITS + Self::GENERATION_BITS)) & 0xffff_ffff
    }

    /// Whether this token names `pid` as its owner. The syscall layer calls
    /// this with the caller's own process id before any registry operation.
    pub const fn is_owned_by(self, pid: u64) -> bool {
        self.owner() == pid & 0xffff_ffff
    }
}

struct Slot {
    owner: u64,
    generation: u32,
    surface: Option<Surface>,
    /// The one process the owner has allowed to read this surface, if any.
    /// Cleared whenever the slot is freed, so a grant never outlives the
    /// surface it names. Process ids are never reused, so a grant naming an
    /// exited process is inert.
    shared_with: Option<u64>,
    /// Region the owner has changed since the grantee last composited it,
    /// clipped to the surface. Kept as one bounding rectangle so the state
    /// per slot stays fixed-size.
    damage: Option<Rect>,
}

impl Slot {
    const fn empty() -> Self {
        Self {
            owner: 0,
            generation: 1,
            surface: None,
            shared_with: None,
            damage: None,
        }
    }

    /// Empty the slot and bump its generation. Returns the surface so the
    /// caller can free it after releasing the registry lock.
    fn free(&mut self) -> Option<Surface> {
        self.shared_with = None;
        self.damage = None;
        self.generation = self.generation.wrapping_add(1).max(1);
        self.surface.take()
    }

    fn pixels(&self) -> usize {
        self.surface
            .as_ref()
            .map_or(0, |surface| surface.width() * surface.height())
    }
}

/// Grantees that may sleep waiting for damage at once. A grantee is a
/// compositor; there is normally one.
const MAX_DAMAGE_WAITERS: usize = 4;

struct Registry {
    slots: Vec<Slot>,
    /// (grantee pid, task) pairs sleeping until a surface shared with that
    /// grantee is damaged. Woken after REGISTRY is released.
    damage_waiters: [Option<(u64, crate::task::TaskId)>; MAX_DAMAGE_WAITERS],
}

impl Registry {
    fn new() -> Self {
        let mut slots = Vec::with_capacity(MAX_SURFACES);
        for _ in 0..MAX_SURFACES {
            slots.push(Slot::empty());
        }
        Self {
            slots,
            damage_waiters: [None; MAX_DAMAGE_WAITERS],
        }
    }

    fn count_for(&self, owner: u64) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.surface.is_some() && slot.owner == owner)
            .count()
    }

    /// Why a new `pixels`-pixel surface for `owner` would be refused, if it
    /// would be.
    fn admit(&self, owner: u64, pixels: usize) -> Result<usize, Error> {
        if self.count_for(owner) >= MAX_PER_OWNER {
            return Err(Error::OwnerFull);
        }
        let owned: usize = self
            .slots
            .iter()
            .filter(|slot| slot.owner == owner)
            .map(Slot::pixels)
            .sum();
        if owned + pixels > OWNER_PIXEL_BUDGET {
            return Err(Error::OwnerFull);
        }
        let total: usize = self.slots.iter().map(Slot::pixels).sum();
        if total + pixels > SYSTEM_PIXEL_BUDGET {
            return Err(Error::SystemFull);
        }
        self.slots
            .iter()
            .position(|slot| slot.surface.is_none())
            .ok_or(Error::SystemFull)
    }

    fn resolve_mut(&mut self, handle: Handle) -> Result<&mut Surface, Error> {
        let slot_index = handle.slot();
        let slot = self.slots.get_mut(slot_index).ok_or(Error::InvalidHandle)?;
        if slot.surface.is_none()
            || slot.generation != handle.generation()
            || slot.owner != handle.owner()
        {
            return Err(Error::InvalidHandle);
        }
        Ok(slot.surface.as_mut().expect("checked present"))
    }
}

/// Rank 10, consistent with other kernel registries at the same altitude.
static REGISTRY: Mutex<Option<Registry>> = Mutex::with_rank(None, 10);

fn with_registry<R>(operation: impl FnOnce(&mut Registry) -> R) -> R {
    let mut guard = REGISTRY.lock();
    let registry = guard.get_or_insert_with(Registry::new);
    operation(registry)
}

/// Allocate a surface for `owner`.
///
/// The caps and budgets are checked before allocating, so a request that
/// cannot succeed allocates nothing. The allocation then happens with the
/// lock released, and everything is checked again before the surface is
/// installed, because another create may have taken the room meanwhile.
pub fn create(owner: u64, width: u32, height: u32) -> Result<Handle, Error> {
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err(Error::OutOfRange);
    }
    let pixels = width as usize * height as usize;
    with_registry(|registry| registry.admit(owner, pixels))?;
    let surface = Surface::try_new(width as usize, height as usize).ok_or(Error::SystemFull)?;
    let (result, refused) = with_registry(|registry| match registry.admit(owner, pixels) {
        Ok(index) => {
            let slot = &mut registry.slots[index];
            slot.owner = owner;
            slot.shared_with = None;
            slot.damage = None;
            slot.surface = Some(surface);
            (Ok(Handle::pack(index, slot.generation, owner)), None)
        }
        Err(error) => (Err(error), Some(surface)),
    });
    // A surface that lost the race is freed with the lock released.
    drop(refused);
    result
}

/// Fill a rectangle in the surface named by `handle`.
///
/// Does not mark damage: Stage 19.9's accepted contract is that a client
/// opts in to exposing what it drew by calling [`mark_damage`] (via the
/// `damage` syscall) itself, so it may draw privately (e.g. preparing a
/// frame) without every draw call being implicitly visible to a compositor
/// watching for damage. Every packaged app instead draws into its own
/// offscreen `Canvas` and uploads the whole thing with [`write_pixels`],
/// which does mark damage -- that is the one path real apps need auto-marked
/// (see the 2026-10-19 "register client surface damage" commit), and it does
/// not carry this one's private-drawing contract.
pub fn fill_rect(handle: Handle, rect: Rect, color: u32) -> Result<(), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        surface.fill_rect(rect, color);
        Ok(())
    })
}

/// Stroke a rectangle outline in the surface named by `handle`. Does not mark
/// damage; see [`fill_rect`].
pub fn stroke_rect(handle: Handle, rect: Rect, color: u32) -> Result<(), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        surface.stroke_rect(rect, color);
        Ok(())
    })
}

/// Draw `text` into the surface at `(x, y)`. Does not mark damage; see
/// [`fill_rect`].
///
/// No syscall exposes this yet, because the three-register ABI cannot carry a
/// handle, coordinates, a user pointer, a length, and a colour at the same
/// time. A richer text API belongs in a later increment that splits the call,
/// so for now the function is reachable only from the Stage 18.1 self-test.
#[cfg_attr(not(feature = "stage18-1-test"), allow(dead_code))]
pub fn draw_text(handle: Handle, x: i32, y: i32, text: &str, color: u32) -> Result<(), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        surface.draw_text(x, y, text, color);
        Ok(())
    })
}

/// Produce a stand-alone copy of the surface named by `handle`.
///
/// Used by the compositor to copy a client's pixels under the registry lock
/// and then release the lock before touching its own surface. Holding both
/// locks across a composite would require a lock-order rule between the two
/// and invite inversions; copying out costs one allocation per blit and keeps
/// the lock dependency single.
pub fn snapshot(handle: Handle) -> Result<Surface, Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        let mut copy =
            Surface::try_new(surface.width(), surface.height()).ok_or(Error::OutOfMemory)?;
        copy.blit(surface, 0, 0);
        Ok(copy)
    })
}

/// Copy just `region` of a surface, clipped to it: the caller's own surface,
/// or with `grantee`, a surface its owner shared with that process (the same
/// rules as [`snapshot_shared`]). Returns the clipped region and its pixels.
///
/// The compositor's region blit uses this. Copying only the exposed part of a
/// window, rather than the whole window, keeps a small repaint cheap however
/// large the window is.
pub fn snapshot_region(
    handle: Handle,
    region: Rect,
    grantee: Option<u64>,
) -> Result<(Rect, Surface), Error> {
    with_registry(|registry| {
        if let Some(grantee) = grantee {
            let shared = registry
                .slots
                .get(handle.slot())
                .is_some_and(|slot| slot.shared_with == Some(grantee));
            if !shared {
                return Err(Error::InvalidHandle);
            }
        }
        let surface = registry.resolve_mut(handle)?;
        let clipped = region.intersect(surface.bounds());
        let mut copy = Surface::try_new(clipped.width, clipped.height).ok_or(Error::OutOfMemory)?;
        copy.blit_region(surface, clipped, 0, 0);
        Ok((clipped, copy))
    })
}

/// Allow `grantee` to read the surface named by `handle`, replacing any earlier
/// grant; `None` revokes. The caller must already have bound `handle` to its
/// owner, which is the only process allowed to share it.
pub fn share(handle: Handle, grantee: Option<u64>) -> Result<(), Error> {
    with_registry(|registry| {
        registry.resolve_mut(handle)?;
        registry.slots[handle.slot()].shared_with = grantee;
        Ok(())
    })
}

/// Copy a surface that its owner has shared with `grantee`.
///
/// `handle` is the owner's token, passed to the grantee out of band, so its
/// owner field names the owner rather than the caller. The slot, generation
/// and owner must still match, and the slot's grant must name `grantee`: a
/// token alone confers nothing.
pub fn snapshot_shared(handle: Handle, grantee: u64) -> Result<Surface, Error> {
    with_registry(|registry| {
        let shared = registry
            .slots
            .get(handle.slot())
            .is_some_and(|slot| slot.shared_with == Some(grantee));
        if !shared {
            return Err(Error::InvalidHandle);
        }
        let surface = registry.resolve_mut(handle)?;
        let mut copy =
            Surface::try_new(surface.width(), surface.height()).ok_or(Error::OutOfMemory)?;
        copy.blit(surface, 0, 0);
        Ok(copy)
    })
}

/// Record that the owner changed `rect` of the surface named by `handle`, and
/// wake the grantee's damage waiter if the surface is shared. The caller must
/// have bound `handle` to its owner.
pub fn mark_damage(handle: Handle, rect: Rect) -> Result<(), Error> {
    let wake = with_registry(|registry| {
        let bounds = registry.resolve_mut(handle)?.bounds();
        let clipped = rect.intersect(bounds);
        if clipped.is_empty() {
            return Ok(None);
        }
        let slot = &mut registry.slots[handle.slot()];
        slot.damage = Some(slot.damage.map_or(clipped, |damage| damage.union(clipped)));
        let Some(grantee) = slot.shared_with else {
            return Ok(None);
        };
        Ok(registry
            .damage_waiters
            .iter_mut()
            .find(|entry| entry.is_some_and(|(pid, _)| pid == grantee))
            .and_then(|entry| entry.take())
            .map(|(_, task)| task))
    })?;
    // Woken with REGISTRY released, so the scheduler lock is never taken
    // under it.
    if let Some(task) = wake {
        crate::task::signal_event(task);
    }
    Ok(())
}

/// Take the pending damage of a surface shared with `grantee`, with a copy of
/// just that region. `Ok(None)` when nothing is pending. Same grant rules as
/// [`snapshot_shared`].
pub fn take_damage_shared(handle: Handle, grantee: u64) -> Result<Option<(Rect, Surface)>, Error> {
    with_registry(|registry| {
        let shared = registry
            .slots
            .get(handle.slot())
            .is_some_and(|slot| slot.shared_with == Some(grantee));
        if !shared {
            return Err(Error::InvalidHandle);
        }
        registry.resolve_mut(handle)?;
        let Some(region) = registry.slots[handle.slot()].damage.take() else {
            return Ok(None);
        };
        let source = registry.slots[handle.slot()]
            .surface
            .as_ref()
            .expect("resolved above");
        let mut copy = Surface::try_new(region.width, region.height).ok_or(Error::OutOfMemory)?;
        for row in 0..region.height as i32 {
            for column in 0..region.width as i32 {
                if let Some(color) = source.pixel(region.x + column, region.y + row) {
                    copy.set_pixel(column, row, color);
                }
            }
        }
        Ok(Some((region, copy)))
    })
}

/// Whether any surface shared with `grantee` has pending damage.
pub fn damage_pending_for(grantee: u64) -> bool {
    with_registry(|registry| {
        registry.slots.iter().any(|slot| {
            slot.surface.is_some() && slot.shared_with == Some(grantee) && slot.damage.is_some()
        })
    })
}

/// Register `task` to be woken when a surface shared with `grantee` is
/// damaged. Returns the other task already registered for `grantee`, without
/// replacing it.
pub fn register_damage_waiter(
    grantee: u64,
    task: crate::task::TaskId,
) -> Result<Option<crate::task::TaskId>, Error> {
    with_registry(|registry| {
        if let Some((_, other)) = registry
            .damage_waiters
            .iter()
            .flatten()
            .find(|(pid, _)| *pid == grantee)
        {
            return Ok((*other != task).then_some(*other));
        }
        let free = registry
            .damage_waiters
            .iter_mut()
            .find(|entry| entry.is_none())
            .ok_or(Error::SystemFull)?;
        *free = Some((grantee, task));
        Ok(None)
    })
}

/// Drop `task`'s registration for `grantee`, if it is the one registered.
pub fn clear_damage_waiter(grantee: u64, task: crate::task::TaskId) {
    with_registry(|registry| {
        for entry in registry.damage_waiters.iter_mut() {
            if *entry == Some((grantee, task)) {
                *entry = None;
            }
        }
    });
}

/// Width and height of the surface named by `handle`.
pub fn dimensions(handle: Handle) -> Result<(usize, usize), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        Ok((surface.width(), surface.height()))
    })
}

/// Overwrite `rect` of the surface named by `handle` with `bytes`, which the
/// caller has already copied out of user memory: little-endian `0x00RRGGBB`
/// pixels, row by row. The rectangle must lie inside the surface and the
/// buffer must match it exactly.
pub fn write_pixels(handle: Handle, rect: Rect, bytes: &[u8]) -> Result<(), Error> {
    let touched = with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        let touched = surface.write_pixels(rect, bytes);
        if touched.is_empty() {
            Err(Error::OutOfRange)
        } else {
            Ok(touched)
        }
    })?;
    mark_damage(handle, touched)
}

/// Read one pixel back, so a client can verify what it drew.
pub fn pixel(handle: Handle, x: i32, y: i32) -> Result<u32, Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        Ok(surface.pixel(x, y).unwrap_or(0))
    })
}

/// Release the surface named by `handle`.
///
/// The slot's generation is bumped, so a later syscall carrying the old handle
/// is refused rather than addressing a reused slot.
pub fn destroy(handle: Handle) -> Result<(), Error> {
    let freed = with_registry(|registry| {
        let slot_index = handle.slot();
        let slot = registry
            .slots
            .get_mut(slot_index)
            .ok_or(Error::InvalidHandle)?;
        if slot.surface.is_none()
            || slot.generation != handle.generation()
            || slot.owner != handle.owner()
        {
            return Err(Error::InvalidHandle);
        }
        Ok(slot.free())
    })?;
    // Freed with the registry lock released.
    drop(freed);
    Ok(())
}

/// Free every slot owned by `owner`. Returns how many were released.
///
/// Called from the task-exit path, mirroring `async_network::release_owner`.
pub fn release_owner(owner: u64) -> usize {
    with_registry(|registry| {
        let mut released = 0usize;
        // A grantee that exits leaves no waiter behind. Not woken: this runs
        // under the scheduler lock on the exit path.
        for entry in registry.damage_waiters.iter_mut() {
            if entry.is_some_and(|(pid, _)| pid == owner) {
                *entry = None;
            }
        }
        for slot in registry.slots.iter_mut() {
            if slot.surface.is_some() && slot.owner == owner {
                // Freed here: this runs under the scheduler lock on the exit
                // path, which the registry lock cannot be released ahead of.
                drop(slot.free());
                released += 1;
            }
        }
        released
    })
}

/// Diagnostic: how many surfaces `owner` currently holds.
#[cfg_attr(not(feature = "stage18-1-test"), allow(dead_code))]
pub fn count_for(owner: u64) -> usize {
    with_registry(|registry| registry.count_for(owner))
}

#[cfg(feature = "stage18-1-test")]
pub fn structural_self_test() -> bool {
    use crate::display::rgb;

    // Fresh registry at boot: no process holds anything.
    if count_for(1) != 0 || count_for(2) != 0 {
        return false;
    }

    // Dimension caps hold at both ends and on zero.
    if create(1, 0, 10) != Err(Error::OutOfRange)
        || create(1, 10, 0) != Err(Error::OutOfRange)
        || create(1, MAX_WIDTH + 1, 10) != Err(Error::OutOfRange)
    {
        return false;
    }

    // Allocation, draw, readback.
    let Ok(handle) = create(1, 32, 24) else {
        return false;
    };
    if fill_rect(handle, Rect::new(0, 0, 32, 24), rgb(0, 0, 0)).is_err()
        || fill_rect(handle, Rect::new(4, 4, 10, 5), rgb(255, 128, 0)).is_err()
    {
        return false;
    }
    if pixel(handle, 4, 4) != Ok(rgb(255, 128, 0))
        || pixel(handle, 3, 4) != Ok(rgb(0, 0, 0))
        || pixel(handle, 13, 8) != Ok(rgb(255, 128, 0))
        || pixel(handle, 14, 9) != Ok(rgb(0, 0, 0))
    {
        return false;
    }
    if draw_text(handle, 2, 2, "Hi", rgb(255, 255, 255)).is_err() {
        return false;
    }

    // Per-owner cap: four is allowed, a fifth is refused.
    let mut extras: Vec<Handle> = Vec::new();
    for _ in 1..MAX_PER_OWNER {
        let Ok(extra) = create(1, 16, 16) else {
            return false;
        };
        extras.push(extra);
    }
    if create(1, 16, 16) != Err(Error::OwnerFull) {
        return false;
    }
    if count_for(1) != MAX_PER_OWNER {
        return false;
    }

    // A second process has its own cap and does not see the first's surfaces.
    let Ok(other) = create(2, 16, 16) else {
        return false;
    };
    if count_for(2) != 1 {
        return false;
    }
    // Impostor handle: slot and generation of process 1, owner swapped to 2.
    let forged = Handle::pack(handle.slot(), handle.generation(), 2);
    if fill_rect(forged, Rect::new(0, 0, 1, 1), rgb(1, 1, 1)) != Err(Error::InvalidHandle) {
        return false;
    }

    // Destroy must bump the generation so a later use of the stale handle is
    // refused rather than addressing the next tenant.
    if destroy(handle).is_err() {
        return false;
    }
    if pixel(handle, 0, 0) != Err(Error::InvalidHandle) {
        return false;
    }
    let Ok(reused) = create(1, 32, 24) else {
        return false;
    };
    if reused == handle {
        return false;
    }

    // release_owner frees everything the process held; the other process is
    // untouched.
    let released = release_owner(1);
    if released != MAX_PER_OWNER {
        return false;
    }
    if count_for(1) != 0 {
        return false;
    }
    if pixel(other, 0, 0) != Ok(0) {
        return false;
    }
    release_owner(2);
    true
}

/// Grant rules for the Stage 19.7 gate.
#[cfg(feature = "stage19-7-test")]
pub fn sharing_self_test() -> bool {
    let (owner, grantee, stranger) = (501u64, 502u64, 503u64);
    let Ok(surface) = create(owner, 4, 4) else {
        return false;
    };
    // Nothing is shared until the owner says so.
    if snapshot_shared(surface, grantee).is_ok() {
        return false;
    }
    if share(surface, Some(grantee)).is_err()
        || snapshot_shared(surface, grantee).is_err()
        || snapshot_shared(surface, stranger).is_ok()
        || snapshot_shared(surface, owner).is_ok()
    {
        return false;
    }
    // A token with the right slot but the wrong generation or owner is still
    // refused even for the grantee.
    let stale = Handle::pack(surface.slot(), surface.generation().wrapping_add(1), owner);
    let forged = Handle::pack(surface.slot(), surface.generation(), stranger);
    if snapshot_shared(stale, grantee).is_ok() || snapshot_shared(forged, grantee).is_ok() {
        return false;
    }
    // Revocation, then re-grant, then the grant dies with the surface and does
    // not carry over to the slot's next occupant.
    if share(surface, None).is_err() || snapshot_shared(surface, grantee).is_ok() {
        return false;
    }
    if share(surface, Some(grantee)).is_err() || destroy(surface).is_err() {
        return false;
    }
    let Ok(next) = create(owner, 4, 4) else {
        return false;
    };
    let ok = next.slot() != surface.slot() || snapshot_shared(next, grantee).is_err();
    release_owner(owner);
    ok && count_for(owner) == 0
}

/// Damage rules for the Stage 19.9 gate.
#[cfg(feature = "stage19-9-test")]
pub fn damage_self_test() -> bool {
    use crate::display::rgb;
    let (owner, grantee) = (601u64, 602u64);
    let Ok(surface) = create(owner, 10, 10) else {
        return false;
    };
    let _ = fill_rect(surface, Rect::new(0, 0, 10, 10), rgb(1, 1, 1));
    // fill_rect itself now marks the whole surface damaged (so a client's
    // draw actually reaches the compositor); drain that setup damage before
    // testing this function's own mark_damage calls below, which is the
    // only way to clear pending damage without a real client in the loop.
    let _ = share(surface, Some(grantee));
    let _ = take_damage_shared(surface, grantee);
    let _ = share(surface, None);
    // Damage is clipped to the surface, unioned, and readable only through a
    // grant; taking it clears it.
    let ok_unshared = mark_damage(surface, Rect::new(8, 8, 5, 5)).is_ok()
        && take_damage_shared(surface, grantee).is_err();
    let _ = share(surface, Some(grantee));
    let _ = mark_damage(surface, Rect::new(1, 1, 2, 2));
    let pending = damage_pending_for(grantee) && !damage_pending_for(owner);
    let taken = take_damage_shared(surface, grantee);
    let region_ok = matches!(&taken, Ok(Some((rect, copy)))
        if *rect == Rect::new(1, 1, 9, 9) && copy.width() == 9 && copy.height() == 9);
    let cleared =
        matches!(take_damage_shared(surface, grantee), Ok(None)) && !damage_pending_for(grantee);
    // An empty or wholly outside rectangle records nothing.
    let _ = mark_damage(surface, Rect::new(20, 20, 3, 3));
    let ignored = matches!(take_damage_shared(surface, grantee), Ok(None));
    // Damage dies with the slot.
    let _ = mark_damage(surface, Rect::new(0, 0, 1, 1));
    release_owner(owner);
    let gone = !damage_pending_for(grantee);
    ok_unshared && pending && region_ok && cleared && ignored && gone
}
