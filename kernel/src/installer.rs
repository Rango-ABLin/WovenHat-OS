//! Kernel-backed installation transaction.
//!
//! The installer service is the only Ring-3 caller. It supplies a bounded,
//! confirmed plan; this module copies no caller pointers and performs all
//! target I/O through the capability-gated kernel block-device boundary. The
//! boot artifact is imported from the currently booted EFI System Partition
//! before the target GPT is replaced, which keeps the installed image tied to
//! the exact kernel that is running the installer.

use alloc::{format, string::String, vec::Vec};

use crate::{
    block::{BlockDevice, SECTOR_SIZE},
    block_io, fat32, gpt, partition::PartitionDevice,
};

const ESP_SECTORS: u64 = 70_000;
const MAX_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NoDevice,
    InvalidPlan,
    Block,
    Gpt,
    Fat,
    Artifact,
    Provisioning,
}

#[derive(Clone, Copy)]
struct Plan {
    target_sectors: u64,
    data_sectors: u64,
    user: [u8; 32],
    user_len: usize,
    password: [u8; 16],
    password_len: usize,
    timezone: [u8; 32],
    timezone_len: usize,
    keyboard: [u8; 8],
    keyboard_len: usize,
    network: [u8; 8],
    network_len: usize,
}

pub fn install(plan_bytes: &[u8]) -> Result<u8, Error> {
    if crate::storage::mnt_mounted() {
        crate::storage::unmount_mnt().map_err(|_| Error::Provisioning)?;
    }
    let result = block_io::with_root_install_target(|device| install_on_device(device, plan_bytes));
    result.ok_or(Error::NoDevice)?.map(|_| 100)
}

fn install_on_device(device: &mut dyn BlockDevice, plan_bytes: &[u8]) -> Result<(), Error> {
    let mut target = device;
    let target_sectors = target.sector_count();
    let plan = parse_plan(plan_bytes, target_sectors)?;
    if plan.data_sectors < 32_768
        || ESP_SECTORS
            .checked_add(plan.data_sectors)
            .and_then(|sectors| sectors.checked_add(4096))
            .is_none_or(|minimum| minimum > target_sectors)
    {
        return Err(Error::InvalidPlan);
    }

    // Preserve the exact running boot artifact before changing either GPT
    // copy. A blank target is rejected rather than receiving a synthetic EFI
    // payload or an unbootable placeholder.
    let partitions = gpt::fat_partitions(&mut target).map_err(|_| Error::Gpt)?;
    let source = partitions.first().copied().ok_or(Error::Artifact)?;
    let artifact = read_boot_artifact(&mut target, source)?;

    let layout = gpt::write_layout(&mut target, ESP_SECTORS, plan.data_sectors).map_err(|_| Error::Gpt)?;
    let esp_partition = crate::partition::Partition {
        start_lba: layout.esp_start_lba,
        sectors: layout.esp_sectors,
        kind: 0xee,
    };
    let data_partition = crate::partition::Partition {
        start_lba: layout.data_start_lba,
        sectors: layout.data_sectors,
        kind: 0xee,
    };
    {
        let mut esp = PartitionDevice::new(&mut target, esp_partition).map_err(|_| Error::Gpt)?;
        let esp_volume = fat32::format(&mut esp, b"WOVEN EFI  ").map_err(|_| Error::Fat)?;
        fat32::create_path_file(&mut esp, esp_volume, "EFI/BOOT/BOOTX64.EFI", &artifact)
            .map_err(|_| Error::Artifact)?;
        esp.flush().map_err(|_| Error::Block)?;
    }

    let mut data = PartitionDevice::new(&mut target, data_partition).map_err(|_| Error::Gpt)?;
    let data_volume = fat32::format(&mut data, b"WOVENHAT   ").map_err(|_| Error::Fat)?;
    let user = core::str::from_utf8(&plan.user[..plan.user_len]).map_err(|_| Error::Provisioning)?;
    let timezone = core::str::from_utf8(&plan.timezone[..plan.timezone_len]).map_err(|_| Error::Provisioning)?;
    let keyboard = core::str::from_utf8(&plan.keyboard[..plan.keyboard_len]).map_err(|_| Error::Provisioning)?;
    let network = core::str::from_utf8(&plan.network[..plan.network_len]).map_err(|_| Error::Provisioning)?;
    let password = &plan.password[..plan.password_len];
    let mut salt = [0_u8; 16];
    for chunk in salt.chunks_mut(8) {
        chunk.copy_from_slice(&crate::entropy::random_u64().to_le_bytes()[..chunk.len()]);
    }
    let digest = libwoven::accounts::hash_pin(&salt, password);
    let mut salt_hex = String::new();
    let mut digest_hex = String::new();
    push_hex(&mut salt_hex, &salt);
    push_hex(&mut digest_hex, &digest);
    let accounts = format!("{user}:{salt_hex}:{digest_hex}\n");
    let settings = format!(
        "{{\"schema\":\"wovenhat-installed-v1\",\"user\":\"{user}\",\"timezone\":\"{timezone}\",\"keyboard\":\"{keyboard}\",\"network\":\"{network}\"}}\n"
    );
    fat32::create_path_file(&mut data, data_volume, "etc/wovenhat/accounts", accounts.as_bytes())
        .map_err(|_| Error::Provisioning)?;
    fat32::create_path_file(&mut data, data_volume, "etc/wovenhat/settings.json", settings.as_bytes())
        .map_err(|_| Error::Provisioning)?;
    data.flush().map_err(|_| Error::Block)?;
    Ok(())
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
}

