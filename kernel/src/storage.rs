use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};

use crate::block::BlockDevice;
use crate::{ata, block_io, fat32, gpt, partition, swap, vfs};

const READ_ONLY_ATTRIBUTE: u8 = 0x01;
const DIRECTORY_ATTRIBUTE: u8 = 0x10;
const MAX_BOOT_IMPORT_DEPTH: usize = 2;

const MOUNT_UNKNOWN: u8 = 0;
const MOUNT_NO_DEVICE: u8 = 1;
const MOUNT_NOT_FAT32: u8 = 2;
const MOUNT_MOUNTED: u8 = 3;
const MOUNT_FAILED: u8 = 4;
const MOUNT_UNMOUNTED: u8 = 5;

static MNT_STATUS: AtomicU8 = AtomicU8::new(MOUNT_UNKNOWN);
static MNT_DIRTY: AtomicBool = AtomicBool::new(false);
static MNT_IMPORTED: AtomicUsize = AtomicUsize::new(0);
static MNT_SYNCS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountLifecycleStatus {
    Unknown,
    NoDevice,
    NotFat32,
    Mounted,
    Failed,
    Unmounted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountInfo {
    pub status: MountLifecycleStatus,
    pub imported_entries: usize,
    pub dirty: bool,
    pub sync_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountControlError {
    NoDevice,
    NotMounted,
    SyncFailed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceError {
    Unmounted,
    NoDevice,
    NotFat32,
    Failed,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MountStatus {
    NoDevice,
    NotFat32,
    Mounted(usize),
    Failed,
}

fn status_code(status: MountStatus) -> u8 {
    match status {
        MountStatus::NoDevice => MOUNT_NO_DEVICE,
        MountStatus::NotFat32 => MOUNT_NOT_FAT32,
        MountStatus::Mounted(_) => MOUNT_MOUNTED,
        MountStatus::Failed => MOUNT_FAILED,
    }
}

fn lifecycle_from_code(code: u8) -> MountLifecycleStatus {
    match code {
        MOUNT_NO_DEVICE => MountLifecycleStatus::NoDevice,
        MOUNT_NOT_FAT32 => MountLifecycleStatus::NotFat32,
        MOUNT_MOUNTED => MountLifecycleStatus::Mounted,
        MOUNT_FAILED => MountLifecycleStatus::Failed,
        MOUNT_UNMOUNTED => MountLifecycleStatus::Unmounted,
        _ => MountLifecycleStatus::Unknown,
    }
}

fn record_mount_status(status: MountStatus) {
    MNT_STATUS.store(status_code(status), Ordering::Release);
    match status {
        MountStatus::Mounted(count) => {
            MNT_IMPORTED.store(count, Ordering::Release);
            MNT_DIRTY.store(false, Ordering::Release);
        }
        _ => {
            MNT_IMPORTED.store(0, Ordering::Release);
            MNT_DIRTY.store(false, Ordering::Release);
        }
    }
}

pub fn mount_info() -> MountInfo {
    MountInfo {
        status: lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)),
        imported_entries: MNT_IMPORTED.load(Ordering::Acquire),
        dirty: MNT_DIRTY.load(Ordering::Acquire),
        sync_count: MNT_SYNCS.load(Ordering::Acquire),
    }
}

pub fn mnt_mounted() -> bool {
    MNT_STATUS.load(Ordering::Acquire) == MOUNT_MOUNTED
}

pub fn mark_mnt_dirty() {
    if mnt_mounted() {
        MNT_DIRTY.store(true, Ordering::Release);
    }
}

fn mark_mnt_clean() {
    MNT_DIRTY.store(false, Ordering::Release);
}

fn mnt_path(path: &str) -> bool {
    path == "/mnt" || path.starts_with("/mnt/")
}

fn unavailable_ensure_error() -> EnsureError {
    match lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)) {
        MountLifecycleStatus::NoDevice => EnsureError::NoDevice,
        MountLifecycleStatus::NotFat32 => EnsureError::NotFat32,
        MountLifecycleStatus::Failed => EnsureError::Failed,
        _ => EnsureError::Unmounted,
    }
}

fn unavailable_persist_error() -> PersistError {
    match lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)) {
        MountLifecycleStatus::NoDevice => PersistError::NoDevice,
        _ => PersistError::Unmounted,
    }
}

fn unavailable_mutation_error() -> MutationError {
    match lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)) {
        MountLifecycleStatus::NoDevice => MutationError::NoDevice,
        _ => MutationError::Unmounted,
    }
}

fn unavailable_space_error() -> SpaceError {
    match lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)) {
        MountLifecycleStatus::NoDevice => SpaceError::NoDevice,
        MountLifecycleStatus::NotFat32 => SpaceError::NotFat32,
        MountLifecycleStatus::Failed => SpaceError::Failed,
        _ => SpaceError::Unmounted,
    }
}
pub fn mount_ata_root() -> MountStatus {
    // Keep the raw ATA lock strictly scoped to discovery/import. Snapshot
    // recovery below uses block_io::PrimaryAta, whose I/O path acquires the
    // same ATA lock. Running recovery inside with_primary_master would
    // recursively acquire that lock and deadlock during reboot rollback.
    let status = ata::with_primary_master(|disk| {
        let direct = mount_device(disk);
        if direct != MountStatus::NotFat32 {
            if matches!(direct, MountStatus::Mounted(_)) {
                configure_direct_swap(disk);
            }
            return direct;
        }
        match partition::find_fat32(disk) {
            Ok(Some(partition)) => {
                let status = mount_partition(disk, partition);
                if matches!(status, MountStatus::Mounted(_)) {
                    configure_partition_swap(disk, partition);
                }
                status
            }
            Ok(None) => match gpt::find_fat_partition(disk) {
                Ok(Some(partition)) => {
                    let status = mount_partition(disk, partition);
                    if matches!(status, MountStatus::Mounted(_)) {
                        configure_partition_swap(disk, partition);
                    }
                    status
                }
                Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => MountStatus::NotFat32,
                Err(_) => MountStatus::Failed,
            },
            Err(_) => MountStatus::Failed,
        }
    })
    .unwrap_or(MountStatus::NoDevice);

    // Publish the mounted lifecycle only after the raw ATA guard above has
    // been released. Recovery helpers intentionally require mnt_mounted().
    record_mount_status(status);
    if matches!(status, MountStatus::Mounted(_)) {
        #[cfg(feature = "stage12-4-reboot-test")]
        crate::serial::write_line(format_args!("[S12.4R] recovery: catalog"));
        if crate::snapshots::recover_catalog().is_err() {
            #[cfg(feature = "stage12-4-reboot-test")]
            crate::serial::write_line(format_args!("[S12.4R] recovery: catalog FAILED"));
            let failed = MountStatus::Failed;
            record_mount_status(failed);
            return failed;
        }
        #[cfg(feature = "stage12-4-reboot-test")]
        crate::serial::write_line(format_args!("[S12.4R] recovery: WSR1"));
        if crate::snapshots::recover_restore().is_err() {
            #[cfg(feature = "stage12-4-reboot-test")]
            crate::serial::write_line(format_args!("[S12.4R] recovery: WSR1 FAILED"));
            let failed = MountStatus::Failed;
            record_mount_status(failed);
            return failed;
        }
        #[cfg(feature = "stage12-4-reboot-test")]
        crate::serial::write_line(format_args!("[S12.4R] recovery: replay"));
        if let Err(error) = crate::snapshots::resume_pending_restore() {
            #[cfg(feature = "stage12-4-reboot-test")]
            crate::serial::write_line(format_args!(
                "[S12.4R] recovery: replay FAILED code={}",
                error as u8
            ));
            #[cfg(not(feature = "stage12-4-reboot-test"))]
            let _ = error;
            let failed = MountStatus::Failed;
            record_mount_status(failed);
            return failed;
        }
        #[cfg(feature = "stage12-4-reboot-test")]
        crate::serial::write_line(format_args!("[S12.4R] recovery: complete"));
    }
    status
}

fn mount_partition(
    device: &mut impl crate::block::BlockDevice,
    partition: partition::Partition,
) -> MountStatus {
    let Ok(mut view) = partition::PartitionDevice::new(device, partition) else {
        return MountStatus::Failed;
    };
    mount_device(&mut view)
}

fn configure_direct_swap(device: &mut impl crate::block::BlockDevice) {
    let Ok(volume) = fat32::mount(device) else {
        return;
    };
    configure_swap_area(volume.total_sectors as u64, device.sector_count());
}

fn configure_partition_swap(
    device: &mut impl crate::block::BlockDevice,
    partition: partition::Partition,
) {
    let Ok(mut view) = partition::PartitionDevice::new(device, partition) else {
        return;
    };
    let Ok(volume) = fat32::mount(&mut view) else {
        return;
    };
    let Some(start_lba) = partition.start_lba.checked_add(volume.total_sectors as u64) else {
        return;
    };
    let Some(limit_lba) = partition.start_lba.checked_add(partition.sectors) else {
        return;
    };
    configure_swap_area(start_lba, limit_lba);
}

fn configure_swap_area(start_lba: u64, limit_lba: u64) {
    if limit_lba <= start_lba {
        return;
    }
    let _ = swap::configure_ata_backing(start_lba, limit_lba - start_lba);
}

fn mount_device(device: &mut impl crate::block::BlockDevice) -> MountStatus {
    let volume = match fat32::mount(device) {
        Ok(volume) => volume,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            return MountStatus::NotFat32;
        }
        Err(_) => return MountStatus::Failed,
    };

    let _ = vfs::mkdir("/mnt");

    let writable_import = !device.is_read_only();

    match import_directory(
        device,
        volume,
        volume.root_cluster,
        "/mnt",
        0,
        writable_import,
    ) {
        Ok(count) => MountStatus::Mounted(count),
        Err(_) => MountStatus::Failed,
    }
}

