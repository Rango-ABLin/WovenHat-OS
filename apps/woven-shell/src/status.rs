//! Bounded system-status indicators for the taskbar and Quick Settings.
//!
//! This module is a presentation model over status the running system exposes.
//! It adds no capability, kernel object or retained userspace pointer; its one
//! new kernel interface is the capability-free read-only status call below:
//!
//! - **Network** uses the new read-only `SystemStatus` syscall (248). It is
//!   capability-free telemetry (no socket authority, no MAC address), unlike
//!   `NetInfo` (43) which requires `NetworkIo` that the desktop does not have.
//!   Its bytes are copied into a local, fixed-layout value; nothing is held
//!   across calls.
//! - **Volume** uses the same telemetry record: version 2 carries audio
//!   readiness and the kernel's last applied output percentage, cached by the
//!   HDA driver when it programs the amplifier. `audio_set_volume()` still
//!   applies local changes; `audio_info()` is only a fallback for an older
//!   version-1 kernel. Muting keeps the last non-zero level so the icon can
//!   restore it.
//! - **Battery** is reported as unavailable. WovenHat has no ACPI battery
//!   driver (the only battery-like code is a Bluetooth LE *service* self-test),
//!   so showing a level would invent data. The glyph is drawn dim with a slash
//!   and the Quick Settings row says so explicitly.

use libwoven::graphics::Canvas;
use libwoven::syscall;

/// Fallback when a version-1 kernel has no volume field: HDA starts at 75%.
const LEGACY_VOLUME_PERCENT: u8 = 75;

/// Layout of the kernel's fixed 12-byte `SystemStatus` telemetry record:
/// ABI version, online flag, IPv4 address, then (from version 2) audio ready
/// and the applied volume percentage. Manual field decoding avoids
/// reinterpreting kernel bytes as a Rust value.
const STATUS_BYTES: usize = 12;
const STATUS_VERSION: usize = 0;
const STATUS_ONLINE: usize = 1;
const STATUS_IPV4: usize = 2;
const STATUS_AUDIO_READY: usize = 6;
const STATUS_VOLUME_PERCENT: usize = 7;
const STATUS_VERSION_1: u8 = 1;
const STATUS_VERSION_2: u8 = 2;

/// The state of the network link, as far as the shell can observe it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkStatus {
    /// The status call is unavailable on this kernel.
    Unavailable,
    /// The kernel has no initialised network device.
    Offline,
    /// A link is up; this is its IPv4 address (all zero if DHCP is pending).
    Online { ipv4: [u8; 4] },
}

/// The state of the audio output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeStatus {
    /// No output device is ready.
    Unavailable,
    /// The shell's current output level and mute state.
    Level { percent: u8, muted: bool },
}

/// Battery state. Only the honest "no battery controller" case exists until an
/// ACPI battery driver is added; adding a level without a source would lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryStatus {
    Unavailable,
}

/// One bounded snapshot of everything the indicators show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub network: NetworkStatus,
    pub volume: VolumeStatus,
    pub battery: BatteryStatus,
}

/// Read the fixed-layout link-status record through the read-only telemetry
/// call. `None` means the call failed or the record is an unknown version; the
/// caller shows "Unavailable", never a guess.
fn read_system_status() -> Option<[u8; STATUS_BYTES]> {
    let mut bytes = [0u8; STATUS_BYTES];
    // SAFETY: `bytes` is a caller-owned, writable 12-byte buffer. The kernel
    // validates the user range and copies exactly its 12-byte telemetry record
    // into it, retaining no pointer after returning.
    let result = unsafe {
        syscall::invoke(
            syscall::SYSTEM_STATUS,
            bytes.as_mut_ptr() as u64,
            STATUS_BYTES as u64,
            0,
        )
    };
    let version = bytes[STATUS_VERSION];
    (result != syscall::ERROR && (version == STATUS_VERSION_1 || version == STATUS_VERSION_2))
        .then_some(bytes)
}

/// Decode a copied telemetry record. Pure so the wire layout can be tested on
/// a host without invoking a WovenHat syscall.
fn network_status(bytes: [u8; STATUS_BYTES]) -> NetworkStatus {
    if bytes[STATUS_VERSION] < STATUS_VERSION_1 {
        return NetworkStatus::Unavailable;
    }
    if bytes[STATUS_ONLINE] == 0 {
        return NetworkStatus::Offline;
    }
    let mut ipv4 = [0u8; 4];
    ipv4.copy_from_slice(&bytes[STATUS_IPV4..STATUS_IPV4 + 4]);
    NetworkStatus::Online { ipv4 }
}

/// Decode the version-2 audio fields. `last_percent` is the level to restore
/// when the kernel reports zero because the output is muted.
fn volume_from_record(bytes: &[u8; STATUS_BYTES], last_percent: u8) -> VolumeStatus {
    if bytes[STATUS_VERSION] != STATUS_VERSION_2 || bytes[STATUS_AUDIO_READY] == 0 {
        return VolumeStatus::Unavailable;
    }
    let applied = bytes[STATUS_VOLUME_PERCENT].min(100);
    if applied == 0 {
        VolumeStatus::Level {
            percent: last_percent,
            muted: true,
        }
    } else {
        VolumeStatus::Level {
            percent: applied,
            muted: false,
        }
    }
}

