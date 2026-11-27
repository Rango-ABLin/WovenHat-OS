//! Bounded GPT relocation and installer disk authority, shared by host tests.
#![no_std]
extern crate alloc;
use alloc::vec::Vec;

pub const SECTOR: usize = 512;
pub const TABLE_SIZE: usize = 128 * 128;
pub const END: [u8; 4] = [0x7f, 0xff, 4, 0];

/// A whole-disk device path must be a strict ancestor of the source partition.
/// Missing, empty, multi-instance, or malformed paths are never usable identities.
pub fn disk_path(path: &[u8]) -> Option<&[u8]> {
    let mut offset = 0;
    while offset + 4 <= path.len() {
        let node = &path[offset..];
        let len = usize::from(u16::from_le_bytes([node[2], node[3]]));
        if node[0] == 0x7f {
            return (node == END && offset > 0).then_some(&path[..offset]);
        }
        if len < 4 || offset.checked_add(len)? > path.len() {
            return None;
        }
        offset += len;
    }
    None
}

pub fn source_disk(candidate: &[u8], source: &[u8]) -> Option<bool> {
    Some(disk_path(source)?.starts_with(disk_path(candidate)?))
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn put64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
fn checksum(h: &mut [u8]) {
    h[16..20].fill(0);
    let crc = crc32(&h[..92]);
    h[16..20].copy_from_slice(&crc.to_le_bytes());
}

pub struct InstallPlan {
    /// Original boot sectors and ESP, with relocated primary header.
    pub prefix: Vec<u8>,
    pub backup_table: Vec<u8>,
    pub backup_header: [u8; SECTOR],
    pub table_lba: u64,
    pub last_lba: u64,
}

/// Accept only our canonical single-ESP, 512-byte GPT image. Keep the ESP size
/// unchanged and relocate the backup GPT to the *actual* end of the target.
pub fn plan(image: &[u8], target_sectors: u64) -> Result<InstallPlan, &'static str> {
    if image.len() < 68 * SECTOR || !image.len().is_multiple_of(SECTOR) {
        return Err("invalid image size");
    }
    let sectors = (image.len() / SECTOR) as u64;
    if target_sectors < sectors {
        return Err("disk too small");
    }
    let h = &image[SECTOR..2 * SECTOR];
    if &image[510..512] != b"\x55\xaa"
        || image[450] != 0xee
        || &h[..8] != b"EFI PART"
        || u32_at(h, 8) != 0x10000
        || u32_at(h, 12) != 92
        || u32_at(h, 20) != 0
        || u64_at(h, 24) != 1
        || u64_at(h, 32) != sectors - 1
        || u64_at(h, 40) != 34
        || u64_at(h, 48) != sectors - 34
        || u64_at(h, 72) != 2
        || u32_at(h, 80) != 128
        || u32_at(h, 84) != 128
    {
        return Err("unsupported GPT layout");
    }
    let mut header = h.to_vec();
    checksum(&mut header);
    if header[16..20] != h[16..20] {
        return Err("bad GPT header checksum");
    }
    let table = &image[2 * SECTOR..2 * SECTOR + TABLE_SIZE];
    if crc32(table) != u32_at(h, 88) {
        return Err("bad GPT table checksum");
    }
    let esp = [
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
    ];
    if table[..16] != esp
        || table[128..].iter().any(|v| *v != 0)
        || u64_at(table, 32) < 34
        || u64_at(table, 32) > u64_at(table, 40)
        || u64_at(table, 40) > sectors - 34
    {
        return Err("unsupported ESP layout");
    }
    let backup = &image[image.len() - SECTOR..];
    let mut check = backup.to_vec();
    checksum(&mut check);
    if &backup[..8] != b"EFI PART"
        || check[16..20] != backup[16..20]
        || u64_at(backup, 24) != sectors - 1
        || u64_at(backup, 32) != 1
        || u64_at(backup, 72) != sectors - 33
        || image[image.len() - 33 * SECTOR..image.len() - SECTOR] != *table
    {
        return Err("bad backup GPT");
    }
    let last = target_sectors - 1;
    let mut prefix = image[..image.len() - 33 * SECTOR].to_vec();
    let mbr_len = u32::try_from(last).unwrap_or(u32::MAX);
    prefix[458..462].copy_from_slice(&mbr_len.to_le_bytes());
    put64(&mut prefix, SECTOR + 32, last);
    put64(&mut prefix, SECTOR + 48, last - 33);
    checksum(&mut prefix[SECTOR..2 * SECTOR]);
    let mut backup_header = [0; SECTOR];
    backup_header.copy_from_slice(&prefix[SECTOR..2 * SECTOR]);
    put64(&mut backup_header, 24, last);
    put64(&mut backup_header, 32, 1);
    put64(&mut backup_header, 72, last - 32);
    checksum(&mut backup_header);
    Ok(InstallPlan {
        prefix,
        backup_table: table.to_vec(),
        backup_header,
        table_lba: last - 32,
        last_lba: last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authority_rejects_source_alias_and_unknown_paths() {
        let disk = [1, 1, 6, 0, 0, 2, 0x7f, 0xff, 4, 0];
        let source = [1, 1, 6, 0, 0, 2, 4, 1, 4, 0, 0x7f, 0xff, 4, 0];
        assert_eq!(source_disk(&disk, &source), Some(true));
        assert_eq!(source_disk(&source, &source), Some(true));
        let other = [1, 1, 6, 0, 0, 3, 0x7f, 0xff, 4, 0];
        assert_eq!(source_disk(&other, &source), Some(false));
        assert_eq!(source_disk(&END, &source), None);
        assert_eq!(source_disk(&disk, &[1, 1, 0, 0]), None);
    }
    #[test]
    fn checksum_vector_and_malformed_images() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        assert!(plan(&[0; 512], 100).is_err());
        assert!(plan(&[0; 512 * 100], 100).is_err());
    }
    fn fixture() -> Vec<u8> {
        let mut b = alloc::vec![0;100*SECTOR];
        b[510..512].copy_from_slice(b"\x55\xaa");
        b[450] = 0xee;
        let t = &mut b[1024..1024 + TABLE_SIZE];
        t[..16].copy_from_slice(&[
            0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0, 0xa0, 0xc9, 0x3e, 0xc9,
            0x3b,
        ]);
        put64(t, 32, 34);
        put64(t, 40, 60);
        let crc = crc32(t);
        let h = &mut b[SECTOR..2 * SECTOR];
        h[..8].copy_from_slice(b"EFI PART");
        h[8..12].copy_from_slice(&0x10000u32.to_le_bytes());
        h[12..16].copy_from_slice(&92u32.to_le_bytes());
        put64(h, 24, 1);
        put64(h, 32, 99);
        put64(h, 40, 34);
        put64(h, 48, 66);
        put64(h, 72, 2);
        h[80..84].copy_from_slice(&128u32.to_le_bytes());
        h[84..88].copy_from_slice(&128u32.to_le_bytes());
        h[88..92].copy_from_slice(&crc.to_le_bytes());
        checksum(h);
        b.copy_within(1024..1024 + TABLE_SIZE, 67 * SECTOR);
        b.copy_within(SECTOR..2 * SECTOR, 99 * SECTOR);
        let h = &mut b[99 * SECTOR..];
        put64(h, 24, 99);
        put64(h, 32, 1);
        put64(h, 72, 67);
        checksum(h);
        b
    }
    #[test]
    fn relocate_backup_and_preserve_esp() {
        let b = fixture();
        let p = plan(&b, 1000).unwrap();
        assert_eq!(p.table_lba, 967);
        assert_eq!(p.last_lba, 999);
        assert_eq!(u64_at(&p.prefix, SECTOR + 32), 999);
        assert_eq!(u64_at(&p.prefix, SECTOR + 48), 966);
        assert_eq!(u64_at(&p.backup_header, 24), 999);
        assert_eq!(u64_at(&p.backup_header, 32), 1);
        assert_eq!(u64_at(&p.backup_header, 72), 967);
        assert_eq!(&p.prefix[1024..], &b[1024..67 * SECTOR]);
        let mut h = p.backup_header;
        checksum(&mut h);
        assert_eq!(h, p.backup_header);
        assert!(plan(&b, 99).is_err());
        for offset in [SECTOR + 16, 1024, 99 * SECTOR + 16, 67 * SECTOR] {
            let mut bad = b.clone();
            bad[offset] ^= 1;
            assert!(plan(&bad, 1000).is_err());
        }
    }
}