fn import_directory(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    dir_cluster: u32,
    vfs_prefix: &str,
    depth: usize,
    writable_import: bool,
) -> Result<usize, fat32::Error> {
    let mut mounted = 0usize;
    let mut children = alloc::vec::Vec::new();
    let mut metadata_paths = alloc::vec::Vec::new();

    fat32::for_each_directory_entry_named(device, volume, dir_cluster, |entry, long_name| {
        if entry.attributes & 0x08 != 0 {
            return Ok(());
        }
        // Skip . and ..
        if entry.short_name[0] == b'.' {
            return Ok(());
        }

        let mut name = [0u8; 12];
        let name_str = if let Some(long_name) = long_name {
            long_name
        } else {
            let Some(name_len) = short_name_to_str(&entry.short_name, &mut name) else {
                return Ok(());
            };
            let Ok(short_name) = core::str::from_utf8(&name[..name_len]) else {
                return Ok(());
            };
            short_name
        };

        let mut path_buf = [0u8; crate::config::MAX_PATH_SIZE];
        let Some(path_len) = join_path(vfs_prefix, name_str, &mut path_buf) else {
            return Ok(());
        };
        let Ok(path) = core::str::from_utf8(&path_buf[..path_len]) else {
            return Ok(());
        };

        if depth == 0 && vfs_prefix == "/mnt" && name_str.eq_ignore_ascii_case("WHSNAP") {
            return Ok(());
        }

        let is_dir = entry.attributes & DIRECTORY_ATTRIBUTE != 0;
        if is_dir {
            match vfs::mkdir(path) {
                Ok(()) => mounted = mounted.saturating_add(1),
                Err(vfs::Error::AlreadyExists) | Err(vfs::Error::Full) => {}
                Err(_) => return Err(fat32::Error::DirectoryFull),
            }
            if depth < MAX_BOOT_IMPORT_DEPTH
                && entry.first_cluster >= 2
                && children.len() < crate::config::MAX_VFS_NODES
            {
                children.push((entry.first_cluster, alloc::string::String::from(path)));
            }
            return Ok(());
        }

        if entry.size as usize > vfs::NODE_CAPACITY {
            return Ok(());
        }
        let writable = writable_import && entry.attributes & READ_ONLY_ATTRIBUTE == 0;
        match vfs::create_disk_file_with_writable(path, entry.size as usize, writable) {
            Ok(()) => {
                if metadata_paths.len() < crate::config::MAX_VFS_NODES {
                    metadata_paths.push((alloc::string::String::from(path), entry.first_cluster));
                }
                mounted = mounted.saturating_add(1)
            }
            Err(vfs::Error::AlreadyExists) | Err(vfs::Error::Full) => {}
            Err(_) => {}
        }
        Ok(())
    })?;

    let mut recovered_metadata = false;
    for (path, inode) in metadata_paths {
        let path_hash = fat32::metadata_path_hash(&path);
        let path_tag = fat32::metadata_path_tag(&path);
        let mut skip_metadata = false;
        if let Some(intent) = fat32::read_journal_intent(device, volume, path_hash, path_tag)
            .ok()
            .flatten()
        {
            let mut bytes = [0u8; vfs::NODE_CAPACITY];
            let checksum_ok = fat32::resolve_path(device, volume, &path[5..])
                .ok()
                .and_then(|entry| fat32::read_file(device, volume, entry, &mut bytes).ok())
                .is_some_and(|length| checksum_bytes(&bytes[..length]) == intent.checksum);
            if checksum_ok {
                if fat32::write_file_metadata(
                    device,
                    volume,
                    intent.path_hash,
                    intent.path_tag,
                    intent.metadata,
                )
                .is_ok()
                    && fat32::remove_journal_intent(device, volume, path_hash, path_tag).is_ok()
                {
                    recovered_metadata = true;
                }
            } else {
                // Keep the intent for a later repair pass; never apply metadata
                // over data that does not match the recorded transaction.
                skip_metadata = true;
            }
        }
        if !skip_metadata {
            let inode_metadata = fat32::read_inode_metadata(device, volume, inode)
                .ok()
                .flatten();
            let had_inode_metadata = inode_metadata.is_some();
            let metadata = if inode_metadata.is_some() {
                inode_metadata
            } else {
                fat32::read_file_metadata(device, volume, path_hash, path_tag)
                    .ok()
                    .flatten()
            };
            if let Some(metadata) = metadata {
                let _ = vfs::set_metadata(&path, metadata.uid, metadata.gid, metadata.mode);
                if inode >= 2
                    && !had_inode_metadata
                    && fat32::write_inode_metadata(device, volume, inode, metadata).is_ok()
                {
                    let _ = fat32::remove_file_metadata(device, volume, path_hash, path_tag);
                    recovered_metadata = true;
                }
                if fat32::finalize_file_metadata(device, volume, path_hash, path_tag)
                    .ok()
                    .unwrap_or(false)
                {
                    recovered_metadata = true;
                }
            }
        }
        let mut durable_bytes = [0u8; vfs::NODE_CAPACITY];
        if let Ok(entry) = fat32::resolve_path(device, volume, &path[5..]) {
            if let Ok(length) = fat32::read_file(device, volume, entry, &mut durable_bytes) {
                let mode = vfs::stat(&path).ok().map_or(0, |stat| u32::from(stat.mode));
                if !crate::wovenfs::record(
                    &path,
                    length as u64,
                    mode,
                    crate::timer::ticks(),
                    &durable_bytes[..length],
                ) {
                    return Err(fat32::Error::DirectoryFull);
                }
            }
        }
    }
    if recovered_metadata {
        let _ = device.flush();
    }

    // Release the directory iterator's mutable device borrow before recursion.
    for (cluster, path) in children {
        mounted = mounted.saturating_add(import_directory(
            device,
            volume,
            cluster,
            &path,
            depth + 1,
            writable_import,
        )?);
    }
    Ok(mounted)
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    // This checksum is part of the WovenFS live-file contract. Keep it
    // byte-for-byte identical to wovenfs::record(); the snapshot envelope
    // and catalog have separate integrity domains below.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
fn join_path(prefix: &str, name: &str, out: &mut [u8]) -> Option<usize> {
    let slash = !prefix.ends_with('/');
    let need = prefix.len() + usize::from(slash) + name.len();
    if need > out.len() {
        return None;
    }
    out[..prefix.len()].copy_from_slice(prefix.as_bytes());
    let mut len = prefix.len();
    if slash {
        out[len] = b'/';
        len += 1;
    }
    out[len..len + name.len()].copy_from_slice(name.as_bytes());
    Some(len + name.len())
}

fn short_name_to_str(short_name: &[u8; 11], out: &mut [u8; 12]) -> Option<usize> {
    let mut len = 0usize;
    for byte in short_name[..8].iter().copied().take_while(|b| *b != b' ') {
        out[len] = to_lower(byte)?;
        len += 1;
    }
    if len == 0 {
        return None;
    }
    if short_name[8..].iter().any(|b| *b != b' ') {
        out[len] = b'.';
        len += 1;
        for byte in short_name[8..].iter().copied().take_while(|b| *b != b' ') {
            out[len] = to_lower(byte)?;
            len += 1;
        }
    }
    Some(len)
}

fn to_lower(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte + (b'a' - b'A')),
        b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => Some(byte),
        _ => None,
    }
}

pub fn self_test() -> bool {
    let mut name = [0u8; 12];
    let nlen = short_name_to_str(b"KERNEL  BIN", &mut name).unwrap_or(0);
    let kernel_ok = &name[..nlen] == b"kernel.bin";
    let nlen = short_name_to_str(b"README     ", &mut name).unwrap_or(0);
    let readme_ok = &name[..nlen] == b"readme";
    let bad_ok = short_name_to_str(b"BAD?    TXT", &mut name).is_none();
    let encode_ok = fat32::encode_short_name("kernel.bin") == Some(*b"KERNEL  BIN")
        && fat32::encode_short_name("readme") == Some(*b"README     ");
    kernel_ok && readme_ok && bad_ok && encode_ok
}

/// Ensure a path under `/mnt` exists in the VFS by resolving it on the live ATA volume.
///
/// If the path is already present, returns success immediately. Otherwise opens the
/// primary ATA master, mounts FAT32 (superfloppy / MBR / GPT), resolves the relative
/// path, and imports the file or directory into the VFS.
pub fn ensure_path(path: &str) -> Result<(), EnsureError> {
    if mnt_path(path) && !mnt_mounted() {
        return Err(unavailable_ensure_error());
    }
    if !mnt_path(path) {
        return if vfs::stat(path).is_ok() {
            Ok(())
        } else {
            Err(EnsureError::NotUnderMount)
        };
    }

    if let Ok(stat) = vfs::stat(path) {
        if stat.kind != vfs::NodeKind::Directory {
            return Ok(());
        }
    }

    if !block_io::primary_ata_present() {
        return Err(EnsureError::NoDevice);
    }
    let mut disk = block_io::primary_ata();

    if path == "/mnt" {
        match vfs::mkdir("/mnt") {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return Err(EnsureError::Vfs),
        }
        return ensure_root_listing_on_disk(&mut disk);
    }

    let relative = &path[5..]; // strip "/mnt/"
    ensure_on_disk(&mut disk, relative, path)
}
fn ensure_root_listing_on_disk(
    device: &mut impl crate::block::BlockDevice,
) -> Result<(), EnsureError> {
    match fat32::mount(device) {
        Ok(volume) => {
            let imported = import_directory(
                device,
                volume,
                volume.root_cluster,
                "/mnt",
                MAX_BOOT_IMPORT_DEPTH,
                !device.is_read_only(),
            )
            .map_err(map_fat_err)?;
            MNT_IMPORTED.fetch_add(imported, Ordering::AcqRel);
            return Ok(());
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(EnsureError::Failed),
    }

    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| EnsureError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_fat_err)?;
        let writable_import = !view.is_read_only();
        let imported = import_directory(
            &mut view,
            volume,
            volume.root_cluster,
            "/mnt",
            MAX_BOOT_IMPORT_DEPTH,
            writable_import,
        )
        .map_err(map_fat_err)?;
        MNT_IMPORTED.fetch_add(imported, Ordering::AcqRel);
        return Ok(());
    }

    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| EnsureError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_fat_err)?;
            let writable_import = !view.is_read_only();
            let imported = import_directory(
                &mut view,
                volume,
                volume.root_cluster,
                "/mnt",
                MAX_BOOT_IMPORT_DEPTH,
                writable_import,
            )
            .map_err(map_fat_err)?;
            MNT_IMPORTED.fetch_add(imported, Ordering::AcqRel);
            Ok(())
        }
        Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => Err(EnsureError::NotFat32),
        Err(_) => Err(EnsureError::Failed),
    }
}
fn ensure_on_disk(
    device: &mut impl crate::block::BlockDevice,
    relative: &str,
    full_path: &str,
) -> Result<(), EnsureError> {
    // Prefer superfloppy at LBA 0.
    match fat32::mount(device) {
        Ok(volume) => return import_resolved(device, volume, relative, full_path),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(EnsureError::Failed),
    }

    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| EnsureError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_fat_err)?;
        return import_resolved(&mut view, volume, relative, full_path);
    }

    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| EnsureError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_fat_err)?;
            import_resolved(&mut view, volume, relative, full_path)
        }
        Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => Err(EnsureError::NotFat32),
        Err(_) => Err(EnsureError::Failed),
    }
}

fn import_resolved(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    relative: &str,
    full_path: &str,
) -> Result<(), EnsureError> {
    let entry = fat32::resolve_path(device, volume, relative).map_err(map_fat_err)?;
    let is_dir = entry.attributes & DIRECTORY_ATTRIBUTE != 0;

    create_parent_dirs(full_path)?;

    if is_dir {
        match vfs::mkdir(full_path) {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return Err(EnsureError::Vfs),
        }
        let imported = import_directory(
            device,
            volume,
            entry.first_cluster,
            full_path,
            MAX_BOOT_IMPORT_DEPTH,
            !device.is_read_only(),
        )
        .map_err(map_fat_err)?;
        MNT_IMPORTED.fetch_add(imported, Ordering::AcqRel);
        return Ok(());
    }
    if entry.size as usize > vfs::NODE_CAPACITY {
        return Err(EnsureError::TooLarge);
    }
    let writable = !device.is_read_only() && entry.attributes & READ_ONLY_ATTRIBUTE == 0;
    match vfs::create_disk_file_with_writable(full_path, entry.size as usize, writable) {
        Ok(()) | Err(vfs::Error::AlreadyExists) => Ok(()),
        Err(_) => Err(EnsureError::Vfs),
    }
}

