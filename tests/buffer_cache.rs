// Standalone host harness: rustc --test tests/buffer_cache.rs -o target/buffer-cache-tests.exe
#![allow(dead_code)]
#[path = "../kernel/src/block.rs"]
mod block;
#[path = "../kernel/src/block_cache.rs"]
mod block_cache;
#[path = "../kernel/src/partition.rs"]
mod partition;

/// Proves that `PartitionDevice::new` and `find_fat32` enforce every
/// partition-geometry boundary that would otherwise allow writes to arbitrary
/// sectors (zero start_lba, zero sector count, u64 overflow, end past disk,
/// missing MBR signature, invalid boot flag, and partition extending past disk).
#[test]
fn partition_device_refuses_invalid_geometry_and_find_fat32_rejects_bad_mbr() {
    use block::BlockDevice;
    // 1. PartitionDevice::new: start_lba == 0 is refused (MBR sector is off-limits).
    let mut disk = block::RamDisk::<8>::new();
    assert_eq!(
        partition::PartitionDevice::new(
            &mut disk,
            partition::Partition {
                start_lba: 0,
                sectors: 2,
                kind: 0x0b
            },
        )
        .err(),
        Some(partition::Error::OutOfBounds)
    );
    // 2. PartitionDevice::new: sectors == 0 is refused.
    assert_eq!(
        partition::PartitionDevice::new(
            &mut disk,
            partition::Partition {
                start_lba: 1,
                sectors: 0,
                kind: 0x0b
            },
        )
        .err(),
        Some(partition::Error::OutOfBounds)
    );
    // 3. PartitionDevice::new: start_lba + sectors overflows u64.
    assert_eq!(
        partition::PartitionDevice::new(
            &mut disk,
            partition::Partition {
                start_lba: u64::MAX,
                sectors: 1,
                kind: 0x0b
            },
        )
        .err(),
        Some(partition::Error::OutOfBounds)
    );
    // 4. PartitionDevice::new: partition end (5+4=9) exceeds disk size (8 sectors).
    assert_eq!(
        partition::PartitionDevice::new(
            &mut disk,
            partition::Partition {
                start_lba: 5,
                sectors: 4,
                kind: 0x0b
            },
        )
        .err(),
        Some(partition::Error::OutOfBounds)
    );
    // 5. find_fat32: bytes 510-511 are zero → no MBR signature → Ok(None).
    let mut nosig = block::RamDisk::<8>::new();
    assert_eq!(partition::find_fat32(&mut nosig), Ok(None));
    // 6. find_fat32: invalid boot flag (0x40, not 0x00 or 0x80) → Err(InvalidTable).
    let mut bad_flag = block::RamDisk::<8>::new();
    let mut s = [0u8; block::SECTOR_SIZE];
    s[510] = 0x55;
    s[511] = 0xaa;
    s[446] = 0x40; // invalid: neither 0x00 nor 0x80
    bad_flag.write_sector(0, &s).unwrap();
    assert_eq!(
        partition::find_fat32(&mut bad_flag),
        Err(partition::Error::InvalidTable)
    );
    // 7. find_fat32: FAT32 entry with start_lba == 0 → Err(OutOfBounds).
    let mut zero_start = block::RamDisk::<8>::new();
    s = [0u8; block::SECTOR_SIZE];
    s[510] = 0x55;
    s[511] = 0xaa;
    s[446] = 0x80; // bootable
    s[446 + 4] = 0x0c; // FAT32 LBA type
    s[446 + 8..446 + 12].copy_from_slice(&0u32.to_le_bytes()); // start_lba = 0
    s[446 + 12..446 + 16].copy_from_slice(&1u32.to_le_bytes()); // sectors = 1
    zero_start.write_sector(0, &s).unwrap();
    assert_eq!(
        partition::find_fat32(&mut zero_start),
        Err(partition::Error::OutOfBounds)
    );
    // 8. find_fat32: partition end (5+5=10) exceeds disk size (8) → Err(OutOfBounds).
    let mut past_end = block::RamDisk::<8>::new();
    s = [0u8; block::SECTOR_SIZE];
    s[510] = 0x55;
    s[511] = 0xaa;
    s[446] = 0x80;
    s[446 + 4] = 0x0c;
    s[446 + 8..446 + 12].copy_from_slice(&5u32.to_le_bytes()); // start_lba = 5
    s[446 + 12..446 + 16].copy_from_slice(&5u32.to_le_bytes()); // sectors = 5
    past_end.write_sector(0, &s).unwrap();
    assert_eq!(
        partition::find_fat32(&mut past_end),
        Err(partition::Error::OutOfBounds)
    );
    // 9. find_fat32: valid MBR with only non-FAT32 entries (NTFS type 0x07) → Ok(None).
    let mut ntfs = block::RamDisk::<8>::new();
    s = [0u8; block::SECTOR_SIZE];
    s[510] = 0x55;
    s[511] = 0xaa;
    s[446] = 0x80;
    s[446 + 4] = 0x07; // NTFS, not FAT32
    s[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes()); // start_lba = 1
    s[446 + 12..446 + 16].copy_from_slice(&4u32.to_le_bytes()); // sectors = 4
    ntfs.write_sector(0, &s).unwrap();
    assert_eq!(partition::find_fat32(&mut ntfs), Ok(None));
}

#[test]
fn partition_flush_reaches_cache_and_device() {
    use block::BlockDevice;
    let mut disk = block::RamDisk::<4>::new();
    {
        let mut cache = block_cache::CachedDevice::<_, 2>::new(&mut disk);
        {
            let part = partition::Partition {
                start_lba: 1,
                sectors: 2,
                kind: 0x0b,
            };
            let mut view = partition::PartitionDevice::new(&mut cache, part)
                .ok()
                .unwrap();
            assert!(view.write_sector(0, &[4; block::SECTOR_SIZE]).is_ok());
            assert!(view.flush().is_ok());
        }
        assert_eq!(cache.stats().dirty, 0);
    }
    let mut out = [0; block::SECTOR_SIZE];
    assert!(disk.read_sector(1, &mut out).is_ok());
    assert_eq!(out, [4; block::SECTOR_SIZE]);
}
