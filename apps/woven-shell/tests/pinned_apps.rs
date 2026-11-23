//! Host-side tests for the bounded pinned-application model. The shell is a
//! `no_std` Ring-3 binary, so its pure model module is included directly.

#[path = "../src/pinned_apps.rs"]
mod pinned_apps;

use pinned_apps::{PinError, PinnedApps, MAX_PINNED};

#[test]
fn absent_setting_restores_curated_defaults() {
    let pins = PinnedApps::from_setting(None);
    assert_eq!(pins.len(), 3);
    assert_eq!(pins.get(0), Some("WovenFiles"));
    assert_eq!(pins.get(2), Some("WovenStore"));
}

#[test]
fn empty_setting_means_everything_unpinned() {
    assert!(PinnedApps::from_setting(Some("")).is_empty());
}

#[test]
fn parsing_trims_drops_duplicates_and_invalid_names() {
    let pins = PinnedApps::from_setting(Some(" WovenFiles, ,WovenFiles,bad name,../x,WovenCalculator"));
    assert_eq!(pins.len(), 2);
    assert_eq!(pins.get(0), Some("WovenFiles"));
    assert_eq!(pins.get(1), Some("WovenCalculator"));
}

#[test]
fn list_is_capped_at_the_bound() {
    let value = "a1,a2,a3,a4,a5,a6,a7,a8,a9,a10";
    let pins = PinnedApps::from_setting(Some(value));
    assert_eq!(pins.len(), MAX_PINNED);
    assert_eq!(pins.get(MAX_PINNED - 1), Some("a8"));
}

#[test]
fn pin_reports_full_duplicate_and_invalid() {
    let mut pins = PinnedApps::empty();
    assert_eq!(pins.pin("WovenFiles"), Ok(()));
    assert_eq!(pins.pin("WovenFiles"), Err(PinError::Duplicate));
    assert_eq!(pins.pin("has space"), Err(PinError::Invalid));
    assert_eq!(pins.pin(""), Err(PinError::Invalid));
    assert_eq!(pins.pin(&"x".repeat(33)), Err(PinError::Invalid));
    for index in 1..MAX_PINNED {
        pins.pin(&format!("app{index}")).unwrap();
    }
    assert_eq!(pins.pin("overflow"), Err(PinError::Full));
}

#[test]
fn unpin_keeps_order_and_reports_absence() {
    let mut pins = PinnedApps::from_setting(Some("a,b,c"));
    assert!(pins.unpin("b"));
    assert_eq!(pins.get(0), Some("a"));
    assert_eq!(pins.get(1), Some("c"));
    assert_eq!(pins.get(2), None);
    assert!(!pins.unpin("b"));
}
