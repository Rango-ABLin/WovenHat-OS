//! Capability-free, cross-feature view of the software audio output state.
//!
//! The HDA driver is compiled out of some `qemu-test` configurations, but the
//! read-only `SystemStatus` syscall must still build there. This module is
//! therefore always compiled: the driver publishes the state it knows, and
//! telemetry publishes a truthful "no ready output" view when it is absent.
//! No hardware access or authority lives here.

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Set once the bound controller has a prepared output path.
pub static OUTPUT_READY: AtomicBool = AtomicBool::new(false);
/// Last output level applied by the HDA driver, in percent (0 = muted).
pub static OUTPUT_VOLUME_PERCENT: AtomicU8 = AtomicU8::new(0);

/// Read (ready, volume_percent) without touching the controller.
pub fn audio_status() -> (u8, u8) {
    (
        u8::from(OUTPUT_READY.load(Ordering::Acquire)),
        OUTPUT_VOLUME_PERCENT.load(Ordering::Acquire),
    )
}
