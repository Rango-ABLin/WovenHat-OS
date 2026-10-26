//! Nonzero mapping-lifetime lineage tags from one boot-wide authority.
//! Snapshots/fork may copy a tag; owner and address-space root remain part of
//! identity. Fresh mapping constructors issue before page/backing side effects.
use core::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};
pub(crate) struct Issuer {
    next: AtomicU64,
}
impl Issuer {
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }
    /// MAX is a sticky exhaustion sentinel, never a reusable issued tag.
    /// Relaxed orders only uniqueness; canonical mapping locks publish metadata.
    pub fn issue(&self) -> Option<NonZeroU64> {
        let value = self
            .next
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .ok()?;
        NonZeroU64::new(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starts_nonzero_and_never_reuses_consumed_tags() {
        let issuer = Issuer::new();
        for expected in 1..=256 {
            assert_eq!(issuer.issue().unwrap().get(), expected);
        }
    }
    #[test]
    fn concurrent_issuance_has_no_duplicates_or_gaps() {
        let issuer = std::sync::Arc::new(Issuer::new());
        let threads: std::vec::Vec<_> = (0..8)
            .map(|_| {
                let issuer = issuer.clone();
                std::thread::spawn(move || {
                    (0..512)
                        .map(|_| issuer.issue().unwrap().get())
                        .collect::<std::vec::Vec<_>>()
                })
            })
            .collect();
        let mut values: std::vec::Vec<_> = threads
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect();
        values.sort_unstable();
        assert_eq!(values, (1..=4096).collect::<std::vec::Vec<_>>());
    }
    #[test]
    fn exhaustion_is_sticky_without_wrapping_to_zero() {
        let issuer = Issuer {
            next: AtomicU64::new(u64::MAX - 1),
        };
        assert_eq!(issuer.issue().unwrap().get(), u64::MAX - 1);
        for _ in 0..32 {
            assert!(issuer.issue().is_none());
        }
        assert_eq!(issuer.next.load(Ordering::Relaxed), u64::MAX);
    }
    #[test]
    fn racing_exhaustion_cannot_reissue_a_generation() {
        let issuer = std::sync::Arc::new(Issuer {
            next: AtomicU64::new(u64::MAX - 9),
        });
        let threads: std::vec::Vec<_> = (0..16)
            .map(|_| {
                let issuer = issuer.clone();
                std::thread::spawn(move || issuer.issue().map(NonZeroU64::get))
            })
            .collect();
        let mut issued: std::vec::Vec<_> = threads
            .into_iter()
            .filter_map(|t| t.join().unwrap())
            .collect();
        issued.sort_unstable();
        assert_eq!(
            issued,
            ((u64::MAX - 9)..u64::MAX).collect::<std::vec::Vec<_>>()
        );
        assert!(issuer.issue().is_none());
    }
}
