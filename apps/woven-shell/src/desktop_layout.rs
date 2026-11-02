//! Pure, allocation-free desktop geometry for the WovenShell session.
//! This module deliberately has no framebuffer ownership or kernel dependency.

pub const TASKBAR_HEIGHT: i32 = 50;
pub const START_SIZE: i32 = 36;
pub const APP_SLOT: i32 = 40;
pub const APP_GAP: i32 = 8;
pub const TASKBAR_PADDING: i32 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn taskbar(width: i32, height: i32) -> Rect {
    Rect {
        x: 0,
        y: height.saturating_sub(TASKBAR_HEIGHT).max(0),
        width: width.max(0),
        height: height.clamp(0, TASKBAR_HEIGHT),
    }
}

pub fn work_area(width: i32, height: i32) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: width.max(0),
        height: height.saturating_sub(TASKBAR_HEIGHT).max(0),
    }
}

pub fn start_button(height: i32) -> Rect {
    Rect {
        x: 8,
        y: taskbar(0, height).y + TASKBAR_PADDING,
        width: START_SIZE,
        height: START_SIZE,
    }
}

/// Centre the app slots when there is sufficient room. Reserve a right-hand
/// status area and the Start button; never let icons paint over either.
pub fn app_slot(
    width: i32,
    height: i32,
    index: usize,
    total: usize,
    status_width: i32,
) -> Rect {
    let left = 8 + START_SIZE + APP_GAP;
    let right = width.saturating_sub(status_width.max(0) + APP_GAP);
    let available = right.saturating_sub(left).max(0);
    let stride = APP_SLOT + APP_GAP;
    let count = (total.min(256)) as i32;
    let wanted = if count == 0 { 0 } else { count * stride - APP_GAP };
    let origin = left + (available - wanted).max(0) / 2;
    let x = origin.saturating_add((index.min(256) as i32).saturating_mul(stride));
    Rect {
        x,
        y: taskbar(0, height).y + (TASKBAR_HEIGHT - APP_SLOT) / 2,
        width: APP_SLOT,
        height: APP_SLOT,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Snap {
    Left,
    Right,
    Maximized,
}

pub fn snap_rect(width: i32, height: i32, kind: Snap) -> Rect {
    let work = work_area(width, height);
    let half = work.width / 2;
    match kind {
        Snap::Left => Rect { x: 0, y: 0, width: half, height: work.height },
        Snap::Right => Rect {
            x: half,
            y: 0,
            width: work.width - half,
            height: work.height,
        },
        Snap::Maximized => work,
    }
}

pub fn toast_rect(width: i32, height: i32, toast_width: i32, toast_height: i32) -> Rect {
    Rect {
        x: width.saturating_sub(toast_width + 8).max(0),
        y: taskbar(width, height).y.saturating_sub(toast_height + 8).max(0),
        width: toast_width.max(0),
        height: toast_height.max(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_bar_has_exact_height() {
        assert_eq!(taskbar(1920, 1080), Rect { x: 0, y: 1030, width: 1920, height: 50 });
        assert_eq!(start_button(1080).y, 1037);
    }

    #[test]
    fn snap_excludes_taskbar() {
        assert_eq!(snap_rect(1920, 1080, Snap::Left), Rect { x: 0, y: 0, width: 960, height: 1030 });
        assert_eq!(snap_rect(1920, 1080, Snap::Right), Rect { x: 960, y: 0, width: 960, height: 1030 });
        assert_eq!(snap_rect(1920, 1080, Snap::Maximized).height, 1030);
    }

    #[test]
    fn icons_are_centred_and_clear_start_and_status() {
        let a = app_slot(1920, 1080, 0, 4, 240);
        let b = app_slot(1920, 1080, 1, 4, 240);
        assert_eq!(b.x - a.x, 48);
        assert_eq!(a.width, 40);
        assert!(a.x >= 52);
        assert!(app_slot(1920, 1080, 3, 4, 240).x + 40 <= 1920 - 240);
    }

    #[test]
    fn toast_stays_above_bar() {
        let toast = toast_rect(1920, 1080, 180, 50);
        assert_eq!(toast.y + toast.height, 1022);
    }

    #[test]
    fn small_screens_never_produce_negative_geometry() {
        assert_eq!(work_area(160, 40).height, 0);
        assert_eq!(taskbar(160, 40).height, 40);
        assert_eq!(toast_rect(160, 40, 180, 50).x, 0);
    }
}
