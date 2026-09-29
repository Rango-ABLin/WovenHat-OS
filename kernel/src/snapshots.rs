//! Bounded copy-on-write snapshot catalog for filesystem generations.
use crate::irq_lock::IrqMutex as Mutex;

const MAX: usize = 8;
const MAX_CHANGES: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub id: u64,
    pub generation: u64,
    pub checksum: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CowRecord {
    pub snapshot_id: u64,
    pub path_hash: u64,
    pub old_checksum: u64,
    pub new_checksum: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RestoreError {
    MissingSnapshot,
    #[cfg(feature = "stage12-4-test")]
    ChecksumMismatch,
    ChangeLogFull,
    #[cfg(feature = "stage12-4-test")]
    RestoreBusy,
    PersistenceFailed,
}

/// Rank 10 protects short snapshot metadata operations only. Filesystem I/O
/// must happen outside this lock and revalidate generation/checksum afterwards.
static TABLE: Mutex<[Option<Snapshot>; MAX]> = Mutex::with_rank([None; MAX], 10);
static CHANGES: Mutex<[Option<CowRecord>; MAX_CHANGES]> =
    Mutex::with_rank([None; MAX_CHANGES], 10);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RestoreIntent {
    pub snapshot_id: u64,
    pub generation: u64,
    pub expected_root: u64,
    pub applied: usize,
}

static RESTORE_INTENT: Mutex<Option<RestoreIntent>> = Mutex::with_rank(None, 10);

fn durable_catalog() -> crate::fat32::SnapshotCatalog {
    let table = TABLE.lock();
    let changes = CHANGES.lock();
    let mut catalog = crate::fat32::SnapshotCatalog::empty();
    for (dst, src) in catalog.snapshots.iter_mut().zip(table.iter()) {
        *dst = src.map(|entry| crate::fat32::SnapshotCatalogEntry {
            id: entry.id,
            generation: entry.generation,
            checksum: entry.checksum,
        });
    }
    for (dst, src) in catalog.changes.iter_mut().zip(changes.iter()) {
        *dst = src.map(|entry| crate::fat32::SnapshotCowEntry {
            snapshot_id: entry.snapshot_id,
            path_hash: entry.path_hash,
            old_checksum: entry.old_checksum,
            new_checksum: entry.new_checksum,
        });
    }
    catalog
}

fn persist_catalog() -> Result<(), RestoreError> {
    crate::storage::persist_snapshot_catalog(durable_catalog())
        .map_err(|_| RestoreError::PersistenceFailed)
}

pub fn recover_catalog() -> Result<bool, RestoreError> {
    let Some(catalog) = crate::storage::load_snapshot_catalog()
        .map_err(|_| RestoreError::PersistenceFailed)?
    else {
        return Ok(false);
    };
    let mut table_image = [None; MAX];
    let mut changes_image = [None; MAX_CHANGES];
    for (dst, src) in table_image.iter_mut().zip(catalog.snapshots.iter()) {
        *dst = src.map(|entry| Snapshot {
            id: entry.id,
            generation: entry.generation,
            checksum: entry.checksum,
        });
    }
    for (dst, src) in changes_image.iter_mut().zip(catalog.changes.iter()) {
        *dst = src.map(|entry| CowRecord {
            snapshot_id: entry.snapshot_id,
            path_hash: entry.path_hash,
            old_checksum: entry.old_checksum,
            new_checksum: entry.new_checksum,
        });
    }
    for change in changes_image.iter().flatten() {
        if !table_image
            .iter()
            .flatten()
            .any(|snapshot| snapshot.id == change.snapshot_id)
        {
            return Err(RestoreError::PersistenceFailed);
        }
    }
    *TABLE.lock() = table_image;
    *CHANGES.lock() = changes_image;
    Ok(true)
}

pub fn create(generation: u64, checksum: u64) -> Option<u64> {
    if generation == 0 || checksum == 0 {
        return None;
    }
    let mut t = TABLE.lock();
    let slot = t.iter_mut().position(|s| s.is_none())?;
    let id = slot as u64 + 1;
    t[slot] = Some(Snapshot { id, generation, checksum });
    drop(t);
    if persist_catalog().is_err() {
        let mut table = TABLE.lock();
        if table[slot].is_some_and(|entry| entry.id == id) {
            table[slot] = None;
        }
        return None;
    }
    Some(id)
}

/// Capture the current WovenFS metadata root as a snapshot generation.
pub fn capture_wovenfs(generation: u64) -> Option<u64> {
    create(generation, crate::wovenfs::root_checksum())
}

pub fn get(id: u64) -> Option<Snapshot> {
    TABLE.lock().iter().flatten().find(|s| s.id == id).copied()
}

/// Record the first pre-image checksum for a path after a snapshot. Repeated
/// writes update only the newest checksum, preserving the rollback pre-image.
pub fn record_change(
    snapshot_id: u64,
    path_hash: u64,
    old_checksum: u64,
    new_checksum: u64,
) -> Result<(), RestoreError> {
    if get(snapshot_id).is_none() {
        return Err(RestoreError::MissingSnapshot);
    }
    let mut changes = CHANGES.lock();
    let before = *changes;
    if let Some(change) = changes
        .iter_mut()
        .flatten()
        .find(|change| change.snapshot_id == snapshot_id && change.path_hash == path_hash)
    {
        change.new_checksum = new_checksum;
    } else {
        let Some(slot) = changes.iter_mut().find(|entry| entry.is_none()) else {
            return Err(RestoreError::ChangeLogFull);
        };
        *slot = Some(CowRecord {
            snapshot_id,
            path_hash,
            old_checksum,
            new_checksum,
        });
    }
    drop(changes);
    if persist_catalog().is_err() {
        *CHANGES.lock() = before;
        return Err(RestoreError::PersistenceFailed);
    }
    Ok(())
}

/// Record one mutation against every live snapshot. The bounded operation is
/// fail-closed: if any snapshot cannot retain its pre-image metadata, the
/// durable filesystem write must not proceed.
pub fn record_live_change(
    path_hash: u64,
    old_checksum: u64,
    new_checksum: u64,
) -> Result<(), RestoreError> {
    let ids = {
        let table = TABLE.lock();
        let mut ids = [0u64; MAX];
        let mut count = 0usize;
        for snapshot in table.iter().flatten() {
            ids[count] = snapshot.id;
            count += 1;
        }
        (ids, count)
    };
    for id in ids.0.into_iter().take(ids.1) {
        record_change(id, path_hash, old_checksum, new_checksum)?;
    }
    Ok(())
}

/// Begin a rollback only when the caller's durable root still matches the
/// snapshot root. This prevents restoring a catalog entry against unrelated
/// filesystem state. Returned generation becomes the rollback target.
#[cfg(feature = "stage12-4-test")]
pub fn begin_restore(id: u64, durable_checksum: u64) -> Result<u64, RestoreError> {
    let snapshot = get(id).ok_or(RestoreError::MissingSnapshot)?;
    if snapshot.checksum != durable_checksum {
        return Err(RestoreError::ChecksumMismatch);
    }
    Ok(snapshot.generation)
}

/// Persist the logical rollback intent before replaying any pre-images. A
/// reboot can query pending_restore() and resume from the applied index.
#[cfg(feature = "stage12-4-test")]
pub fn prepare_restore(id: u64, durable_checksum: u64) -> Result<RestoreIntent, RestoreError> {
    let generation = begin_restore(id, durable_checksum)?;
    let pending = RESTORE_INTENT.lock();
    if pending.is_some() {
        return Err(RestoreError::RestoreBusy);
    }
    let intent = RestoreIntent {
        snapshot_id: id,
        generation,
        expected_root: durable_checksum,
        applied: 0,
    };
    drop(pending);
    crate::storage::persist_snapshot_restore_intent(intent)
        .map_err(|_| RestoreError::PersistenceFailed)?;
    let mut pending = RESTORE_INTENT.lock();
    if pending.is_some() {
        return Err(RestoreError::RestoreBusy);
    }
    *pending = Some(intent);
    Ok(intent)
}

#[cfg(feature = "stage12-4-test")]
pub fn mark_restore_applied(id: u64) -> Result<RestoreIntent, RestoreError> {
    let pending = RESTORE_INTENT.lock();
    let Some(mut intent) = *pending else {
        return Err(RestoreError::MissingSnapshot);
    };
    if intent.snapshot_id != id {
        return Err(RestoreError::RestoreBusy);
    }
    intent.applied = intent.applied.saturating_add(1);
    drop(pending);
    crate::storage::persist_snapshot_restore_intent(intent)
        .map_err(|_| RestoreError::PersistenceFailed)?;
    let mut pending = RESTORE_INTENT.lock();
    let Some(current) = *pending else {
        return Err(RestoreError::MissingSnapshot);
    };
    if current.snapshot_id != id {
        return Err(RestoreError::RestoreBusy);
    }
    *pending = Some(intent);
    Ok(intent)
}

pub fn recover_restore() -> Result<Option<RestoreIntent>, RestoreError> {
    let durable = crate::storage::load_snapshot_restore_intent()
        .map_err(|_| RestoreError::PersistenceFailed)?;
    let mut pending = RESTORE_INTENT.lock();
    *pending = durable;
    Ok(durable)
}

#[cfg(feature = "stage12-4-test")]
pub fn pending_restore() -> Option<RestoreIntent> {
    *RESTORE_INTENT.lock()
}

#[cfg(feature = "stage12-4-test")]
pub fn commit_restore(id: u64, restored_root: u64) -> Result<u64, RestoreError> {
    let pending = RESTORE_INTENT.lock();
    let Some(intent) = *pending else {
        return Err(RestoreError::MissingSnapshot);
    };
    if intent.snapshot_id != id {
        return Err(RestoreError::RestoreBusy);
    }
    if intent.expected_root != restored_root {
        return Err(RestoreError::ChecksumMismatch);
    }
    drop(pending);
    crate::storage::clear_snapshot_restore_intent()
        .map_err(|_| RestoreError::PersistenceFailed)?;
    let mut pending = RESTORE_INTENT.lock();
    let Some(current) = *pending else {
        return Err(RestoreError::MissingSnapshot);
    };
    if current.snapshot_id != id || current.expected_root != restored_root {
        return Err(RestoreError::RestoreBusy);
    }
    *pending = None;
    Ok(intent.generation)
}

#[cfg(feature = "stage12-4-test")]
pub fn change(id: u64, path_hash: u64) -> Option<CowRecord> {
    CHANGES
        .lock()
        .iter()
        .flatten()
        .find(|change| change.snapshot_id == id && change.path_hash == path_hash)
        .copied()
}

/// Compatibility metadata query. Production rollback uses begin_restore plus
/// the COW records and commits only after durable filesystem replay succeeds.
#[cfg(feature = "stage12-4-test")]
pub fn restore(id: u64) -> Option<(u64, u64)> {
    get(id).map(|s| (s.generation, s.checksum))
}

#[cfg(feature = "stage12-4-test")]
pub fn remove(id: u64) -> bool {
    let table_before = *TABLE.lock();
    let changes_before = *CHANGES.lock();
    let mut t = TABLE.lock();
    let Some(s) = t.iter_mut().find(|s| s.is_some_and(|v| v.id == id)) else {
        return false;
    };
    *s = None;
    drop(t);
    let mut changes = CHANGES.lock();
    for change in changes.iter_mut() {
        if change.is_some_and(|record| record.snapshot_id == id) {
            *change = None;
        }
    }
    drop(changes);
    if persist_catalog().is_err() {
        *TABLE.lock() = table_before;
        *CHANGES.lock() = changes_before;
        return false;
    }
    true
}

#[cfg(feature = "stage12-4-test")]
pub fn structural_self_test() -> bool {
    let Some(id) = create(4, 99) else {
        return false;
    };
    if record_change(id, 0x10, 11, 12).is_err()
        || record_change(id, 0x10, 11, 13).is_err()
        || change(id, 0x10)
            != Some(CowRecord {
                snapshot_id: id,
                path_hash: 0x10,
                old_checksum: 11,
                new_checksum: 13,
            })
    {
        return false;
    }
    *TABLE.lock() = [None; MAX];
    *CHANGES.lock() = [None; MAX_CHANGES];
    if recover_catalog() != Ok(true)
        || get(id) != Some(Snapshot { id, generation: 4, checksum: 99 })
        || change(id, 0x10)
            != Some(CowRecord {
                snapshot_id: id,
                path_hash: 0x10,
                old_checksum: 11,
                new_checksum: 13,
            })
        || begin_restore(id, 98) != Err(RestoreError::ChecksumMismatch)
        || begin_restore(id, 99) != Ok(4)
        || prepare_restore(id, 99).is_err()
        || pending_restore().is_none()
        || mark_restore_applied(id).map(|intent| intent.applied) != Ok(1)
        || commit_restore(id, 98) != Err(RestoreError::ChecksumMismatch)
        || pending_restore().is_none()
        || commit_restore(id, 99) != Ok(4)
        || pending_restore().is_some()
        || restore(id) != Some((4, 99))
        || !remove(id)
        || get(id).is_some()
        || change(id, 0x10).is_some()
    {
        return false;
    }
    true
}