impl StatusSnapshot {
    /// Build a snapshot by sampling the system now.
    pub fn new() -> Self {
        let mut snapshot = Self {
            network: NetworkStatus::Unavailable,
            volume: VolumeStatus::Unavailable,
            battery: BatteryStatus::Unavailable,
        };
        snapshot.refresh();
        snapshot
    }

    /// Re-sample the read-only telemetry record. The volume percentage now
    /// comes from the kernel's cached gain, so an external change is visible;
    /// a version-1 kernel falls back to the old audio-presence-only view.
    pub fn refresh(&mut self) {
        let record = read_system_status();
        self.network = record.map_or(NetworkStatus::Unavailable, network_status);
        let last_percent = match self.volume {
            VolumeStatus::Level { percent, .. } => percent,
            VolumeStatus::Unavailable => LEGACY_VOLUME_PERCENT,
        };
        self.volume = match record {
            Some(bytes) if bytes[STATUS_VERSION] == STATUS_VERSION_2 => {
                volume_from_record(&bytes, last_percent)
            }
            _ if libwoven::audio_info().is_ok() => VolumeStatus::Level {
                percent: last_percent,
                muted: false,
            },
            _ => VolumeStatus::Unavailable,
        };
        // No battery driver exists yet; keep this explicit for future work.
        self.battery = BatteryStatus::Unavailable;
    }

    /// Apply a 0-100 output level. The kernel returns the amplifier gain it
    /// actually set; a failure marks the output unavailable.
    pub fn set_volume(&mut self, percent: u8) -> bool {
        let level = percent.min(100);
        match libwoven::audio_set_volume(level) {
            Ok(_gain) => {
                let last_percent = match self.volume {
                    VolumeStatus::Level { percent, .. } => percent,
                    VolumeStatus::Unavailable => LEGACY_VOLUME_PERCENT,
                };
                self.volume = if level == 0 {
                    VolumeStatus::Level {
                        percent: last_percent,
                        muted: true,
                    }
                } else {
                    VolumeStatus::Level {
                        percent: level,
                        muted: false,
                    }
                };
                true
            }
            Err(_) => {
                self.volume = VolumeStatus::Unavailable;
                false
            }
        }
    }

    /// Toggle mute. Unmuting restores the level that was set before muting.
    /// Returns the newly applied level, or `None` when there is no output.
    pub fn toggle_mute(&mut self) -> Option<u8> {
        let (percent, muted) = match self.volume {
            VolumeStatus::Level { percent, muted } => (percent, muted),
            VolumeStatus::Unavailable => return None,
        };
        let target = if muted { percent.max(1) } else { 0 };
        self.set_volume(target).then_some(target)
    }
}

/// Write a dotted-quad IPv4 address into `out`; returns the byte length.
pub fn format_ipv4(ipv4: [u8; 4], out: &mut [u8; 15]) -> usize {
    let mut len = 0;
    for (index, octet) in ipv4.iter().enumerate() {
        if index != 0 {
            out[len] = b'.';
            len += 1;
        }
        if *octet >= 100 {
            out[len] = b'0' + octet / 100;
            len += 1;
        }
        if *octet >= 10 {
            out[len] = b'0' + (octet / 10) % 10;
            len += 1;
        }
        out[len] = b'0' + octet % 10;
        len += 1;
    }
    len
}

/// Write a 0-100 percentage into `out`; returns the byte length.
pub fn format_percent(percent: u8, out: &mut [u8; 3]) -> usize {
    let value = percent.min(100);
    let mut len = 0;
    if value == 100 {
        out[..3].copy_from_slice(b"100");
        return 3;
    }
    if value >= 10 {
        out[len] = b'0' + value / 10;
        len += 1;
    }
    out[len] = b'0' + value % 10;
    len + 1
}

// ---- 16x16 tray glyphs --------------------------------------------------
// All three are drawn with bounded integer rectangles into the same `Canvas`
// as the rest of the taskbar. `bg` is the taskbar colour for punching out the
// battery's unfilled interior; `ink` follows the current theme.

fn slash(canvas: &mut Canvas<'_>, x: i32, y: i32, color: u32) {
    for step in 0..10 {
        canvas.fill_rect(x + 3 + step, y + 12 - step, 1, 1, color);
    }
}

/// Wi-Fi-style signal bars: three ascending bars when online, dim bars plus a
/// slash when offline or unavailable.
pub fn draw_network(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    status: NetworkStatus,
    ink: u32,
    dim: u32,
) {
    let (color, good) = match status {
        NetworkStatus::Online { .. } => (ink, true),
        NetworkStatus::Offline => (dim, false),
        NetworkStatus::Unavailable => (dim, false),
    };
    for (index, width) in [4, 8, 12].iter().enumerate() {
        let bar_y = y + 12 - index as i32 * 4;
        canvas.fill_rect(x + 2, bar_y, *width, 2, color);
    }
    if !good {
        slash(canvas, x, y, ink);
    }
}

