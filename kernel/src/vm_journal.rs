//! Bounded, allocation-free VM cancellation journal. Host-qualified only.
//! Caller serializes every method with the canonical mapping registry and
//! verifies executor liveness/quiescence using scheduler handoff state.
//! Descriptors are not pins: actual pages/backing objects must be retained by
//! the future kernel integration. Never infer page rollback from journal phase
//! alone: a task may die after physical application or owner-table publication
//! but before recording that step here.
use crate::vm_lease::{Error as LeaseError, Identity, Lease, Registry};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Lease(LeaseError),
    Full,
    Exhausted,
    WrongOwner,
    Stale,
    StillRunning,
    Quiesced,
    Recovering,
    AlreadyApplied,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Slots(u16),
    Exclusive,
}

/// Opaque asynchronous completion identity. Copying it grants no new lease.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ticket {
    identity: Identity,
    sequence: u64,
    index: usize,
}

struct Entry<T> {
    lease: Lease,
    sequence: u64,
    executor: u64,
    intent: T,
    applied: Option<T>,
    quiesced: bool,
    cleanup: Option<(u64, u64)>, // executor and unique cleanup claim sequence
}

#[must_use = "finish recovery or retain the journal until its cleanup executor quiesces"]
#[derive(Debug)]
pub(crate) struct Recovery<T> {
    ticket: Ticket,
    claim: u64,
    pub intent: T,
    pub applied: Option<T>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Outcome {
    Committed,
    RolledBack,
    Uncertain,
}

pub(crate) struct Journal<T: Copy, const N: usize> {
    identity: Identity,
    leases: Registry,
    entries: [Option<Entry<T>>; N],
    next_sequence: u64,
}
impl<T: Copy, const N: usize> Journal<T, N> {
    pub const fn new(identity: Identity) -> Self {
        Self {
            identity,
            leases: Registry::new(identity),
            entries: [const { None }; N],
            next_sequence: 1,
        }
    }
    pub fn pending(&self) -> usize {
        self.entries.iter().flatten().count()
    }
    fn entry(&self, ticket: Ticket) -> Result<&Entry<T>, Error> {
        if ticket.identity != self.identity {
            return Err(Error::WrongOwner);
        }
        self.entries
            .get(ticket.index)
            .and_then(Option::as_ref)
            .filter(|entry| entry.sequence == ticket.sequence)
            .ok_or(Error::Stale)
    }
    fn next(&mut self) -> Result<u64, Error> {
        let value = self.next_sequence;
        self.next_sequence = value.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(value)
    }
    /// Persist complete cleanup intent BEFORE the out-of-lock page operation.
    /// Mapping occupancy and executor liveness must be checked under the same
    /// authoritative lock. No stack-owned Lease escapes this journal.
    pub fn begin(&mut self, executor: u64, kind: Kind, intent: T) -> Result<Ticket, Error> {
        let index = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        // Check overflow before taking a lease, so exhaustion cannot leak one.
        let next = self.next_sequence.checked_add(1).ok_or(Error::Exhausted)?;
        let lease = match kind {
            Kind::Slots(slots) => self.leases.reserve(self.identity, slots),
            Kind::Exclusive => self.leases.exclusive(self.identity),
        }
        .map_err(Error::Lease)?;
        let sequence = self.next_sequence;
        self.next_sequence = next;
        self.entries[index] = Some(Entry {
            lease,
            sequence,
            executor,
            intent,
            applied: None,
            quiesced: false,
            cleanup: None,
        });
        Ok(Ticket {
            identity: self.identity,
            sequence,
            index,
        })
    }
    fn live(&self, ticket: Ticket) -> Result<(), Error> {
        let entry = self.entry(ticket)?;
        if entry.quiesced {
            Err(Error::Quiesced)
        } else if entry.cleanup.is_some() {
            Err(Error::Recovering)
        } else {
            Ok(())
        }
    }
    pub fn record_applied(&mut self, ticket: Ticket, metadata: T) -> Result<(), Error> {
        self.live(ticket)?;
        let entry = self.entries[ticket.index]
            .as_mut()
            .expect("validated ticket");
        if entry.applied.is_some() {
            return Err(Error::AlreadyApplied);
        }
        entry.applied = Some(metadata);
        Ok(())
    }
    /// Finish AFTER canonical publication or confirmed rollback. A racing late
    /// completion from a quiesced executor cannot release its retained lease.
    pub fn complete(&mut self, ticket: Ticket, outcome: Outcome) -> Result<(), Error> {
        self.live(ticket)?;
        self.release(ticket.index, outcome);
        Ok(())
    }
    fn release(&mut self, index: usize, outcome: Outcome) {
        let entry = self.entries[index].take().expect("validated journal entry");
        // The journal exclusively owns every Lease and never exposes Registry.
        // A failure is an internal invariant violation, not a recoverable stale
        // caller: retaining a corrupt mapping must fail closed.
        self.leases
            .finish(entry.lease, !matches!(outcome, Outcome::Uncertain))
            .expect("journal lease invariant");
    }
    /// Caller must confirm actual executor quiescence, including remote CPU
    /// handoff and deferred pager/IO work. A cancellation request is NOT proof.
    /// Live executors must never be admitted with the same task generation.
    pub fn executor_quiesced(&mut self, executor: u64) {
        for entry in self.entries.iter_mut().flatten() {
            if entry.executor == executor {
                entry.quiesced = true;
            }
            if entry.cleanup.is_some_and(|(owner, _)| owner == executor) {
                entry.cleanup = None;
            }
        }
    }
    /// Claim cleanup only after the original executor is confirmed quiescent.
    /// If a cleanup worker dies, confirm its quiescence before handing off.
    pub fn recover(&mut self, ticket: Ticket, cleanup_executor: u64) -> Result<Recovery<T>, Error> {
        let entry = self.entry(ticket)?;
        if !entry.quiesced {
            return Err(Error::StillRunning);
        }
        if entry.executor == cleanup_executor {
            return Err(Error::Quiesced);
        }
        if entry.cleanup.is_some() {
            return Err(Error::Recovering);
        }
        let claim = self.next()?;
        let entry = self.entries[ticket.index]
            .as_mut()
            .expect("validated ticket");
        entry.cleanup = Some((cleanup_executor, claim));
        Ok(Recovery {
            ticket,
            claim,
            intent: entry.intent,
            applied: entry.applied,
        })
    }
    /// Outcome is determined from authoritative generation-tagged owner/page
    /// state, not merely `applied`. Uncertain results quarantine affected slots.
    pub fn recovered(&mut self, recovery: Recovery<T>, outcome: Outcome) -> Result<(), Error> {
        let entry = self.entry(recovery.ticket)?;
        if !entry.quiesced || !entry.cleanup.is_some_and(|(_, seq)| seq == recovery.claim) {
            return Err(Error::Stale);
        }
        self.release(recovery.ticket.index, outcome);
        Ok(())
    }
    pub fn retire(&mut self) -> Result<(), Error> {
        self.leases.retire(self.identity).map_err(Error::Lease)
    }
    pub fn drained(&self) -> bool {
        self.pending() == 0 && self.leases.drained(self.identity) == Ok(true)
    }
    pub fn destroyed(&mut self) -> Result<(), Error> {
        if self.pending() != 0 {
            return Err(Error::Lease(LeaseError::Busy));
        }
        self.leases.destroyed(self.identity).map_err(Error::Lease)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: Identity = Identity {
        owner: 11,
        generation: 7,
    };
    #[test]
    fn dropped_executor_ticket_keeps_space_owned_until_recovery() {
        let mut j = Journal::<u64, 2>::new(ID);
        let ticket = j.begin(31, Kind::Slots(1), 0x1000).unwrap();
        j.record_applied(ticket, 0x2000).unwrap();
        j.retire().unwrap();
        assert_eq!(j.pending(), 1);
        assert!(!j.drained());
        assert!(j.destroyed().is_err());
        assert_eq!(j.recover(ticket, 40).unwrap_err(), Error::StillRunning);
        j.executor_quiesced(31);
        assert_eq!(j.complete(ticket, Outcome::Committed), Err(Error::Quiesced));
        let recovery = j.recover(ticket, 40).unwrap();
        assert_eq!(recovery.intent, 0x1000);
        assert_eq!(recovery.applied, Some(0x2000));
        j.recovered(recovery, Outcome::RolledBack).unwrap();
        assert!(j.drained());
        j.destroyed().unwrap();
    }
    #[test]
    fn dead_cleanup_worker_handoff_rejects_its_late_completion() {
        let mut j = Journal::<u64, 1>::new(ID);
        let ticket = j.begin(31, Kind::Exclusive, 9).unwrap();
        j.executor_quiesced(31);
        let old = j.recover(ticket, 40).unwrap();
        assert_eq!(j.recover(ticket, 41).unwrap_err(), Error::Recovering);
        j.executor_quiesced(40);
        let new = j.recover(ticket, 41).unwrap();
        assert_eq!(j.recovered(old, Outcome::RolledBack), Err(Error::Stale));
        assert_eq!(j.pending(), 1);
        j.recovered(new, Outcome::Committed).unwrap();
        assert_eq!(j.pending(), 0);
    }
    #[test]
    fn slot_reuse_and_foreign_completion_cannot_mutate_new_operation() {
        let mut j = Journal::<u64, 1>::new(ID);
        let old = j.begin(31, Kind::Slots(1), 9).unwrap();
        j.complete(old, Outcome::RolledBack).unwrap();
        let new = j.begin(32, Kind::Slots(1), 10).unwrap();
        assert_eq!(j.record_applied(old, 99), Err(Error::Stale));
        assert_eq!(j.complete(old, Outcome::Committed), Err(Error::Stale));
        let mut foreign = Journal::<u64, 1>::new(Identity {
            generation: 8,
            ..ID
        });
        assert_eq!(
            foreign.complete(new, Outcome::Committed),
            Err(Error::WrongOwner)
        );
        j.record_applied(new, 11).unwrap();
        assert_eq!(j.record_applied(new, 12), Err(Error::AlreadyApplied));
        j.complete(new, Outcome::Committed).unwrap();
    }
    #[test]
    fn bounded_capacity_and_sequence_exhaustion_do_not_leak_extra_leases() {
        let mut j = Journal::<u64, 1>::new(ID);
        let first = j.begin(31, Kind::Slots(1), 9).unwrap();
        assert_eq!(j.begin(32, Kind::Slots(2), 10).unwrap_err(), Error::Full);
        j.complete(first, Outcome::RolledBack).unwrap();
        j.next_sequence = u64::MAX;
        assert_eq!(
            j.begin(32, Kind::Slots(2), 10).unwrap_err(),
            Error::Exhausted
        );
        assert_eq!(j.pending(), 0);
        j.retire().unwrap();
        assert!(j.drained());
    }
    #[test]
    fn cleanup_sequence_exhaustion_retains_intent_and_blocks_destruction() {
        let mut j = Journal::<u64, 1>::new(ID);
        let ticket = j.begin(31, Kind::Slots(1), 9).unwrap();
        j.executor_quiesced(31);
        j.next_sequence = u64::MAX;
        assert_eq!(j.recover(ticket, 40).unwrap_err(), Error::Exhausted);
        j.retire().unwrap();
        assert_eq!(j.pending(), 1);
        assert!(!j.drained());
        assert!(j.destroyed().is_err());
        assert_eq!(j.entry(ticket).unwrap().intent, 9);
    }

    #[test]
    fn uncertainty_quarantines_slots_and_retirement_closes_admission() {
        let mut j = Journal::<u64, 2>::new(ID);
        let first = j.begin(31, Kind::Slots(3), 9).unwrap();
        j.executor_quiesced(31);
        let recovery = j.recover(first, 40).unwrap();
        j.recovered(recovery, Outcome::Uncertain).unwrap();
        assert_eq!(
            j.begin(32, Kind::Slots(1), 10).unwrap_err(),
            Error::Lease(LeaseError::Quarantined)
        );
        let other = j.begin(32, Kind::Slots(4), 11).unwrap();
        j.retire().unwrap();
        assert_eq!(
            j.begin(33, Kind::Slots(8), 12).unwrap_err(),
            Error::Lease(LeaseError::Retiring)
        );
        j.complete(other, Outcome::RolledBack).unwrap();
        assert!(j.drained());
        j.destroyed().unwrap();
    }
    #[test]
    fn crash_windows_require_authoritative_publication_check_before_rollback() {
        // Crash before/after page application and after canonical publication,
        // with or without loss of the first cleanup worker: 128 schedules.
        for bit in 0..16 {
            for step in 0..4 {
                for lose_cleanup in [false, true] {
                    let mut j = Journal::<u64, 1>::new(ID);
                    let ticket = j.begin(31, Kind::Slots(1 << bit), 7).unwrap();
                    // Actual page operation is outside the journal. Recording
                    // may lag it; an absent applied marker does not prove absence.
                    let mut page_table = [None; 16];
                    let mut owner_table = [None; 16];
                    if step > 0 {
                        page_table[bit] = Some(7);
                    }
                    if step == 3 {
                        owner_table[bit] = Some(7);
                    }
                    if step == 2 {
                        j.record_applied(ticket, 8).unwrap();
                    }
                    j.retire().unwrap();
                    j.executor_quiesced(31);
                    let mut recovery = j.recover(ticket, 40).unwrap();
                    if lose_cleanup {
                        j.executor_quiesced(40);
                        recovery = j.recover(ticket, 41).unwrap();
                    }
                    assert_eq!(recovery.intent, 7);
                    // Recovery consults the authoritative generation, even if
                    // publication completed without an applied journal marker.
                    let outcome = if owner_table[bit] == Some(recovery.intent) {
                        assert_eq!(page_table[bit], owner_table[bit]);
                        Outcome::Committed
                    } else {
                        if page_table[bit] == Some(recovery.intent) {
                            page_table[bit] = None;
                        }
                        Outcome::RolledBack
                    };
                    j.recovered(recovery, outcome).unwrap();
                    assert_eq!(page_table, owner_table);
                    assert!(j.drained());
                }
            }
        }
    }
}
