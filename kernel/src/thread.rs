//! Bounded process-thread registry used by the Stage 11.2 runtime boundary.
use crate::irq_lock::IrqMutex as Mutex;
use crate::userspace::AnonymousMapping;

pub const MAX_THREADS: usize = 64;
pub const MAX_ROBUST_ENTRIES: usize = 16;
const MAX_ROBUST_RECORDS: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ThreadId(u64);

impl ThreadId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
    const fn new(slot: usize, generation: u32) -> Self {
        Self(((generation as u64) << 32) | slot as u64)
    }
}

#[derive(Clone, Copy)]
pub struct Resources {
    pub stack: AnonymousMapping,
    pub tls: Option<AnonymousMapping>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Free,
    Running,
    Exited,
}

#[derive(Clone, Copy)]
struct Record {
    state: State,
    generation: u32,
    owner: u64,
    exit: i32,
    errno: i32,
    tls: u64,
    resources: Option<Resources>,
    waiter: Option<u64>,
    robust_count: u8,
    robust_mutexes: [u64; MAX_ROBUST_ENTRIES],
}

impl Record {
    const EMPTY: Self = Self {
        state: State::Free,
        generation: 1,
        owner: 0,
        exit: 0,
        errno: 0,
        tls: 0,
        resources: None,
        waiter: None,
        robust_count: 0,
        robust_mutexes: [0; MAX_ROBUST_ENTRIES],
    };

    fn reusable(&self) -> bool {
        // MAX is a permanent retirement sentinel. Saturating release paths
        // must never make an exhausted slot allocatable under the same tag.
        self.state == State::Free && self.generation != u32::MAX
    }
}

struct Table {
    records: [Record; MAX_THREADS],
}
impl Table {
    const fn new() -> Self {
        Self {
            records: [Record::EMPTY; MAX_THREADS],
        }
    }
}
static TABLE: Mutex<Table> = Mutex::with_rank(Table::new(), 10);

#[derive(Clone, Copy, PartialEq, Eq)]
enum RobustState { Free, Consistent, Abandoned, Unrecoverable }
#[derive(Clone, Copy)]
struct RobustRecord { address: u64, owner: Option<ThreadId>, generation: u32, state: RobustState }
static ROBUST: Mutex<[RobustRecord; MAX_ROBUST_RECORDS]> = Mutex::with_rank(
    [RobustRecord { address: 0, owner: None, generation: 1, state: RobustState::Free }; MAX_ROBUST_RECORDS], 11);

fn lookup(table: &mut Table, id: ThreadId) -> Option<&mut Record> {
    let slot = (id.0 & 0xffff_ffff) as usize;
    let generation = (id.0 >> 32) as u32;
    let record = table.records.get_mut(slot)?;
    (record.generation == generation && record.state != State::Free).then_some(record)
}

pub fn create(owner: u64) -> Option<ThreadId> {
    let mut table = TABLE.lock();
    let (slot, record) = table
        .records
        .iter_mut()
        .enumerate()
        .find(|(_, r)| r.reusable())?;
    record.state = State::Running;
    record.owner = owner;
    record.exit = 0;
    record.errno = 0;
    record.tls = 0;
    record.resources = None;
    record.waiter = None;
    record.robust_count = 0;
    record.robust_mutexes = [0; MAX_ROBUST_ENTRIES];
    Some(ThreadId::new(slot, record.generation))
}

pub fn register_robust_mutex(id: ThreadId, address: u64) -> bool {
    if address == 0 || !address.is_multiple_of(core::mem::align_of::<u32>() as u64) {
        return false;
    }
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else { return false; };
    if record.robust_mutexes[..record.robust_count as usize].contains(&address) {
        return true;
    }
    let index = record.robust_count as usize;
    if index >= MAX_ROBUST_ENTRIES { return false; }
    record.robust_mutexes[index] = address;
    record.robust_count += 1;
    true
}