fn parse_plan(bytes: &[u8], target_sectors: u64) -> Result<Plan, Error> {
    libwoven::installer_ipc::validate_plan(bytes, target_sectors, true).map_err(|_| Error::InvalidPlan)?;
    let mut plan = Plan {
        target_sectors,
        data_sectors: 0,
        user: [0; 32],
        user_len: 0,
        password: [0; 16],
        password_len: 0,
        timezone: [0; 32],
        timezone_len: 0,
        keyboard: [0; 8],
        keyboard_len: 0,
        network: [0; 8],
        network_len: 0,
    };
    for field in bytes.split(|byte| *byte == b';') {
        let separator = field.iter().position(|byte| *byte == b'=').ok_or(Error::InvalidPlan)?;
        let key = &field[..separator];
        let value = &field[separator + 1..];
        match key {
            b"target-sectors" => plan.target_sectors = parse_number(value)?,
            b"partition-sectors" => plan.data_sectors = parse_number(value)?,
            b"user" => plan.user_len = copy_field(value, &mut plan.user)?,
            b"password" => plan.password_len = copy_field(value, &mut plan.password)?,
            b"timezone" => plan.timezone_len = copy_field(value, &mut plan.timezone)?,
            b"keyboard" => plan.keyboard_len = copy_field(value, &mut plan.keyboard)?,
            b"network" => plan.network_len = copy_field(value, &mut plan.network)?,
            _ => {}
        }
    }
    (plan.target_sectors == target_sectors && plan.data_sectors != 0).then_some(plan).ok_or(Error::InvalidPlan)
}

fn parse_number(value: &[u8]) -> Result<u64, Error> {
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return Err(Error::InvalidPlan);
    }
    value.iter().try_fold(0_u64, |result, byte| result.checked_mul(10).and_then(|value| value.checked_add(u64::from(byte - b'0')))).ok_or(Error::InvalidPlan)
}

fn copy_field(value: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    if value.is_empty() || value.len() > output.len() {
        return Err(Error::InvalidPlan);
    }
    output[..value.len()].copy_from_slice(value);
    Ok(value.len())
}

#[derive(Clone, Copy)]
struct Fat16 {
    sectors_per_cluster: u8,
    reserved: u16,
    fat32: bool,
    root_cluster: u32,
    data_start: u64,
    root_start: u64,
    root_sectors: u64,
}

#[derive(Clone, Copy)]
struct DirEntry {
    directory: bool,
    cluster: u32,
    size: u32,
}

fn read_boot_artifact(device: &mut dyn BlockDevice, partition: crate::partition::Partition) -> Result<Vec<u8>, Error> {
    let mut target = device;
    let mut view = PartitionDevice::new(&mut target, partition).map_err(|_| Error::Artifact)?;
    let fat = Fat16::new(&mut view)?;
    let mut directory = fat.root(&mut view)?;
    let efi = find_entry(&directory, b"EFI").ok_or(Error::Artifact)?;
    if !efi.directory {
        return Err(Error::Artifact);
    }
    directory = fat.chain(&mut view, efi.cluster, MAX_DIRECTORY_BYTES)?;
    let boot = find_entry(&directory, b"BOOT").ok_or(Error::Artifact)?;
    if !boot.directory {
        return Err(Error::Artifact);
    }
    directory = fat.chain(&mut view, boot.cluster, MAX_DIRECTORY_BYTES)?;
    let artifact = find_entry(&directory, b"BOOTX64.EFI").ok_or(Error::Artifact)?;
    if artifact.directory || artifact.size == 0 || artifact.size as usize > MAX_ARTIFACT_BYTES {
        return Err(Error::Artifact);
    }
    fat.chain(&mut view, artifact.cluster, artifact.size as usize)
}

