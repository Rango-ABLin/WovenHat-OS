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
    ChecksumMismatch,
    ChangeLogFull,
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

/// Serializes catalog state transitions across the in-memory update and the
/// following durable A/B WSC1 write. Rank 9 is acquired before the rank-10
/// TABLE/CHANGES locks; filesystem I/O happens while no rank-10 lock is held.
static CATALOG_TRANSACTION: Mutex<()> = Mutex::with_rank((), 9);
/// Monotonic snapshot identity source. Recovered catalogs advance this floor so
/// a deleted slot is never reissued the same WHSNAP namespace in one boot.
static NEXT_SNAPSHOT_ID: Mutex<u64> = Mutex::with_rank(1, 10);

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
    let _transaction = CATALOG_TRANSACTION.lock();
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
    let next_id = table_image
        .iter()
        .flatten()
        .map(|snapshot| snapshot.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RestoreError::PersistenceFailed)?;
    *TABLE.lock() = table_image;
    *CHANGES.lock() = changes_image;
    *NEXT_SNAPSHOT_ID.lock() = next_id.max(1);
    Ok(true)
}

pub fn create(generation: u64, checksum: u64) -> Option<u64> {
    if generation == 0 || checksum == 0 {
        return None;
    }
    let _transaction = CATALOG_TRANSACTION.lock();
    let mut t = TABLE.lock();
    let slot = t.iter_mut().position(|s| s.is_none())?;
    let mut next_id = NEXT_SNAPSHOT_ID.lock();
    let id = *next_id;
    if id == 0 || t.iter().flatten().any(|snapshot| snapshot.id == id) {
        return None;
    }
    let successor = id.checked_add(1)?;
    *next_id = successor;
    drop(next_id);
    t[slot] = Some(Snapshot { id, generation, checksum });
    drop(t);
    if persist_catalog().is_err() {
        let mut table = TABLE.lock();
        if table[slot].is_some_and(|entry| entry.id == id) {
            table[slot] = None;
        }
        // Safe because the catalog transaction lock excludes another creator:
        // a failed durable create never publishes this identity.
        *NEXT_SNAPSHOT_ID.lock() = id;
        return None;
    }
    Some(id)
}

/// Capture the current WovenFS metadata root as a snapshot generation.
pub fn capture_wovenfs(generation: u64) -> Option<u64> {
    create(generation, crate::wovenfs::root_checksum())
}

pub fn live_snapshot_ids() -> ([u64; MAX], usize) {
    let table = TABLE.lock();
    let mut ids = [0u64; MAX];
    let mut count = 0usize;
    for snapshot in table.iter().flatten() {
        ids[count] = snapshot.id;
        count += 1;
    }
    (ids, count)
}

pub fn get(id: u64) -> Option<Snapshot> {
    TABLE.lock().iter().flatten().find(|s| s.id == id).copied()
}

/// Record the first pre-image checksum for a path after a snapshot. Repeated
/// writes update only the newest checksum, preserving the rollback pre-image.
#[cfg(feature = "stage12-4-test")]
pub fn record_change(
    snapshot_id: u64,
    path_hash: u64,
    old_checksum: u64,
    new_checksum: u64,
) -> Result<(), RestoreError> {
    let _transaction = CATALOG_TRANSACTION.lock();
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
    record_live_changes(&[(path_hash, old_checksum, new_checksum)])
}

/// Atomically stage a logical filesystem mutation against every live snapshot.
/// All COW records are installed under one lock and persisted as one catalog
/// image. Capacity or persistence failure restores the complete prior table.
pub fn record_live_changes(
    mutations: &[(u64, u64, u64)],
) -> Result<(), RestoreError> {
    let _transaction = CATALOG_TRANSACTION.lock();
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
    if ids.1 == 0 || mutations.is_empty() {
        return Ok(());
    }

    let mut changes = CHANGES.lock();
    let before = *changes;
    for id in ids.0.into_iter().take(ids.1) {
        for &(path_hash, old_checksum, new_checksum) in mutations {
            if let Some(change) = changes
                .iter_mut()
                .flatten()
                .find(|change| change.snapshot_id == id && change.path_hash == path_hash)
            {
                change.new_checksum = new_checksum;
                continue;
            }
            let Some(slot) = changes.iter_mut().find(|entry| entry.is_none()) else {
                *changes = before;
                return Err(RestoreError::ChangeLogFull);
            };
            *slot = Some(CowRecord {
                snapshot_id: id,
                path_hash,
                old_checksum,
                new_checksum,
            });
        }
    }
    drop(changes);
    if persist_catalog().is_err() {
        *CHANGES.lock() = before;
        return Err(RestoreError::PersistenceFailed);
    }
    Ok(())
}