pub fn unregister_robust_mutex(id: ThreadId, address: u64) -> bool {
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else { return false; };
    let count = record.robust_count as usize;
    let Some(index) = record.robust_mutexes[..count].iter().position(|entry| *entry == address) else {
        return false;
    };
    record.robust_mutexes[index..count].rotate_left(1);
    record.robust_count -= 1;
    record.robust_mutexes[record.robust_count as usize] = 0;
    true
}

pub fn robust_lock(id: ThreadId, address: u64) -> u64 {
    let mut table = ROBUST.lock();
    let Some(record) = table.iter_mut().find(|record| record.address == address) else { return u64::MAX; };
    if record.owner.is_some() && record.owner != Some(id) { return u64::MAX; }
    if record.state == RobustState::Unrecoverable { return 2; }
    record.owner = Some(id);
    if record.state == RobustState::Abandoned { 1 } else { 0 }
}
pub fn robust_unlock(id: ThreadId, address: u64) -> bool {
    let mut table = ROBUST.lock();
    let Some(record) = table.iter_mut().find(|record| record.address == address) else { return false; };
    if record.owner != Some(id) { return false; }
    record.owner = None;
    if record.state == RobustState::Abandoned { record.state = RobustState::Unrecoverable; }
    true
}
pub fn robust_consistent(id: ThreadId, address: u64) -> bool {
    let mut table = ROBUST.lock();
    let Some(record) = table.iter_mut().find(|record| record.address == address) else { return false; };
    if record.owner != Some(id) || record.state != RobustState::Abandoned { return false; }
    record.state = RobustState::Consistent;
    true
}

pub fn terminate(id: ThreadId, exit: i32) -> bool {
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else {
        return false;
    };
    if record.state != State::Running {
        return false;
    }
    record.state = State::Exited;
    record.exit = exit;
    true
}

/// Register the current task as the sole join waiter. The boolean is true
/// when termination won the race before registration, so the caller can retry
/// the join without blocking.
pub fn register_waiter(owner: u64, id: ThreadId, waiter: u64) -> Result<bool, JoinError> {
    let mut table = TABLE.lock();
    let record = lookup(&mut table, id).ok_or(JoinError::Invalid)?;
    if record.owner != owner {
        return Err(JoinError::NotOwner);
    }
    if record.state == State::Exited {
        return Ok(true);
    }
    if record.waiter.is_some() {
        return Err(JoinError::Invalid);
    }
    record.waiter = Some(waiter);
    Ok(false)
}

/// Consume a waiter after the exiting task is no longer physically running.
pub fn take_waiter(id: ThreadId) -> Option<u64> {
    let mut table = TABLE.lock();
    lookup(&mut table, id)?.waiter.take()
}

pub fn join(owner: u64, id: ThreadId) -> Result<i32, JoinError> {
    join_with_resources(owner, id).map(|(exit, _)| exit)
}

/// Join a terminated thread and return the mappings owned by that thread.
/// The caller must unmap them from the shared process address space before it
/// exposes the slots to a subsequent thread.
pub fn join_with_resources(
    owner: u64,
    id: ThreadId,
) -> Result<(i32, Option<Resources>), JoinError> {
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else {
        return Err(JoinError::Invalid);
    };
    if record.owner != owner {
        return Err(JoinError::NotOwner);
    }
    if record.state == State::Running {
        return Err(JoinError::Running);
    }
    let exit = record.exit;
    let resources = record.resources.take();
    record.state = State::Free;
    record.generation = record
        .generation
        .checked_add(1)
        .filter(|g| *g != 0)
        .unwrap_or(u32::MAX);
    Ok((exit, resources))
}

pub fn attach_resources(id: ThreadId, resources: Resources) -> bool {
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else {
        return false;
    };
    if record.resources.is_some() {
        return false;
    }
    record.resources = Some(resources);
    true
}

