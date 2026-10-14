//! Bounded in-memory intent tracker for the current boot.
//!
//! # This is not a write-ahead log and provides no crash safety
//!
//! Every record lives in the `LOG` array below, in RAM, and never reaches a
//! device. After a power loss or reset the table is empty, so [`recover`]
//! finds nothing to retire and replays nothing. Treating this module as a
//! durability mechanism would be a mistake.
//!
//! Crash safety for file replacement is provided by the on-volume data journal
//! in [`crate::storage`] — the `WOVENJR.BIN` record carrying the `WDJ` magic,
//! written to the volume with a prepared/committed state and a checksum, and
//! replayed by `recover_data_journal` during mount. That is the mechanism to
//! extend when durability requirements grow.
//!
//! What this module does provide is a bounded, same-boot record of which
//! replacement intents were started and which completed, so a diagnostic
//! caller can observe that an operation was abandoned part-way through within
//! the current uptime.
use crate::irq_lock::IrqMutex as Mutex;

const MAX: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Token(u64);

/// Composite identity for a tracked path.
///
/// A single 64-bit hash is not sufficient to identify a record. Paths are
/// unbounded caller input, and matching on one hash means two distinct paths
/// that collide silently share an entry — the earlier record is returned or
/// overwritten for the wrong file. The key therefore carries two hashes built
/// with different offset bases and multipliers, plus the path length. A false
/// match requires both independent hashes and the length to agree.
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
struct Entry {
    token: Token,
    key: PathKey,
    checksum: u64,
    committed: bool,
}

static LOG: Mutex<[Option<Entry>; MAX]> = Mutex::with_rank([None; MAX], 10);
static NEXT: Mutex<u64> = Mutex::with_rank(1, 10);

/// Begin tracking a replacement intent for `path`.
///
/// Returns `None` when the bounded table is full, or when an unfinished intent
/// for the same path is already outstanding. Refusing the duplicate keeps one
/// owner per in-flight replacement, so a second caller cannot retire the
/// first caller's intent with its own token.
pub fn begin(path: &str, checksum: u64) -> Option<Token> {
    let key = PathKey::new(path);
    let mut log = LOG.lock();
    if log
        .iter()
        .flatten()
        .any(|entry| entry.key == key && !entry.committed)
    {
        return None;
    }
    let slot = log.iter_mut().position(|entry| entry.is_none())?;
    let mut next = NEXT.lock();
    let token = Token(*next);
    *next = next.saturating_add(1);
    log[slot] = Some(Entry {
        token,
        key,
        checksum,
        committed: false,
    });
    Some(token)
}

/// Mark the intent owned by `token` as complete.
///
/// `checksum` must equal the value recorded by [`begin`]. A mismatch means the
/// bytes that were committed are not the bytes the intent described, so the
/// entry is left uncommitted and reported as abandoned rather than closed.
pub fn commit(token: Token, checksum: u64) -> bool {
    let mut log = LOG.lock();
    let Some(entry) = log.iter_mut().flatten().find(|entry| entry.token == token) else {
        return false;
    };
    if entry.checksum != checksum {
        return false;
    }
    entry.committed = true;
    true
}

/// Retire every tracked intent and report how many were still uncommitted.
///
/// Both committed and abandoned entries are cleared: a committed entry has
/// nothing left to do, and an abandoned one cannot be replayed because this
/// table holds no durable record of the previous bytes. The return value is
/// advisory only — it says how many intents did not finish during this uptime.
pub fn recover() -> usize {
    let mut log = LOG.lock();
    let mut abandoned = 0;
    for slot in log.iter_mut() {
        if let Some(entry) = slot.take() {
            if !entry.committed {
                abandoned += 1;
            }
        }
    }
    abandoned
}

/// Number of intents currently tracked, committed or not.
#[cfg(feature = "stage1-5-test")]
pub fn tracked() -> usize {
    LOG.lock().iter().flatten().count()
}

#[cfg(feature = "stage1-5-test")]
pub fn structural_self_test() -> bool {
    recover();

    // Distinct paths must not share an entry, and a key must not match on the
    // primary hash alone.
    let left = PathKey::new("/mnt/alpha");
    let right = PathKey::new("/mnt/beta");
    let forged = PathKey {
        primary: left.primary,
        secondary: right.secondary,
        length: left.length,
    };
    let keys_distinguished = left != right && left != forged && forged != right;

    let Some(first) = begin("/mnt/alpha", 22) else {
        return false;
    };
    let Some(second) = begin("/mnt/beta", 44) else {
        return false;
    };
    let both_tracked = tracked() == 2;

    // Committing one intent must leave exactly one abandoned, and a retired
    // token must no longer be committable.
    // A duplicate intent for a path already in flight must be refused.
    let duplicate_refused = begin("/mnt/alpha", 22).is_none();

    // Committing with the wrong checksum must not close the entry.
    let mismatch_refused = !commit(second, 45) && commit(second, 44);
    let abandoned = recover();

    keys_distinguished
        && both_tracked
        && duplicate_refused
        && mismatch_refused
        && abandoned == 1
        && tracked() == 0
        && !commit(first, 22)
}
