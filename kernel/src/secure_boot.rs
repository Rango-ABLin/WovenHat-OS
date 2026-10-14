//! Signed kernel handoff verification for the secure-boot profile.
//!
//! The UEFI builder places this manifest in the bootloader ramdisk. The kernel
//! verifies it against the same measured kernel digest before activating
//! drivers or userspace. Firmware signature database enrollment and TPM PCR
//! extension are separate platform layers.

use bootloader_api::{info::Optional, BootInfo};
use sha2::{Digest, Sha256};

const MAGIC: [u8; 4] = *b"WBOT";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 4 + 2 + 2 + 8 + 32 + 64;
const TEST_BOOT_TRUSTED_PUBLIC_KEY: [u8; 32] = [
    0x57, 0x7d, 0xf0, 0x48, 0x73, 0x37, 0x81, 0xa3, 0x6f, 0xdf, 0x3f, 0x36, 0xad, 0xeb, 0x4f, 0x12,
    0x8b, 0x15, 0x55, 0x1c, 0x49, 0x65, 0xcc, 0x08, 0x60, 0x0c, 0x9f, 0x79, 0x3a, 0xeb, 0x62, 0x8c,
];

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => panic!("invalid boot trust public key"),
    }
}

const fn provisioned_boot_key(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        panic!("boot trust public key must be 64 hex digits");
    }
    let mut key = [0u8; 32];
    let mut index = 0;
    while index < 32 {
        key[index] = (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1]);
        index += 1;
    }
    key
}

const BOOT_TRUSTED_PUBLIC_KEY: [u8; 32] = match option_env!("WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX")
{
    Some(value) => provisioned_boot_key(value),
    None => TEST_BOOT_TRUSTED_PUBLIC_KEY,
};

pub fn verify_manifest(boot_info: &BootInfo) -> bool {
    let Optional::Some(ramdisk_addr) = boot_info.ramdisk_addr else {
        return false;
    };
    if boot_info.ramdisk_len != HEADER_LEN as u64 {
        return false;
    }
    // `BootInfo::ramdisk_addr` is the bootloader-created virtual mapping;
    // unlike `kernel_addr`, it must not be rebased through the physical-memory
    // offset a second time.
    let Ok(start) = usize::try_from(ramdisk_addr) else {
        return false;
    };
    // SAFETY: the bootloader owns the immutable ramdisk range described by
    // BootInfo. The physical-memory mapping is active before this check.
    let manifest = unsafe { core::slice::from_raw_parts(start as *const u8, HEADER_LEN) };
    if manifest[..4] != MAGIC
        || u16::from_le_bytes([manifest[4], manifest[5]]) != VERSION
        || u16::from_le_bytes([manifest[6], manifest[7]]) as usize != HEADER_LEN
    {
        return false;
    }
    let image_len = u64::from_le_bytes([
        manifest[8],
        manifest[9],
        manifest[10],
        manifest[11],
        manifest[12],
        manifest[13],
        manifest[14],
        manifest[15],
    ]);
    let Ok(image_len) = usize::try_from(image_len) else {
        return false;
    };
    if image_len == 0 || image_len > 64 * 1024 * 1024 {
        return false;
    }
    let Optional::Some(physical_memory_offset) = boot_info.physical_memory_offset else {
        return false;
    };
    let Some(raw_start) = physical_memory_offset.checked_add(boot_info.kernel_addr) else {
        return false;
    };
    let Ok(raw_start) = usize::try_from(raw_start) else {
        return false;
    };
    let Ok(raw_len) = usize::try_from(boot_info.kernel_len) else {
        return false;
    };
    if !(64..=64 * 1024 * 1024).contains(&raw_len) {
        return false;
    }
    // SAFETY: BootInfo describes the immutable physical kernel ELF and the
    // physical-memory mapping is active before this early check.
    let raw = unsafe { core::slice::from_raw_parts(raw_start as *const u8, raw_len) };
    if raw[..4] != *b"\x7fELF" || raw[4] != 2 || raw[5] != 1 {
        return false;
    }
    let read_u16 = |offset: usize| u16::from_le_bytes([raw[offset], raw[offset + 1]]);
    let read_u32 = |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
    let read_u64 = |offset: usize| u64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap());
    let Ok(ph_offset) = usize::try_from(read_u64(32)) else {
        return false;
    };
    let ph_size = usize::from(read_u16(54));
    let ph_count = usize::from(read_u16(56));
    if ph_size < 56 || ph_offset.saturating_add(ph_count.saturating_mul(ph_size)) > raw.len() {
        return false;
    }
    let mut hasher = Sha256::new();
    let mut canonical_len = 0usize;
    let zeros = [0u8; 4096];
    for index in 0..ph_count {
        let offset = ph_offset + index * ph_size;
        let flags = read_u32(offset + 4);
        if read_u32(offset) != 1 || flags & 4 == 0 || flags & 2 != 0 {
            continue;
        }
        let Ok(file_offset) = usize::try_from(read_u64(offset + 8)) else {
            return false;
        };
        let Ok(file_size) = usize::try_from(read_u64(offset + 32)) else {
            return false;
        };
        let Ok(memory_size) = usize::try_from(read_u64(offset + 40)) else {
            return false;
        };
        if file_size > memory_size || file_offset.saturating_add(file_size) > raw.len() {
            return false;
        }
        hasher.update(&raw[file_offset..file_offset + file_size]);
        let mut remaining = memory_size - file_size;
        while remaining != 0 {
            let count = core::cmp::min(remaining, zeros.len());
            hasher.update(&zeros[..count]);
            remaining -= count;
        }
        canonical_len = canonical_len.saturating_add(memory_size);
    }
    if canonical_len != image_len {
        return false;
    }
    let digest: [u8; 32] = hasher.finalize().into();
    if manifest[16..48] != digest {
        return false;
    }
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&manifest[48..112]);
    let verified = crate::signed_app::verify(&digest, &BOOT_TRUSTED_PUBLIC_KEY, &signature).is_ok();
    verified
}
