//! Host-side tests for the bottom taskbar preference model. The shell is a
//! `no_std` Ring-3 binary, so its pure model module is included directly.

#[path = "../src/taskbar_prefs.rs"]
mod taskbar_prefs;

use taskbar_prefs::{Alignment, IconSize, TaskbarPrefs};

#[test]
fn defaults_are_centered_and_medium() {
    let prefs = TaskbarPrefs::DEFAULT;
    assert_eq!(prefs.alignment, Alignment::Center);
    assert_eq!(prefs.icon_size, IconSize::Medium);
    assert_eq!(prefs.icon_size.entry_width(), 48);
}

#[test]
fn recognized_values_are_applied() {
    let prefs = TaskbarPrefs::from_values(Some("left"), Some("large"));
    assert_eq!(prefs.alignment, Alignment::Left);
    assert_eq!(prefs.icon_size, IconSize::Large);
    assert_eq!(prefs.icon_size.entry_width(), 56);
}

#[test]
fn absent_values_select_defaults() {
    assert_eq!(TaskbarPrefs::from_values(None, None), TaskbarPrefs::DEFAULT);
}

#[test]
fn malformed_values_fall_back_per_field() {
    let prefs = TaskbarPrefs::from_values(Some("top"), Some("large"));
    assert_eq!(prefs.alignment, Alignment::Center);
    assert_eq!(prefs.icon_size, IconSize::Large);

    let prefs = TaskbarPrefs::from_values(Some("LEFT"), Some(""));
    assert_eq!(prefs, TaskbarPrefs::DEFAULT);
}

#[test]
fn top_placement_is_not_a_preference() {
    assert_eq!(Alignment::parse("top"), None);
    assert_eq!(Alignment::parse("bottom"), None);
}

#[test]
fn entry_widths_are_strictly_ordered() {
    assert!(IconSize::Small.entry_width() < IconSize::Medium.entry_width());
    assert!(IconSize::Medium.entry_width() < IconSize::Large.entry_width());
}