pub fn resources(owner: u64, id: ThreadId) -> Option<Resources> {
    let mut table = TABLE.lock();
    let record = lookup(&mut table, id)?;
    if record.owner != owner {
        return None;
    }
    record.resources
}

pub fn joinable(owner: u64, id: ThreadId) -> Result<bool, JoinError> {
    let mut table = TABLE.lock();
    let record = lookup(&mut table, id).ok_or(JoinError::Invalid)?;
    if record.owner != owner {
        return Err(JoinError::NotOwner);
    }
    Ok(record.state == State::Exited)
}

pub fn abort(id: ThreadId) -> bool {
    let mut table = TABLE.lock();
    let Some(record) = lookup(&mut table, id) else {
        return false;
    };
    record.state = State::Free;
    record.owner = 0;
    record.resources = None;
    record.generation = record
        .generation
        .checked_add(1)
        .filter(|generation| *generation != 0)
        .unwrap_or(u32::MAX);
    true
}

/// Reclaim every thread record owned by a process that is terminating.
///
/// The generation is advanced for each reclaimed slot, so a stale handle
/// from the terminated process cannot address a subsequently created thread.
pub fn reap_owner(owner: u64) -> usize {
    let mut table = TABLE.lock();
    let mut reclaimed = 0;
    for record in &mut table.records {
        if record.state != State::Free && record.owner == owner {
            record.state = State::Free;
            record.owner = 0;
            record.exit = 0;
            record.errno = 0;
            record.tls = 0;
            record.resources = None;
            record.waiter = None;
            record.generation = record
                .generation
                .checked_add(1)
                .filter(|generation| *generation != 0)
                .unwrap_or(u32::MAX);
            reclaimed += 1;
        }
    }
    reclaimed
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    Invalid,
    NotOwner,
    Running,
}

pub fn set_tls(id: ThreadId, value: u64) -> bool {
    let mut t = TABLE.lock();
    lookup(&mut t, id).map(|r| r.tls = value).is_some()
}
pub fn tls(id: ThreadId) -> Option<u64> {
    let mut t = TABLE.lock();
    lookup(&mut t, id).map(|r| r.tls)
}
pub fn set_errno(id: ThreadId, value: i32) -> bool {
    let mut t = TABLE.lock();
    lookup(&mut t, id).map(|r| r.errno = value).is_some()
}
pub fn errno(id: ThreadId) -> Option<i32> {
    let mut t = TABLE.lock();
    lookup(&mut t, id).map(|r| r.errno)
}

#[cfg(test)]
mod tests {
    use super::*;

    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn isolated(test: impl FnOnce()) {
        // A failed regression must not prevent independent cases from running.
        // Each case starts with a fresh host-only table; assertions still fail.
        let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
        *TABLE.lock() = Table::new();
        test();
        *TABLE.lock() = Table::new();
    }

    fn exhaust(retire: fn(ThreadId)) {
        isolated(|| {
            for record in &mut TABLE.lock().records {
                record.generation = u32::MAX - 1;
            }
            let mut retired = [0_u64; MAX_THREADS];
            for index in 0..MAX_THREADS {
                let id = create(77).expect("one final generation per available slot");
                assert!(
                    !retired[..index].contains(&id.raw()),
                    "retired handle reissued: {}",
                    id.raw()
                );
                assert_eq!(id.raw() >> 32, u64::from(u32::MAX - 1));
                assert!(set_tls(id, 123));
                assert!(set_errno(id, 9));
                retire(id);
                assert!(tls(id).is_none());
                assert!(errno(id).is_none());
                assert!(!set_tls(id, 456));
                assert!(!terminate(id, 0));
                assert!(join(77, id) == Err(JoinError::Invalid));
                retired[index] = id.raw();
            }
            assert!(create(77).is_none(), "exhausted slots must remain retired");
        });
    }

    #[test]
    fn join_permanently_retires_exhausted_generations() {
        exhaust(|id| {
            assert!(terminate(id, 42));
            assert!(join(77, id) == Ok(42));
        });
    }

