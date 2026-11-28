//! Host-side tests for the bottom taskbar preference model. The shell is a
//! `no_std` Ring-3 binary, so its pure model module is included directly.

#[path = "../src/taskbar_prefs.rs"]
mod taskbar_prefs;

use taskbar_prefs::{Alignment, AutoHide, IconSize, TaskbarPrefs, AUTO_HIDE_HOLD};

#[test]
fn open_panel_defers_hiding_until_it_closes() {
    let mut state = AutoHide::from_setting(Some("true"));
    assert!(state.pointer(399, 399, 376, 0, AUTO_HIDE_HOLD));
    assert!(!state.tick(AUTO_HIDE_HOLD + 1, true));
    assert!(!state.is_hidden());
    assert!(state.tick(AUTO_HIDE_HOLD + 2, false));
    assert!(state.is_hidden());
}

#[test]
fn defaults_are_centered_and_medium() {
    let prefs = TaskbarPrefs::DEFAULT;
    assert_eq!(prefs.alignment, Alignment::Center);
    assert_eq!(prefs.icon_size, IconSize::Medium);
    assert_eq!(prefs.icon_size.entry_width(), 40);
}

#[test]
fn recognized_values_are_applied() {
    let prefs = TaskbarPrefs::from_values(Some("left"), Some("large"));
    assert_eq!(prefs.alignment, Alignment::Left);
    assert_eq!(prefs.icon_size, IconSize::Large);
    assert_eq!(prefs.icon_size.entry_width(), 48);
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

#[test]
fn auto_hide_setting_is_strict_and_starts_hidden() {
    let off = AutoHide::from_setting(None);
    assert!(!off.enabled());
    assert!(!off.is_hidden());
    assert!(!AutoHide::from_setting(Some("false")).enabled());
    assert!(!AutoHide::from_setting(Some("TRUE")).enabled());
    assert!(!AutoHide::from_setting(Some("yes")).enabled());
    let on = AutoHide::from_setting(Some("true"));
    assert!(on.enabled());
    assert!(on.is_hidden());
}

#[test]
fn auto_hide_off_never_hides() {
    let mut state = AutoHide::off();
    // Pointer anywhere, ticks forever: a disabled bar can never change.
    assert!(!state.pointer(399, 399, 376, 1_000, AUTO_HIDE_HOLD));
    assert!(!state.tick(10_000, false));
    assert!(!state.is_hidden());
}

#[test]
fn auto_hide_reveals_on_edge_and_hides_after_hold() {
    let mut state = AutoHide::from_setting(Some("true"));
    // Above the edge: stays hidden.
    assert!(!state.pointer(300, 399, 376, 100, AUTO_HIDE_HOLD));
    assert!(state.is_hidden());
    // The reveal edge reveals and starts the hold.
    assert!(state.pointer(399, 399, 376, 100, AUTO_HIDE_HOLD));
    assert!(!state.is_hidden());
    // Before the hold expires: stays revealed.
    assert!(!state.tick(199, false));
    assert!(!state.is_hidden());
    // Past the hold: hides again.
    assert!(state.tick(201, false));
    assert!(state.is_hidden());
}

#[test]
fn auto_hide_pointer_in_strip_refreshes_the_hold() {
    let mut state = AutoHide::from_setting(Some("true"));
    assert!(state.pointer(399, 399, 376, 100, AUTO_HIDE_HOLD));
    // A position inside the strip pushes the deadline out.
    assert!(!state.pointer(380, 399, 376, 150, AUTO_HIDE_HOLD));
    // Past the ORIGINAL deadline but not the refreshed one: stays up.
    assert!(!state.tick(201, false));
    assert!(!state.is_hidden());
    assert!(!state.tick(250, false));
    assert!(!state.is_hidden());
    // Past the refreshed deadline: hides again.
    assert!(state.tick(251, false));
    assert!(state.is_hidden());
}

#[test]
fn auto_hide_pointer_above_strip_lets_the_hold_run_out() {
    let mut state = AutoHide::from_setting(Some("true"));
    assert!(state.pointer(399, 399, 376, 100, AUTO_HIDE_HOLD));
    // Moving up out of the strip does not refresh anything.
    assert!(!state.pointer(200, 399, 376, 140, AUTO_HIDE_HOLD));
    assert!(state.tick(140 + AUTO_HIDE_HOLD, false));
    assert!(state.is_hidden());
}

#[test]
fn auto_hide_toggle_hides_and_reveals() {
    let mut state = AutoHide::off();
    // Off -> on: hides immediately.
    assert!(state.toggle());
    assert!(state.enabled());
    assert!(state.is_hidden());
    // On -> off: reveals.
    assert!(state.toggle());
    assert!(!state.enabled());
    assert!(!state.is_hidden());
    // Revealed by the edge, then disabled: already visible, so the pixels
    // do not change and the return value says so.
    let mut revealed = AutoHide::from_setting(Some("true"));
    assert!(revealed.pointer(399, 399, 376, 100, AUTO_HIDE_HOLD));
    assert!(!revealed.is_hidden());
    assert!(!revealed.toggle());
    assert!(!revealed.enabled());
    assert!(!revealed.is_hidden());
}
