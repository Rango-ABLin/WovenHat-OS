//! Kernel-owned admission boundary for the Stage 27.5 recovery environment.
//!
//! This module deliberately does not implement reset, reinstall, snapshot
//! restore, or boot repair. It records the small amount of state that a
//! recovery userspace can inspect and keeps destructive admission behind two
//! independent gates: an explicitly delegated `Recovery` capability and a
//! kernel-authorized recovery boot mode. The boot-mode setter is not exposed
//! to userspace; the QEMU acceptance feature is the only test hook.

use core::sync::atomic::{AtomicBool, Ordering};

pub const OP_STATUS: u64 = 0;
pub const OP_FILESYSTEM_CHECK: u64 = 1;
pub const OP_LOG_READ: u64 = 2;
pub const OP_RESET_ADMISSION: u64 = 3;
pub const OP_REINSTALL_ADMISSION: u64 = 4;

pub const STATUS_BYTES: usize = 32;
pub const CHECK_BYTES: usize = 32;
pub const MAX_LOG_READ: usize = 4096;

/// `u64::MAX` is the established syscall error value. These distinct values
/// let the typed userspace wrapper report denied versus deliberately absent
/// destructive implementation without treating either as success.
pub const ADMISSION_DENIED: u64 = u64::MAX;
pub const ADMISSION_NOT_IMPLEMENTED: u64 = u64::MAX - 2;

static RECOVERY_BOOT_MODE: AtomicBool = AtomicBool::new(false);

/// Whether the boot path has authorized recovery operations.
pub fn boot_mode_authorized() -> bool {
    RECOVERY_BOOT_MODE.load(Ordering::Acquire)
}

/// Test-only handoff used by the QEMU acceptance. A future bootloader/recovery
/// record validator must replace this with a real authenticated boot-mode
/// transition before recovery can be entered on hardware.
#[cfg(feature = "stage27-5-recovery-test")]
pub fn set_test_boot_mode(authorized: bool) {
    RECOVERY_BOOT_MODE.store(authorized, Ordering::Release);
}

/// Encode read-only recovery status into the fixed userspace ABI.
pub fn encode_status(output: &mut [u8; STATUS_BYTES]) {
    output.fill(0);
    let mount = crate::storage::mount_info();
    output[0] = 1; // ABI version
    output[1] = u8::from(boot_mode_authorized());
    output[2] = u8::from(crate::wfs::vfs_mounted());
    output[3] = match mount.status {
        crate::storage::MountLifecycleStatus::Unknown => 0,
        crate::storage::MountLifecycleStatus::NoDevice => 1,
        crate::storage::MountLifecycleStatus::NotFat32 => 2,
        crate::storage::MountLifecycleStatus::Mounted => 3,
        crate::storage::MountLifecycleStatus::Failed => 4,
        crate::storage::MountLifecycleStatus::Unmounted => 5,
    };
    output[4] = u8::from(mount.dirty);
    output[8..16].copy_from_slice(&(mount.imported_entries as u64).to_le_bytes());
    output[16..24].copy_from_slice(&mount.sync_count.to_le_bytes());
    output[24..32].copy_from_slice(&(crate::serial::log_position() as u64).to_le_bytes());
}

/// Encode a successful FAT32 check report into the fixed userspace ABI.
pub fn encode_check(report: &crate::fat32::CheckReport, output: &mut [u8; CHECK_BYTES]) {
    output.fill(0);
    output[0] = 1; // ABI version
    output[1] = u8::from(report.fs_info_matches);
    output[8..16].copy_from_slice(&(report.files as u64).to_le_bytes());
    output[16..24].copy_from_slice(&(report.directories as u64).to_le_bytes());
    output[24..28].copy_from_slice(&report.total_clusters.to_le_bytes());
    output[28..32].copy_from_slice(&report.free_clusters.to_le_bytes());
}
