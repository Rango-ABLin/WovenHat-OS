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

#[cfg(not(test))]
pub const UPDATE_PACKAGE_MAGIC: [u8; 4] = *b"WUPD";
#[cfg(not(test))]
pub const UPDATE_PACKAGE_VERSION: u16 = 1;
#[cfg(not(test))]
pub const UPDATE_PACKAGE_HEADER_LEN: usize = 4 + 2 + 2 + 8 + 8 + 32 + 64;

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
    #[cfg(not(test))]
    MalformedPackage,
    #[cfg(not(test))]
    UnsupportedPackage,
    #[cfg(not(test))]
    DigestMismatch,
    #[cfg(not(test))]
    InvalidSignature,
}

#[cfg(not(test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedCandidate<'a> {
    pub generation: u64,
    pub image_length: u64,
    pub digest: [u8; 32],
    pub image: &'a [u8],
}

/// Verify a signed update envelope before it reaches `UpdateState`.
///
/// The signature covers a SHA-256 digest of a domain separator, all immutable
/// manifest fields, and the exact image bytes. This prevents an attacker from
/// keeping a valid image signature while changing its generation or declared
/// length. The key is the kernel's provisioned update trust root; the package
/// cannot supply its own authority.
#[cfg(not(test))]
pub fn verify_package(package: &[u8]) -> Result<VerifiedCandidate<'_>, Error> {
    use sha2::{Digest, Sha256};

    if package.len() < UPDATE_PACKAGE_HEADER_LEN || package[..4] != UPDATE_PACKAGE_MAGIC {
        return Err(Error::MalformedPackage);
    }
    let version = u16::from_le_bytes([package[4], package[5]]);
    let header_len = u16::from_le_bytes([package[6], package[7]]) as usize;
    if version != UPDATE_PACKAGE_VERSION || header_len != UPDATE_PACKAGE_HEADER_LEN {
        return Err(Error::UnsupportedPackage);
    }
    let generation = u64::from_le_bytes(package[8..16].try_into().unwrap());
    let image_length = u64::from_le_bytes(package[16..24].try_into().unwrap());
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&package[24..56]);
    let image_length_usize = usize::try_from(image_length).map_err(|_| Error::MalformedPackage)?;
    let image_end = header_len
        .checked_add(image_length_usize)
        .ok_or(Error::MalformedPackage)?;
    if generation == 0
        || image_length == 0
        || image_length > MAX_IMAGE_BYTES
        || image_end != package.len()
    {
        return Err(Error::MalformedPackage);
    }
    let image = &package[header_len..];
    let computed: [u8; 32] = Sha256::digest(image).into();
    if computed != digest {
        return Err(Error::DigestMismatch);
    }
    let mut signed = Sha256::new();
    signed.update(b"WOVENHAT-UPDATE-V1");
    signed.update(&package[..56]);
    signed.update(image);
    let signed_digest: [u8; 32] = signed.finalize().into();
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&package[56..120]);
    crate::signed_app::verify(
        &signed_digest,
        &crate::signed_app::TRUSTED_PUBLIC_KEY,
        &signature,
    )
    .map_err(|_| Error::InvalidSignature)?;
    Ok(VerifiedCandidate {
        generation,
        image_length,
        digest,
        image,
    })
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
    let state_ok = state.activate_candidate(slot).is_ok()
        && state.boot_target() == Ok(BootDecision::Attempt { slot, attempt: 1 })
        && state.boot_succeeded(slot).is_ok()
        && state.active_slot() == slot
        && state.slot(slot).is_some_and(|entry| entry.state == SlotState::Active)
        && state.slot(0).is_some_and(|entry| entry.state == SlotState::Ready);
    if !state_ok {
        return false;
    }

    use ed25519_compact::{KeyPair, Seed};
    use sha2::{Digest, Sha256};
    const SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
        0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
        0x1c, 0xae, 0x7f, 0x60,
    ];
    let image = b"signed-update-image";
    let mut package = [0u8; UPDATE_PACKAGE_HEADER_LEN + 19];
    package[..4].copy_from_slice(&UPDATE_PACKAGE_MAGIC);
    package[4..6].copy_from_slice(&UPDATE_PACKAGE_VERSION.to_le_bytes());
    package[6..8].copy_from_slice(&(UPDATE_PACKAGE_HEADER_LEN as u16).to_le_bytes());
    package[8..16].copy_from_slice(&4_u64.to_le_bytes());
    package[16..24].copy_from_slice(&(image.len() as u64).to_le_bytes());
    let image_digest: [u8; 32] = Sha256::digest(image).into();
    package[24..56].copy_from_slice(&image_digest);
    package[UPDATE_PACKAGE_HEADER_LEN..].copy_from_slice(image);
    let mut signed = Sha256::new();
    signed.update(b"WOVENHAT-UPDATE-V1");
    signed.update(&package[..56]);
    signed.update(image);
    let signature = KeyPair::from_seed(Seed::new(SEED))
        .sk
        .sign(signed.finalize(), None);
    package[56..120].copy_from_slice(&*signature);
    let verified = verify_package(&package).is_ok();
    package[8] ^= 1;
    verified && verify_package(&package) == Err(Error::InvalidSignature)
}