fn create_parent_dirs(path: &str) -> Result<(), EnsureError> {
    let bytes = path.as_bytes();
    if !path.starts_with('/') {
        return Err(EnsureError::InvalidPath);
    }
    let mut i = 1;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i] != b'/' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let prefix = core::str::from_utf8(&bytes[..i]).map_err(|_| EnsureError::InvalidPath)?;
        match vfs::mkdir(prefix) {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return Err(EnsureError::Vfs),
        }
        i += 1;
    }
    Ok(())
}

fn map_fat_err(err: fat32::Error) -> EnsureError {
    match err {
        fat32::Error::NotFound => EnsureError::NotFound,
        fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry => {
            EnsureError::NotFat32
        }
        _ => EnsureError::Failed,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnsureError {
    Unmounted,
    NoDevice,
    NotFat32,
    NotFound,
    NotUnderMount,
    InvalidPath,
    TooLarge,
    Vfs,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistError {
    Unmounted,
    NotSupported,
    NotFound,
    NoDevice,
    BadName,
    TooLarge,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationError {
    Unmounted,
    NotSupported,
    NotFound,
    NoDevice,
    BadName,
    AlreadyExists,
    NotEmpty,
    ReadOnly,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LiveMutationTestStatus {
    Skipped,
    Passed,
    Failed(&'static str),
}

/// Exercise the live ATA/FAT32 mutation path when a mounted `/mnt` volume exists.
///
/// This is intended for disposable QEMU disks. It mirrors the diagnostic-shell
/// sequence for mkdir/write/persist/directory rename/delete and skips when the
/// boot has no writable FAT32 ATA mount.
pub fn live_mutation_self_test() -> LiveMutationTestStatus {
    if !block_io::primary_ata_present() || vfs::stat("/mnt").is_err() {
        return LiveMutationTestStatus::Skipped;
    }

    const OLD_DIR: &str = "/mnt/tmutd";
    const NEW_DIR: &str = "/mnt/tmuta";
    const OLD_FILE: &str = "/mnt/tmutd/a.txt";
    const NEW_FILE: &str = "/mnt/tmuta/a.txt";
    const CONTENT: &[u8] = b"hello";

    if vfs::stat(OLD_DIR).is_ok() || vfs::stat(NEW_DIR).is_ok() {
        return LiveMutationTestStatus::Failed("fixture exists");
    }
    if vfs::mkdir(OLD_DIR).is_err() {
        return LiveMutationTestStatus::Failed("vfs mkdir");
    }
    if let Err(error) = persist_directory(OLD_DIR) {
        crate::serial::write_line(format_args!(
            "[STORAGE MUTATION] fat mkdir error: {:?}",
            error
        ));
        return LiveMutationTestStatus::Failed("fat mkdir");
    }
    if vfs::write_file(OLD_FILE, CONTENT).is_err() {
        return LiveMutationTestStatus::Failed("vfs write");
    }
    if persist_path(OLD_FILE).is_err() {
        return LiveMutationTestStatus::Failed("fat persist");
    }
    if rename_path(OLD_DIR, NEW_DIR).is_err() {
        return LiveMutationTestStatus::Failed("fat rename");
    }
    if vfs::rename(OLD_DIR, NEW_DIR).is_err() {
        return LiveMutationTestStatus::Failed("vfs rename");
    }
    if ensure_path(OLD_FILE) != Err(EnsureError::NotFound) || vfs::stat(OLD_FILE).is_ok() {
        return LiveMutationTestStatus::Failed("old path survived");
    }
    if ensure_path(NEW_FILE).is_err() {
        return LiveMutationTestStatus::Failed("new path missing");
    }
    let mut bytes = [0_u8; 8];
    if vfs::read_all(NEW_FILE, &mut bytes) != Ok(CONTENT.len())
        || &bytes[..CONTENT.len()] != CONTENT
    {
        return LiveMutationTestStatus::Failed("new data mismatch");
    }
    if delete_path(NEW_FILE).is_err() {
        return LiveMutationTestStatus::Failed("fat delete file");
    }
    if vfs::remove(NEW_FILE).is_err() {
        return LiveMutationTestStatus::Failed("vfs delete file");
    }
    if delete_path(NEW_DIR).is_err() {
        return LiveMutationTestStatus::Failed("fat delete dir");
    }
    if vfs::remove(NEW_DIR).is_err() {
        return LiveMutationTestStatus::Failed("vfs delete dir");
    }

    if let Err(stage) = live_directory_growth_self_test() {
        return LiveMutationTestStatus::Failed(stage);
    }
    if df_mnt().is_err() {
        return LiveMutationTestStatus::Failed("df");
    }
    match fscheck_mnt() {
        Ok(report) if report.fs_info_matches => {}
        Ok(_) => return LiveMutationTestStatus::Failed("fsinfo mismatch"),
        Err(_) => return LiveMutationTestStatus::Failed("fscheck"),
    }
    if sync_all_mounted().is_err() {
        return LiveMutationTestStatus::Failed("sync");
    }
    if mount_info().dirty {
        return LiveMutationTestStatus::Failed("sync dirty");
    }
    if unmount_mnt().is_err() {
        return LiveMutationTestStatus::Failed("umount");
    }
    if ensure_path("/mnt/cache.txt") != Err(EnsureError::Unmounted) {
        return LiveMutationTestStatus::Failed("umount ensure guard");
    }
    if persist_path("/mnt/cache.txt") != Err(PersistError::Unmounted) {
        return LiveMutationTestStatus::Failed("umount persist guard");
    }
    if !matches!(remount_mnt(), MountStatus::Mounted(_)) || !mnt_mounted() {
        return LiveMutationTestStatus::Failed("remount");
    }

    LiveMutationTestStatus::Passed
}

fn live_numbered_child_path<'a>(
    dir: &str,
    prefix: u8,
    index: usize,
    out: &'a mut [u8; 40],
) -> Option<&'a str> {
    if index >= 100 {
        return None;
    }
    let dir_bytes = dir.as_bytes();
    let total = dir_bytes.len().checked_add(1)?.checked_add(7)?;
    if total > out.len() {
        return None;
    }
    out[..dir_bytes.len()].copy_from_slice(dir_bytes);
    let mut offset = dir_bytes.len();
    out[offset] = b'/';
    offset += 1;
    out[offset] = prefix;
    out[offset + 1] = b'0' + (index / 10) as u8;
    out[offset + 2] = b'0' + (index % 10) as u8;
    out[offset + 3] = b'.';
    out[offset + 4] = b't';
    out[offset + 5] = b'x';
    out[offset + 6] = b't';
    core::str::from_utf8(&out[..total]).ok()
}

fn create_live_test_file(path: &str, content: &[u8]) -> Result<(), &'static str> {
    if vfs::write_file(path, content).is_err() {
        return Err("growth vfs write");
    }
    if persist_path(path).is_err() {
        return Err("growth fat persist");
    }
    Ok(())
}

fn remove_live_test_file(path: &str) -> Result<(), &'static str> {
    if delete_path(path).is_err() {
        return Err("growth fat delete file");
    }
    if vfs::remove(path).is_err() {
        return Err("growth vfs delete file");
    }
    Ok(())
}

fn remove_live_test_dir(path: &str) -> Result<(), &'static str> {
    if delete_path(path).is_err() {
        return Err("growth fat delete dir");
    }
    if vfs::remove(path).is_err() {
        return Err("growth vfs delete dir");
    }
    Ok(())
}

fn live_directory_growth_self_test() -> Result<(), &'static str> {
    const GROW_DIR: &str = "/mnt/tgrow";
    const FULL_DIR: &str = "/mnt/tfull";
    const MOVE_FILE: &str = "/mnt/tmv.txt";
    const MOVED_FILE: &str = "/mnt/tfull/tmv.txt";

    if vfs::stat(GROW_DIR).is_ok() || vfs::stat(FULL_DIR).is_ok() || vfs::stat(MOVE_FILE).is_ok() {
        return Err("growth fixture exists");
    }

    if vfs::mkdir(GROW_DIR).is_err() {
        return Err("growth vfs mkdir");
    }
    if persist_directory(GROW_DIR).is_err() {
        return Err("growth fat mkdir");
    }
    for index in 0..15 {
        let mut path = [0_u8; 40];
        let Some(path) = live_numbered_child_path(GROW_DIR, b'g', index, &mut path) else {
            return Err("growth path build");
        };
        create_live_test_file(path, b"x")?;
    }
    let mut last_path = [0_u8; 40];
    let Some(last_path) = live_numbered_child_path(GROW_DIR, b'g', 14, &mut last_path) else {
        return Err("growth path build");
    };
    if ensure_path(last_path).is_err() {
        return Err("growth ensure");
    }
    let mut byte = [0_u8; 1];
    if vfs::read_all(last_path, &mut byte) != Ok(1) || byte[0] != b'x' {
        return Err("growth readback");
    }

    if vfs::mkdir(FULL_DIR).is_err() {
        return Err("full vfs mkdir");
    }
    if persist_directory(FULL_DIR).is_err() {
        return Err("full fat mkdir");
    }
    for index in 0..14 {
        let mut path = [0_u8; 40];
        let Some(path) = live_numbered_child_path(FULL_DIR, b'd', index, &mut path) else {
            return Err("full path build");
        };
        create_live_test_file(path, b"q")?;
    }
    create_live_test_file(MOVE_FILE, b"z")?;
    if rename_path(MOVE_FILE, MOVED_FILE).is_err() {
        return Err("full fat rename");
    }
    if vfs::rename(MOVE_FILE, MOVED_FILE).is_err() {
        return Err("full vfs rename");
    }
    if ensure_path(MOVE_FILE) != Err(EnsureError::NotFound) || vfs::stat(MOVE_FILE).is_ok() {
        return Err("full old survived");
    }
    if ensure_path(MOVED_FILE).is_err() {
        return Err("full new missing");
    }
    if vfs::read_all(MOVED_FILE, &mut byte) != Ok(1) || byte[0] != b'z' {
        return Err("full readback");
    }

    remove_live_test_file(MOVED_FILE)?;
    for index in 0..14 {
        let mut path = [0_u8; 40];
        let Some(path) = live_numbered_child_path(FULL_DIR, b'd', index, &mut path) else {
            return Err("full path build");
        };
        remove_live_test_file(path)?;
    }
    remove_live_test_dir(FULL_DIR)?;
    for index in 0..15 {
        let mut path = [0_u8; 40];
        let Some(path) = live_numbered_child_path(GROW_DIR, b'g', index, &mut path) else {
            return Err("growth path build");
        };
        remove_live_test_file(path)?;
    }
    remove_live_test_dir(GROW_DIR)
}