/// A speaker with one to three level bars; a slash replaces the bars when
/// muted, and the whole glyph is dim when there is no output device.
pub fn draw_volume(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    status: VolumeStatus,
    ink: u32,
    dim: u32,
) {
    let (percent, muted, available) = match status {
        VolumeStatus::Level { percent, muted } => (percent, muted, true),
        VolumeStatus::Unavailable => (0, false, false),
    };
    let color = if available { ink } else { dim };
    // Speaker body and a small stepped cone.
    canvas.fill_rect(x + 2, y + 6, 2, 4, color);
    for (dx, dy, w, h) in [(4, 6, 2, 4), (6, 4, 2, 8), (8, 2, 2, 12)] {
        canvas.fill_rect(x + dx, y + dy, w, h, color);
    }
    if !available {
        slash(canvas, x, y, ink);
        return;
    }
    if muted || percent == 0 {
        slash(canvas, x, y, ink);
        return;
    }
    let bars = if percent >= 67 {
        3
    } else if percent >= 34 {
        2
    } else {
        1
    };
    for index in 0..bars {
        canvas.fill_rect(
            x + 11 + index * 2,
            y + 7 - index,
            1,
            (2 + index * 2) as usize,
            color,
        );
    }
}

/// A battery outline with a proportional fill. No driver means no level: draw
/// the dim outline with a slash, never a made-up charge.
pub fn draw_battery(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    status: BatteryStatus,
    bg: u32,
    ink: u32,
    dim: u32,
) {
    let color = match status {
        BatteryStatus::Unavailable => dim,
    };
    canvas.fill_rect(x + 2, y + 3, 10, 1, color);
    canvas.fill_rect(x + 2, y + 13, 10, 1, color);
    canvas.fill_rect(x + 1, y + 4, 1, 9, color);
    canvas.fill_rect(x + 12, y + 4, 1, 9, color);
    canvas.fill_rect(x + 13, y + 6, 2, 5, color);
    canvas.fill_rect(x + 2, y + 4, 10, 9, bg);
    let BatteryStatus::Unavailable = status;
    slash(canvas, x, y, ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_layout_matches_the_kernel_telemetry_record() {
        // The kernel copies size_of::<SystemStatusRecord>() bytes. If that
        // changes, this decoder and the kernel must be updated together.
        assert_eq!(STATUS_BYTES, 12);
        assert_eq!(STATUS_VERSION, 0);
        assert_eq!(STATUS_ONLINE, 1);
        assert_eq!(STATUS_IPV4, 2);
        assert_eq!(STATUS_AUDIO_READY, 6);
        assert_eq!(STATUS_VOLUME_PERCENT, 7);
    }

    #[test]
    fn decodes_online_offline_and_unknown_versions() {
        let mut bytes = [0u8; STATUS_BYTES];
        assert_eq!(network_status(bytes), NetworkStatus::Unavailable);
        bytes[STATUS_VERSION] = STATUS_VERSION_1;
        assert_eq!(network_status(bytes), NetworkStatus::Offline);
        bytes[STATUS_IPV4..STATUS_IPV4 + 4].copy_from_slice(&[10, 0, 2, 15]);
        bytes[STATUS_ONLINE] = 1;
        assert_eq!(
            network_status(bytes),
            NetworkStatus::Online {
                ipv4: [10, 0, 2, 15]
            }
        );
    }

    #[test]
    fn decodes_version2_volume_and_mute() {
        let mut bytes = [0u8; STATUS_BYTES];
        bytes[STATUS_VERSION] = STATUS_VERSION_2;
        assert_eq!(volume_from_record(&bytes, 75), VolumeStatus::Unavailable);
        bytes[STATUS_AUDIO_READY] = 1;
        bytes[STATUS_VOLUME_PERCENT] = 42;
        assert_eq!(
            volume_from_record(&bytes, 75),
            VolumeStatus::Level {
                percent: 42,
                muted: false
            }
        );
        bytes[STATUS_VOLUME_PERCENT] = 0;
        assert_eq!(
            volume_from_record(&bytes, 42),
            VolumeStatus::Level {
                percent: 42,
                muted: true
            }
        );
    }

    #[test]
    fn formats_ipv4_without_leading_zeroes() {
        let mut out = [0u8; 15];
        let len = format_ipv4([10, 0, 2, 15], &mut out);
        assert_eq!(&out[..len], b"10.0.2.15");
        let len = format_ipv4([255, 255, 255, 255], &mut out);
        assert_eq!(&out[..len], b"255.255.255.255");
    }

    #[test]
    fn formats_percent_boundaries() {
        let mut out = [0u8; 3];
        for (value, expected) in [
            (0, "0"),
            (9, "9"),
            (10, "10"),
            (75, "75"),
            (100, "100"),
            (255, "100"),
        ] {
            let len = format_percent(value, &mut out);
            assert_eq!(&out[..len], expected.as_bytes());
        }
    }
}
