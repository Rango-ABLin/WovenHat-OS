//! Bounded pager work ownership. Caller serializes methods under the pager lock.
//! A queued-to-active transition never drops the owner pin. Only a matching
//! completion releases active work, after page access/publication has finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Key {
    pub owner: u64,
    pub task: u64,
    pub address: u64,
    sequence: u64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Work<T> {
    pub key: Key,
    pub payload: T,
}
#[derive(Clone, Copy)]
struct Completion {
    key: Key,
    succeeded: bool,
}

pub(crate) struct Queue<T: Copy, const N: usize> {
    entries: [Option<Work<T>>; N],
    active: Option<Key>,
    completions: [Option<Completion>; N],
    next_sequence: u64,
}
impl<T: Copy, const N: usize> Queue<T, N> {
    pub const fn empty() -> Self {
        Self {
            entries: [None; N],
            active: None,
            completions: [None; N],
            next_sequence: 1,
        }
    }
    fn outstanding(&self) -> usize {
        self.entries.iter().flatten().count()
            + usize::from(self.active.is_some())
            + self.completions.iter().flatten().count()
    }
    /// Admission reserves capacity for the eventual completion too.
    pub fn push(&mut self, owner: u64, task: u64, address: u64, payload: T) -> bool {
        if self.outstanding() >= N {
            return false;
        }
        let Some(next) = self.next_sequence.checked_add(1) else {
            return false;
        };
        let Some(slot) = self.entries.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some(Work {
            key: Key {
                owner,
                task,
                address,
                sequence: self.next_sequence,
            },
            payload,
        });
        self.next_sequence = next;
        true
    }
    /// Single worker: atomically transfer the queued pin into active work.
    pub fn pop(&mut self) -> Option<Work<T>> {
        if self.active.is_some() {
            return None;
        }
        let work = self.entries.iter_mut().find_map(Option::take)?;
        self.active = Some(work.key);
        Some(work)
    }
    pub fn finish(&mut self, key: Key, succeeded: bool) -> bool {
        if self.active != Some(key) {
            return false;
        }
        // Admission guarantees space. Fail closed if an internal invariant was
        // violated: never drop the active pin before recording completion.
        let Some(slot) = self.completions.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some(Completion { key, succeeded });
        self.active = None;
        true
    }
    pub fn take_completion(&mut self, task: u64, address: u64) -> Option<bool> {
        let slot = self.completions.iter().position(|slot| {
            slot.is_some_and(|done| done.key.task == task && done.key.address == address)
        })?;
        self.completions[slot].take().map(|done| done.succeeded)
    }
    /// Only call after the scheduler confirms all owner's tasks have handed off.
    /// Deferred page work retains the space even if its requesting task died.
    pub fn release_quiescent_owner(&mut self, owner: u64) -> bool {
        if self.active.is_some_and(|key| key.owner == owner)
            || self
                .entries
                .iter()
                .flatten()
                .any(|work| work.key.owner == owner)
        {
            return false;
        }
        for slot in &mut self.completions {
            if slot.is_some_and(|done| done.key.owner == owner) {
                *slot = None;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_and_active_work_retain_owner_until_matching_completion() {
        let mut q = Queue::<u64, 2>::empty();
        assert!(q.push(7, 11, 4096, 99));
        assert!(!q.release_quiescent_owner(7));
        let work = q.pop().unwrap();
        assert_eq!(work.payload, 99);
        assert!(!q.release_quiescent_owner(7));
        assert!(q.pop().is_none());
        assert!(q.finish(work.key, true));
        assert!(q.release_quiescent_owner(7));
        assert_eq!(q.take_completion(11, 4096), None);
    }
    #[test]
    fn stale_completion_cannot_release_reused_task_address_work() {
        let mut q = Queue::<(), 1>::empty();
        assert!(q.push(7, 11, 4096, ()));
        let old = q.pop().unwrap().key;
        assert!(q.finish(old, true));
        assert_eq!(q.take_completion(11, 4096), Some(true));
        assert!(q.push(7, 11, 4096, ()));
        let new = q.pop().unwrap().key;
        assert_ne!(old, new);
        assert!(!q.finish(old, false));
        assert!(!q.release_quiescent_owner(7));
        assert!(q.finish(new, true));
    }
    #[test]
    fn owner_cleanup_preserves_other_owner_work_and_results() {
        let mut q = Queue::<(), 3>::empty();
        assert!(q.push(7, 11, 4096, ()));
        assert!(q.push(8, 12, 8192, ()));
        let first = q.pop().unwrap();
        assert!(q.finish(first.key, true));
        assert!(q.release_quiescent_owner(7));
        assert!(!q.release_quiescent_owner(8));
        let second = q.pop().unwrap();
        assert!(q.finish(second.key, false));
        assert_eq!(q.take_completion(12, 8192), Some(false));
        assert!(q.release_quiescent_owner(8));
    }
    #[test]
    fn completion_capacity_is_reserved_at_admission_and_reusable() {
        let mut q = Queue::<(), 2>::empty();
        assert!(q.push(7, 11, 4096, ()));
        let first = q.pop().unwrap();
        assert!(q.push(8, 12, 8192, ()));
        assert!(!q.push(9, 13, 12288, ()));
        assert!(q.finish(first.key, true));
        let second = q.pop().unwrap();
        assert!(q.finish(second.key, true));
        assert!(!q.push(9, 13, 12288, ()));
        assert_eq!(q.take_completion(11, 4096), Some(true));
        assert!(q.push(9, 13, 12288, ()));
    }
    #[test]
    fn sequence_exhaustion_and_zero_capacity_refuse_without_pins() {
        let mut q = Queue::<(), 1>::empty();
        q.next_sequence = u64::MAX;
        assert!(!q.push(7, 11, 4096, ()));
        assert!(q.release_quiescent_owner(7));
        assert!(q.pop().is_none());
        let mut zero = Queue::<(), 0>::empty();
        assert!(!zero.push(7, 11, 4096, ()));
        assert!(zero.release_quiescent_owner(7));
    }
    #[test]
    fn wrong_completion_identity_never_releases_active_owner() {
        let mut q = Queue::<(), 1>::empty();
        assert!(q.push(7, 11, 4096, ()));
        let key = q.pop().unwrap().key;
        for wrong in [
            Key { owner: 8, ..key },
            Key { task: 12, ..key },
            Key {
                address: 8192,
                ..key
            },
            Key { sequence: 0, ..key },
        ] {
            assert!(!q.finish(wrong, true));
            assert!(!q.release_quiescent_owner(7));
        }
        assert!(q.finish(key, true));
    }
    #[test]
    fn every_owner_pair_retains_pop_handoff_pin() {
        for first in 1..9 {
            for second in 1..9 {
                let mut q = Queue::<(), 2>::empty();
                assert!(q.push(first, 11, 4096, ()));
                assert!(q.push(second, 12, 8192, ()));
                let a = q.pop().unwrap();
                assert!(!q.release_quiescent_owner(first));
                assert!(!q.release_quiescent_owner(second));
                assert!(q.finish(a.key, true));
                if first != second {
                    assert!(q.release_quiescent_owner(first));
                }
                let b = q.pop().unwrap();
                assert!(!q.release_quiescent_owner(second));
                assert!(q.finish(b.key, true));
                assert!(q.release_quiescent_owner(first));
                assert!(q.release_quiescent_owner(second));
                assert_eq!(q.outstanding(), 0);
            }
        }
    }
}
