//! Bounded WovenFS metadata, checksums, and snapshot records.
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
#[derive(Clone, Copy)]
struct State {
    entries: [Option<Metadata>; MAX],
}
impl State {
    const fn new() -> Self {
        Self {
            entries: [None; MAX],
        }
    }
}
/// Bounded WovenFS metadata registry with IRQ-safe rank tracking.
static STATE: Mutex<State> = Mutex::with_rank(State::new(), 10);
pub fn path_hash(path: &str) -> u64 {
    hash(path)
}

pub fn root_checksum() -> u64 {
    let s = STATE.lock();
    s.entries.iter().flatten().fold(0xcbf29ce484222325, |acc, entry| {
        acc.wrapping_mul(0x100000001b3)
            ^ entry.path_hash
            ^ entry.size
            ^ u64::from(entry.mode)
            ^ entry.checksum
    })
}

fn hash(path: &str) -> u64 {
    path.bytes().fold(1469598103934665603, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(1099511628211)
    })
}
pub fn record(path: &str, size: u64, mode: u32, now: u64, data: &[u8]) -> bool {
    let key = hash(path);
    let checksum = data.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    let mut s = STATE.lock();
    if let Some(entry) = s.entries.iter_mut().flatten().find(|e| e.path_hash == key) {
        entry.size = size;
        entry.mode = mode;
        entry.modified = now;
        entry.checksum = checksum;
        return true;
    }
    let Some(slot) = s.entries.iter_mut().find(|e| e.is_none()) else {
        return false;
    };
    *slot = Some(Metadata {
        path_hash: key,
        size,
        mode,
        created: now,
        modified: now,
        checksum,
        xattrs: [0; 4],
    });
    true
}
pub fn remove(path: &str) -> bool {
    let key = hash(path);
    let mut state = STATE.lock();
    let Some(slot) = state
        .entries
        .iter_mut()
        .find(|entry| entry.is_some_and(|metadata| metadata.path_hash == key))
    else {
        return false;
    };
    *slot = None;
    true
}

pub fn rename(old: &str, new: &str) -> bool {
    let old_key = hash(old);
    let new_key = hash(new);
    let mut state = STATE.lock();
    if state
        .entries
        .iter()
        .flatten()
        .any(|metadata| metadata.path_hash == new_key)
    {
        return false;
    }
    let Some(metadata) = state
        .entries
        .iter_mut()
        .flatten()
        .find(|metadata| metadata.path_hash == old_key)
    else {
        return false;
    };
    metadata.path_hash = new_key;
    true
}

pub fn metadata(path: &str) -> Option<Metadata> {
    STATE
        .lock()
        .entries
        .iter()
        .flatten()
        .find(|e| e.path_hash == hash(path))
        .copied()
}
#[cfg(any(feature = "stage12-2-test", feature = "stage12-4-test"))]
pub fn set_xattr(path: &str, index: usize, value: u64) -> bool {
    let mut s = STATE.lock();
    let Some(e) = s
        .entries
        .iter_mut()
        .flatten()
        .find(|e| e.path_hash == hash(path))
    else {
        return false;
    };
    let Some(x) = e.xattrs.get_mut(index) else {
        return false;
    };
    *x = value;
    true
}
#[cfg(any(feature = "stage12-2-test", feature = "stage12-4-test"))]
pub fn xattr(path: &str, index: usize) -> Option<u64> {
    STATE
        .lock()
        .entries
        .iter()
        .flatten()
        .find(|e| e.path_hash == hash(path))
        .and_then(|e| e.xattrs.get(index).copied())
}
#[cfg(feature = "stage12-2-test")]
pub fn structural_self_test() -> bool {
    record("/tmp/woven", 5, 0o644, 10, b"woven")
        && set_xattr("/tmp/woven", 0, 77)
        && metadata("/tmp/woven")
            .is_some_and(|m| m.size == 5 && m.checksum != 0 && m.created == 10 && m.mode == 0o644)
        && xattr("/tmp/woven", 0) == Some(77)
}
