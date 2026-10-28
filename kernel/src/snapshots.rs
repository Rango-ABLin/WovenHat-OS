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
/// Bounded snapshot catalog. Rank 10 keeps catalog updates interrupt-safe;
/// snapshot records contain no references that require a lower-ranked lock.
static TABLE: Mutex<[Option<Snapshot>; MAX]> = Mutex::with_rank([None; MAX], 10);
pub fn create(generation: u64, checksum: u64) -> Option<u64> {
    let mut t = TABLE.lock();
    let slot = t.iter_mut().position(|s| s.is_none())?;
    let id = slot as u64 + 1;
    t[slot] = Some(Snapshot {
        id,
        generation,
        checksum,
    });
    Some(id)
}
pub fn get(id: u64) -> Option<Snapshot> {
    TABLE.lock().iter().flatten().find(|s| s.id == id).copied()
}
pub fn restore(id: u64) -> Option<(u64, u64)> {
    get(id).map(|s| (s.generation, s.checksum))
}
pub fn remove(id: u64) -> bool {
    let mut t = TABLE.lock();
    if let Some(s) = t.iter_mut().find(|s| s.is_some_and(|v| v.id == id)) {
        *s = None;
        true
    } else {
        false
    }
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
        if page >= PAGES { return false; }
        let old = self.live[page];
        let target = if self.refs[old] > 1 {
            let Some(free) = self.refs.iter().position(|count| *count == 0) else { return false; };
            self.pages[free] = self.pages[old];
            self.refs[old] -= 1;
            self.refs[free] = 1;
            self.live[page] = free;
            free
        } else { old };
        self.pages[target] = *data;
        true
    }

    pub fn read_page(&self, page: usize, out: &mut [u8; PAGE_SIZE]) -> bool {
        if page >= PAGES { return false; }
        *out = self.pages[self.live[page]];
        true
    }

    pub fn snapshot(&mut self) -> Option<u64> {
        let slot = self.snapshots.iter().position(|entry| entry.is_none())?;
        let roots = self.live;
        for &root in &roots { self.refs[root] = self.refs[root].saturating_add(1); }
        self.snapshots[slot] = Some(roots);
        Some(slot as u64 + 1)
    }

    pub fn restore(&mut self, id: u64) -> bool {
        let slot = usize::try_from(id.checked_sub(1).unwrap_or(u64::MAX)).ok();
        let Some(slot) = slot.filter(|slot| *slot < MAX) else { return false; };
        let Some(roots) = self.snapshots[slot] else { return false; };
        for &root in &self.live { self.refs[root] = self.refs[root].saturating_sub(1); }
        for &root in &roots { self.refs[root] = self.refs[root].saturating_add(1); }
        self.live = roots;
        true
    }

    pub fn remove_snapshot(&mut self, id: u64) -> bool {
        let slot = usize::try_from(id.checked_sub(1).unwrap_or(u64::MAX)).ok();
        let Some(slot) = slot.filter(|slot| *slot < MAX) else { return false; };
        let Some(roots) = self.snapshots[slot].take() else { return false; };
        for &root in &roots { self.refs[root] = self.refs[root].saturating_sub(1); }
        true
    }
}

impl Default for CowImage { fn default() -> Self { Self::new() } }
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
    if !image.write_page(0, &first) { return false; }
    let Some(snapshot) = image.snapshot() else { return false; };
    let mut second = first;
    second[0] = 9;
    if !image.write_page(0, &second) { return false; }
    let mut read = [0u8; PAGE_SIZE];
    if !image.read_page(0, &mut read) || read[0] != 9 { return false; }
    if !image.restore(snapshot) || !image.read_page(0, &mut read) || read[0] != 7 { return false; }
    image.remove_snapshot(snapshot) && image.read_page(0, &mut read) && read[0] == 7
}
