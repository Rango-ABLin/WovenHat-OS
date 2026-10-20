//! Kernel-owned A/B update activation state.
//!
//! This module owns the metadata transition rules; storage and the privileged
//! update service must persist the same record atomically on a real system
//! volume.  A candidate is never made active in place: it is staged in the
//! inactive slot, selected for the next boot, and only becomes active after
//! that boot explicitly reports success.  Repeated failed boots roll back to
//! the last known-good slot.

pub const SLOT_COUNT: usize = 2;
pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_BOOT_ATTEMPTS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotState {
    Empty = 0,
    Ready = 1,
    Active = 2,
    Staged = 3,
    Pending = 4,
    Bad = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotMetadata {
    pub generation: u64,
    pub image_length: u64,
    pub digest: [u8; 32],
    pub state: SlotState,
    pub boot_attempts: u8,
}

impl SlotMetadata {
    const EMPTY: Self = Self {
        generation: 0,
        image_length: 0,
        digest: [0; 32],
        state: SlotState::Empty,
        boot_attempts: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidImage,
    GenerationRollback,
    NoInactiveSlot,
    InvalidSlot,
    NotStaged,
    CandidateAlreadyPending,
    WrongBootSlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootDecision {
    Stable(usize),
    Attempt { slot: usize, attempt: u8 },
    RolledBack(usize),
}

#[derive(Clone, Copy)]
pub struct UpdateState {
    slots: [SlotMetadata; SLOT_COUNT],
    active: usize,
    pending: Option<usize>,
}

impl UpdateState {
    pub const fn new(
        generation: u64,
        image_length: u64,
        digest: [u8; 32],
    ) -> Result<Self, Error> {
        if !valid_image(image_length, &digest) || generation == 0 {
            return Err(Error::InvalidImage);
        }
        let active = SlotMetadata {
            generation,
            image_length,
            digest,
            state: SlotState::Active,
            boot_attempts: 0,
        };
        Ok(Self {
            slots: [active, SlotMetadata::EMPTY],
            active: 0,
            pending: None,
        })
    }

    pub const fn active_slot(&self) -> usize {
        self.active
    }

    pub const fn pending_slot(&self) -> Option<usize> {
        self.pending
    }

    pub const fn slot(&self, index: usize) -> Option<SlotMetadata> {
        if index < SLOT_COUNT {
            Some(self.slots[index])
        } else {
            None
        }
    }

    /// Stage a verified image in the inactive slot. Signature verification is
    /// intentionally performed by the privileged update service before this
    /// metadata transition; this method rejects malformed/empty candidates and
    /// generation rollback but never mints trust from metadata alone.
    pub fn stage_candidate(
        &mut self,
        generation: u64,
        image_length: u64,
        digest: [u8; 32],
    ) -> Result<usize, Error> {
        if !valid_image(image_length, &digest) {
            return Err(Error::InvalidImage);
        }
        if generation <= self.slots[self.active].generation {
            return Err(Error::GenerationRollback);
        }
        if self.pending.is_some() {
            return Err(Error::CandidateAlreadyPending);
        }
        let slot = (0..SLOT_COUNT)
            .find(|index| *index != self.active && self.slots[*index].state != SlotState::Pending)
            .ok_or(Error::NoInactiveSlot)?;
        self.slots[slot] = SlotMetadata {
            generation,
            image_length,
            digest,
            state: SlotState::Staged,
            boot_attempts: 0,
        };
        Ok(slot)
    }

    /// Make a staged image the next boot target without changing the active
    /// slot. The persistent writer must commit this record atomically.
    pub fn activate_candidate(&mut self, slot: usize) -> Result<(), Error> {
        if slot >= SLOT_COUNT {
            return Err(Error::InvalidSlot);
        }
        if self.pending.is_some() {
            return Err(Error::CandidateAlreadyPending);
        }
        if self.slots[slot].state != SlotState::Staged {
            return Err(Error::NotStaged);
        }
        self.slots[slot].state = SlotState::Pending;
        self.slots[slot].boot_attempts = 0;
        self.pending = Some(slot);
        Ok(())
    }

    /// Select the boot target. A failed boot must call `boot_failed`; after
    /// three failures the candidate is marked bad and the known-good slot is
    /// restored before the next decision is made.
    pub fn boot_target(&mut self) -> Result<BootDecision, Error> {
        let Some(slot) = self.pending else {
            return Ok(BootDecision::Stable(self.active));
        };
        let entry = &mut self.slots[slot];
        if entry.boot_attempts >= MAX_BOOT_ATTEMPTS {
            entry.state = SlotState::Bad;
            entry.boot_attempts = 0;
            self.pending = None;
            return Ok(BootDecision::RolledBack(self.active));
        }
        entry.boot_attempts += 1;
        Ok(BootDecision::Attempt {
            slot,
            attempt: entry.boot_attempts,
        })
    }

    pub fn boot_failed(&mut self, slot: usize) -> Result<(), Error> {
        if self.pending != Some(slot) || self.slots[slot].state != SlotState::Pending {
            return Err(Error::WrongBootSlot);
        }
        Ok(())
    }

    /// Commit only after the candidate boot has authenticated and reached the
    /// service-owned health check. The old slot remains recoverable as Ready.
    pub fn boot_succeeded(&mut self, slot: usize) -> Result<(), Error> {
        if self.pending != Some(slot) || self.slots[slot].state != SlotState::Pending {
            return Err(Error::WrongBootSlot);
        }
        let old = self.active;
        self.slots[old].state = SlotState::Ready;
        self.slots[slot].state = SlotState::Active;
        self.slots[slot].boot_attempts = 0;
        self.active = slot;
        self.pending = None;
        Ok(())
    }
}

const fn valid_image(image_length: u64, digest: &[u8; 32]) -> bool {
    if image_length == 0 || image_length > MAX_IMAGE_BYTES {
        return false;
    }
    let mut index = 0;
    while index < digest.len() {
        if digest[index] != 0 {
            return true;
        }
        index += 1;
    }
    false
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    let digest_a = [1; 32];
    let digest_b = [2; 32];
    let Ok(mut state) = UpdateState::new(1, 1024, digest_a) else {
        return false;
    };
    if state.stage_candidate(1, 1024, digest_b) != Err(Error::GenerationRollback) {
        return false;
    }
    let Ok(slot) = state.stage_candidate(2, 2048, digest_b) else {
        return false;
    };
    if state.activate_candidate(slot).is_err()
        || state.boot_target() != Ok(BootDecision::Attempt { slot, attempt: 1 })
        || state.boot_failed(slot).is_err()
        || state.boot_target() != Ok(BootDecision::Attempt { slot, attempt: 2 })
        || state.boot_failed(slot).is_err()
        || state.boot_target() != Ok(BootDecision::Attempt { slot, attempt: 3 })
        || state.boot_failed(slot).is_err()
        || state.boot_target() != Ok(BootDecision::RolledBack(0))
        || state.pending_slot().is_some()
    {
        return false;
    }
    let Ok(slot) = state.stage_candidate(3, 4096, [3; 32]) else {
        return false;
    };
    state.activate_candidate(slot).is_ok()
        && state.boot_target() == Ok(BootDecision::Attempt { slot, attempt: 1 })
        && state.boot_succeeded(slot).is_ok()
        && state.active_slot() == slot
        && state.slot(slot).is_some_and(|entry| entry.state == SlotState::Active)
        && state.slot(0).is_some_and(|entry| entry.state == SlotState::Ready)
}

