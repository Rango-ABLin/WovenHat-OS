//! Bounded shell input work and normalized tablet coordinates.

/// Maximum messages and input events processed before repaint/presentation.
pub const EVENT_BATCH: usize = 32;

/// Map the HID ABI's 0..=32767 axis to logical desktop pixel centers.
/// Clamp malformed values; dimensions of one pixel remain pinned at zero.
pub fn absolute_pointer_axis(value: u16, extent: i32) -> i32 {
    (i64::from(value.min(32767)) * i64::from(extent.saturating_sub(1).max(0)) / 32767) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tablet_reaches_each_logical_edge_at_both_display_scales() {
        for extent in [640, 1280, 400, 800] {
            assert_eq!(absolute_pointer_axis(0, extent), 0);
            assert_eq!(absolute_pointer_axis(32767, extent), extent - 1);
            assert_eq!(absolute_pointer_axis(16384, extent), extent / 2 - 1);
            assert_eq!(absolute_pointer_axis(u16::MAX, extent), extent - 1);
        }
        assert_eq!(absolute_pointer_axis(32767, 1), 0);
        assert_eq!(absolute_pointer_axis(32767, 0), 0);
    }
}
