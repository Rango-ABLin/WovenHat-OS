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
}

/// Rank 10 protects short snapshot metadata operations only. Filesystem I/O
/// must happen outside this lock and revalidate generation/checksum afterwards.
static TABLE: Mutex<[Option<Snapshot>; MAX]> = Mutex::with_rank([None; MAX], 10);
static CHANGES: Mutex<[Option<CowRecord>; MAX_CHANGES]> =
    Mutex::with_rank([None; MAX_CHANGES], 10);

pub fn create(generation: u64, checksum: u64) -> Option<u64> {
    if generation == 0 || checksum == 0 {
        return None;
    }
    let mut t = TABLE.lock();
    let slot = t.iter_mut().position(|s| s.is_none())?;
    let id = slot as u64 + 1;
    t[slot] = Some(Snapshot { id, generation, checksum });
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
    if let Some(change) = changes
        .iter_mut()
        .flatten()
        .find(|change| change.snapshot_id == snapshot_id && change.path_hash == path_hash)
    {
        change.new_checksum = new_checksum;
        return Ok(());
    }
    let Some(slot) = changes.iter_mut().find(|entry| entry.is_none()) else {
        return Err(RestoreError::ChangeLogFull);
    };
    *slot = Some(CowRecord {
        snapshot_id,
        path_hash,
        old_checksum,
        new_checksum,
    });
    Ok(())
}

/// Begin a rollback only when the caller's durable root still matches the
/// snapshot root. This prevents restoring a catalog entry against unrelated
/// filesystem state. Returned generation becomes the rollback target.
pub fn begin_restore(id: u64, durable_checksum: u64) -> Result<u64, RestoreError> {
    let snapshot = get(id).ok_or(RestoreError::MissingSnapshot)?;
    if snapshot.checksum != durable_checksum {
        return Err(RestoreError::ChecksumMismatch);
    }
    Ok(snapshot.generation)
}

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
pub fn restore(id: u64) -> Option<(u64, u64)> {
    get(id).map(|s| (s.generation, s.checksum))
}

pub fn remove(id: u64) -> bool {
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
        || begin_restore(id, 98) != Err(RestoreError::ChecksumMismatch)
        || begin_restore(id, 99) != Ok(4)
        || restore(id) != Some((4, 99))
        || !remove(id)
        || get(id).is_some()
        || change(id, 0x10).is_some()
    {
        return false;
    }
    true
}