pub fn persist_snapshot_catalog(catalog: fat32::SnapshotCatalog) -> Result<(), PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    with_mounted_volume(&mut disk, |device, volume| {
        fat32::write_snapshot_catalog(device, volume, catalog)
    })
    .map_err(map_persist_err)
}

pub fn load_snapshot_catalog() -> Result<Option<fat32::SnapshotCatalog>, PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    with_mounted_volume(&mut disk, |device, volume| {
        fat32::read_snapshot_catalog(device, volume)
    })
    .map_err(map_persist_err)
}

pub fn persist_snapshot_restore_intent(
    intent: crate::snapshots::RestoreIntent,
) -> Result<(), PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let durable = fat32::SnapshotRestoreIntent {
        snapshot_id: intent.snapshot_id,
        generation: intent.generation,
        expected_root: intent.expected_root,
        applied: intent.applied as u64,
    };
    let mut disk = block_io::primary_ata();
    with_mounted_volume(&mut disk, |device, volume| {
        fat32::write_snapshot_restore_intent(device, volume, durable)
    })
    .map_err(map_persist_err)
}

pub fn load_snapshot_restore_intent(
) -> Result<Option<crate::snapshots::RestoreIntent>, PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    let durable = with_mounted_volume(&mut disk, |device, volume| {
        fat32::read_snapshot_restore_intent(device, volume)
    })
    .map_err(map_persist_err)?;
    durable
        .map(|intent| {
            let applied = usize::try_from(intent.applied).map_err(|_| PersistError::Failed)?;
            Ok(crate::snapshots::RestoreIntent {
                snapshot_id: intent.snapshot_id,
                generation: intent.generation,
                expected_root: intent.expected_root,
                applied,
            })
        })
        .transpose()
}

pub fn clear_snapshot_restore_intent() -> Result<(), PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    with_mounted_volume(&mut disk, |device, volume| {
        fat32::clear_snapshot_restore_intent(device, volume)
    })
    .map_err(map_persist_err)
}

fn with_mounted_volume<T>(
    device: &mut impl crate::block::BlockDevice,
    operation: impl FnOnce(&mut dyn crate::block::BlockDevice, fat32::Volume) -> Result<T, fat32::Error>,
) -> Result<T, fat32::Error> {
    match fat32::mount(device) {
        Ok(volume) => operation(device, volume),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| fat32::Error::UnsupportedGeometry)?;
                let volume = fat32::mount(&mut view)?;
                operation(&mut view, volume)
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| fat32::Error::UnsupportedGeometry)?;
                let volume = fat32::mount(&mut view)?;
                operation(&mut view, volume)
            } else {
                Err(fat32::Error::InvalidBootSector)
            }
        }
        Err(error) => Err(error),
    }
}

const SNAPSHOT_PREIMAGE_MAGIC: &[u8; 4] = b"WHP1";
const SNAPSHOT_PREIMAGE_VERSION: u16 = 3;
const SNAPSHOT_PREIMAGE_HEADER: usize = 48;
const SNAPSHOT_PREIMAGE_ENVELOPE_OFFSET: usize = 40;
const SNAPSHOT_PREIMAGE_DIRECTORY_FLAG: u32 = 1 << 31;
const SNAPSHOT_DIRECTORY_CHECKSUM: u64 = 0xcbf2_9ce4_8422_2325;

fn snapshot_preimage_envelope_checksum(image: &[u8]) -> Result<u64, PersistError> {
    if image.len() < SNAPSHOT_PREIMAGE_HEADER {
        return Err(PersistError::Failed);
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (index, byte) in image.iter().enumerate() {
        let value = if (SNAPSHOT_PREIMAGE_ENVELOPE_OFFSET..SNAPSHOT_PREIMAGE_ENVELOPE_OFFSET + 8)
            .contains(&index)
        {
            0
        } else {
            *byte
        };
        hash = (hash ^ u64::from(value)).wrapping_mul(0x1000_0000_01b3);
    }
    Ok(hash)
}

fn read_durable_path_on_device(
    device: &mut impl crate::block::BlockDevice,
    relative: &str,
    buffer: &mut [u8],
) -> Result<usize, PersistError> {
    fn read_in_volume(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        relative: &str,
        buffer: &mut [u8],
    ) -> Result<usize, PersistError> {
        let entry = fat32::resolve_path(device, volume, relative).map_err(map_persist_err)?;
        if entry.size as usize > buffer.len() {
            return Err(PersistError::TooLarge);
        }
        fat32::read_file(device, volume, entry, buffer).map_err(map_persist_err)
    }

    match fat32::mount(device) {
        Ok(volume) => return read_in_volume(device, volume, relative, buffer),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(PersistError::Failed),
    }
    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
        return read_in_volume(&mut view, volume, relative, buffer);
    }
    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
            read_in_volume(&mut view, volume, relative, buffer)
        }
        _ => Err(PersistError::Failed),
    }
}

pub struct SnapshotPreimage {
    pub path: alloc::string::String,
    pub checksum: u64,
    pub mode: u32,
    pub directory: bool,
    pub data: alloc::vec::Vec<u8>,
}

impl SnapshotPreimage {
    /// Replay phase: restore directories before files so parents exist, then
    /// remove post-snapshot creations deepest-first after restoration.
    #[allow(dead_code)]
    pub fn replay_phase(&self) -> u8 {
        if self.checksum == 0 {
            2
        } else if self.directory {
            0
        } else {
            1
        }
    }

    #[allow(dead_code)]
    pub fn path_depth(&self) -> usize {
        self.path.as_bytes().iter().filter(|byte| **byte == b'/').count()
    }
}

fn read_snapshot_preimage_on_device(
    device: &mut impl crate::block::BlockDevice,
    snapshot_id: u64,
    path_hash: u64,
) -> Result<SnapshotPreimage, PersistError> {
    let internal = alloc::format!(
        "WHSNAP/{:08X}/{:08X}/{:08X}/{:08X}.COW",
        (snapshot_id >> 32) as u32,
        snapshot_id as u32,
        (path_hash >> 32) as u32,
        path_hash as u32,
    );
    let mut image = alloc::vec![0u8; SNAPSHOT_PREIMAGE_HEADER + fat32::MAX_LONG_NAME * 8 + vfs::NODE_CAPACITY];
    let length = read_durable_path_on_device(device, &internal, &mut image)?;
    image.truncate(length);
    if image.len() < SNAPSHOT_PREIMAGE_HEADER
        || &image[..4] != SNAPSHOT_PREIMAGE_MAGIC
        || u16::from_le_bytes([image[4], image[5]]) != SNAPSHOT_PREIMAGE_VERSION
    {
        return Err(PersistError::Failed);
    }
    let path_len = u16::from_le_bytes([image[6], image[7]]) as usize;
    let stored_snapshot = u64::from_le_bytes(image[8..16].try_into().map_err(|_| PersistError::Failed)?);
    let checksum = u64::from_le_bytes(image[16..24].try_into().map_err(|_| PersistError::Failed)?);
    let data_len = u32::from_le_bytes(image[24..28].try_into().map_err(|_| PersistError::Failed)?) as usize;
    let mode = u32::from_le_bytes(image[28..32].try_into().map_err(|_| PersistError::Failed)?);
    let stored_path_hash = u64::from_le_bytes(image[32..40].try_into().map_err(|_| PersistError::Failed)?);
    let stored_envelope = u64::from_le_bytes(image[40..48].try_into().map_err(|_| PersistError::Failed)?);
    let data_start = SNAPSHOT_PREIMAGE_HEADER.checked_add(path_len).ok_or(PersistError::Failed)?;
    let end = data_start.checked_add(data_len).ok_or(PersistError::Failed)?;
    if stored_snapshot != snapshot_id
        || stored_path_hash != path_hash
        || path_len == 0
        || end != image.len()
        || data_len > vfs::NODE_CAPACITY
        || snapshot_preimage_envelope_checksum(&image)? != stored_envelope
    {
        return Err(PersistError::Failed);
    }
    let path = core::str::from_utf8(&image[SNAPSHOT_PREIMAGE_HEADER..data_start])
        .map_err(|_| PersistError::Failed)?;
    if !path.starts_with("/mnt/")
        || crate::wovenfs::path_hash(path) != path_hash
        || path[5..].split('/').any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(PersistError::Failed);
    }
    let data = &image[data_start..end];
    let directory = mode & SNAPSHOT_PREIMAGE_DIRECTORY_FLAG != 0;
    let object_mode = mode & !SNAPSHOT_PREIMAGE_DIRECTORY_FLAG;
    if checksum == 0 {
        if data_len != 0 || mode != 0 {
            return Err(PersistError::Failed);
        }
    } else if directory {
        if checksum != SNAPSHOT_DIRECTORY_CHECKSUM || data_len != 0 || object_mode == 0 {
            return Err(PersistError::Failed);
        }
    } else if checksum_bytes(data) != checksum || object_mode == 0 {
        return Err(PersistError::Failed);
    }
    Ok(SnapshotPreimage {
        path: alloc::string::String::from(path),
        checksum,
        mode: object_mode,
        directory,
        data: data.to_vec(),
    })
}

pub fn load_snapshot_preimage(
    snapshot_id: u64,
    path_hash: u64,
) -> Result<SnapshotPreimage, PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    read_snapshot_preimage_on_device(&mut disk, snapshot_id, path_hash)
}

