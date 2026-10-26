//! Mount/partition inventory boundary over GPT and legacy partition parsers.
use crate::irq_lock::IrqMutex as Mutex;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Mount {
    pub id: u64,
    pub start_lba: u64,
    pub sectors: u64,
    pub removable: bool,
    pub encryption: Option<EncryptedMount>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EncryptedMount {
    pub owner: u64,
    pub volume_id: u64,
    pub key_generation: u32,
    pub key: crate::volume_crypto::KeyHandle,
}
static MOUNTS: Mutex<[Option<Mount>; 8]> = Mutex::with_rank([None; 8], 10);
pub fn mount(start_lba: u64, sectors: u64, removable: bool) -> Option<u64> {
    if start_lba == 0 || sectors == 0 {
        return None;
    }
    let mut m = MOUNTS.lock();
    // A volume must not be writable through two mount records at once.
    // Reject all overlapping ranges, including exact duplicates.
    let end = start_lba.checked_add(sectors)?;
    if m.iter().flatten().any(|existing| {
        let Some(existing_end) = existing.start_lba.checked_add(existing.sectors) else {
            return true;
        };
        start_lba < existing_end && existing.start_lba < end
    }) {
        return None;
    }
    let i = m.iter_mut().position(|x| x.is_none())?;
    let id = i as u64 + 1;
    m[i] = Some(Mount {
        id,
        start_lba,
        sectors,
        removable,
        encryption: None,
    });
    Some(id)
}
/// Attach an already authenticated/provisioned key generation to a mounted
/// volume. Mount metadata retains only the opaque vault handle; raw key bytes
/// never enter the storage inventory.
pub fn attach_encryption(
    id: u64,
    owner: u64,
    identity: crate::volume_crypto::KeyGeneration,
    key: crate::volume_crypto::KeyHandle,
) -> bool {
    if identity.generation == 0 {
        return false;
    }
    let mut mounts = MOUNTS.lock();
    let Some(mount) = mounts.iter_mut().flatten().find(|mount| mount.id == id) else {
        return false;
    };
    if mount.encryption.is_some() {
        return false;
    }
    mount.encryption = Some(EncryptedMount {
        owner,
        volume_id: identity.volume_id,
        key_generation: identity.generation,
        key,
    });
    true
}

/// Encrypt one mounted-volume record in place. The mount identity and key
/// generation are authenticated as associated data, preventing ciphertext
/// relocation between volumes or across key rotations.
pub fn seal_record(
    id: u64,
    record_nonce: u64,
    data: &mut [u8],
) -> Option<crate::volume_crypto::Tag> {
    let encryption = {
        let mounts = MOUNTS.lock();
        mounts
            .iter()
            .flatten()
            .find(|mount| mount.id == id)?
            .encryption?
    };
    let mut aad = [0u8; 20];
    aad[..8].copy_from_slice(&encryption.volume_id.to_le_bytes());
    aad[8..12].copy_from_slice(&encryption.key_generation.to_le_bytes());
    aad[12..].copy_from_slice(&id.to_le_bytes());
    crate::volume_crypto::seal_with_owner(
        encryption.owner,
        encryption.key,
        record_nonce,
        &aad,
        data,
    )
}

/// Authenticate and decrypt one mounted-volume record. Failed authentication
/// leaves the caller's ciphertext untouched because volume_crypto verifies the
/// detached tag before applying the stream cipher.
pub fn open_record(
    id: u64,
    record_nonce: u64,
    data: &mut [u8],
    tag: &crate::volume_crypto::Tag,
) -> bool {
    let Some(encryption) = ({
        let mounts = MOUNTS.lock();
        mounts
            .iter()
            .flatten()
            .find(|mount| mount.id == id)
            .and_then(|mount| mount.encryption)
    }) else {
        return false;
    };
    let mut aad = [0u8; 20];
    aad[..8].copy_from_slice(&encryption.volume_id.to_le_bytes());
    aad[8..12].copy_from_slice(&encryption.key_generation.to_le_bytes());
    aad[12..].copy_from_slice(&id.to_le_bytes());
    crate::volume_crypto::open_with_owner(
        encryption.owner,
        encryption.key,
        record_nonce,
        &aad,
        data,
        tag,
    )
}

pub fn unmount(id: u64) -> bool {
    let mut m = MOUNTS.lock();
    if let Some(x) = m.iter_mut().find(|x| x.is_some_and(|v| v.id == id)) {
        *x = None;
        true
    } else {
        false
    }
}
#[cfg(feature = "stage12-3-test")]
pub fn encrypted_mount_self_test() -> bool {
    let owner = 3000;
    let key = [0x73u8; crate::volume_crypto::KEY_SIZE];
    let Some(handle) = crate::volume_crypto::provision_for(owner, key) else {
        return false;
    };
    let Some(id) = mount(4096, 8192, false) else {
        let _ = crate::volume_crypto::revoke_for(owner, handle);
        return false;
    };
    let identity = crate::volume_crypto::KeyGeneration {
        volume_id: 0x1234,
        generation: 3,
    };
    if !attach_encryption(id, owner, identity, handle) {
        let _ = unmount(id);
        let _ = crate::volume_crypto::revoke_for(owner, handle);
        return false;
    }

    let original = *b"encrypted mounted volume record";
    let mut ciphertext = original;
    let Some(tag) = seal_record(id, 55, &mut ciphertext) else {
        let _ = unmount(id);
        let _ = crate::volume_crypto::revoke_for(owner, handle);
        return false;
    };
    if ciphertext == original {
        return false;
    }

    // Authentication failure must not mutate ciphertext.
    let mut tampered_tag = tag;
    tampered_tag[0] ^= 1;
    let before = ciphertext;
    if open_record(id, 55, &mut ciphertext, &tampered_tag) || ciphertext != before {
        return false;
    }
    if !open_record(id, 55, &mut ciphertext, &tag) || ciphertext != original {
        return false;
    }

    // Revocation makes the mounted authority stale immediately.
    if !crate::volume_crypto::revoke_for(owner, handle)
        || seal_record(id, 56, &mut ciphertext).is_some()
    {
        return false;
    }
    unmount(id)
}

/// Verify exclusive mount ranges and overflow refusal without touching disks.
#[cfg(feature = "stage12-5-test")]
pub fn mount_exclusivity_self_test() -> bool {
    let Some(first) = mount(50_000, 100, false) else {
        return false;
    };
    let rejected = mount(50_000, 100, false).is_none()
        && mount(50_050, 100, false).is_none()
        && mount(49_950, 100, false).is_none()
        && mount(u64::MAX - 1, 10, false).is_none();
    let adjacent = mount(50_100, 100, false);
    let cleaned = unmount(first);
    let adjacent_cleaned = adjacent.is_some_and(unmount);
    let remount = mount(50_000, 100, false);
    let remount_cleaned = remount.is_some_and(unmount);
    rejected && cleaned && adjacent_cleaned && remount_cleaned
}

#[cfg(feature = "stage12-5-test")]
pub fn structural_self_test() -> bool {
    let Some(id) = mount(2048, 10000, true) else {
        return false;
    };
    unmount(id)
}