    #[test]
    fn abort_permanently_retires_exhausted_generations() {
        exhaust(|id| assert!(abort(id)));
    }

    #[test]
    fn owner_reaping_permanently_retires_exhausted_generations() {
        exhaust(|_| assert_eq!(reap_owner(77), 1));
    }

    #[test]
    fn ordinary_reuse_preserves_owner_and_waiter_boundaries() {
        isolated(|| {
            let id = create(7).unwrap();
            assert!(set_tls(id, 123));
            assert!(set_errno(id, 9));
            assert!(join(8, id) == Err(JoinError::NotOwner));
            assert!(register_waiter(8, id, 99) == Err(JoinError::NotOwner));
            assert!(register_waiter(7, id, 99) == Ok(false));
            assert!(register_waiter(7, id, 100) == Err(JoinError::Invalid));
            assert!(terminate(id, 42));
            assert!(register_waiter(7, id, 99) == Ok(true));
            assert_eq!(take_waiter(id), Some(99));
            assert_eq!(take_waiter(id), None);
            assert!(join(7, id) == Ok(42));
            assert!(tls(id).is_none());
            let fresh = create(8).unwrap();
            assert_eq!(fresh.raw() & 0xffff_ffff, id.raw() & 0xffff_ffff);
            assert_ne!(fresh.raw(), id.raw());
            assert_eq!(tls(fresh), Some(0));
            assert_eq!(errno(fresh), Some(0));
            assert_eq!(reap_owner(7), 0);
            assert!(joinable(8, fresh) == Ok(false));
            assert!(abort(fresh));
            assert!(tls(fresh).is_none());
        });
    }

    #[test]
    fn ordinary_capacity_is_preserved_across_full_reuse() {
        isolated(|| {
            let first: Vec<_> = (0..MAX_THREADS).map(|_| create(7).unwrap()).collect();
            assert!(create(7).is_none());
            for id in &first {
                assert!(abort(*id));
            }
            let second: Vec<_> = (0..MAX_THREADS).map(|_| create(8).unwrap()).collect();
            assert!(create(8).is_none());
            for (old, fresh) in first.iter().zip(&second) {
                assert_eq!(old.raw() & 0xffff_ffff, fresh.raw() & 0xffff_ffff);
                assert_ne!(old.raw(), fresh.raw());
                assert!(tls(*old).is_none());
            }
            assert_eq!(reap_owner(8), MAX_THREADS);
        });
    }
}

#[cfg(feature = "stage11-2-test")]
pub fn structural_self_test() -> bool {
    let exhausted = Record {
        generation: u32::MAX,
        ..Record::EMPTY
    };
    let final_generation = Record {
        generation: u32::MAX - 1,
        ..Record::EMPTY
    };
    if exhausted.reusable() || !final_generation.reusable() {
        return false;
    }
    let Some(id) = create(1) else { return false };
    set_tls(id, 0xfeed_beef)
        && set_errno(id, 37)
        && tls(id) == Some(0xfeed_beef)
        && errno(id) == Some(37)
        && join(1, id) == Err(JoinError::Running)
        && terminate(id, 23)
        && join(1, id) == Ok(23)
        && tls(id).is_none()
        && {
            let Some(waited) = create(1) else {
                return false;
            };
            register_waiter(1, waited, 99) == Ok(false)
                && terminate(waited, 41)
                && take_waiter(waited) == Some(99)
                && join(1, waited) == Ok(41)
        }
        && {
            let Some(stale) = create(9) else { return false };
            set_tls(stale, 0x1234)
                && reap_owner(9) == 1
                && tls(stale).is_none()
                && create(9).is_some_and(|fresh| {
                    let distinct = fresh != stale;
                    let cleaned = terminate(fresh, 0) && join(9, fresh) == Ok(0);
                    distinct && cleaned
                })
        }
}