pub fn replay_snapshot_preimage(
    preimage: &SnapshotPreimage,
) -> Result<(), PersistError> {
    if !mnt_mounted() || !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let relative = preimage.path.strip_prefix("/mnt/").ok_or(PersistError::BadName)?;
    fn replay_in_volume(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        relative: &str,
        preimage: &SnapshotPreimage,
    ) -> Result<(), PersistError> {
        if preimage.checksum == 0 {
            match fat32::delete_path(device, volume, relative) {
                Ok(()) | Err(fat32::Error::NotFound) => Ok(()),
                Err(error) => Err(map_persist_err(error)),
            }
        } else if preimage.directory {
            match fat32::mkdir_path(device, volume, relative) {
                Ok(()) | Err(fat32::Error::AlreadyExists) => Ok(()),
                Err(error) => Err(map_persist_err(error)),
            }
        } else {
            fat32::create_path_file(device, volume, relative, &preimage.data)
                .map_err(map_persist_err)
        }
    }

    let mut disk = block_io::primary_ata();
    match fat32::mount(&mut disk) {
        Ok(volume) => replay_in_volume(&mut disk, volume, relative, preimage)?,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(&mut disk) {
                let mut view = partition::PartitionDevice::new(&mut disk, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                replay_in_volume(&mut view, volume, relative, preimage)?;
            } else if let Ok(Some(part)) = gpt::find_fat_partition(&mut disk) {
                let mut view = partition::PartitionDevice::new(&mut disk, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                replay_in_volume(&mut view, volume, relative, preimage)?;
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(error) => return Err(map_persist_err(error)),
    }
    disk.flush().map_err(|_| PersistError::Failed)?;
    if preimage.checksum == 0 {
        match vfs::remove(&preimage.path) {
            Ok(()) | Err(vfs::Error::NotFound) => {}
            Err(_) => return Err(PersistError::Failed),
        }
        let _ = crate::wovenfs::remove(&preimage.path);
    } else if preimage.directory {
        match vfs::mkdir(&preimage.path) {
            Ok(()) | Err(vfs::Error::AlreadyExists) => {}
            Err(_) => return Err(PersistError::Failed),
        }
        let mode = u16::try_from(preimage.mode).map_err(|_| PersistError::Failed)?;
        let (uid, gid) = vfs::stat(&preimage.path)
            .map(|stat| (stat.uid, stat.gid))
            .unwrap_or((0, 0));
        vfs::set_metadata(&preimage.path, uid, gid, mode).map_err(|_| PersistError::Failed)?;
    } else {
        if let Ok(stat) = vfs::stat(&preimage.path) {
            let _ = vfs::set_metadata(&preimage.path, stat.uid, stat.gid, 0o666);
        }
        vfs::write_file(&preimage.path, &preimage.data).map_err(|_| PersistError::Failed)?;
        let mode = u16::try_from(preimage.mode).map_err(|_| PersistError::Failed)?;
        let (uid, gid) = vfs::stat(&preimage.path).map(|stat| (stat.uid, stat.gid)).unwrap_or((0, 0));
        vfs::set_metadata(&preimage.path, uid, gid, mode).map_err(|_| PersistError::Failed)?;
        if !crate::wovenfs::record(&preimage.path, preimage.data.len() as u64, preimage.mode, 0, &preimage.data) {
            return Err(PersistError::Failed);
        }
    }
    mark_mnt_clean();
    Ok(())
}

fn write_snapshot_preimage_on_device(
    device: &mut impl crate::block::BlockDevice,
    snapshot_id: u64,
    path: &str,
    checksum: u64,
    mode: u32,
    data: &[u8],
) -> Result<(), PersistError> {
    if path.len() > u16::MAX as usize || data.len() > u32::MAX as usize {
        return Err(PersistError::TooLarge);
    }
    let path_hash = crate::wovenfs::path_hash(path);
    let mut image = alloc::vec![0u8; SNAPSHOT_PREIMAGE_HEADER + path.len() + data.len()];
    image[..4].copy_from_slice(SNAPSHOT_PREIMAGE_MAGIC);
    image[4..6].copy_from_slice(&SNAPSHOT_PREIMAGE_VERSION.to_le_bytes());
    image[6..8].copy_from_slice(&(path.len() as u16).to_le_bytes());
    image[8..16].copy_from_slice(&snapshot_id.to_le_bytes());
    image[16..24].copy_from_slice(&checksum.to_le_bytes());
    image[24..28].copy_from_slice(&(data.len() as u32).to_le_bytes());
    image[28..32].copy_from_slice(&mode.to_le_bytes());
    image[32..40].copy_from_slice(&path_hash.to_le_bytes());
    image[SNAPSHOT_PREIMAGE_HEADER..SNAPSHOT_PREIMAGE_HEADER + path.len()]
        .copy_from_slice(path.as_bytes());
    image[SNAPSHOT_PREIMAGE_HEADER + path.len()..].copy_from_slice(data);
    let envelope = snapshot_preimage_envelope_checksum(&image)?;
    image[40..48].copy_from_slice(&envelope.to_le_bytes());
    let internal = alloc::format!(
        "WHSNAP/{:08X}/{:08X}/{:08X}/{:08X}.COW",
        (snapshot_id >> 32) as u32,
        snapshot_id as u32,
        (path_hash >> 32) as u32,
        path_hash as u32,
    );

    fn write_in_volume(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        internal: &str,
        image: &[u8],
    ) -> Result<(), PersistError> {
        match fat32::resolve_path(device, volume, internal) {
            Ok(existing) => {
                if existing.size as usize != image.len() {
                    return Err(PersistError::Failed);
                }
                let mut retained = alloc::vec![0u8; image.len()];
                let length =
                    fat32::read_file(device, volume, existing, &mut retained).map_err(map_persist_err)?;
                if length != image.len() || retained != image {
                    return Err(PersistError::Failed);
                }
                Ok(())
            }
            Err(fat32::Error::NotFound) => {
                fat32::create_path_file(device, volume, internal, image).map_err(map_persist_err)
            }
            Err(error) => Err(map_persist_err(error)),
        }
    }

    match fat32::mount(device) {
        Ok(volume) => write_in_volume(device, volume, &internal, &image)?,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_in_volume(&mut view, volume, &internal, &image)?;
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_in_volume(&mut view, volume, &internal, &image)?;
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(_) => return Err(PersistError::Failed),
    }
    device.flush().map_err(|_| PersistError::Failed)
}

fn retain_snapshot_preimages_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    relative: &str,
    old_checksum: u64,
) -> Result<(), PersistError> {
    if old_checksum == 0 {
        return Ok(());
    }
    let (ids, count) = crate::snapshots::live_snapshot_ids();
    if count == 0 {
        return Ok(());
    }
    let mut bytes = alloc::vec![0u8; vfs::NODE_CAPACITY];
    let length = read_durable_path_on_device(device, relative, &mut bytes)?;
    if checksum_bytes(&bytes[..length]) != old_checksum {
        return Err(PersistError::Failed);
    }
    for snapshot_id in ids.into_iter().take(count) {
        write_snapshot_preimage_on_device(
            device,
            snapshot_id,
            path,
            old_checksum,
            crate::wovenfs::metadata(path).map_or(0, |metadata| metadata.mode),
            &bytes[..length],
        )?;
    }
    Ok(())
}

fn retain_snapshot_creation_markers_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
) -> Result<(), PersistError> {
    let (ids, count) = crate::snapshots::live_snapshot_ids();
    for snapshot_id in ids.into_iter().take(count) {
        write_snapshot_preimage_on_device(device, snapshot_id, path, 0, 0, &[])?;
    }
    Ok(())
}

/// Persist a VFS file under `/mnt/` to the live ATA FAT32 volume.
///
/// Multi-component paths are supported. Missing FAT32 directories are created
/// automatically; components use the bounded FAT32 long-name rules.
pub fn persist_path(path: &str) -> Result<(), PersistError> {
    let _replayed_intents = crate::journal::recover();
    if !path.starts_with("/mnt/") {
        return Err(PersistError::NotSupported);
    }
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    let relative = &path[5..];
    if relative.is_empty() {
        return Err(PersistError::BadName);
    }
    for component in relative.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.len() > fat32::MAX_LONG_NAME
        {
            return Err(PersistError::BadName);
        }
    }

    let mut data = [0_u8; vfs::NODE_CAPACITY];
    let length = match vfs::read_all(path, &mut data) {
        Ok(n) => n,
        Err(vfs::Error::NotFound) => return Err(PersistError::NotFound),
        Err(_) => return Err(PersistError::Failed),
    };
    if length > vfs::NODE_CAPACITY {
        return Err(PersistError::TooLarge);
    }

    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let checksum = data[..length].iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    if let Some(previous) = crate::wovenfs::metadata(path) {
        {
            let mut disk = block_io::primary_ata();
            if disk.is_read_only() {
                return Err(PersistError::Failed);
            }
            retain_snapshot_preimages_on_device(&mut disk, path, relative, previous.checksum)?;
        }
        crate::snapshots::record_live_change(
            crate::wovenfs::path_hash(path),
            previous.checksum,
            checksum,
        )
        .map_err(|_| PersistError::Failed)?;
    } else {
        {
            let mut disk = block_io::primary_ata();
            if disk.is_read_only() {
                return Err(PersistError::Failed);
            }
            retain_snapshot_creation_markers_on_device(&mut disk, path)?;
        }
        crate::snapshots::record_live_change(
            crate::wovenfs::path_hash(path),
            0,
            checksum,
        )
        .map_err(|_| PersistError::Failed)?;
    }
    let Some(journal_token) = crate::journal::begin(crate::journal::path_hash(path), checksum)
    else {
        return Err(PersistError::Failed);
    };
    let metadata = vfs::stat(path).ok().map(|stat| fat32::FileMetadata {
        uid: stat.uid,
        gid: stat.gid,
        mode: stat.mode,
    });
    let mut disk = block_io::primary_ata();
    let mut result = if let Some(metadata) = metadata {
        let intent = fat32::JournalIntent {
            path_hash: fat32::metadata_path_hash(path),
            path_tag: fat32::metadata_path_tag(path),
            checksum,
            metadata,
        };
        // Write and flush the intent before FAT data so a reboot can finish
        // the ownership update during the next mounted import.
        persist_journal_on_device(&mut disk, intent, false)
            .and_then(|_| persist_metadata_on_device(&mut disk, path, metadata, true))
    } else {
        Ok(())
    };
    if result.is_ok() {
        result = persist_on_device(&mut disk, relative, &data[..length]);
    }
    if result.is_ok() {
        if let Some(metadata) = metadata {
            result = persist_metadata_on_device(&mut disk, path, metadata, false);
            if result.is_ok() {
                result = persist_inode_metadata_on_device(&mut disk, path, metadata)
                    .map(|written| {
                        if written {
                            clear_path_metadata_on_device(&mut disk, path)
                        } else {
                            Ok(())
                        }
                    })
                    .and_then(|result| result);
            }
            if result.is_ok() {
                result = persist_journal_on_device(
                    &mut disk,
                    fat32::JournalIntent {
                        path_hash: fat32::metadata_path_hash(path),
                        path_tag: fat32::metadata_path_tag(path),
                        checksum,
                        metadata,
                    },
                    true,
                );
            }
        }
    }
    if result.is_ok() {
        let _ = crate::journal::commit(journal_token);
        let mode = vfs::stat(path).ok().map_or(0, |stat| u32::from(stat.mode));
        if !crate::wovenfs::record(path, length as u64, mode, crate::timer::ticks(), &data[..length]) {
            return Err(PersistError::Failed);
        }
    }
    result
}

fn persist_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    metadata: fat32::FileMetadata,
    pending: bool,
) -> Result<(), PersistError> {
    let hash = fat32::metadata_path_hash(path);
    let tag = fat32::metadata_path_tag(path);
    match fat32::mount(device) {
        Ok(volume) => write_metadata_state(device, volume, hash, tag, metadata, pending)
            .map_err(map_persist_err)?,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_metadata_state(&mut view, volume, hash, tag, metadata, pending)
                    .map_err(map_persist_err)?;
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_metadata_state(&mut view, volume, hash, tag, metadata, pending)
                    .map_err(map_persist_err)?;
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(_) => return Err(PersistError::Failed),
    }
    device.flush().map_err(|_| PersistError::Failed)
}

