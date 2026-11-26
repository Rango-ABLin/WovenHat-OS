//! Host-side tests for the shell's bounded status model. The shell itself is a
//! `no_std` Ring-3 binary, so its pure model module is included here directly.

#[path = "../src/status.rs"]
mod status;

use libwoven::graphics::Canvas;
use status::{draw_battery, draw_network, draw_volume, BatteryStatus, NetworkStatus, VolumeStatus};

#[test]
fn all_three_glyphs_draw_inside_their_16x16_slot() {
    let mut pixels = [0u32; 16 * 16];
    let mut canvas = Canvas::new(&mut pixels, 0, 0, 16, 16).expect("16x16 canvas");
    draw_network(
        &mut canvas,
        0,
        0,
        NetworkStatus::Online {
            ipv4: [10, 0, 2, 15],
        },
        0xffffff,
        0x808080,
    );
    draw_network(
        &mut canvas,
        0,
        0,
        NetworkStatus::Offline,
        0xffffff,
        0x808080,
    );
    draw_volume(
        &mut canvas,
        0,
        0,
        VolumeStatus::Level {
            percent: 75,
            muted: false,
        },
        0xffffff,
        0x808080,
    );
    draw_volume(
        &mut canvas,
        0,
        0,
        VolumeStatus::Level {
            percent: 75,
            muted: true,
        },
        0xffffff,
        0x808080,
    );
    draw_volume(
        &mut canvas,
        0,
        0,
        VolumeStatus::Unavailable,
        0xffffff,
        0x808080,
    );
    draw_battery(
        &mut canvas,
        0,
        0,
        BatteryStatus::Unavailable,
        0x101010,
        0xffffff,
        0x808080,
    );
}
