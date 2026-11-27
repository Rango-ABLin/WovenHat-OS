//! Host-side tests for bounded Alt+Tab target selection.

#[path = "../src/window_switcher.rs"]
mod window_switcher;

#[test]
fn empty_and_single_window_stacks_do_not_switch() {
    assert_eq!(window_switcher::target(&[], false), None);
    assert_eq!(window_switcher::target(&[4], false), None);
    assert_eq!(window_switcher::target(&[4], true), None);
}

#[test]
fn forward_selects_next_most_recent_window() {
    assert_eq!(window_switcher::target(&[2, 7, 4], false), Some(7));
}

#[test]
fn reverse_selects_opposite_end_of_stack() {
    assert_eq!(window_switcher::target(&[2, 7, 4], true), Some(2));
}