fn persist_inode_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    metadata: fat32::FileMetadata,
) -> Result<bool, PersistError> {
    let relative = path
        .strip_prefix("/mnt/")
        .ok_or(PersistError::NotSupported)?;
    let written = match fat32::mount(device) {
        Ok(volume) => {
            write_inode_for_volume(device, volume, relative, metadata).map_err(map_persist_err)?
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_inode_for_volume(&mut view, volume, relative, metadata)
                    .map_err(map_persist_err)?
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                write_inode_for_volume(&mut view, volume, relative, metadata)
                    .map_err(map_persist_err)?
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(_) => return Err(PersistError::Failed),
    };
    device.flush().map_err(|_| PersistError::Failed)?;
    Ok(written)
}

fn write_inode_for_volume(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    relative: &str,
    metadata: fat32::FileMetadata,
) -> Result<bool, fat32::Error> {
    let entry = fat32::resolve_path(device, volume, relative)?;
    if entry.first_cluster < 2 {
        return Ok(false);
    }
    fat32::write_inode_metadata(device, volume, entry.first_cluster, metadata)?;
    Ok(true)
}

fn clear_path_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
) -> Result<(), PersistError> {
    let hash = fat32::metadata_path_hash(path);
    let tag = fat32::metadata_path_tag(path);
    match fat32::mount(device) {
        Ok(volume) => {
            fat32::remove_file_metadata(device, volume, hash, tag).map_err(map_persist_err)?
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                fat32::remove_file_metadata(&mut view, volume, hash, tag)
                    .map_err(map_persist_err)?;
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                fat32::remove_file_metadata(&mut view, volume, hash, tag)
                    .map_err(map_persist_err)?;
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(_) => return Err(PersistError::Failed),
    }
    device.flush().map_err(|_| PersistError::Failed)
}

fn write_metadata_state(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    hash: u64,
    tag: u32,
    metadata: fat32::FileMetadata,
    pending: bool,
) -> Result<(), fat32::Error> {
    if pending {
        fat32::write_file_metadata_pending(device, volume, hash, tag, metadata)
    } else {
        fat32::finalize_file_metadata(device, volume, hash, tag).map(|_| ())
    }
}

fn persist_journal_on_device(
    device: &mut impl crate::block::BlockDevice,
    intent: fat32::JournalIntent,
    remove: bool,
) -> Result<(), PersistError> {
    let update = |target: &mut dyn crate::block::BlockDevice,
                  volume: fat32::Volume|
     -> Result<(), fat32::Error> {
        if remove {
            fat32::remove_journal_intent(target, volume, intent.path_hash, intent.path_tag)
        } else {
            fat32::append_journal_intent(target, volume, intent)
        }
    };
    match fat32::mount(device) {
        Ok(volume) => update(device, volume).map_err(map_persist_err)?,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                update(&mut view, volume).map_err(map_persist_err)?;
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| PersistError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
                update(&mut view, volume).map_err(map_persist_err)?;
            } else {
                return Err(PersistError::Failed);
            }
        }
        Err(_) => return Err(PersistError::Failed),
    }
    device.flush().map_err(|_| PersistError::Failed)
}

fn persist_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    data: &[u8],
) -> Result<(), PersistError> {
    // The ATA cache survives this transaction; flush before reporting success.
    FILE_PAGES.lock().invalidate();
    let result = persist_on_cached_device(device, path, data);
    let flushed = device.flush().map_err(|_| PersistError::Failed);
    let status = result.and(flushed);
    if status.is_ok() {
        mark_mnt_dirty();
    }
    status
}

fn persist_on_cached_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    data: &[u8],
) -> Result<(), PersistError> {
    // Same mount order as import: superfloppy → MBR → GPT.
    match fat32::mount(device) {
        Ok(volume) => {
            return fat32::create_path_file(device, volume, path, data).map_err(map_persist_err);
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(PersistError::Failed),
    }

    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
        return fat32::create_path_file(&mut view, volume, path, data).map_err(map_persist_err);
    }

    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
            fat32::create_path_file(&mut view, volume, path, data).map_err(map_persist_err)
        }
        Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => Err(PersistError::Failed),
        Err(_) => Err(PersistError::Failed),
    }
}

/// Persist a VFS directory under `/mnt/`, creating missing FAT32 components.
pub fn persist_directory(path: &str) -> Result<(), PersistError> {
    if path == "/mnt" {
        return if mnt_mounted() {
            Ok(())
        } else {
            Err(unavailable_persist_error())
        };
    }
    if !path.starts_with("/mnt/") {
        return Err(PersistError::NotSupported);
    }
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    let relative = &path[5..];
    if relative.is_empty() {
        return Err(PersistError::BadName);
    }
    for component in relative.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || fat32::encode_short_name(component).is_none()
        {
            return Err(PersistError::BadName);
        }
    }
    if !block_io::primary_ata_present() {
        return Err(PersistError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    if disk.is_read_only() {
        return Err(PersistError::Failed);
    }
    FILE_PAGES.lock().invalidate();
    // A directory created after a snapshot must disappear on rollback just
    // like a newly-created file. Retain an empty creation marker for every
    // live snapshot, then durably publish the COW catalog before FAT mkdir.
    retain_snapshot_creation_markers_on_device(&mut disk, path)?;
    crate::snapshots::record_live_change(crate::wovenfs::path_hash(path), 0, 0)
        .map_err(|_| PersistError::Failed)?;
    let result = mkdir_on_cached_device(&mut disk, relative);
    let flushed = disk.flush().map_err(|_| PersistError::Failed);
    let status = result.and(flushed);
    if status.is_ok() {
        mark_mnt_dirty();
    }
    status
}

fn mkdir_on_cached_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
) -> Result<(), PersistError> {
    match fat32::mount(device) {
        Ok(volume) => return fat32::mkdir_path(device, volume, path).map_err(map_persist_err),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(PersistError::Failed),
    }
    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
        return fat32::mkdir_path(&mut view, volume, path).map_err(map_persist_err);
    }
    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| PersistError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_persist_err)?;
            fat32::mkdir_path(&mut view, volume, path).map_err(map_persist_err)
        }
        _ => Err(PersistError::Failed),
    }
}

pub fn delete_path(path: &str) -> Result<(), MutationError> {
    if path == "/mnt" || !path.starts_with("/mnt/") {
        return Err(MutationError::NotSupported);
    }
    if !mnt_mounted() {
        return Err(unavailable_mutation_error());
    }
    let relative = &path[5..];
    validate_fat_relative(relative)?;
    if !block_io::primary_ata_present() {
        return Err(MutationError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    if disk.is_read_only() {
        return Err(MutationError::ReadOnly);
    }
    FILE_PAGES.lock().invalidate();
    let inode = inode_on_device(&mut disk, relative).ok().flatten();
    if let Some(previous) = crate::wovenfs::metadata(path) {
        retain_snapshot_preimages_on_device(&mut disk, path, relative, previous.checksum)
            .map_err(|_| MutationError::Failed)?;
        crate::snapshots::record_live_change(
            crate::wovenfs::path_hash(path),
            previous.checksum,
            0,
        )
        .map_err(|_| MutationError::Failed)?;
    }
    let mut result = delete_on_cached_device(&mut disk, relative);
    if result.is_ok() {
        result = remove_metadata_on_device(&mut disk, path);
    }
    if result.is_ok() {
        if let Some(inode) = inode {
            result = remove_inode_metadata_on_device(&mut disk, inode);
        }
    }
    let flushed = disk.flush().map_err(|_| MutationError::Failed);
    let status = result.and(flushed);
    if status.is_ok() {
        let _ = crate::wovenfs::remove(path);
        mark_mnt_dirty();
    }
    status
}

fn delete_on_cached_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
) -> Result<(), MutationError> {
    match fat32::mount(device) {
        Ok(volume) => return fat32::delete_path(device, volume, path).map_err(map_mutation_err),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(MutationError::Failed),
    }
    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| MutationError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
        return fat32::delete_path(&mut view, volume, path).map_err(map_mutation_err);
    }
    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| MutationError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
            fat32::delete_path(&mut view, volume, path).map_err(map_mutation_err)
        }
        _ => Err(MutationError::Failed),
    }
}

fn remove_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
) -> Result<(), MutationError> {
    let hash = fat32::metadata_path_hash(path);
    let tag = fat32::metadata_path_tag(path);
    match fat32::mount(device) {
        Ok(volume) => {
            fat32::remove_file_metadata(device, volume, hash, tag).map_err(map_mutation_err)
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                fat32::remove_file_metadata(&mut view, volume, hash, tag).map_err(map_mutation_err)
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                fat32::remove_file_metadata(&mut view, volume, hash, tag).map_err(map_mutation_err)
            } else {
                Err(MutationError::Failed)
            }
        }
        Err(_) => Err(MutationError::Failed),
    }
}

fn inode_on_device(
    device: &mut impl crate::block::BlockDevice,
    relative: &str,
) -> Result<Option<u32>, MutationError> {
    match fat32::mount(device) {
        Ok(volume) => Ok(fat32::resolve_path(device, volume, relative)
            .ok()
            .map(|entry| entry.first_cluster)
            .filter(|inode| *inode >= 2)),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                Ok(fat32::resolve_path(&mut view, volume, relative)
                    .ok()
                    .map(|entry| entry.first_cluster)
                    .filter(|inode| *inode >= 2))
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                Ok(fat32::resolve_path(&mut view, volume, relative)
                    .ok()
                    .map(|entry| entry.first_cluster)
                    .filter(|inode| *inode >= 2))
            } else {
                Err(MutationError::Failed)
            }
        }
        Err(_) => Err(MutationError::Failed),
    }
}

fn remove_inode_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    inode: u32,
) -> Result<(), MutationError> {
    match fat32::mount(device) {
        Ok(volume) => fat32::remove_inode_metadata(device, volume, inode).map_err(map_mutation_err),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                fat32::remove_inode_metadata(&mut view, volume, inode).map_err(map_mutation_err)
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                fat32::remove_inode_metadata(&mut view, volume, inode).map_err(map_mutation_err)
            } else {
                Err(MutationError::Failed)
            }
        }
        Err(_) => Err(MutationError::Failed),
    }
}