impl Fat16 {
    fn new(device: &mut impl BlockDevice) -> Result<Self, Error> {
        let mut sector = [0_u8; SECTOR_SIZE];
        device.read_sector(0, &mut sector).map_err(|_| Error::Artifact)?;
        let bytes_per_sector = u16::from_le_bytes([sector[11], sector[12]]);
        let sectors_per_cluster = sector[13];
        let reserved = u16::from_le_bytes([sector[14], sector[15]]);
        let fats = sector[16];
        let root_entries = u16::from_le_bytes([sector[17], sector[18]]);
        let sectors_per_fat_16 = u16::from_le_bytes([sector[22], sector[23]]);
        let (fat32, sectors_per_fat, root_cluster) = if root_entries == 0 && sectors_per_fat_16 == 0 {
            (true, u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]), u32::from_le_bytes([sector[44], sector[45], sector[46], sector[47]]))
        } else {
            (false, u32::from(sectors_per_fat_16), 0)
        };
        if bytes_per_sector as usize != SECTOR_SIZE || sectors_per_cluster == 0 || fats == 0 || sectors_per_fat == 0 || (fat32 && root_cluster < 2) || (!fat32 && root_entries == 0) || sector[510..512] != [0x55, 0xaa] {
            return Err(Error::Artifact);
        }
        let root_sectors = (u64::from(root_entries) * 32).div_ceil(SECTOR_SIZE as u64);
        let root_start = u64::from(reserved) + u64::from(fats) * u64::from(sectors_per_fat);
        let data_start = if fat32 { root_start } else { root_start + root_sectors };
        Ok(Self { sectors_per_cluster, reserved, fat32, root_cluster, data_start, root_start, root_sectors })
    }

    fn root(&self, device: &mut impl BlockDevice) -> Result<Vec<u8>, Error> {
        if self.fat32 {
            return self.chain(device, self.root_cluster, MAX_DIRECTORY_BYTES);
        }
        let mut output = Vec::with_capacity((self.root_sectors * SECTOR_SIZE as u64) as usize);
        for lba in 0..self.root_sectors {
            let mut sector = [0_u8; SECTOR_SIZE];
            device.read_sector(self.root_start + lba, &mut sector).map_err(|_| Error::Artifact)?;
            output.extend_from_slice(&sector);
        }
        Ok(output)
    }

    fn fat_entry(&self, device: &mut impl BlockDevice, cluster: u32) -> Result<u32, Error> {
        let entry_size = if self.fat32 { 4 } else { 2 };
        let offset = u64::from(self.reserved) * SECTOR_SIZE as u64 + u64::from(cluster) * entry_size;
        let lba = offset / SECTOR_SIZE as u64;
        let index = (offset % SECTOR_SIZE as u64) as usize;
        let mut sector = [0_u8; SECTOR_SIZE];
        device.read_sector(lba, &mut sector).map_err(|_| Error::Artifact)?;
        if self.fat32 {
            if index <= SECTOR_SIZE - 4 {
                Ok(u32::from_le_bytes([sector[index], sector[index + 1], sector[index + 2], sector[index + 3]]) & 0x0fff_ffff)
            } else {
                let mut bytes = [0_u8; 4];
                let first = SECTOR_SIZE - index;
                bytes[..first].copy_from_slice(&sector[index..]);
                device.read_sector(lba + 1, &mut sector).map_err(|_| Error::Artifact)?;
                bytes[first..].copy_from_slice(&sector[..4 - first]);
                Ok(u32::from_le_bytes(bytes) & 0x0fff_ffff)
            }
        } else {
            Ok(u32::from(u16::from_le_bytes([sector[index], sector[index + 1]])))
        }
    }

    fn chain(&self, device: &mut impl BlockDevice, first: u32, limit: usize) -> Result<Vec<u8>, Error> {
        if first < 2 {
            return Err(Error::Artifact);
        }
        let mut output = Vec::new();
        let mut cluster = first;
        let mut seen = Vec::new();
        while cluster < 0xfff8 {
            if cluster < 2 || seen.contains(&cluster) || output.len() > limit {
                return Err(Error::Artifact);
            }
            seen.push(cluster);
            let start = self.data_start + u64::from(cluster - 2) * u64::from(self.sectors_per_cluster);
            for offset in 0..u64::from(self.sectors_per_cluster) {
                let mut sector = [0_u8; SECTOR_SIZE];
                device.read_sector(start + offset, &mut sector).map_err(|_| Error::Artifact)?;
                output.extend_from_slice(&sector);
                if output.len() > limit + SECTOR_SIZE {
                    return Err(Error::Artifact);
                }
            }
            cluster = self.fat_entry(device, cluster)?;
        }
        output.truncate(limit.min(output.len()));
        Ok(output)
    }
}

