//! Build disposable configuration fixtures with the production image builder.

use std::{env, fs, path::{Path, PathBuf}};

const FAT_DIRECTORY: u8 = 0x10;
const FAT_END_OF_CHAIN: u32 = 0x0fff_ffff;

struct FatLayout {
    bytes_per_sector: usize,
    sectors_per_cluster: usize,
    first_fat: usize,
    root_directory: usize,
    fat_entry_bytes: usize,
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    let value = bytes.get(offset..offset.checked_add(2).ok_or("FAT offset overflow")?)
        .ok_or("truncated FAT field")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let value = bytes.get(offset..offset.checked_add(4).ok_or("FAT offset overflow")?)
        .ok_or("truncated FAT field")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, &'static str> {
    let value = bytes.get(offset..offset.checked_add(8).ok_or("FAT offset overflow")?)
        .ok_or("truncated GPT field")?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn fat_layout(image: &[u8]) -> Result<FatLayout, &'static str> {
    let volume_start = if image.get(512..520) == Some(b"EFI PART") {
        let entries_lba = usize::try_from(le_u64(image, 512 + 72)?).map_err(|_| "GPT entry table too large")?;
        let entry_count = usize::try_from(le_u32(image, 512 + 80)?).map_err(|_| "GPT entry count too large")?;
        let entry_size = usize::try_from(le_u32(image, 512 + 84)?).map_err(|_| "GPT entry size too large")?;
        if entry_size < 56 || entry_count == 0 {
            return Err("unsupported GPT layout");
        }
        let entries = entries_lba.checked_mul(512).ok_or("GPT offset overflow")?;
        let mut first_partition = None;
        for index in 0..entry_count {
            let offset = entries.checked_add(index.checked_mul(entry_size).ok_or("GPT entry overflow")?)
                .ok_or("GPT entry overflow")?;
            let entry = image.get(offset..offset.checked_add(entry_size).ok_or("GPT entry overflow")?)
                .ok_or("truncated GPT entry")?;
            if entry[..16].iter().any(|byte| *byte != 0) {
                let start_lba = usize::try_from(le_u64(entry, 32)?).map_err(|_| "GPT partition too large")?;
                first_partition = Some(start_lba.checked_mul(512).ok_or("GPT partition offset overflow")?);
                break;
            }
        }
        first_partition.ok_or("GPT has no usable partition")?
    } else {
        0
    };
    if image.get(volume_start.checked_add(510).ok_or("FAT offset overflow")?..volume_start.checked_add(512).ok_or("FAT offset overflow")?) != Some(&[0x55, 0xaa]) {
        return Err("fixture image is not a FAT volume");
    }
    let bytes_per_sector = usize::from(le_u16(image, volume_start + 11)?);
    let sectors_per_cluster = usize::from(*image.get(volume_start + 13).ok_or("truncated FAT BPB")?);
    let reserved_sectors = usize::from(le_u16(image, volume_start + 14)?);
    let fats = usize::from(*image.get(volume_start + 16).ok_or("truncated FAT BPB")?);
    let fat32_sectors = usize::try_from(le_u32(image, volume_start + 36)?).map_err(|_| "FAT too large")?;
    let root_entries = usize::from(le_u16(image, volume_start + 17)?);
    let (fat_sectors, fat_entry_bytes) = if root_entries == 0 {
        (fat32_sectors, 4)
    } else {
        (usize::from(le_u16(image, volume_start + 22)?), 2)
    };
    let root_cluster = usize::try_from(le_u32(image, volume_start + 44)?).map_err(|_| "root cluster too large")?;
    if bytes_per_sector < 512 || !bytes_per_sector.is_power_of_two()
        || sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two()
        || fats == 0 || fat_sectors == 0 {
        return Err("unsupported FAT layout");
    }
    let first_fat = volume_start.checked_add(reserved_sectors.checked_mul(bytes_per_sector).ok_or("FAT offset overflow")?)
        .ok_or("FAT offset overflow")?;
    let root_directory_sector = reserved_sectors.checked_add(fats.checked_mul(fat_sectors).ok_or("FAT offset overflow")?)
        .ok_or("FAT offset overflow")?;
    let cluster_bytes = sectors_per_cluster.checked_mul(bytes_per_sector).ok_or("cluster size overflow")?;
    let root_directory = if root_entries == 0 {
        if root_cluster < 2 {
            return Err("unsupported FAT32 root cluster");
        }
        let data_start = volume_start.checked_add(root_directory_sector.checked_mul(bytes_per_sector).ok_or("FAT offset overflow")?)
            .ok_or("FAT offset overflow")?;
        data_start.checked_add((root_cluster - 2).checked_mul(cluster_bytes).ok_or("root offset overflow")?)
            .ok_or("root offset overflow")?
    } else {
        volume_start.checked_add(root_directory_sector.checked_mul(bytes_per_sector).ok_or("root offset overflow")?)
            .ok_or("root offset overflow")?
    };
    if root_directory.checked_add(cluster_bytes).ok_or("root extent overflow")? > image.len()
        || first_fat.checked_add(fat_sectors.checked_mul(bytes_per_sector).ok_or("FAT extent overflow")?).ok_or("FAT extent overflow")? > image.len() {
        return Err("FAT layout extends beyond fixture image");
    }
    Ok(FatLayout { bytes_per_sector, sectors_per_cluster, first_fat, root_directory, fat_entry_bytes })
}