pub fn rename_path(old: &str, new: &str) -> Result<(), MutationError> {
    if old == "/mnt" || new == "/mnt" || !old.starts_with("/mnt/") || !new.starts_with("/mnt/") {
        return Err(MutationError::NotSupported);
    }
    if !mnt_mounted() {
        return Err(unavailable_mutation_error());
    }
    let old_relative = &old[5..];
    let new_relative = &new[5..];
    validate_fat_relative(old_relative)?;
    validate_fat_relative(new_relative)?;
    if !block_io::primary_ata_present() {
        return Err(MutationError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    if disk.is_read_only() {
        return Err(MutationError::ReadOnly);
    }
    FILE_PAGES.lock().invalidate();
    if let Some(mutations) = retain_snapshot_rename_objects_on_device(&mut disk, old, new)? {
        crate::snapshots::record_live_changes(&mutations)
            .map_err(|_| MutationError::Failed)?;
    }
    let mut result = rename_on_cached_device(&mut disk, old_relative, new_relative);
    if result.is_ok() {
        result = move_metadata_on_device(&mut disk, old, new);
    }
    let flushed = disk.flush().map_err(|_| MutationError::Failed);
    let status = result.and(flushed);
    if status.is_ok() {
        let _ = crate::wovenfs::rename(old, new);
        mark_mnt_dirty();
    }
    status
}

fn rename_on_cached_device(
    device: &mut impl crate::block::BlockDevice,
    old: &str,
    new: &str,
) -> Result<(), MutationError> {
    match fat32::mount(device) {
        Ok(volume) => {
            return fat32::rename_path(device, volume, old, new).map_err(map_mutation_err)
        }
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(MutationError::Failed),
    }
    if let Ok(Some(part)) = partition::find_fat32(device) {
        let mut view =
            partition::PartitionDevice::new(device, part).map_err(|_| MutationError::Failed)?;
        let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
        return fat32::rename_path(&mut view, volume, old, new).map_err(map_mutation_err);
    }
    match gpt::find_fat_partition(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| MutationError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
            fat32::rename_path(&mut view, volume, old, new).map_err(map_mutation_err)
        }
        _ => Err(MutationError::Failed),
    }
}

fn move_metadata_on_device(
    device: &mut impl crate::block::BlockDevice,
    old: &str,
    new: &str,
) -> Result<(), MutationError> {
    fn move_record(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        old: &str,
        new: &str,
    ) -> Result<(), MutationError> {
        let old_hash = fat32::metadata_path_hash(old);
        let old_tag = fat32::metadata_path_tag(old);
        let new_hash = fat32::metadata_path_hash(new);
        let new_tag = fat32::metadata_path_tag(new);
        let Some(metadata) = fat32::read_file_metadata(device, volume, old_hash, old_tag)
            .map_err(map_mutation_err)?
        else {
            return Ok(());
        };
        fat32::write_file_metadata(device, volume, new_hash, new_tag, metadata)
            .map_err(map_mutation_err)?;
        fat32::remove_file_metadata(device, volume, old_hash, old_tag).map_err(map_mutation_err)
    }
    fn move_in_volume(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        old: &str,
        new: &str,
    ) -> Result<(), MutationError> {
        let relative = new
            .strip_prefix("/mnt/")
            .ok_or(MutationError::NotSupported)?;
        let entry = fat32::resolve_path(device, volume, relative).map_err(map_mutation_err)?;
        if entry.attributes & 0x10 == 0 {
            return move_record(device, volume, old, new);
        }
        let mut moves = alloc::vec::Vec::new();
        collect_metadata_moves(device, volume, entry.first_cluster, old, new, 0, &mut moves)?;
        for (old_path, new_path) in moves {
            move_record(device, volume, &old_path, &new_path)?;
            // Keep the live WovenFS registry aligned with a FAT32 directory
            // subtree move. WovenFS keys files by full path hash, so renaming
            // only the directory object leaves stale descendant identities
            // that poison snapshot roots across reboot reconstruction.
            if crate::wovenfs::metadata(&old_path).is_some()
                && !crate::wovenfs::rename(&old_path, &new_path)
            {
                return Err(MutationError::Failed);
            }
        }
        Ok(())
    }
    match fat32::mount(device) {
        Ok(volume) => move_in_volume(device, volume, old, new),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                move_in_volume(&mut view, volume, old, new)
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                move_in_volume(&mut view, volume, old, new)
            } else {
                Err(MutationError::Failed)
            }
        }
        Err(_) => Err(MutationError::Failed),
    }
}

fn append_metadata_path(prefix: &str, name: &str) -> Option<alloc::string::String> {
    let slash = !prefix.ends_with('/');
    if prefix.len() + usize::from(slash) + name.len() > crate::config::MAX_PATH_SIZE {
        return None;
    }
    let mut path = alloc::string::String::from(prefix);
    if slash {
        path.push('/');
    }
    path.push_str(name);
    Some(path)
}

fn collect_metadata_moves(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    dir_cluster: u32,
    old_prefix: &str,
    new_prefix: &str,
    depth: usize,
    moves: &mut alloc::vec::Vec<(alloc::string::String, alloc::string::String)>,
) -> Result<(), MutationError> {
    if depth >= MAX_BOOT_IMPORT_DEPTH || moves.len() >= crate::config::MAX_VFS_NODES {
        return Ok(());
    }
    let mut children = alloc::vec::Vec::new();
    fat32::for_each_directory_entry_named(device, volume, dir_cluster, |entry, long_name| {
        if entry.attributes & 0x08 != 0 || entry.short_name[0] == b'.' {
            return Ok(());
        }
        let mut short = [0u8; 12];
        let name = if let Some(long_name) = long_name {
            alloc::string::String::from(long_name)
        } else {
            let Some(length) = short_name_to_str(&entry.short_name, &mut short) else {
                return Ok(());
            };
            let Ok(name) = core::str::from_utf8(&short[..length]) else {
                return Ok(());
            };
            alloc::string::String::from(name)
        };
        let Some(old_path) = append_metadata_path(old_prefix, &name) else {
            return Ok(());
        };
        let Some(new_path) = append_metadata_path(new_prefix, &name) else {
            return Ok(());
        };
        if entry.attributes & 0x10 != 0 && entry.first_cluster >= 2 {
            children.push((entry.first_cluster, old_path, new_path));
        } else if moves.len() < crate::config::MAX_VFS_NODES {
            moves.push((old_path, new_path));
        }
        Ok(())
    })
    .map_err(map_mutation_err)?;
    for (cluster, old_path, new_path) in children {
        collect_metadata_moves(
            device,
            volume,
            cluster,
            &old_path,
            &new_path,
            depth + 1,
            moves,
        )?;
    }
    Ok(())
}


type SnapshotMutation = (u64, u64, u64);

#[derive(Clone)]
struct SnapshotRenameObject {
    old_path: alloc::string::String,
    new_path: alloc::string::String,
    checksum: u64,
    mode: u32,
    directory: bool,
}

fn collect_snapshot_rename_objects_in_volume(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    dir_cluster: u32,
    old_prefix: &str,
    new_prefix: &str,
    depth: usize,
    objects: &mut alloc::vec::Vec<SnapshotRenameObject>,
) -> Result<(), MutationError> {
    if depth >= MAX_BOOT_IMPORT_DEPTH || objects.len() >= crate::config::MAX_VFS_NODES {
        return Err(MutationError::Failed);
    }
    let mut children = alloc::vec::Vec::new();
    fat32::for_each_directory_entry_named(device, volume, dir_cluster, |entry, long_name| {
        if entry.attributes & 0x08 != 0 || entry.short_name[0] == b'.' {
            return Ok(());
        }
        let mut short = [0u8; 12];
        let name = if let Some(long_name) = long_name {
            alloc::string::String::from(long_name)
        } else {
            let Some(length) = short_name_to_str(&entry.short_name, &mut short) else {
                return Err(fat32::Error::InvalidPath);
            };
            let name = core::str::from_utf8(&short[..length]).map_err(|_| fat32::Error::InvalidPath)?;
            alloc::string::String::from(name)
        };
        let old_path = append_metadata_path(old_prefix, &name).ok_or(fat32::Error::InvalidPath)?;
        let new_path = append_metadata_path(new_prefix, &name).ok_or(fat32::Error::InvalidPath)?;
        if entry.attributes & 0x10 != 0 {
            objects.push(SnapshotRenameObject {
                old_path: old_path.clone(),
                new_path: new_path.clone(),
                checksum: SNAPSHOT_DIRECTORY_CHECKSUM,
                mode: 0o755,
                directory: true,
            });
            if entry.first_cluster >= 2 {
                children.push((entry.first_cluster, old_path, new_path));
            }
        } else {
            let metadata = crate::wovenfs::metadata(&old_path).ok_or(fat32::Error::InvalidPath)?;
            objects.push(SnapshotRenameObject {
                old_path,
                new_path,
                checksum: metadata.checksum,
                mode: metadata.mode,
                directory: false,
            });
        }
        Ok(())
    })
    .map_err(map_mutation_err)?;
    for (cluster, old_path, new_path) in children {
        collect_snapshot_rename_objects_in_volume(
            device, volume, cluster, &old_path, &new_path, depth + 1, objects,
        )?;
    }
    Ok(())
}

fn retain_snapshot_rename_objects_on_device(
    device: &mut impl crate::block::BlockDevice,
    old: &str,
    new: &str,
) -> Result<Option<alloc::vec::Vec<SnapshotMutation>>, MutationError> {
    fn collect(
        device: &mut impl crate::block::BlockDevice,
        volume: fat32::Volume,
        old: &str,
        new: &str,
    ) -> Result<alloc::vec::Vec<SnapshotRenameObject>, MutationError> {
        let relative = old.strip_prefix("/mnt/").ok_or(MutationError::NotSupported)?;
        let entry = fat32::resolve_path(device, volume, relative).map_err(map_mutation_err)?;
        let mut objects = alloc::vec::Vec::new();
        if entry.attributes & 0x10 != 0 {
            objects.push(SnapshotRenameObject {
                old_path: alloc::string::String::from(old),
                new_path: alloc::string::String::from(new),
                checksum: SNAPSHOT_DIRECTORY_CHECKSUM,
                mode: 0o755,
                directory: true,
            });
            collect_snapshot_rename_objects_in_volume(
                device, volume, entry.first_cluster, old, new, 0, &mut objects,
            )?;
        } else {
            let metadata = crate::wovenfs::metadata(old).ok_or(MutationError::Failed)?;
            objects.push(SnapshotRenameObject {
                old_path: alloc::string::String::from(old),
                new_path: alloc::string::String::from(new),
                checksum: metadata.checksum,
                mode: metadata.mode,
                directory: false,
            });
        }
        Ok(objects)
    }

    let objects = match fat32::mount(device) {
        Ok(volume) => collect(device, volume, old, new)?,
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {
            if let Ok(Some(part)) = partition::find_fat32(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                collect(&mut view, volume, old, new)?
            } else if let Ok(Some(part)) = gpt::find_fat_partition(device) {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| MutationError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_mutation_err)?;
                collect(&mut view, volume, old, new)?
            } else {
                return Err(MutationError::Failed);
            }
        }
        Err(_) => return Err(MutationError::Failed),
    };
    let (ids, count) = crate::snapshots::live_snapshot_ids();
    if count == 0 {
        return Ok(None);
    }
    let mutation_count = objects.len().checked_mul(2).ok_or(MutationError::Failed)?;
    if mutation_count > 32 {
        return Err(MutationError::Failed);
    }
    let mut mutations = alloc::vec::Vec::with_capacity(mutation_count);
    for object in &objects {
        if object.directory {
            for snapshot_id in ids.into_iter().take(count) {
                write_snapshot_preimage_on_device(
                    device,
                    snapshot_id,
                    &object.old_path,
                    object.checksum,
                    object.mode | SNAPSHOT_PREIMAGE_DIRECTORY_FLAG,
                    &[],
                )
                .map_err(|_| MutationError::Failed)?;
            }
        } else {
            let relative = object.old_path.strip_prefix("/mnt/").ok_or(MutationError::Failed)?;
            retain_snapshot_preimages_on_device(device, &object.old_path, relative, object.checksum)
                .map_err(|_| MutationError::Failed)?;
        }
        for snapshot_id in ids.into_iter().take(count) {
            write_snapshot_preimage_on_device(device, snapshot_id, &object.new_path, 0, 0, &[])
                .map_err(|_| MutationError::Failed)?;
        }
        mutations.push((crate::wovenfs::path_hash(&object.old_path), object.checksum, 0));
        mutations.push((crate::wovenfs::path_hash(&object.new_path), 0, object.checksum));
    }
    Ok(Some(mutations))
}

