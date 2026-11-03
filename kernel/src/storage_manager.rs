//! Mount/partition inventory boundary over GPT and legacy partition parsers.
use crate::irq_lock::IrqMutex as Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FilesystemKind {
    Fat32,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DiskInfo {
    pub sectors: u64,
    pub start_lba: u64,
    pub partition_sectors: u64,
    pub filesystem: FilesystemKind,
    pub removable: bool,
}

/// Discover the first supported filesystem partition without retaining a
/// device borrow. MBR FAT32 is preferred, then the existing GPT FAT finder.
pub fn inspect_device(device: &mut impl crate::block::BlockDevice, removable: bool) -> Option<DiskInfo> {
    let partition = crate::partition::find_fat32(device)
        .ok()
        .flatten()
        .or_else(|| crate::gpt::find_fat_partition(device).ok().flatten())?;
    Some(DiskInfo {
        sectors: device.sector_count(),
        start_lba: partition.start_lba,
        partition_sectors: partition.sectors,
        filesystem: FilesystemKind::Fat32,
        removable,
    })
}

/// Discover and publish a mount record as one bounded manager operation.
pub fn discover_and_mount(
    device: &mut impl crate::block::BlockDevice,
    removable: bool,
) -> Option<(u64, DiskInfo)> {
    let info = inspect_device(device, removable)?;
    let id = mount(info.start_lba, info.partition_sectors, removable)?;
    Some((id, info))
}
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

#[cfg(feature = "stage12-5-test")]
pub fn structural_self_test() -> bool {
    let Some(id) = mount(2048, 10000, true) else {
        return false;
    };
    if !unmount(id) {
        return false;
    }
    let mut disk = crate::block::RamDisk::<8>::new();
    let mut mbr = [0u8; crate::block::SECTOR_SIZE];
    mbr[510] = 0x55;
    mbr[511] = 0xaa;
    mbr[446 + 4] = 0x0c;
    mbr[446 + 8..446 + 12].copy_from_slice(&2u32.to_le_bytes());
    mbr[446 + 12..446 + 16].copy_from_slice(&4u32.to_le_bytes());
    if crate::block::BlockDevice::write_sector(&mut disk, 0, &mbr).is_err() {
        return false;
    }
    let Some((id, info)) = discover_and_mount(&mut disk, true) else {
        return false;
    };
    info.sectors == 8
        && info.start_lba == 2
        && info.partition_sectors == 4
        && info.filesystem == FilesystemKind::Fat32
        && info.removable
        && unmount(id)
}


// Mount Registry Integration - Stage 12.5

/// Mount point with filesystem metadata and access tracking
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MountPoint {
    pub mount_id: u64,
    pub mount_path: u32,  // Path name hash
    pub disk_info: DiskInfo,
    pub active: bool,
    pub access_count: u32,
}

/// Registry tracking all active mount points
static MOUNT_REGISTRY: Mutex<[Option<MountPoint>; 8]> = Mutex::with_rank([None; 8], 11);

/// Discover, mount, and register a filesystem in one atomic operation
pub fn discover_and_register(
    device: &mut impl crate::block::BlockDevice,
    mount_path: &str,
    removable: bool,
) -> Option<MountPoint> {
    // Discover filesystem on device
    let info = inspect_device(device, removable)?;
    
    // Mount in storage manager
    let mount_id = mount(info.start_lba, info.partition_sectors, removable)?;
    
    // Register in mount point registry
    let path_hash = (crate::fat32::metadata_path_hash(mount_path) as u32) ^ (mount_path.len() as u32);
    
    let mut registry = MOUNT_REGISTRY.lock();
    let slot = registry.iter_mut().position(|x| x.is_none())?;
    
    let mount_point = MountPoint {
        mount_id,
        mount_path: path_hash,
        disk_info: info,
        active: true,
        access_count: 0,
    };
    
    registry[slot] = Some(mount_point);
    Some(mount_point)
}

/// Query a mount point by ID
pub fn query_mount(mount_id: u64) -> Option<MountPoint> {
    let registry = MOUNT_REGISTRY.lock();
    registry.iter()
        .flatten()
        .find(|mp| mp.mount_id == mount_id)
        .copied()
}

/// Query all active mount points
pub fn list_mounts() -> [Option<MountPoint>; 8] {
    let registry = MOUNT_REGISTRY.lock();
    *registry
}

/// Record filesystem access
pub fn record_access(mount_id: u64) -> bool {
    let mut registry = MOUNT_REGISTRY.lock();
    if let Some(mp) = registry.iter_mut()
        .flatten()
        .find(|mp| mp.mount_id == mount_id) {
        mp.access_count = mp.access_count.saturating_add(1);
        true
    } else {
        false
    }
}

/// Deregister and unmount a filesystem
pub fn deregister_and_unmount(mount_id: u64) -> bool {
    // Unmount in storage manager
    if !unmount(mount_id) {
        return false;
    }
    
    // Remove from registry
    let mut registry = MOUNT_REGISTRY.lock();
    if let Some(slot) = registry.iter_mut()
        .position(|x| x.is_some_and(|mp| mp.mount_id == mount_id)) {
        registry[slot] = None;
        true
    } else {
        false
    }
}

/// Get mount registry statistics
pub fn registry_stats() -> (usize, u32) {
    let registry = MOUNT_REGISTRY.lock();
    let active_count = registry.iter().flatten().count();
    let total_accesses: u32 = registry.iter()
        .flatten()
        .map(|mp| mp.access_count)
        .sum();
    (active_count, total_accesses)
}

#[cfg(feature = "stage12-5-test")]
pub fn mount_registry_self_test() -> bool {
    // Test basic mount discovery and registration
    let mut disk = crate::block::RamDisk::<8>::new();
    let mut mbr = [0u8; crate::block::SECTOR_SIZE];
    mbr[510] = 0x55;
    mbr[511] = 0xaa;
    mbr[446 + 4] = 0x0c;
    mbr[446 + 8..446 + 12].copy_from_slice(&2u32.to_le_bytes());
    mbr[446 + 12..446 + 16].copy_from_slice(&4u32.to_le_bytes());
    if crate::block::BlockDevice::write_sector(&mut disk, 0, &mbr).is_err() {
        return false;
    }
    
    // Discover and register
    let Some(mp1) = discover_and_register(&mut disk, "/mnt/data", true) else {
        return false;
    };
    
    // Query should return the same mount point
    let Some(queried) = query_mount(mp1.mount_id) else {
        return false;
    };
    if queried.mount_id != mp1.mount_id || queried.disk_info.removable != true {
        return false;
    }
    
    // Record accesses
    if !record_access(mp1.mount_id) {
        return false;
    }
    if !record_access(mp1.mount_id) {
        return false;
    }
    
    // Check stats
    let (active, total_accesses) = registry_stats();
    if active != 1 || total_accesses != 2 {
        return false;
    }
    
    // List mounts should show one active
    let mounts = list_mounts();
    if mounts[0].is_none() || mounts[1].is_some() {
        return false;
    }
    
    // Deregister and unmount
    if !deregister_and_unmount(mp1.mount_id) {
        return false;
    }
    
    // Stats should show zero active mounts
    let (active, _) = registry_stats();
    active == 0 && query_mount(mp1.mount_id).is_none()
}
