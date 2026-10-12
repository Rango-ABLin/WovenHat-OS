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
//! There are at most [`MAX_SURFACES`] surfaces system-wide, [`MAX_PER_OWNER`]
//! per process, and each surface is at most [`MAX_WIDTH`] by [`MAX_HEIGHT`]
//! pixels. The caps are deliberately small — this is the primitive, not the
//! final shape — so an unbounded number of surfaces or very large allocations
//! cannot be requested from userspace.
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
use crate::irq_lock::IrqMutex as Mutex;

pub const MAX_SURFACES: usize = 16;
pub const MAX_PER_OWNER: usize = 4;
pub const MAX_WIDTH: u32 = 320;
pub const MAX_HEIGHT: u32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The handle names no live surface, or its generation is stale, or its
    /// owner does not match the token's.
    InvalidHandle,
    /// This process already owns [`MAX_PER_OWNER`] surfaces.
    OwnerFull,
    /// The system-wide table is full.
    SystemFull,
    /// Requested dimensions exceed the caps.
    OutOfRange,
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
        Self(slot | (generation << Self::SLOT_BITS) | (owner << (Self::SLOT_BITS + Self::GENERATION_BITS)))
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
}

impl Slot {
    const fn empty() -> Self {
        Self {
            owner: 0,
            generation: 1,
            surface: None,
            shared_with: None,
        }
    }

    fn free(&mut self) {
        self.surface = None;
        self.shared_with = None;
        self.generation = self.generation.wrapping_add(1).max(1);
    }
}

struct Registry {
    slots: Vec<Slot>,
}

impl Registry {
    fn new() -> Self {
        let mut slots = Vec::with_capacity(MAX_SURFACES);
        for _ in 0..MAX_SURFACES {
            slots.push(Slot::empty());
        }
        Self { slots }
    }

    fn count_for(&self, owner: u64) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.surface.is_some() && slot.owner == owner)
            .count()
    }

    fn resolve_mut(&mut self, handle: Handle) -> Result<&mut Surface, Error> {
        let slot_index = handle.slot();
        let slot = self
            .slots
            .get_mut(slot_index)
            .ok_or(Error::InvalidHandle)?;
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
pub fn create(owner: u64, width: u32, height: u32) -> Result<Handle, Error> {
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err(Error::OutOfRange);
    }
    with_registry(|registry| {
        if registry.count_for(owner) >= MAX_PER_OWNER {
            return Err(Error::OwnerFull);
        }
        let index = registry
            .slots
            .iter()
            .position(|slot| slot.surface.is_none())
            .ok_or(Error::SystemFull)?;
        let slot = &mut registry.slots[index];
        slot.owner = owner;
        slot.shared_with = None;
        slot.surface = Some(Surface::new(width as usize, height as usize));
        Ok(Handle::pack(index, slot.generation, owner))
    })
}

/// Fill a rectangle in the surface named by `handle`.
pub fn fill_rect(handle: Handle, rect: Rect, color: u32) -> Result<(), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        surface.fill_rect(rect, color);
        Ok(())
    })
}

/// Stroke a rectangle outline in the surface named by `handle`.
pub fn stroke_rect(handle: Handle, rect: Rect, color: u32) -> Result<(), Error> {
    with_registry(|registry| {
        let surface = registry.resolve_mut(handle)?;
        surface.stroke_rect(rect, color);
        Ok(())
    })
}

/// Draw `text` into the surface at `(x, y)`.
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
        let mut copy = Surface::new(surface.width(), surface.height());
        copy.blit(surface, 0, 0);
        Ok(copy)
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
        let mut copy = Surface::new(surface.width(), surface.height());
        copy.blit(surface, 0, 0);
        Ok(copy)
    })
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
    with_registry(|registry| {
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
        slot.free();
        Ok(())
    })
}

/// Free every slot owned by `owner`. Returns how many were released.
///
/// Called from the task-exit path, mirroring `async_network::release_owner`.
pub fn release_owner(owner: u64) -> usize {
    with_registry(|registry| {
        let mut released = 0usize;
        for slot in registry.slots.iter_mut() {
            if slot.surface.is_some() && slot.owner == owner {
                slot.free();
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
