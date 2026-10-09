//! Bounded metadata and extended-attribute side table.
//!
//! # Scope
//!
//! This is **not** a filesystem. It holds a fixed number of metadata records in
//! RAM, keyed by path, and loses them on reboot. There is no on-disk format, no
//! inode allocation, no directory structure, no block allocator and no data
//! storage. File data and durable metadata live in FAT32 — see
//! [`crate::fat32`] and the on-volume journal in [`crate::storage`].
//!
//! A native WovenHat filesystem remains open work; the gap is recorded in
//! `docs/stage1-12-production-gap-audit.md` as "Native WovenFS, journaling/COW
//! disk replay, full data rollback".
use crate::irq_lock::IrqMutex as Mutex;

const MAX: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    pub path_hash: u64,
    pub size: u64,
    pub mode: u32,
    pub created: u64,
    pub modified: u64,
    pub checksum: u64,
    pub xattrs: [u64; 4],
}

/// Composite identity for a record.
///
/// Matching on a single 64-bit hash is not safe here. Paths are unbounded
/// caller input, so two distinct paths that collide would silently share one
/// record — returning or overwriting another file's `mode` bits among the rest.
/// The key carries two hashes built with different offset bases and
/// multipliers, plus the path length, so a false match needs both independent
/// hashes and the length to agree.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PathKey {
    primary: u64,
    secondary: u64,
    length: u32,
}

impl PathKey {
    fn new(path: &str) -> Self {
        Self {
            primary: path.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            }),
            secondary: path.bytes().fold(0x9e37_79b9_7f4a_7c15, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0000_0100_0193)
            }),
            length: path.len() as u32,
        }
    }
}

#[derive(Clone, Copy)]
struct Record {
    key: PathKey,
    metadata: Metadata,
}

#[derive(Clone, Copy)]
struct State {
    entries: [Option<Record>; MAX],
}

impl State {
    const fn new() -> Self {
        Self {
            entries: [None; MAX],
        }
    }
}

/// Bounded metadata registry with IRQ-safe rank tracking.
static STATE: Mutex<State> = Mutex::with_rank(State::new(), 10);

fn checksum_of(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Insert or update the record for `path`.
///
/// Returns `false` when the table is full and `path` has no existing record.
pub fn record(path: &str, size: u64, mode: u32, now: u64, data: &[u8]) -> bool {
    let key = PathKey::new(path);
    let checksum = checksum_of(data);
    let mut state = STATE.lock();
    if let Some(existing) = state
        .entries
        .iter_mut()
        .flatten()
        .find(|entry| entry.key == key)
    {
        existing.metadata.size = size;
        existing.metadata.mode = mode;
        existing.metadata.modified = now;
        existing.metadata.checksum = checksum;
        return true;
    }
    let Some(slot) = state.entries.iter_mut().find(|entry| entry.is_none()) else {
        return false;
    };
    *slot = Some(Record {
        key,
        metadata: Metadata {
            path_hash: key.primary,
            size,
            mode,
            created: now,
            modified: now,
            checksum,
            xattrs: [0; 4],
        },
    });
    true
}

pub fn metadata(path: &str) -> Option<Metadata> {
    let key = PathKey::new(path);
    STATE
        .lock()
        .entries
        .iter()
        .flatten()
        .find(|entry| entry.key == key)
        .map(|entry| entry.metadata)
}

/// Drop the record for `path`, freeing its slot.
///
/// Without this the table could only ever fill: once every slot held a record,
/// `record` would refuse new paths and metadata would silently stop being
/// tracked.
pub fn forget(path: &str) -> bool {
    let key = PathKey::new(path);
    let mut state = STATE.lock();
    let Some(slot) = state
        .entries
        .iter_mut()
        .find(|entry| entry.is_some_and(|record| record.key == key))
    else {
        return false;
    };
    *slot = None;
    true
}

/// Number of records currently held.
pub fn len() -> usize {
    STATE.lock().entries.iter().flatten().count()
}

pub fn set_xattr(path: &str, index: usize, value: u64) -> bool {
    let key = PathKey::new(path);
    let mut state = STATE.lock();
    let Some(entry) = state
        .entries
        .iter_mut()
        .flatten()
        .find(|entry| entry.key == key)
    else {
        return false;
    };
    let Some(slot) = entry.metadata.xattrs.get_mut(index) else {
        return false;
    };
    *slot = value;
    true
}

pub fn xattr(path: &str, index: usize) -> Option<u64> {
    let key = PathKey::new(path);
    STATE
        .lock()
        .entries
        .iter()
        .flatten()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.metadata.xattrs.get(index).copied())
}

#[cfg(feature = "stage12-2-test")]
pub fn structural_self_test() -> bool {
    let _ = forget("/tmp/woven");
    let _ = forget("/tmp/other");

    let before = len();

    // Distinct paths must not share a record, and a key must not match on the
    // primary hash alone.
    let left = PathKey::new("/tmp/woven");
    let right = PathKey::new("/tmp/other");
    let forged = PathKey {
        primary: left.primary,
        secondary: right.secondary,
        length: left.length,
    };
    let keys_distinguished = left != right && left != forged && forged != right;

    let stored = record("/tmp/woven", 5, 0o644, 10, b"woven")
        && set_xattr("/tmp/woven", 0, 77)
        && metadata("/tmp/woven")
            .is_some_and(|m| m.size == 5 && m.checksum != 0 && m.created == 10 && m.mode == 0o644)
        && xattr("/tmp/woven", 0) == Some(77);

    // A second path must receive its own slot and must not disturb the first.
    let isolated = record("/tmp/other", 9, 0o600, 11, b"other")
        && metadata("/tmp/other").is_some_and(|m| m.size == 9 && m.mode == 0o600)
        && metadata("/tmp/woven").is_some_and(|m| m.size == 5 && m.mode == 0o644)
        && len() == before + 2;

    // A released slot must become reusable, which is what stops the table from
    // filling permanently.
    let reclaimed = forget("/tmp/other")
        && metadata("/tmp/other").is_none()
        && len() == before + 1
        && !forget("/tmp/other");

    let cleaned = forget("/tmp/woven") && len() == before;

    keys_distinguished && stored && isolated && reclaimed && cleaned
}
