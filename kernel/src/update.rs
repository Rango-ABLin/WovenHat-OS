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
pub const UPDATE_RECORD_MAGIC: [u8; 4] = *b"WUAB";
pub const UPDATE_RECORD_VERSION: u16 = 1;
const UPDATE_RECORD_HEADER_LEN: usize = 18;
const UPDATE_SLOT_RECORD_LEN: usize = 50;
pub const UPDATE_RECORD_LEN: usize =
    UPDATE_RECORD_HEADER_LEN + SLOT_COUNT * UPDATE_SLOT_RECORD_LEN + 4;

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
    MalformedRecord,
    RecordChecksumMismatch,
    InvalidRecord,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// Encode one complete state snapshot for an atomic durable record.
    ///
    /// The caller must commit the whole record through the storage service and
    /// use a monotonically increasing sequence. The CRC protects selection of
    /// a torn/corrupt copy; it is not an authenticity boundary.
    pub fn encode_record(
        &self,
        sequence: u64,
        out: &mut [u8; UPDATE_RECORD_LEN],
    ) -> Result<(), Error> {
        if sequence == 0 || !self.is_valid() {
            return Err(Error::InvalidRecord);
        }
        out.fill(0);
        out[..4].copy_from_slice(&UPDATE_RECORD_MAGIC);
        out[4..6].copy_from_slice(&UPDATE_RECORD_VERSION.to_le_bytes());
        out[6..8].copy_from_slice(&(UPDATE_RECORD_LEN as u16).to_le_bytes());
        out[8..16].copy_from_slice(&sequence.to_le_bytes());
        out[16] = self.active as u8;
        out[17] = self.pending.map_or(0xff, |slot| slot as u8);
        let mut offset = UPDATE_RECORD_HEADER_LEN;
        for slot in self.slots {
            out[offset..offset + 8].copy_from_slice(&slot.generation.to_le_bytes());
            out[offset + 8..offset + 16].copy_from_slice(&slot.image_length.to_le_bytes());
            out[offset + 16..offset + 48].copy_from_slice(&slot.digest);
            out[offset + 48] = slot.state as u8;
            out[offset + 49] = slot.boot_attempts;
            offset += UPDATE_SLOT_RECORD_LEN;
        }
        let checksum = crc32(&out[..UPDATE_RECORD_LEN - 4]);
        out[UPDATE_RECORD_LEN - 4..].copy_from_slice(&checksum.to_le_bytes());
        Ok(())
    }

    /// Decode a complete state snapshot. The returned sequence is selected by
    /// the storage service only after comparing it with the alternate record.
    pub fn decode_record(bytes: &[u8]) -> Result<(u64, Self), Error> {
        if bytes.len() != UPDATE_RECORD_LEN
            || bytes[..4] != UPDATE_RECORD_MAGIC
            || u16::from_le_bytes([bytes[4], bytes[5]]) != UPDATE_RECORD_VERSION
            || usize::from(u16::from_le_bytes([bytes[6], bytes[7]])) != UPDATE_RECORD_LEN
        {
            return Err(Error::MalformedRecord);
        }
        let expected = u32::from_le_bytes(bytes[UPDATE_RECORD_LEN - 4..].try_into().unwrap());
        if crc32(&bytes[..UPDATE_RECORD_LEN - 4]) != expected {
            return Err(Error::RecordChecksumMismatch);
        }
        let sequence = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        if sequence == 0 || bytes[16] as usize >= SLOT_COUNT {
            return Err(Error::InvalidRecord);
        }
        let pending = match bytes[17] {
            0xff => None,
            value if (value as usize) < SLOT_COUNT => Some(value as usize),
            _ => return Err(Error::InvalidRecord),
        };
        let mut slots = [SlotMetadata::EMPTY; SLOT_COUNT];
        let mut offset = UPDATE_RECORD_HEADER_LEN;
        for slot in &mut slots {
            slot.generation = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            slot.image_length =
                u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
            slot.digest.copy_from_slice(&bytes[offset + 16..offset + 48]);
            slot.state = match bytes[offset + 48] {
                0 => SlotState::Empty,
                1 => SlotState::Ready,
                2 => SlotState::Active,
                3 => SlotState::Staged,
                4 => SlotState::Pending,
                5 => SlotState::Bad,
                _ => return Err(Error::InvalidRecord),
            };
            slot.boot_attempts = bytes[offset + 49];
            offset += UPDATE_SLOT_RECORD_LEN;
        }
        let state = Self {
            slots,
            active: bytes[16] as usize,
            pending,
        };
        if !state.is_valid() {
            return Err(Error::InvalidRecord);
        }
        Ok((sequence, state))
    }

    fn is_valid(&self) -> bool {
        if self.active >= SLOT_COUNT
            || self.slots[self.active].state != SlotState::Active
            || self.slots.iter().filter(|slot| slot.state == SlotState::Active).count() != 1
        {
            return false;
        }
        for slot in self.slots {
            if slot.boot_attempts > MAX_BOOT_ATTEMPTS
                || (slot.state != SlotState::Empty && !valid_image(slot.image_length, &slot.digest))
            {
                return false;
            }
        }
        match self.pending {
            Some(slot) => slot != self.active && self.slots[slot].state == SlotState::Pending,
            None => !self.slots.iter().any(|slot| slot.state == SlotState::Pending),
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

const fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    let mut index = 0;
    while index < bytes.len() {
        crc ^= bytes[index] as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        index += 1;
    }
    !crc
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

    let mut record = [0u8; UPDATE_RECORD_LEN];
    let Ok(()) = state.encode_record(7, &mut record) else {
        return false;
    };
    let Ok((sequence, restored)) = UpdateState::decode_record(&record) else {
        return false;
    };
    if sequence != 7 || restored.active_slot() != state.active_slot() {
        return false;
    }
    record[UPDATE_RECORD_HEADER_LEN] ^= 1;
    if UpdateState::decode_record(&record) != Err(Error::RecordChecksumMismatch) {
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
