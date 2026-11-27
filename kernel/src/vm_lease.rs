//! Allocation-free VM transaction model. Caller must serialize every method.
//! This model does not map pages or synchronize page tables. It is host-qualified
//! scaffolding until all task/pager/thread/lifecycle paths use the same registry.
//! A mutation retains a slot lease across the out-of-lock page operation. A
//! snapshot/replacement lease excludes every mutation. Retirement stops admission
//! immediately but cannot release the address space until all leases finish.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    pub owner: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    WrongOwner,
    Busy,
    Retiring,
    InvalidSlots,
    Stale,
    Exhausted,
    Quarantined,
}

#[must_use = "finish the page transaction before releasing its address space"]
#[derive(Debug)]
pub(crate) struct Lease {
    identity: Identity,
    sequence: u64,
    slots: u16,
    exclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Active,
    Draining,
    Retired,
}

/// Not Copy: process snapshots must never duplicate outstanding lease state.
#[derive(Debug)]
pub(crate) struct Registry {
    identity: Identity,
    phase: Phase,
    next_sequence: u64,
    // Per-slot transaction sequence prevents ABA completion after slot reuse.
    slots: [u64; 16],
    exclusive: u64,
    quarantine: u16,
}

impl Registry {
    pub const fn new(identity: Identity) -> Self {
        Self {
            identity,
            phase: Phase::Active,
            next_sequence: 1,
            slots: [0; 16],
            exclusive: 0,
            quarantine: 0,
        }
    }

    fn check_owner(&self, identity: Identity) -> Result<(), Error> {
        if self.identity != identity {
            Err(Error::WrongOwner)
        } else {
            Ok(())
        }
    }

    fn admit(&self, identity: Identity) -> Result<(), Error> {
        self.check_owner(identity)?;
        if self.phase != Phase::Active {
            Err(Error::Retiring)
        } else {
            Ok(())
        }
    }

    fn sequence(&mut self) -> Result<u64, Error> {
        let value = self.next_sequence;
        self.next_sequence = value.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(value)
    }

