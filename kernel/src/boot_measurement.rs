//! Stage 15.6 software boot measurement.
//!
//! The bootloader supplies the physical kernel ELF range. After the physical
//! memory mapping is available, hash that immutable range before driver or
//! userspace initialization. TPM event-log extension and firmware Secure Boot
//! status are intentionally separate capabilities and remain hardware work.

use bootloader_api::BootInfo;
use sha2::{Digest, Sha256};

pub const MAX_MEASURED_KERNEL_BYTES: u64 = 64 * 1024 * 1024;

pub fn measure_kernel(boot_info: &BootInfo, physical_memory_offset: u64) -> Option<[u8; 32]> {
    if boot_info.kernel_len == 0 || boot_info.kernel_len > MAX_MEASURED_KERNEL_BYTES {
        return None;
    }
    let virtual_start = physical_memory_offset.checked_add(boot_info.kernel_addr)?;
    let start = usize::try_from(virtual_start).ok()?;
    let length = usize::try_from(boot_info.kernel_len).ok()?;
    // SAFETY: bootloader_api documents kernel_addr/kernel_len as the loaded
    // kernel ELF range. The physical-memory mapping is established before
    // this function is called, and the range is immutable during measurement.
    let image = unsafe { core::slice::from_raw_parts(start as *const u8, length) };
    let mut hasher = Sha256::new();
    hasher.update(image);
    Some(hasher.finalize().into())
}

#[cfg(feature = "qemu-test")]
pub fn self_test() -> bool {
    let mut hasher = Sha256::new();
    hasher.update(b"abc");
    let digest: [u8; 32] = hasher.finalize().into();
    digest
        == [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
}
