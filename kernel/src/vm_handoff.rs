//! Scheduler-owned process pins across physical stack/CR3 handoff.
//! Begin under the scheduler lock before publishing incoming ownership. Finish
//! only from the incoming stack, under the same lock. Not an async VM journal.
pub(crate) struct Handoffs<const N: usize> {
    owners: [Option<u64>; N],
}
impl<const N: usize> Handoffs<N> {
    pub const fn empty() -> Self {
        Self { owners: [None; N] }
    }
    /// Includes kernel owner 0, so nested handoffs are rejected on every CPU.
    pub fn begin(&mut self, cpu: usize, owner: u64) -> bool {
        let Some(slot) = self.owners.get_mut(cpu) else {
            return false;
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(owner);
        true
    }
    /// A first-dispatch bootstrap can finalize with no outgoing handoff.
    pub fn finish(&mut self, cpu: usize) -> Option<u64> {
        self.owners.get_mut(cpu).and_then(Option::take)
    }
    pub fn retains(&self, owner: u64) -> bool {
        owner != 0 && self.owners.contains(&Some(owner))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_until_incoming_stack_finishes() {
        let mut h = Handoffs::<4>::empty();
        assert!(h.begin(2, 7));
        assert!(h.retains(7));
        assert!(!h.retains(8));
        assert_eq!(h.finish(1), None);
        assert!(h.retains(7));
        assert_eq!(h.finish(2), Some(7));
        assert!(!h.retains(7));
    }
    #[test]
    fn same_owner_on_two_cpus_requires_both_handoffs() {
        let mut h = Handoffs::<4>::empty();
        assert!(h.begin(1, 7));
        assert!(h.begin(3, 7));
        assert_eq!(h.finish(1), Some(7));
        assert!(h.retains(7));
        assert_eq!(h.finish(3), Some(7));
        assert!(!h.retains(7));
    }
    #[test]
    fn nested_handoff_never_replaces_outgoing_owner() {
        let mut h = Handoffs::<1>::empty();
        assert!(h.begin(0, 7));
        assert!(!h.begin(0, 8));
        assert!(h.retains(7));
        assert!(!h.retains(8));
        assert_eq!(h.finish(0), Some(7));
        assert!(h.begin(0, 8));
        assert!(h.retains(8));
    }
    #[test]
    fn kernel_and_bootstrap_have_no_user_owner() {
        let mut h = Handoffs::<1>::empty();
        assert_eq!(h.finish(0), None);
        assert!(h.begin(0, 0));
        assert!(!h.retains(0));
        assert!(!h.begin(0, 7));
        assert_eq!(h.finish(0), Some(0));
        assert_eq!(h.finish(0), None);
    }
    #[test]
    fn invalid_cpu_and_zero_capacity_cannot_create_or_release_pins() {
        let mut h = Handoffs::<1>::empty();
        assert!(h.begin(0, 7));
        assert!(!h.begin(1, 8));
        assert_eq!(h.finish(usize::MAX), None);
        assert!(h.retains(7));
        let mut zero = Handoffs::<0>::empty();
        assert!(!zero.begin(0, 7));
        assert_eq!(zero.finish(0), None);
    }
    #[test]
    fn every_cpu_pair_and_completion_order_preserves_other_pin() {
        for a in 0..4 {
            for b in 0..4 {
                if a == b {
                    continue;
                }
                for reverse in [false, true] {
                    let mut h = Handoffs::<4>::empty();
                    assert!(h.begin(a, 7));
                    assert!(h.begin(b, 8));
                    let (first, owner, last, remaining) =
                        if reverse { (b, 8, a, 7) } else { (a, 7, b, 8) };
                    assert_eq!(h.finish(first), Some(owner));
                    assert!(!h.retains(owner));
                    assert!(h.retains(remaining));
                    assert_eq!(h.finish(last), Some(remaining));
                    assert!(!h.retains(remaining));
                }
            }
        }
    }
}