    /// Reserve a nonempty set, e.g. stack and TLS together, atomically.
    /// Committed mappings remain in the caller's mapping table; this registry
    /// tracks transactions only. The caller must check mapping occupancy while
    /// holding the same lock used for this reservation.
    pub fn reserve(&mut self, identity: Identity, slots: u16) -> Result<Lease, Error> {
        self.admit(identity)?;
        if slots == 0 {
            return Err(Error::InvalidSlots);
        }
        if slots & self.quarantine != 0 {
            return Err(Error::Quarantined);
        }
        if self.exclusive != 0
            || self
                .slots
                .iter()
                .enumerate()
                .any(|(i, seq)| slots & (1 << i) != 0 && *seq != 0)
        {
            return Err(Error::Busy);
        }
        let sequence = self.sequence()?;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slots & (1 << i) != 0 {
                *slot = sequence;
            }
        }
        Ok(Lease {
            identity,
            sequence,
            slots,
            exclusive: false,
        })
    }

    /// Exclusive lease for a consistent fork snapshot or exec replacement.
    pub fn exclusive(&mut self, identity: Identity) -> Result<Lease, Error> {
        self.admit(identity)?;
        if self.exclusive != 0 || self.slots.iter().any(|seq| *seq != 0) {
            return Err(Error::Busy);
        }
        if self.quarantine != 0 {
            return Err(Error::Quarantined);
        }
        let sequence = self.sequence()?;
        self.exclusive = sequence;
        Ok(Lease {
            identity,
            sequence,
            slots: 0,
            exclusive: true,
        })
    }

    /// Finish only after the page operation and publication/rollback are done.
    /// An uncertain result poisons the affected slots; no later operation can
    /// reuse them. An uncertain exclusive operation poisons the entire space.
    /// Failure consumes the token deliberately: callers must diagnose invariant
    /// violations rather than retry a stale completion against a new owner.
    pub fn finish(&mut self, lease: Lease, certain: bool) -> Result<(), Error> {
        self.check_owner(lease.identity)?;
        if lease.sequence == 0 || self.phase == Phase::Retired {
            return Err(Error::Stale);
        }
        if lease.exclusive {
            if self.exclusive != lease.sequence || lease.slots != 0 {
                return Err(Error::Stale);
            }
            self.exclusive = 0;
            if !certain {
                self.quarantine = u16::MAX;
            }
        } else {
            if lease.slots == 0
                || self
                    .slots
                    .iter()
                    .enumerate()
                    .any(|(i, seq)| lease.slots & (1 << i) != 0 && *seq != lease.sequence)
            {
                return Err(Error::Stale);
            }
            for (i, slot) in self.slots.iter_mut().enumerate() {
                if lease.slots & (1 << i) != 0 {
                    *slot = 0;
                }
            }
            if !certain {
                self.quarantine |= lease.slots;
            }
        }
        Ok(())
    }

    /// Sticky admission barrier. In-flight operations may still finish.
    pub fn retire(&mut self, identity: Identity) -> Result<(), Error> {
        self.check_owner(identity)?;
        if self.phase == Phase::Retired {
            return Err(Error::Stale);
        }
        self.phase = Phase::Draining;
        Ok(())
    }

    pub fn drained(&self, identity: Identity) -> Result<bool, Error> {
        self.check_owner(identity)?;
        Ok(self.phase == Phase::Draining
            && self.exclusive == 0
            && self.slots.iter().all(|seq| *seq == 0))
    }

    /// Call after all users have drained and actual page teardown succeeded.
    /// Poisoned slots do not permit reuse, but can be destroyed with the space.
    pub fn destroyed(&mut self, identity: Identity) -> Result<(), Error> {
        if !self.drained(identity)? {
            return Err(Error::Busy);
        }
        self.phase = Phase::Retired;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: Identity = Identity {
        owner: 7,
        generation: 3,
    };

    // Forging a token is test-only: the public interface has no constructor,
    // Clone or Copy. Used to model a late hardware completion explicitly.
    fn duplicate(lease: &Lease) -> Lease {
        Lease {
            identity: lease.identity,
            sequence: lease.sequence,
            slots: lease.slots,
            exclusive: lease.exclusive,
        }
    }

    #[test]
    fn stack_tls_reservation_is_atomic_and_disjoint_work_can_finish() {
        let mut r = Registry::new(ID);
        let stack_tls = r.reserve(ID, 0b11).unwrap();
        assert_eq!(r.reserve(ID, 0b110).unwrap_err(), Error::Busy);
        // A failed overlapping reservation must not reserve its free bit.
        let heap = r.reserve(ID, 0b100).unwrap();
        assert_eq!(r.exclusive(ID).unwrap_err(), Error::Busy);
        r.finish(heap, true).unwrap();
        r.finish(stack_tls, true).unwrap();
        let all = r.reserve(ID, u16::MAX).unwrap();
        r.finish(all, true).unwrap();
    }

    #[test]
    fn stale_mapping_completion_cannot_release_reused_slot() {
        let mut r = Registry::new(ID);
        let first = r.reserve(ID, 1).unwrap();
        let stale = duplicate(&first);
        r.finish(first, true).unwrap();
        let second = r.reserve(ID, 1).unwrap();
        assert_eq!(r.finish(stale, true), Err(Error::Stale));
        assert_eq!(r.reserve(ID, 1).unwrap_err(), Error::Busy);
        r.finish(second, true).unwrap();
    }

    #[test]
    fn retirement_waits_for_mapping_and_blocks_every_new_admission() {
        let mut r = Registry::new(ID);
        let mapping = r.reserve(ID, 8).unwrap();
        r.retire(ID).unwrap();
        assert!(!r.drained(ID).unwrap());
        assert_eq!(r.destroyed(ID), Err(Error::Busy));
        assert_eq!(r.reserve(ID, 1).unwrap_err(), Error::Retiring);
        assert_eq!(r.exclusive(ID).unwrap_err(), Error::Retiring);
        r.finish(mapping, true).unwrap();
        assert!(r.drained(ID).unwrap());
        r.destroyed(ID).unwrap();
        assert_eq!(r.retire(ID), Err(Error::Stale));
        assert!(!r.drained(ID).unwrap());
    }

    #[test]
    fn snapshot_excludes_mutations_and_retirement_waits_for_snapshot() {
        let mut r = Registry::new(ID);
        let snapshot = r.exclusive(ID).unwrap();
        assert_eq!(r.reserve(ID, 1).unwrap_err(), Error::Busy);
        assert_eq!(r.exclusive(ID).unwrap_err(), Error::Busy);
        r.retire(ID).unwrap();
        assert!(!r.drained(ID).unwrap());
        r.finish(snapshot, true).unwrap();
        assert!(r.drained(ID).unwrap());
    }

    #[test]
    fn uncertain_rollback_poison_is_sticky_but_allows_owner_teardown() {
        let mut r = Registry::new(ID);
        let mapping = r.reserve(ID, 3).unwrap();
        r.finish(mapping, false).unwrap();
        assert_eq!(r.reserve(ID, 1).unwrap_err(), Error::Quarantined);
        assert_eq!(r.exclusive(ID).unwrap_err(), Error::Quarantined);
        let other = r.reserve(ID, 4).unwrap();
        r.finish(other, true).unwrap();
        r.retire(ID).unwrap();
        r.destroyed(ID).unwrap();
    }

    #[test]
    fn owner_generation_and_sequence_never_wrap_or_mutate_other_space() {
        let mut old = Registry::new(ID);
        let token = old.reserve(ID, 1).unwrap();
        let newer = Identity {
            generation: 4,
            ..ID
        };
        let mut new = Registry::new(newer);
        assert_eq!(new.finish(token, true), Err(Error::WrongOwner));
        assert_eq!(new.retire(ID), Err(Error::WrongOwner));
        let valid = new.reserve(newer, 1).unwrap();
        new.finish(valid, true).unwrap();
        new.next_sequence = u64::MAX;
        assert_eq!(new.reserve(newer, 1).unwrap_err(), Error::Exhausted);
        assert_eq!(new.exclusive(newer).unwrap_err(), Error::Exhausted);
        assert!(new.slots.iter().all(|seq| *seq == 0));
    }

    #[test]
    fn stale_multislot_completion_does_not_partially_release_live_transactions() {
        let mut r = Registry::new(ID);
        let first = r.reserve(ID, 1).unwrap();
        let second = r.reserve(ID, 2).unwrap();
        let forged = Lease {
            slots: 3,
            ..duplicate(&first)
        };
        assert_eq!(r.finish(forged, true), Err(Error::Stale));
        assert_eq!(r.reserve(ID, 1).unwrap_err(), Error::Busy);
        assert_eq!(r.reserve(ID, 2).unwrap_err(), Error::Busy);
        r.finish(first, true).unwrap();
        r.finish(second, true).unwrap();
    }

    #[test]
    fn uncertain_exclusive_operation_quarantines_every_slot() {
        let mut r = Registry::new(ID);
        let snapshot = r.exclusive(ID).unwrap();
        r.finish(snapshot, false).unwrap();
        for i in 0..16 {
            assert_eq!(r.reserve(ID, 1 << i).unwrap_err(), Error::Quarantined);
        }
        r.retire(ID).unwrap();
        assert!(r.drained(ID).unwrap());
        r.destroyed(ID).unwrap();
    }

    #[test]
    fn every_two_operation_schedule_retains_retiring_address_space() {
        for first in 0..16 {
            for second in 0..16 {
                for reverse in [false, true] {
                    let mut r = Registry::new(ID);
                    let a = r.reserve(ID, 1 << first).unwrap();
                    let b = r.reserve(ID, 1 << second);
                    r.retire(ID).unwrap();
                    assert!(!r.drained(ID).unwrap());
                    if let Ok(b) = b {
                        if reverse {
                            r.finish(b, true).unwrap();
                            assert!(!r.drained(ID).unwrap());
                            r.finish(a, true).unwrap();
                        } else {
                            r.finish(a, true).unwrap();
                            assert!(!r.drained(ID).unwrap());
                            r.finish(b, true).unwrap();
                        }
                    } else {
                        r.finish(a, true).unwrap();
                    }
                    assert!(r.drained(ID).unwrap());
                    r.destroyed(ID).unwrap();
                }
            }
        }
    }
}