/// Begin a rollback only when the caller's durable root still matches the
/// snapshot root. This prevents restoring a catalog entry against unrelated
/// filesystem state. Returned generation becomes the rollback target.
pub fn begin_restore(id: u64) -> Result<Snapshot, RestoreError> {
    get(id).ok_or(RestoreError::MissingSnapshot)
}

/// Persist the logical rollback intent before replaying any pre-images. A
/// reboot can query pending_restore() and resume from the applied index.
pub fn prepare_restore(id: u64) -> Result<RestoreIntent, RestoreError> {
    let snapshot = begin_restore(id)?;
    let pending = RESTORE_INTENT.lock();
    if pending.is_some() {
        return Err(RestoreError::RestoreBusy);
    }
    let intent = RestoreIntent {
        snapshot_id: id,
        generation: snapshot.generation,
        expected_root: snapshot.checksum,
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

pub fn replay_pending_restore() -> Result<Option<u64>, RestoreError> {
    let Some(intent) = recover_restore()? else {
        return Ok(None);
    };
    let snapshot = get(intent.snapshot_id).ok_or(RestoreError::MissingSnapshot)?;
    if snapshot.generation != intent.generation || snapshot.checksum != intent.expected_root {
        return Err(RestoreError::ChecksumMismatch);
    }
    let mut plan = [None; MAX_CHANGES];
    let mut count = 0usize;
    {
        let changes = CHANGES.lock();
        for record in changes.iter().flatten().filter(|record| record.snapshot_id == intent.snapshot_id) {
            plan[count] = Some(*record);
            count += 1;
        }
    }
    if intent.applied > count {
        return Err(RestoreError::PersistenceFailed);
    }
    // Validate every remaining durable pre-image before mutating the live
    // filesystem. This prevents a corrupt later WHP1 record from leaving a
    // newly-started rollback partially applied. On reboot, records below the
    // durable WSR1 applied index are intentionally skipped and the remainder
    // is validated again before replay resumes.
    let mut preimages = alloc::vec::Vec::with_capacity(count.saturating_sub(intent.applied));
    for record in plan.into_iter().flatten().skip(intent.applied) {
        let preimage = crate::storage::load_snapshot_preimage(record.snapshot_id, record.path_hash)
            .map_err(|_| RestoreError::PersistenceFailed)?;
        if preimage.checksum != record.old_checksum {
            return Err(RestoreError::ChecksumMismatch);
        }
        preimages.push(preimage);
    }
    preimages.sort_by(|left, right| {
        left.replay_phase()
            .cmp(&right.replay_phase())
            .then_with(|| {
                if left.replay_phase() == 2 {
                    right.path_depth().cmp(&left.path_depth())
                } else {
                    left.path_depth().cmp(&right.path_depth())
                }
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    for preimage in &preimages {
        crate::storage::replay_snapshot_preimage(preimage)
            .map_err(|_| RestoreError::PersistenceFailed)?;
        mark_restore_applied(intent.snapshot_id)?;
    }
    Ok(Some(intent.expected_root))
}

pub fn pending_restore() -> Option<RestoreIntent> {
    *RESTORE_INTENT.lock()
}

pub fn resume_pending_restore() -> Result<Option<u64>, RestoreError> {
    let Some(expected_root) = replay_pending_restore()? else { return Ok(None); };
    let intent = pending_restore().ok_or(RestoreError::MissingSnapshot)?;
    let restored_root = crate::wovenfs::root_checksum();
    if restored_root != expected_root { return Err(RestoreError::ChecksumMismatch); }
    commit_restore(intent.snapshot_id, restored_root).map(Some)
}

pub fn restore_now(id: u64) -> Result<u64, RestoreError> {
    prepare_restore(id)?;
    resume_pending_restore()?.ok_or(RestoreError::PersistenceFailed)
}

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
    let _transaction = CATALOG_TRANSACTION.lock();
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
        || begin_restore(id).map(|snapshot| snapshot.generation) != Ok(4)
        || prepare_restore(id).is_err()
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
