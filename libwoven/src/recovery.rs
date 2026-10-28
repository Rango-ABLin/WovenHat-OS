//! Bounded userspace API for the kernel-authorized Stage 27.5 recovery mode.
//!
//! Read-only status, filesystem checking, and log retrieval are available
//! only to a recovery-capability process while the kernel has authorized the
//! recovery boot mode. Reset and reinstall are admission probes only; the
//! kernel deliberately reports them as unimplemented until a real boot target
//! and installer transaction exists.

pub const OP_STATUS: u64 = 0;
pub const OP_FILESYSTEM_CHECK: u64 = 1;
pub const OP_LOG_READ: u64 = 2;
pub const OP_RESET_ADMISSION: u64 = 3;
pub const OP_REINSTALL_ADMISSION: u64 = 4;
pub const STATUS_BYTES: usize = 32;
pub const CHECK_BYTES: usize = 32;
pub const MAX_LOG_READ: usize = 4096;

const ADMISSION_NOT_IMPLEMENTED: u64 = u64::MAX - 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub abi_version: u8,
    pub boot_mode_authorized: bool,
    pub vfs_mounted: bool,
    pub mount_status: u8,
    pub dirty: bool,
    pub imported_entries: u64,
    pub sync_count: u64,
    pub log_position: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckReport {
    pub abi_version: u8,
    pub fs_info_matches: bool,
    pub files: u64,
    pub directories: u64,
    pub total_clusters: u32,
    pub free_clusters: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    NotImplemented,
}

fn decode_status(bytes: &[u8; STATUS_BYTES]) -> Status {
    Status {
        abi_version: bytes[0],
        boot_mode_authorized: bytes[1] != 0,
        vfs_mounted: bytes[2] != 0,
        mount_status: bytes[3],
        dirty: bytes[4] != 0,
        imported_entries: u64::from_le_bytes(bytes[8..16].try_into().unwrap_or([0; 8])),
        sync_count: u64::from_le_bytes(bytes[16..24].try_into().unwrap_or([0; 8])),
        log_position: u64::from_le_bytes(bytes[24..32].try_into().unwrap_or([0; 8])),
    }
}

fn decode_check(bytes: &[u8; CHECK_BYTES]) -> CheckReport {
    CheckReport {
        abi_version: bytes[0],
        fs_info_matches: bytes[1] != 0,
        files: u64::from_le_bytes(bytes[8..16].try_into().unwrap_or([0; 8])),
        directories: u64::from_le_bytes(bytes[16..24].try_into().unwrap_or([0; 8])),
        total_clusters: u32::from_le_bytes(bytes[24..28].try_into().unwrap_or([0; 4])),
        free_clusters: u32::from_le_bytes(bytes[28..32].try_into().unwrap_or([0; 4])),
    }
}

/// Query the recovery status record.
pub fn status() -> Result<Status, crate::Error> {
    let mut bytes = [0u8; STATUS_BYTES];
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::RECOVERY_QUERY,
            OP_STATUS,
            bytes.as_mut_ptr() as u64,
            0,
        )
    })?;
    Ok(decode_status(&bytes))
}

/// Run the kernel's read-only FAT32 consistency checker against the mounted
/// primary volume. A missing or unsupported recovery volume is an error; the
/// caller must not infer that it was checked.
pub fn filesystem_check() -> Result<CheckReport, crate::Error> {
    let mut bytes = [0u8; CHECK_BYTES];
    crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::RECOVERY_QUERY,
            OP_FILESYSTEM_CHECK,
            bytes.as_mut_ptr() as u64,
            0,
        )
    })?;
    Ok(decode_check(&bytes))
}

/// Retrieve at most [`MAX_LOG_READ`] bytes from the bounded kernel log ring.
pub fn read_log(offset: u64, output: &mut [u8; MAX_LOG_READ]) -> Result<usize, crate::Error> {
    let result = crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::RECOVERY_QUERY,
            OP_LOG_READ,
            output.as_mut_ptr() as u64,
            offset,
        )
    })?;
    usize::try_from(result).map_err(|_| crate::Error::Failed)
}

/// Ask the kernel whether the named destructive action is admitted. A return
/// of `Ok` means only that the kernel-authorized recovery mode exists; this
/// foundation still performs no destructive operation.
pub fn admit(operation: u64) -> Result<Admission, crate::Error> {
    if !matches!(operation, OP_RESET_ADMISSION | OP_REINSTALL_ADMISSION) {
        return Err(crate::Error::Failed);
    }
    let result = unsafe { crate::syscall::invoke(crate::syscall::RECOVERY_QUERY, operation, 0, 0) };
    if result == ADMISSION_NOT_IMPLEMENTED {
        Ok(Admission::NotImplemented)
    } else {
        Err(crate::Error::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_decode_is_fixed_and_bounded() {
        let mut bytes = [0u8; STATUS_BYTES];
        bytes[0] = 1;
        bytes[1] = 1;
        bytes[8..16].copy_from_slice(&7u64.to_le_bytes());
        bytes[24..32].copy_from_slice(&99u64.to_le_bytes());
        let value = decode_status(&bytes);
        assert_eq!(value.abi_version, 1);
        assert!(value.boot_mode_authorized);
        assert_eq!(value.imported_entries, 7);
        assert_eq!(value.log_position, 99);
    }

    #[test]
    fn invalid_admission_operation_is_rejected_before_syscall() {
        assert!(matches!(admit(OP_STATUS), Err(crate::Error::Failed)));
    }
}
