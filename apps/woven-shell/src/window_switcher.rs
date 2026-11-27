//! Pure bounded selection policy for the shell's Alt+Tab shortcuts.

/// Choose a focus target from a bottom-to-top visible window stack.
///
/// The focused window is the last entry. Forward switching selects the
/// next-most-recent entry; reverse switching selects the opposite end. A
/// stack with fewer than two visible windows has no switching target.
pub const fn target(stack: &[usize], reverse: bool) -> Option<usize> {
    if stack.len() < 2 {
        None
    } else if reverse {
        Some(stack[0])
    } else {
        Some(stack[stack.len() - 2])
    }
}
