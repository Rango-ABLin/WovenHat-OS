//! Bounded copy-on-write snapshot catalog for filesystem generations.
use crate::irq_lock::IrqMutex as Mutex;

const MAX: usize = 8;
const PAGE_SIZE: usize = 512;
const PAGES: usize = 8;
const POOL: usize = PAGES * 3;
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
    catalog.next_snapshot_id = *NEXT_SNAPSHOT_ID.lock();
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
    let derived_next_id = table_image
        .iter()
        .flatten()
        .map(|snapshot| snapshot.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RestoreError::PersistenceFailed)?
        .max(1);
    if catalog.next_snapshot_id < derived_next_id {
        return Err(RestoreError::PersistenceFailed);
    }
    *TABLE.lock() = table_image;
    *CHANGES.lock() = changes_image;
    *NEXT_SNAPSHOT_ID.lock() = catalog.next_snapshot_id;
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SnapshotMembership {
    ids: [u64; MAX],
    count: usize,
    next_snapshot_id: u64,
}

impl SnapshotMembership {
    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.ids[..self.count].iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

pub fn snapshot_membership() -> SnapshotMembership {
    let (ids, count) = {
        let table = TABLE.lock();
        let mut ids = [0u64; MAX];
        let mut count = 0usize;
        for snapshot in table.iter().flatten() {
            ids[count] = snapshot.id;
            count += 1;
        }
        (ids, count)
    };
    // TABLE and NEXT_SNAPSHOT_ID share rank 10. Never nest their guards:
    // callers that require an atomic membership decision revalidate this
    // token while holding CATALOG_TRANSACTION before publishing COW state.
    let next_snapshot_id = *NEXT_SNAPSHOT_ID.lock();
    SnapshotMembership {
        ids,
        count,
        next_snapshot_id,
    }
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

/// Verify that the current COW table can represent every mutation for the
/// captured membership before storage writes any WHP1 pre-image. Existing
/// snapshot/path records consume no additional slot because publication only
/// advances their newest checksum.
pub fn preflight_live_changes_for_membership(
    membership: SnapshotMembership,
    mutations: &[(u64, u64, u64)],
) -> Result<(), RestoreError> {
    let _transaction = CATALOG_TRANSACTION.lock();
    if snapshot_membership() != membership {
        return Err(RestoreError::RestoreBusy);
    }
    if membership.is_empty() || mutations.is_empty() {
        return Ok(());
    }

    let changes = CHANGES.lock();
    let free = changes.iter().filter(|entry| entry.is_none()).count();
    let mut required = 0usize;
    for id in membership.ids() {
        for &(path_hash, _, _) in mutations {
            let already_present = changes
                .iter()
                .flatten()
                .any(|change| change.snapshot_id == id && change.path_hash == path_hash);
            if already_present {
                continue;
            }
            // A repeated path in the same mutation batch needs only one slot
            // for this snapshot, matching record_live_changes_for_membership.
            let duplicate_in_batch = mutations
                .iter()
                .take_while(|mutation| mutation.0 != path_hash)
                .any(|mutation| mutation.0 == path_hash);
            if !duplicate_in_batch {
                required = required
                    .checked_add(1)
                    .ok_or(RestoreError::ChangeLogFull)?;
            }
        }
    }
    if required > free {
        return Err(RestoreError::ChangeLogFull);
    }
    Ok(())
}

/// Atomically publish a logical filesystem mutation for the exact snapshot
/// membership that retained its WHP1 pre-images. Membership changes fail
/// closed before the underlying FAT mutation can proceed.
pub fn record_live_changes_for_membership(
    membership: SnapshotMembership,
    mutations: &[(u64, u64, u64)],
) -> Result<(), RestoreError> {
    let _transaction = CATALOG_TRANSACTION.lock();
    let current = snapshot_membership();
    if current != membership {
        return Err(RestoreError::RestoreBusy);
    }
    if membership.is_empty() || mutations.is_empty() {
        return Ok(());
    }

    let mut changes = CHANGES.lock();
    let before = *changes;
    for id in membership.ids() {
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
    // Load and validate the complete durable plan before interpreting the
    // WSR1 applied cursor. The cursor indexes the deterministic replay order,
    // not the physical/catalog insertion order. Sorting only the remaining
    // suffix would make a reboot capable of skipping a different object.
    let mut preimages = alloc::vec::Vec::with_capacity(count);
    for record in plan.into_iter().flatten() {
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
    if intent.applied > preimages.len() {
        return Err(RestoreError::PersistenceFailed);
    }
    for preimage in preimages.iter().skip(intent.applied) {
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

/// Bounded copy-on-write image used by the native filesystem snapshot
/// boundary. Each image owns eight fixed sectors; snapshots retain sector
/// references and a write clones only the shared sector.
pub struct CowImage {
    pages: [[u8; PAGE_SIZE]; POOL],
    refs: [u8; POOL],
    live: [usize; PAGES],
    snapshots: [Option<[usize; PAGES]>; MAX],
}

impl CowImage {
    pub const fn new() -> Self {
        let mut image = Self {
            pages: [[0; PAGE_SIZE]; POOL],
            refs: [0; POOL],
            live: [0; PAGES],
            snapshots: [None; MAX],
        };
        let mut page = 0;
        while page < PAGES {
            image.live[page] = page;
            image.refs[page] = 1;
            page += 1;
        }
        image
    }

    pub fn write_page(&mut self, page: usize, data: &[u8; PAGE_SIZE]) -> bool {
        if page >= PAGES {
            return false;
        }
        let old = self.live[page];
        let target = if self.refs[old] > 1 {
            let Some(free) = self.refs.iter().position(|count| *count == 0) else {
                return false;
            };
            self.pages[free] = self.pages[old];
            self.refs[old] -= 1;
            self.refs[free] = 1;
            self.live[page] = free;
            free
        } else {
            old
        };
        self.pages[target] = *data;
        true
    }

    pub fn read_page(&self, page: usize, out: &mut [u8; PAGE_SIZE]) -> bool {
        if page >= PAGES {
            return false;
        }
        *out = self.pages[self.live[page]];
        true
    }

    pub fn snapshot(&mut self) -> Option<u64> {
        let slot = self.snapshots.iter().position(|entry| entry.is_none())?;
        let roots = self.live;
        for &root in &roots {
            self.refs[root] = self.refs[root].saturating_add(1);
        }
        self.snapshots[slot] = Some(roots);
        Some(slot as u64 + 1)
    }

    pub fn restore(&mut self, id: u64) -> bool {
        let slot = usize::try_from(id.checked_sub(1).unwrap_or(u64::MAX)).ok();
        let Some(slot) = slot.filter(|slot| *slot < MAX) else {
            return false;
        };
        let Some(roots) = self.snapshots[slot] else {
            return false;
        };
        for &root in &self.live {
            self.refs[root] = self.refs[root].saturating_sub(1);
        }
        for &root in &roots {
            self.refs[root] = self.refs[root].saturating_add(1);
        }
        self.live = roots;
        true
    }

    pub fn remove_snapshot(&mut self, id: u64) -> bool {
        let slot = usize::try_from(id.checked_sub(1).unwrap_or(u64::MAX)).ok();
        let Some(slot) = slot.filter(|slot| *slot < MAX) else {
            return false;
        };
        let Some(roots) = self.snapshots[slot].take() else {
            return false;
        };
        for &root in &roots {
            self.refs[root] = self.refs[root].saturating_sub(1);
        }
        true
    }
}

impl Default for CowImage {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(feature = "stage12-4-test")]
pub fn structural_self_test() -> bool {
    let Some(id) = create(4, 99) else {
        return false;
    };
    get(id)
        == Some(Snapshot {
            id,
            generation: 4,
            checksum: 99,
        })
        && restore(id) == Some((4, 99))
        && remove(id)
        && get(id).is_none()
        && cow_self_test()
}

#[cfg(feature = "stage12-4-test")]
fn cow_self_test() -> bool {
    let mut image = CowImage::new();
    let mut first = [0u8; PAGE_SIZE];
    first[0] = 7;
    if !image.write_page(0, &first) {
        return false;
    }
    let Some(snapshot) = image.snapshot() else {
        return false;
    };
    let mut second = first;
    second[0] = 9;
    if !image.write_page(0, &second) {
        return false;
    }
    let mut read = [0u8; PAGE_SIZE];
    if !image.read_page(0, &mut read) || read[0] != 9 {
        return false;
    }
    if !image.restore(snapshot) || !image.read_page(0, &mut read) || read[0] != 7 {
        return false;
    }
    image.remove_snapshot(snapshot) && image.read_page(0, &mut read) && read[0] == 7
}