fn root_file_entry(image: &[u8], layout: &FatLayout, filename: &[u8; 11]) -> Result<usize, &'static str> {
    let cluster_bytes = layout.sectors_per_cluster.checked_mul(layout.bytes_per_sector).ok_or("cluster size overflow")?;
    for offset in (layout.root_directory..layout.root_directory + cluster_bytes).step_by(32) {
        let entry = image.get(offset..offset + 32).ok_or("truncated root directory")?;
        if entry[0] == 0 {
            break;
        }
        if entry[0] != 0xe5 && entry[11] != 0x0f && entry[..11] == filename[..] {
            return Ok(offset);
        }
    }
    Err("kernel file is absent from root directory")
}

fn mutate_fixture(image: &Path, mutation: impl FnOnce(&mut [u8], &FatLayout, usize) -> Result<(), &'static str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = fs::read(image)?;
    let layout = fat_layout(&bytes)?;
    // `kernel-x86_64` is stored as a VFAT long name. Its associated 8.3
    // entry is deterministic for an otherwise empty image built above.
    let entry = root_file_entry(&bytes, &layout, b"KERNEL~1   ")?;
    mutation(&mut bytes, &layout, entry)?;
    fs::write(image, bytes)?;
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let kernel = PathBuf::from(args.next().ok_or("missing kernel path")?);
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    if args.next().is_some() {
        return Err("usage: wovenhat-boot-config-fixtures <kernel> <new-output-directory>".into());
    }
    let metadata = fs::symlink_metadata(&kernel)?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 * 1024 {
        return Err("kernel must be a bounded regular ELF file".into());
    }
    let bytes = fs::read(&kernel)?;
    if !bytes.starts_with(b"\x7fELF") {
        return Err("kernel is not ELF".into());
    }
    // Never overwrite an existing fixture or source image. Every image below
    // is created under this newly-created directory, with no device access.
    fs::create_dir(&output)?;
    let cases: [(&str, Option<&[u8]>); 8] = [
        ("default", None),
        ("legacy", Some(b"{}")),
        ("version1", Some(br#"{"version":1,"serial_logging":true}"#)),
        ("malformed", Some(b"{")),
        ("empty", Some(b"")),
        ("trailing", Some(b"{}garbage")),
        ("unsupported", Some(br#"{"version":2}"#)),
        ("unknown", Some(br#"{"recovery_mode":true}"#)),
    ];
    for (name, configuration) in cases {
        let mut builder = bootloader::DiskImageBuilder::new(kernel.clone());
        if let Some(configuration) = configuration {
            builder.set_file_contents("boot.json".into(), configuration.to_vec());
        }
        builder.create_uefi_image(&output.join(format!("{name}.img")))?;
        println!("Created {name}.img");
    }
    let mut oversized = bootloader::DiskImageBuilder::new(kernel.clone());
    oversized.set_file_contents("boot.json".into(), vec![b' '; 4097]);
    oversized.create_uefi_image(&output.join("oversized.img"))?;
    println!("Created oversized.img");
    let mut wrong_arch = bytes[..64].to_vec();
    wrong_arch[18..20].copy_from_slice(&183u16.to_le_bytes());
    let mut bad_table = bytes[..64].to_vec();
    bad_table[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    for (name, payload) in [
        ("kernel-empty", Vec::new()),
        ("kernel-truncated", bytes[..32].to_vec()),
        ("kernel-corrupt", vec![0xff; 64]),
        ("kernel-wrong-arch", wrong_arch),
        ("kernel-bad-table", bad_table),
    ] {
        let mut builder = bootloader::DiskImageBuilder::empty();
        builder.set_file_contents("kernel-x86_64".into(), payload);
        builder.create_uefi_image(&output.join(format!("{name}.img")))?;
        println!("Created {name}.img");
    }
    bootloader::DiskImageBuilder::empty().create_uefi_image(&output.join("kernel-missing.img"))?;

    // These mutate only newly-created disposable FAT volumes. The production
    // loader still treats a missing file as a normal disk-to-TFTP fallback;
    // a present entry with invalid metadata or a broken FAT chain must stop
    // with a firmware-resource diagnostic before handoff. OVMF's FAT driver
    // reports the latter as an EFI read error, which the harness asserts.
    let disk_directory = output.join("disk-directory.img");
    bootloader::DiskImageBuilder::new(kernel.clone()).create_uefi_image(&disk_directory)?;
    mutate_fixture(&disk_directory, |image, _layout, entry| {
        image[entry + 11] = FAT_DIRECTORY;
        Ok(())
    })?;
    println!("Created disk-directory.img");

    let disk_read = output.join("disk-read.img");
    bootloader::DiskImageBuilder::new(kernel.clone()).create_uefi_image(&disk_read)?;
    mutate_fixture(&disk_read, |image, layout, entry| {
        let cluster = usize::from(le_u16(image, entry + 26)?);
        if cluster < 2 {
            return Err("kernel file has no valid first cluster");
        }
        let fat_entry = layout.first_fat.checked_add(cluster.checked_mul(layout.fat_entry_bytes).ok_or("FAT entry overflow")?)
            .ok_or("FAT entry overflow")?;
        let destination = image.get_mut(fat_entry..fat_entry.checked_add(layout.fat_entry_bytes).ok_or("FAT entry overflow")?)
            .ok_or("kernel FAT entry is outside fixture image")?;
        match layout.fat_entry_bytes {
            2 => destination.copy_from_slice(&(FAT_END_OF_CHAIN as u16).to_le_bytes()),
            4 => destination.copy_from_slice(&FAT_END_OF_CHAIN.to_le_bytes()),
            _ => return Err("unsupported FAT entry width"),
        }
        Ok(())
    })?;
    println!("Created disk-read.img");

    // This is within the accepted raw-kernel bound, but cannot fit alongside
    // firmware and the loader in the harness's fixed 256 MiB guest. It
    // exercises the real UEFI allocation failure path without host media I/O.
    let mut allocation = bootloader::DiskImageBuilder::empty();
    allocation.set_file_contents("kernel-x86_64".into(), vec![0; 256 * 1024 * 1024]);
    allocation.create_uefi_image(&output.join("kernel-allocation.img"))?;
    println!("Created kernel-allocation.img");

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("boot configuration fixtures: {error}");
        std::process::exit(1);
    }
}