fn find_entry(directory: &[u8], wanted: &[u8]) -> Option<DirEntry> {
    let short = short_name(wanted)?;
    for entry in directory.as_chunks::<32>().0 {
        if entry[0] == 0 {
            break;
        }
        if entry[0] == 0xe5 || entry[11] == 0x0f || entry[11] & 0x08 != 0 {
            continue;
        }
        if entry[..11] != short {
            continue;
        }
        let cluster = u32::from(u16::from_le_bytes([entry[26], entry[27]]));
        return Some(DirEntry { directory: entry[11] & 0x10 != 0, cluster, size: u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) });
    }
    None
}

fn short_name(name: &[u8]) -> Option<[u8; 11]> {
    let mut output = [b' '; 11];
    let (base, extension) = match name.iter().position(|byte| *byte == b'.') {
        Some(index) => (&name[..index], &name[index + 1..]),
        None => (name, &[][..]),
    };
    if base.is_empty() || base.len() > 8 || extension.len() > 3 || !base.iter().chain(extension).all(u8::is_ascii_alphanumeric) {
        return None;
    }
    for (slot, byte) in output.iter_mut().take(base.len()).zip(base) {
        *slot = byte.to_ascii_uppercase();
    }
    for (slot, byte) in output[8..].iter_mut().take(extension.len()).zip(extension) {
        *slot = byte.to_ascii_uppercase();
    }
    Some(output)
}

#[cfg(feature = "stage28-installer-test")]
pub fn self_test() -> bool {
    use alloc::boxed::Box;
    use crate::block::RamDisk;

    const SECTORS: usize = 110_000;
    let mut disk = Box::new(RamDisk::<SECTORS>::new());
    let Ok(layout) = gpt::write_layout(&mut *disk, ESP_SECTORS, 32_768) else {
        return false;
    };
    let esp_partition = crate::partition::Partition { start_lba: layout.esp_start_lba, sectors: layout.esp_sectors, kind: 0xee };
    {
        let Ok(mut esp) = PartitionDevice::new(&mut *disk, esp_partition) else {
            return false;
        };
        let Ok(volume) = fat32::format(&mut esp, b"WOVEN EFI  ") else {
            return false;
        };
        if fat32::create_path_file(&mut esp, volume, "EFI/BOOT/BOOTX64.EFI", b"stage28-efi").is_err() {
            return false;
        }
    }

    let plan = b"target-sectors=110000;partition-sectors=32768;filesystem=fat32;user=installer;password=change-me;timezone=UTC;keyboard=us;network=disabled;confirmed=1";
    if install_on_device(&mut *disk, plan).is_err() {
        return false;
    }
    let Ok(partitions) = gpt::fat_partitions(&mut *disk) else {
        return false;
    };
    if partitions.len() != 2 {
        return false;
    }
    {
        let Ok(mut installed_esp) = PartitionDevice::new(&mut *disk, partitions[0]) else {
            return false;
        };
        let Ok(esp_volume) = fat32::mount(&mut installed_esp) else {
            return false;
        };
        let Ok(efi_entry) = fat32::resolve_path(&mut installed_esp, esp_volume, "EFI/BOOT/BOOTX64.EFI") else {
            return false;
        };
        let mut efi = [0_u8; 32];
        let Ok(efi_len) = fat32::read_file(&mut installed_esp, esp_volume, efi_entry, &mut efi) else {
            return false;
        };
        if &efi[..efi_len] != b"stage28-efi" {
            return false;
        }
    }

    let Ok(mut data) = PartitionDevice::new(&mut *disk, partitions[1]) else {
        return false;
    };
    let Ok(data_volume) = fat32::mount(&mut data) else {
        return false;
    };
    let Ok(settings_entry) = fat32::resolve_path(&mut data, data_volume, "etc/wovenhat/settings.json") else {
        return false;
    };
    let mut settings = [0_u8; 256];
    let Ok(settings_len) = fat32::read_file(&mut data, data_volume, settings_entry, &mut settings) else {
        return false;
    };
    core::str::from_utf8(&settings[..settings_len]).is_ok_and(|value| value.contains("installer"))
}