fn validate_fat_relative(relative: &str) -> Result<(), MutationError> {
    if relative.is_empty() {
        return Err(MutationError::BadName);
    }
    for component in relative.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || fat32::encode_short_name(component).is_none()
        {
            return Err(MutationError::BadName);
        }
    }
    Ok(())
}

fn map_mutation_err(err: fat32::Error) -> MutationError {
    match err {
        fat32::Error::NotFound => MutationError::NotFound,
        fat32::Error::NameTooLong | fat32::Error::InvalidPath => MutationError::BadName,
        fat32::Error::AlreadyExists => MutationError::AlreadyExists,
        fat32::Error::DirectoryNotEmpty => MutationError::NotEmpty,
        fat32::Error::ReadOnly | fat32::Error::Block(crate::block::Error::ReadOnly) => {
            MutationError::ReadOnly
        }
        fat32::Error::Block(_) | fat32::Error::WriteFailed => MutationError::Failed,
        _ => MutationError::Failed,
    }
}

fn map_persist_err(err: fat32::Error) -> PersistError {
    match err {
        fat32::Error::NotFound => PersistError::NotFound,
        fat32::Error::NameTooLong | fat32::Error::DirectoryFull | fat32::Error::InvalidPath => {
            PersistError::BadName
        }
        fat32::Error::NoSpace => PersistError::TooLarge,
        fat32::Error::Block(_) | fat32::Error::WriteFailed | fat32::Error::ReadOnly => {
            PersistError::Failed
        }
        _ => PersistError::Failed,
    }
}

#[allow(dead_code)]
pub fn fat32_writable() -> bool {
    mnt_mounted() && block_io::primary_ata_present()
}

pub fn remount_mnt() -> MountStatus {
    FILE_PAGES.lock().invalidate();
    mount_ata_root()
}

pub fn unmount_mnt() -> Result<(), MountControlError> {
    if !mnt_mounted() {
        return Err(
            match lifecycle_from_code(MNT_STATUS.load(Ordering::Acquire)) {
                MountLifecycleStatus::NoDevice => MountControlError::NoDevice,
                _ => MountControlError::NotMounted,
            },
        );
    }
    sync_all_mounted().map_err(|_| MountControlError::SyncFailed)?;
    FILE_PAGES.lock().invalidate();
    if !block_io::primary_ata_present() {
        record_mount_status(MountStatus::NoDevice);
        return Err(MountControlError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    if disk.flush().is_err() {
        return Err(MountControlError::Failed);
    }
    MNT_STATUS.store(MOUNT_UNMOUNTED, Ordering::Release);
    MNT_IMPORTED.store(0, Ordering::Release);
    mark_mnt_clean();
    Ok(())
}

pub fn df_mnt() -> Result<fat32::SpaceInfo, SpaceError> {
    if !mnt_mounted() {
        return Err(unavailable_space_error());
    }
    if !block_io::primary_ata_present() {
        return Err(SpaceError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    df_on_device(&mut disk)
}

pub fn fscheck_mnt() -> Result<fat32::CheckReport, SpaceError> {
    if !mnt_mounted() {
        return Err(unavailable_space_error());
    }
    if !block_io::primary_ata_present() {
        return Err(SpaceError::NoDevice);
    }
    let mut disk = block_io::primary_ata();
    check_on_device(&mut disk)
}

fn df_on_device(
    device: &mut impl crate::block::BlockDevice,
) -> Result<fat32::SpaceInfo, SpaceError> {
    match fat32::mount(device) {
        Ok(volume) => return fat32::space_info(device, volume).map_err(map_space_err),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(SpaceError::Failed),
    }
    match partition::find_fat32(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| SpaceError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_space_err)?;
            fat32::space_info(&mut view, volume).map_err(map_space_err)
        }
        Ok(None) => match gpt::find_fat_partition(device) {
            Ok(Some(part)) => {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| SpaceError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_space_err)?;
                fat32::space_info(&mut view, volume).map_err(map_space_err)
            }
            Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => Err(SpaceError::NotFat32),
            Err(_) => Err(SpaceError::Failed),
        },
        Err(_) => Err(SpaceError::Failed),
    }
}

fn check_on_device(
    device: &mut impl crate::block::BlockDevice,
) -> Result<fat32::CheckReport, SpaceError> {
    match fat32::mount(device) {
        Ok(volume) => return fat32::check_volume(device, volume).map_err(map_space_err),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(_) => return Err(SpaceError::Failed),
    }
    match partition::find_fat32(device) {
        Ok(Some(part)) => {
            let mut view =
                partition::PartitionDevice::new(device, part).map_err(|_| SpaceError::Failed)?;
            let volume = fat32::mount(&mut view).map_err(map_space_err)?;
            fat32::check_volume(&mut view, volume).map_err(map_space_err)
        }
        Ok(None) => match gpt::find_fat_partition(device) {
            Ok(Some(part)) => {
                let mut view = partition::PartitionDevice::new(device, part)
                    .map_err(|_| SpaceError::Failed)?;
                let volume = fat32::mount(&mut view).map_err(map_space_err)?;
                fat32::check_volume(&mut view, volume).map_err(map_space_err)
            }
            Ok(None) | Err(gpt::Error::MissingProtectiveMbr) => Err(SpaceError::NotFat32),
            Err(_) => Err(SpaceError::Failed),
        },
        Err(_) => Err(SpaceError::Failed),
    }
}

fn map_space_err(error: fat32::Error) -> SpaceError {
    match error {
        fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry => SpaceError::NotFat32,
        _ => SpaceError::Failed,
    }
}

/// Best-effort walk of every VFS file under `/mnt/` and persist each one.
/// Returns the number of files successfully written.
pub fn sync_all_mounted() -> Result<usize, PersistError> {
    if !mnt_mounted() {
        return Err(unavailable_persist_error());
    }
    // Snapshot names while holding VFS; persistence reacquires VFS to read data.
    // Never call persist_path from the registry's locked callback.
    let mut paths = alloc::vec::Vec::new();
    vfs::for_each_file_with_prefix("/mnt/", |path| {
        paths.push(alloc::string::String::from(path));
    });
    let mut ok = 0usize;
    let mut failed = false;
    for path in paths {
        if persist_path(&path).is_ok() {
            ok += 1;
        } else {
            failed = true;
        }
    }
    if block_io::primary_ata_present() {
        let mut disk = block_io::primary_ata();
        if disk.flush().is_err() {
            failed = true;
        }
    } else {
        failed = true;
    }
    if failed {
        mark_mnt_dirty();
        Err(PersistError::Failed)
    } else {
        mark_mnt_clean();
        MNT_SYNCS.fetch_add(1, Ordering::AcqRel);
        Ok(ok)
    }
}
// Lock order: ATA device (10), then clean file pages (20). Physical reads
// release the page-cache guard while retaining ATA serialization.
static FILE_PAGES: crate::irq_lock::IrqMutex<crate::page_cache::PageCache<16>> =
    crate::irq_lock::IrqMutex::with_rank(crate::page_cache::PageCache::new(), 20);

pub fn page_cache_stats() -> crate::page_cache::Stats {
    FILE_PAGES.lock().stats()
}

pub fn read_disk_file(
    path: &str,
    offset: usize,
    output: &mut [u8],
) -> Result<usize, crate::block::Error> {
    if !mnt_mounted() {
        return Err(crate::block::Error::DeviceFault);
    }
    let relative = path
        .strip_prefix("/mnt/")
        .ok_or(crate::block::Error::InvalidBuffer)?;
    if !block_io::primary_ata_present() {
        return Err(crate::block::Error::DeviceFault);
    }
    let mut disk = block_io::primary_ata();
    FILE_PAGES
        .lock()
        .validate_read(path, offset, output.len())?;
    let mut copied = 0usize;
    while copied < output.len() {
        let position = offset + copied;
        let page = position / crate::page_cache::PAGE_SIZE;
        let within = position % crate::page_cache::PAGE_SIZE;
        let chunk = (crate::page_cache::PAGE_SIZE - within).min(output.len() - copied);
        let (count, page_length) = {
            let mut cache = FILE_PAGES.lock();
            if let Some(hit) =
                cache.read_cached_page(path, page, within, &mut output[copied..copied + chunk])
            {
                hit
            } else {
                let epoch = cache.epoch();
                drop(cache);
                let mut bytes = [0u8; crate::page_cache::PAGE_SIZE];
                let length = read_disk_page(
                    &mut disk,
                    relative,
                    page * crate::page_cache::PAGE_SIZE,
                    &mut bytes,
                )
                .map_err(|_| crate::block::Error::DeviceFault)?;
                if length > crate::page_cache::PAGE_SIZE {
                    return Err(crate::block::Error::InvalidBuffer);
                }
                let published = FILE_PAGES
                    .lock()
                    .publish_loaded_page(path, page, &bytes, length, epoch)?;
                if !published {
                    return Err(crate::block::Error::DeviceFault);
                }
                let count = length.saturating_sub(within).min(chunk);
                output[copied..copied + count].copy_from_slice(&bytes[within..within + count]);
                (count, length)
            }
        };
        copied += count;
        if count == 0 || page_length < crate::page_cache::PAGE_SIZE {
            break;
        }
    }
    Ok(copied)
}

fn read_volume_page(
    device: &mut impl crate::block::BlockDevice,
    volume: fat32::Volume,
    path: &str,
    offset: usize,
    output: &mut [u8],
) -> Result<usize, fat32::Error> {
    let entry = fat32::resolve_path(device, volume, path)?;
    if entry.attributes & DIRECTORY_ATTRIBUTE != 0 {
        return Err(fat32::Error::NotFound);
    }
    fat32::read_file_at(device, volume, entry, offset, output)
}

fn read_disk_page(
    device: &mut impl crate::block::BlockDevice,
    path: &str,
    offset: usize,
    output: &mut [u8],
) -> Result<usize, fat32::Error> {
    match fat32::mount(device) {
        Ok(volume) => return read_volume_page(device, volume, path, offset, output),
        Err(fat32::Error::InvalidBootSector | fat32::Error::UnsupportedGeometry) => {}
        Err(error) => return Err(error),
    }
    let part = match partition::find_fat32(device) {
        Ok(Some(part)) => part,
        _ => gpt::find_fat_partition(device)
            .map_err(|_| fat32::Error::InvalidBootSector)?
            .ok_or(fat32::Error::InvalidBootSector)?,
    };
    let mut view = partition::PartitionDevice::new(device, part)
        .map_err(|_| fat32::Error::InvalidBootSector)?;
    let volume = fat32::mount(&mut view)?;
    read_volume_page(&mut view, volume, path, offset, output)
}
