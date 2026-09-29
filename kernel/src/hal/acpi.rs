use bootloader_api::info::MemoryRegion;

const RSDP_V1_LENGTH: usize = 20;
const RSDP_V2_LENGTH: usize = 36;
const SDT_HEADER_LENGTH: usize = 36;
const MAX_TABLE_LENGTH: usize = 64 * 1024;
const MAX_TABLES: usize = 256;
const MADT_HEADER_LENGTH: usize = SDT_HEADER_LENGTH + 8;
const SRAT_HEADER_LENGTH: usize = SDT_HEADER_LENGTH + 12;
const MAX_MADT_ENTRIES: usize = 256;
const MAX_MADT_ENTRY_LENGTH: usize = u8::MAX as usize;
const MAX_SRAT_MEMORY_AFFINITIES: usize = 16;
const MCFG_HEADER_LENGTH: usize = SDT_HEADER_LENGTH + 8;
const MCFG_ALLOCATION_LENGTH: usize = 16;
pub const MAX_MCFG_ALLOCATIONS: usize = 8;
const MAX_AML_TABLES: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Missing,
    OutOfRange,
    InvalidSignature,
    InvalidChecksum,
    InvalidLength,
    AddressOverflow,
}

#[derive(Clone, Copy, Default)]
pub struct MemoryAffinity {
    pub domain: u32,
    pub base: u64,
    pub length: u64,
}

pub const MAX_PCI_ROOT_RESOURCES: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciRootResourceKind {
    Io,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciRootResource {
    pub kind: PciRootResourceKind,
    pub base: u64,
    pub length: u64,
    pub translation_offset: u64,
    pub prefetchable: bool,
    pub address_width: u8,
}

impl Default for PciRootResource {
    fn default() -> Self {
        Self {
            kind: PciRootResourceKind::Memory,
            base: 0,
            length: 0,
            translation_offset: 0,
            prefetchable: false,
            address_width: 0,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct McfgAllocation {
    pub base_address: u64,
    pub segment_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct AmlTable {
    address: u64,
    length: usize,
}

#[derive(Clone, Copy, Default)]
pub struct Summary {
    pub revision: u8,
    pub tables: u16,
    pub apic: bool,
    pub local_apic_address: u64,
    pub enabled_processors: u16,
    pub processor_ids: [u32; 16],
    /// NUMA proximity domain for each `processor_ids` entry. Zero is the
    /// firmware/default domain when SRAT has no CPU affinity record.
    pub processor_domains: [u32; 16],
    pub numa_domains: u16,
    pub memory_affinities: [MemoryAffinity; MAX_SRAT_MEMORY_AFFINITIES],
    pub memory_affinity_count: usize,
    pub processor_count: usize,
    pub io_apic_address: u32,
    pub io_apic_gsi_base: u32,
    pub isa_gsi: [Option<(u32, u16)>; 16],
    pub io_apics: u16,
    pub interrupt_overrides: u16,
    pub madt_entries: u16,
    pub fadt: bool,
    pub dsdt_address: u64,
    pub dsdt_length: usize,
    // SSDTs extend the DSDT namespace. Retain their validated table bounds so
    // the bounded AML namespace pass can consume every firmware definition
    // block without rescanning the XSDT/RSDT or inventing PCI resources.
    aml_tables: [AmlTable; MAX_AML_TABLES],
    aml_table_count: usize,
    pub hpet: bool,
    pub mcfg: bool,
    pub mcfg_allocations: [McfgAllocation; MAX_MCFG_ALLOCATIONS],
    pub mcfg_allocation_count: usize,
    /// PCI root-bridge resource windows supplied by a firmware namespace
    /// evaluator (ACPI _CRS on ACPI platforms). Raw SDT discovery does not
    /// synthesize these from MCFG or SRAT.
    pub pci_root_resources: [PciRootResource; MAX_PCI_ROOT_RESOURCES],
    pub pci_root_resource_count: usize,
    pub truncated: bool,
}

pub fn discover(
    physical_offset: u64,
    rsdp_address: Option<u64>,
    regions: &[MemoryRegion],
) -> Result<Summary, Error> {
    let rsdp_address = rsdp_address.ok_or(Error::Missing)?;
    let mut rsdp = [0_u8; RSDP_V2_LENGTH];
    read_physical(
        physical_offset,
        rsdp_address,
        &mut rsdp[..RSDP_V1_LENGTH],
        regions,
    )?;
    validate_rsdp_v1(&rsdp[..RSDP_V1_LENGTH])?;

    let revision = rsdp[15];
    let (root_address, entry_size, expected_signature) = if revision >= 2 {
        read_physical(physical_offset, rsdp_address, &mut rsdp, regions)?;
        validate_rsdp_v2(&rsdp)?;
        let xsdt = read_u64(&rsdp, 24);
        if xsdt != 0 {
            (xsdt, 8, *b"XSDT")
        } else {
            (u64::from(read_u32(&rsdp, 16)), 4, *b"RSDT")
        }
    } else {
        (u64::from(read_u32(&rsdp, 16)), 4, *b"RSDT")
    };
    if root_address == 0 {
        return Err(Error::OutOfRange);
    }

    let root = read_sdt_header(physical_offset, root_address, regions)?;
    if root.signature != expected_signature {
        return Err(Error::InvalidSignature);
    }
    validate_sdt_checksum(physical_offset, root_address, root.length, regions)?;
    let payload = root.length - SDT_HEADER_LENGTH;
    if !payload.is_multiple_of(entry_size) {
        return Err(Error::InvalidLength);
    }

    let total_tables = payload / entry_size;
    let scanned = core::cmp::min(total_tables, MAX_TABLES);
    let mut summary = Summary {
        revision,
        truncated: total_tables > MAX_TABLES,
        ..Summary::default()
    };
    let mut srat_tables = [(0_u64, 0_usize); 4];
    let mut srat_count = 0_usize;
    for index in 0..scanned {
        let entry_address = root_address
            .checked_add((SDT_HEADER_LENGTH + index * entry_size) as u64)
            .ok_or(Error::AddressOverflow)?;
        let mut bytes = [0_u8; 8];
        read_physical(
            physical_offset,
            entry_address,
            &mut bytes[..entry_size],
            regions,
        )?;
        let table_address = if entry_size == 8 {
            read_u64(&bytes, 0)
        } else {
            u64::from(read_u32(&bytes, 0))
        };
        let table = read_sdt_header(physical_offset, table_address, regions)?;
        validate_sdt_checksum(physical_offset, table_address, table.length, regions)?;
        summary.tables = summary.tables.saturating_add(1);
        match &table.signature {
            b"APIC" => {
                parse_madt(
                    physical_offset,
                    table_address,
                    table.length,
                    regions,
                    &mut summary,
                )?;
                summary.apic = true;
            }
            b"FACP" => {
                // FADT presence is authoritative even when optional DSDT
                // metadata cannot yet be retained.  DSDT discovery is an
                // incremental Stage 13.2 capability and must not turn an
                // otherwise valid ACPI namespace into ACPI-unavailable,
                // because that would discard MCFG/APIC data and change the
                // PCI initialization path.
                summary.fadt = true;
                let _ = parse_fadt(
                    physical_offset,
                    table_address,
                    table.length,
                    regions,
                    &mut summary,
                );
            }
            b"SSDT" => {
                if summary.aml_table_count < summary.aml_tables.len() {
                    summary.aml_tables[summary.aml_table_count] = AmlTable {
                        address: table_address,
                        length: table.length,
                    };
                    summary.aml_table_count += 1;
                } else {
                    summary.truncated = true;
                }
            }
            b"HPET" => summary.hpet = true,
            b"MCFG" => {
                parse_mcfg(
                    physical_offset,
                    table_address,
                    table.length,
                    regions,
                    &mut summary,
                )?;
                summary.mcfg = true;
            }
            b"SRAT" => {
                if srat_count < srat_tables.len() {
                    srat_tables[srat_count] = (table_address, table.length);
                    srat_count += 1;
                } else {
                    summary.truncated = true;
                }
            }
            _ => {}
        }
    }
    for (address, length) in srat_tables.into_iter().take(srat_count) {
        parse_srat(physical_offset, address, length, regions, &mut summary)?;
    }
    Ok(summary)
}

fn parse_fadt(
    physical_offset: u64,
    address: u64,
    length: usize,
    regions: &[MemoryRegion],
    summary: &mut Summary,
) -> Result<(), Error> {
    // The FADT itself is authoritative, but its DSDT pointer may target an
    // ACPI reclaim/NVS range that the boot memory-region filter does not expose
    // to this early parser. Retain DSDT metadata only when the referenced table
    // is safely readable; absence here must not make otherwise-valid ACPI fatal.
    if length < 44 {
        return Err(Error::InvalidLength);
    }
    let mut dsdt32 = [0_u8; 4];
    read_physical(
        physical_offset,
        address.checked_add(40).ok_or(Error::AddressOverflow)?,
        &mut dsdt32,
        regions,
    )?;
    let legacy = u64::from(read_u32(&dsdt32, 0));
    let extended = if length >= 148 {
        let mut x_dsdt = [0_u8; 8];
        read_physical(
            physical_offset,
            address.checked_add(140).ok_or(Error::AddressOverflow)?,
            &mut x_dsdt,
            regions,
        )?;
        read_u64(&x_dsdt, 0)
    } else {
        0
    };
    let dsdt = if extended != 0 { extended } else { legacy };
    if dsdt == 0 {
        return Ok(());
    }
    let header = match read_sdt_header(physical_offset, dsdt, regions) {
        Ok(header) => header,
        Err(Error::OutOfRange) => return Ok(()),
        Err(error) => return Err(error),
    };
    if header.signature != *b"DSDT" {
        return Err(Error::InvalidSignature);
    }
    validate_sdt_checksum(physical_offset, dsdt, header.length, regions)?;
    summary.dsdt_address = dsdt;
    summary.dsdt_length = header.length;
    Ok(())
}

fn parse_mcfg(
    physical_offset: u64,
    address: u64,
    length: usize,
    regions: &[MemoryRegion],
    summary: &mut Summary,
) -> Result<(), Error> {
    if length < MCFG_HEADER_LENGTH {
        return Err(Error::InvalidLength);
    }
    let payload = length - MCFG_HEADER_LENGTH;
    if !payload.is_multiple_of(MCFG_ALLOCATION_LENGTH) {
        return Err(Error::InvalidLength);
    }
    for index in 0..payload / MCFG_ALLOCATION_LENGTH {
        let entry_address = address
            .checked_add((MCFG_HEADER_LENGTH + index * MCFG_ALLOCATION_LENGTH) as u64)
            .ok_or(Error::AddressOverflow)?;
        let mut entry = [0_u8; MCFG_ALLOCATION_LENGTH];
        read_physical(physical_offset, entry_address, &mut entry, regions)?;
        let allocation = decode_mcfg_allocation(&entry)?;
        if summary.mcfg_allocations[..summary.mcfg_allocation_count]
            .iter()
            .any(|existing| {
                existing.segment_group == allocation.segment_group
                    && existing.start_bus <= allocation.end_bus
                    && allocation.start_bus <= existing.end_bus
            })
        {
            return Err(Error::InvalidLength);
        }
        if summary.mcfg_allocation_count < summary.mcfg_allocations.len() {
            summary.mcfg_allocations[summary.mcfg_allocation_count] = allocation;
            summary.mcfg_allocation_count += 1;
        } else {
            summary.truncated = true;
        }
    }
    Ok(())
}

fn decode_mcfg_allocation(entry: &[u8; MCFG_ALLOCATION_LENGTH]) -> Result<McfgAllocation, Error> {
    let base_address = read_u64(entry, 0);
    let segment_group = u16::from_le_bytes([entry[8], entry[9]]);
    let start_bus = entry[10];
    let end_bus = entry[11];
    if base_address == 0 || base_address & ((1 << 20) - 1) != 0 || start_bus > end_bus {
        return Err(Error::InvalidLength);
    }
    let buses = u64::from(end_bus) - u64::from(start_bus) + 1;
    let bytes = buses.checked_mul(1 << 20).ok_or(Error::AddressOverflow)?;
    base_address
        .checked_add(bytes)
        .ok_or(Error::AddressOverflow)?;
    Ok(McfgAllocation {
        base_address,
        segment_group,
        start_bus,
        end_bus,
    })
}

fn parse_srat(
    physical_offset: u64,
    address: u64,
    length: usize,
    regions: &[MemoryRegion],
    summary: &mut Summary,
) -> Result<(), Error> {
    if length < SRAT_HEADER_LENGTH {
        return Err(Error::InvalidLength);
    }
    let mut offset = SRAT_HEADER_LENGTH;
    let mut entries = 0_usize;
    while offset < length {
        if entries == MAX_MADT_ENTRIES {
            summary.truncated = true;
            break;
        }
        let entry_address = address
            .checked_add(offset as u64)
            .ok_or(Error::AddressOverflow)?;
        let mut header = [0_u8; 2];
        read_physical(physical_offset, entry_address, &mut header, regions)?;
        let entry_length = header[1] as usize;
        if entry_length < 2
            || offset
                .checked_add(entry_length)
                .is_none_or(|end| end > length)
        {
            return Err(Error::InvalidLength);
        }
        let mut entry = [0_u8; MAX_MADT_ENTRY_LENGTH];
        read_physical(
            physical_offset,
            entry_address,
            &mut entry[..entry_length],
            regions,
        )?;
        update_srat_summary(&entry[..entry_length], summary)?;
        entries += 1;
        offset += entry_length;
    }
    Ok(())
}

fn update_srat_summary(entry: &[u8], summary: &mut Summary) -> Result<(), Error> {
    if entry.len() < 2 || entry[1] as usize != entry.len() {
        return Err(Error::InvalidLength);
    }
    if entry[0] == 1 {
        if entry.len() < 40 {
            return Err(Error::InvalidLength);
        }
        let domain = read_u32(entry, 2);
        let base = read_u64(entry, 8);
        let length = read_u64(entry, 16);
        if entry[28] & 1 == 0 || length == 0 || base.checked_add(length).is_none() {
            return Ok(());
        }
        if summary.memory_affinity_count < summary.memory_affinities.len() {
            summary.memory_affinities[summary.memory_affinity_count] = MemoryAffinity {
                domain,
                base,
                length,
            };
            summary.memory_affinity_count += 1;
        } else {
            summary.truncated = true;
        }
        return Ok(());
    }
    let (id, domain, enabled) = match entry[0] {
        // Processor Local APIC affinity: proximity-domain low byte at 2,
        // APIC ID at 3, flags at 4, and the high three domain bytes at 9..12.
        0 => {
            if entry.len() < 16 {
                return Err(Error::InvalidLength);
            }
            let domain = u32::from(entry[2])
                | (u32::from(entry[9]) << 8)
                | (u32::from(entry[10]) << 16)
                | (u32::from(entry[11]) << 24);
            (u32::from(entry[3]), domain, read_u32(entry, 4) & 1 != 0)
        }
        // x2APIC affinity: proximity domain at 4, x2APIC ID at 8, flags at 12.
        2 => {
            if entry.len() < 24 {
                return Err(Error::InvalidLength);
            }
            (
                read_u32(entry, 8),
                read_u32(entry, 4),
                read_u32(entry, 12) & 1 != 0,
            )
        }
        _ => return Ok(()),
    };
    if !enabled {
        return Ok(());
    }
    let Some(index) = summary.processor_ids[..summary.processor_count]
        .iter()
        .position(|candidate| *candidate == id)
    else {
        return Ok(());
    };
    summary.processor_domains[index] = domain;
    let mut domains = 0_u16;
    for (position, candidate) in summary.processor_domains[..summary.processor_count]
        .iter()
        .copied()
        .enumerate()
    {
        if !summary.processor_domains[..position].contains(&candidate) {
            domains = domains.saturating_add(1);
        }
    }
    summary.numa_domains = domains.max(1);
    Ok(())
}

fn parse_madt(
    physical_offset: u64,
    address: u64,
    length: usize,
    regions: &[MemoryRegion],
    summary: &mut Summary,
) -> Result<(), Error> {
    if length < MADT_HEADER_LENGTH {
        return Err(Error::InvalidLength);
    }
    let mut fixed = [0_u8; 8];
    read_physical(
        physical_offset,
        address
            .checked_add(SDT_HEADER_LENGTH as u64)
            .ok_or(Error::AddressOverflow)?,
        &mut fixed,
        regions,
    )?;
    summary.local_apic_address = u64::from(read_u32(&fixed, 0));

    let mut offset = MADT_HEADER_LENGTH;
    let mut entries = 0_usize;
    while offset < length {
        if entries == MAX_MADT_ENTRIES {
            summary.truncated = true;
            break;
        }
        let entry_address = address
            .checked_add(offset as u64)
            .ok_or(Error::AddressOverflow)?;
        let mut header = [0_u8; 2];
        read_physical(physical_offset, entry_address, &mut header, regions)?;
        let entry_length = header[1] as usize;
        if entry_length < 2
            || offset
                .checked_add(entry_length)
                .is_none_or(|end| end > length)
        {
            return Err(Error::InvalidLength);
        }
        let mut entry = [0_u8; MAX_MADT_ENTRY_LENGTH];
        read_physical(
            physical_offset,
            entry_address,
            &mut entry[..entry_length],
            regions,
        )?;
        update_madt_summary(&entry[..entry_length], summary)?;
        entries += 1;
        offset += entry_length;
    }
    summary.madt_entries = entries as u16;
    Ok(())
}

fn update_madt_summary(entry: &[u8], summary: &mut Summary) -> Result<(), Error> {
    if entry.len() < 2 || entry[1] as usize != entry.len() {
        return Err(Error::InvalidLength);
    }
    match entry[0] {
        0 => {
            if entry.len() < 8 {
                return Err(Error::InvalidLength);
            }
            if read_u32(entry, 4) & 3 != 0 {
                summary.enabled_processors = summary.enabled_processors.saturating_add(1);
                let id = if entry[0] == 0 {
                    u32::from(entry[3])
                } else {
                    read_u32(entry, 4)
                };
                if summary.processor_ids[..summary.processor_count].contains(&id) {
                    return Err(Error::InvalidLength);
                }
                if summary.processor_count < summary.processor_ids.len() {
                    summary.processor_ids[summary.processor_count] = id;
                    summary.processor_count += 1;
                } else {
                    summary.truncated = true;
                }
            }
        }
        1 => {
            if entry.len() < 12 {
                return Err(Error::InvalidLength);
            }
            summary.io_apics = summary.io_apics.saturating_add(1);
            if summary.io_apics == 1 {
                summary.io_apic_address = read_u32(entry, 4);
                summary.io_apic_gsi_base = read_u32(entry, 8);
            }
        }
        2 => {
            if entry.len() < 10 {
                return Err(Error::InvalidLength);
            }
            summary.interrupt_overrides = summary.interrupt_overrides.saturating_add(1);
            if entry[2] == 0 && entry[3] < 16 {
                summary.isa_gsi[entry[3] as usize] =
                    Some((read_u32(entry, 4), u16::from_le_bytes([entry[8], entry[9]])));
            }
        }
        5 => {
            if entry.len() < 12 {
                return Err(Error::InvalidLength);
            }
            summary.local_apic_address = read_u64(entry, 4);
        }
        9 => {
            if entry.len() < 16 {
                return Err(Error::InvalidLength);
            }
            if read_u32(entry, 8) & 3 != 0 {
                summary.enabled_processors = summary.enabled_processors.saturating_add(1);
                let id = if entry[0] == 0 {
                    u32::from(entry[3])
                } else {
                    read_u32(entry, 4)
                };
                if summary.processor_ids[..summary.processor_count].contains(&id) {
                    return Err(Error::InvalidLength);
                }
                if summary.processor_count < summary.processor_ids.len() {
                    summary.processor_ids[summary.processor_count] = id;
                    summary.processor_count += 1;
                } else {
                    summary.truncated = true;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
struct SdtHeader {
    signature: [u8; 4],
    length: usize,
}

fn read_sdt_header(
    physical_offset: u64,
    address: u64,
    regions: &[MemoryRegion],
) -> Result<SdtHeader, Error> {
    let mut header = [0_u8; SDT_HEADER_LENGTH];
    read_physical(physical_offset, address, &mut header, regions)?;
    let length = read_u32(&header, 4) as usize;
    if !(SDT_HEADER_LENGTH..=MAX_TABLE_LENGTH).contains(&length) {
        return Err(Error::InvalidLength);
    }
    let mut signature = [0_u8; 4];
    signature.copy_from_slice(&header[..4]);
    Ok(SdtHeader { signature, length })
}

fn validate_sdt_checksum(
    physical_offset: u64,
    address: u64,
    length: usize,
    regions: &[MemoryRegion],
) -> Result<(), Error> {
    validate_range(address, length, regions)?;
    let virtual_address = physical_offset
        .checked_add(address)
        .ok_or(Error::AddressOverflow)?;
    let mut sum = 0_u8;
    for index in 0..length {
        let pointer = (virtual_address as *const u8).wrapping_add(index);
        sum = sum.wrapping_add(unsafe { pointer.read_volatile() });
    }
    if sum == 0 {
        Ok(())
    } else {
        Err(Error::InvalidChecksum)
    }
}

fn read_physical(
    physical_offset: u64,
    address: u64,
    output: &mut [u8],
    regions: &[MemoryRegion],
) -> Result<(), Error> {
    validate_range(address, output.len(), regions)?;
    let virtual_address = physical_offset
        .checked_add(address)
        .ok_or(Error::AddressOverflow)?;
    for (index, byte) in output.iter_mut().enumerate() {
        let pointer = (virtual_address as *const u8).wrapping_add(index);
        *byte = unsafe { pointer.read_volatile() };
    }
    Ok(())
}

fn validate_range(address: u64, length: usize, regions: &[MemoryRegion]) -> Result<(), Error> {
    let end = address
        .checked_add(length as u64)
        .ok_or(Error::AddressOverflow)?;
    if length == 0
        || !regions
            .iter()
            .any(|region| address >= region.start && end <= region.end)
    {
        return Err(Error::OutOfRange);
    }
    Ok(())
}

fn validate_rsdp_v1(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() < RSDP_V1_LENGTH || &bytes[..8] != b"RSD PTR " {
        return Err(Error::InvalidSignature);
    }
    checksum(&bytes[..RSDP_V1_LENGTH])
}

fn validate_rsdp_v2(bytes: &[u8; RSDP_V2_LENGTH]) -> Result<(), Error> {
    if read_u32(bytes, 20) as usize != RSDP_V2_LENGTH {
        return Err(Error::InvalidLength);
    }
    checksum(bytes)
}

fn checksum(bytes: &[u8]) -> Result<(), Error> {
    if bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) == 0 {
        Ok(())
    } else {
        Err(Error::InvalidChecksum)
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_or([0; 4]))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap_or([0; 8]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AmlPackage {
    body_offset: usize,
    end_offset: usize,
}

fn decode_aml_pkg_length(bytes: &[u8], offset: usize) -> Result<(usize, usize), Error> {
    let lead = *bytes.get(offset).ok_or(Error::InvalidLength)?;
    let follow = usize::from(lead >> 6);
    if follow > 3 {
        return Err(Error::InvalidLength);
    }
    let encoded = bytes
        .get(offset..offset.checked_add(follow + 1).ok_or(Error::AddressOverflow)?)
        .ok_or(Error::InvalidLength)?;
    let mut length = if follow == 0 {
        usize::from(lead & 0x3f)
    } else {
        usize::from(lead & 0x0f)
    };
    for index in 0..follow {
        length |= usize::from(encoded[index + 1]) << (4 + index * 8);
    }
    if length < follow + 1 {
        return Err(Error::InvalidLength);
    }
    Ok((length, follow + 1))
}

fn aml_package(bytes: &[u8], pkg_offset: usize) -> Result<AmlPackage, Error> {
    let (length, length_bytes) = decode_aml_pkg_length(bytes, pkg_offset)?;
    let end_offset = pkg_offset.checked_add(length).ok_or(Error::AddressOverflow)?;
    if end_offset > bytes.len() {
        return Err(Error::InvalidLength);
    }
    Ok(AmlPackage {
        body_offset: pkg_offset.checked_add(length_bytes).ok_or(Error::AddressOverflow)?,
        end_offset,
    })
}

fn aml_name_seg(bytes: &[u8], offset: usize) -> Result<([u8; 4], usize), Error> {
    let name = bytes
        .get(offset..offset.checked_add(4).ok_or(Error::AddressOverflow)?)
        .ok_or(Error::InvalidLength)?;
    fn valid_lead(byte: u8) -> bool {
        byte == b'_' || byte.is_ascii_uppercase()
    }
    fn valid_tail(byte: u8) -> bool {
        valid_lead(byte) || byte.is_ascii_digit()
    }
    if !valid_lead(name[0]) || !name[1..].iter().copied().all(valid_tail) {
        return Err(Error::InvalidSignature);
    }
    Ok(([name[0], name[1], name[2], name[3]], offset + 4))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AmlName {
    rooted: bool,
    parents: u8,
    segments: [[u8; 4]; 8],
    segment_count: usize,
}

fn aml_name_string(bytes: &[u8], mut offset: usize) -> Result<(AmlName, usize), Error> {
    let mut name = AmlName::default();
    match bytes.get(offset).copied().ok_or(Error::InvalidLength)? {
        b'\\' => {
            name.rooted = true;
            offset += 1;
        }
        b'^' => {
            while bytes.get(offset) == Some(&b'^') {
                name.parents = name.parents.checked_add(1).ok_or(Error::InvalidLength)?;
                offset += 1;
            }
        }
        _ => {}
    }

    let lead = *bytes.get(offset).ok_or(Error::InvalidLength)?;
    let (count, mut next) = match lead {
        0x00 => return Ok((name, offset + 1)),
        0x2e => (2_usize, offset + 1),
        0x2f => {
            let count = usize::from(*bytes.get(offset + 1).ok_or(Error::InvalidLength)?);
            if count == 0 {
                return Err(Error::InvalidLength);
            }
            (count, offset + 2)
        }
        _ => (1_usize, offset),
    };
    if count > name.segments.len() {
        return Err(Error::InvalidLength);
    }
    for slot in name.segments.iter_mut().take(count) {
        let (segment, after) = aml_name_seg(bytes, next)?;
        *slot = segment;
        next = after;
    }
    name.segment_count = count;
    Ok((name, next))
}

fn aml_skip_data_ref_object(bytes: &[u8], offset: usize) -> Result<usize, Error> {
    let opcode = *bytes.get(offset).ok_or(Error::InvalidLength)?;
    match opcode {
        // ZeroOp, OneOp, OnesOp.
        0x00 | 0x01 | 0xff => Ok(offset + 1),
        // Byte/Word/DWord/QWord integer prefixes.
        0x0a => offset.checked_add(2).filter(|end| *end <= bytes.len()).ok_or(Error::InvalidLength),
        0x0b => offset.checked_add(3).filter(|end| *end <= bytes.len()).ok_or(Error::InvalidLength),
        0x0c => offset.checked_add(5).filter(|end| *end <= bytes.len()).ok_or(Error::InvalidLength),
        0x0e => offset.checked_add(9).filter(|end| *end <= bytes.len()).ok_or(Error::InvalidLength),
        // StringPrefix: NUL-terminated AML string.
        0x0d => {
            let mut end = offset.checked_add(1).ok_or(Error::AddressOverflow)?;
            while *bytes.get(end).ok_or(Error::InvalidLength)? != 0 {
                end = end.checked_add(1).ok_or(Error::AddressOverflow)?;
            }
            Ok(end + 1)
        }
        // BufferOp and PackageOp carry a bounded PkgLength.  We do not
        // evaluate their contents here; skipping the complete package is
        // sufficient for namespace discovery and prevents byte scanning.
        0x11 | 0x12 => Ok(aml_package(bytes, offset + 1)?.end_offset),
        // VarPackageOp has the same package envelope.
        0x13 => Ok(aml_package(bytes, offset + 1)?.end_offset),
        _ => Err(Error::InvalidSignature),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AmlPath {
    segments: [[u8; 4]; 16],
    count: usize,
}

const MAX_AML_NAMESPACE_RECORDS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AmlNamespaceKind {
    Scope,
    Device,
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AmlNamespaceRecord {
    path: AmlPath,
    kind: AmlNamespaceKind,
}

fn aml_record(
    records: &mut [Option<AmlNamespaceRecord>; MAX_AML_NAMESPACE_RECORDS],
    record_count: &mut usize,
    path: AmlPath,
    kind: AmlNamespaceKind,
) -> Result<(), Error> {
    if *record_count >= records.len() {
        return Err(Error::InvalidLength);
    }
    records[*record_count] = Some(AmlNamespaceRecord { path, kind });
    *record_count += 1;
    Ok(())
}

fn aml_resolve_name(base: AmlPath, name: AmlName) -> Result<AmlPath, Error> {
    let mut path = if name.rooted { AmlPath::default() } else { base };
    let parents = usize::from(name.parents);
    if parents > path.count {
        return Err(Error::InvalidSignature);
    }
    path.count -= parents;
    if path.count + name.segment_count > path.segments.len() {
        return Err(Error::InvalidLength);
    }
    for segment in name.segments.into_iter().take(name.segment_count) {
        path.segments[path.count] = segment;
        path.count += 1;
    }
    Ok(path)
}

fn aml_eisa_id(bytes: &[u8], offset: usize) -> Option<u32> {
    match bytes.get(offset).copied()? {
        0x0c => bytes.get(offset + 1..offset + 5).map(|raw| {
            u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])
        }),
        _ => None,
    }
}

fn aml_is_pci_root_id(id: u32) -> bool {
    // AML EISAID("PNP0A03") / EISAID("PNP0A08") integer encodings.
    id == 0x030ad041 || id == 0x080ad041
}

fn aml_name_is(path: AmlPath, segment: [u8; 4]) -> bool {
    path.count != 0 && path.segments[path.count - 1] == segment
}

fn aml_namespace_walk(bytes: &[u8]) -> Result<usize, Error> {
    fn walk(
        bytes: &[u8],
        mut offset: usize,
        end: usize,
        scope: AmlPath,
        objects: &mut usize,
        records: &mut [Option<AmlNamespaceRecord>; MAX_AML_NAMESPACE_RECORDS],
        record_count: &mut usize,
    ) -> Result<(), Error> {
        while offset < end {
            match bytes.get(offset).copied().ok_or(Error::InvalidLength)? {
                0x10 => {
                    let package = aml_package(bytes, offset + 1)?;
                    if package.end_offset > end {
                        return Err(Error::InvalidLength);
                    }
                    let (name, body) = aml_name_string(bytes, package.body_offset)?;
                    let child = aml_resolve_name(scope, name)?;
                    *objects = objects.checked_add(1).ok_or(Error::AddressOverflow)?;
                    aml_record(records, record_count, child, AmlNamespaceKind::Scope)?;
                    walk(bytes, body, package.end_offset, child, objects, records, record_count)?;
                    offset = package.end_offset;
                }
                0x5b if bytes.get(offset + 1) == Some(&0x82) => {
                    let package = aml_package(bytes, offset + 2)?;
                    if package.end_offset > end {
                        return Err(Error::InvalidLength);
                    }
                    let (name, body) = aml_name_string(bytes, package.body_offset)?;
                    let child = aml_resolve_name(scope, name)?;
                    *objects = objects.checked_add(1).ok_or(Error::AddressOverflow)?;
                    aml_record(records, record_count, child, AmlNamespaceKind::Device)?;
                    walk(bytes, body, package.end_offset, child, objects, records, record_count)?;
                    offset = package.end_offset;
                }
                0x08 => {
                    let (name, value) = aml_name_string(bytes, offset + 1)?;
                    let path = aml_resolve_name(scope, name)?;
                    // Recognize static PCI-root hardware/compatible IDs while
                    // walking their owning device scope. This is intentionally
                    // read-only: resource apertures are not published until
                    // _CRS is decoded and validated separately.
                    let _pci_root_identity = (aml_name_is(path, *b"_HID")
                        || aml_name_is(path, *b"_CID"))
                        && aml_eisa_id(bytes, value).is_some_and(aml_is_pci_root_id);
                    match aml_skip_data_ref_object(bytes, value) {
                        Ok(next) if next <= end => {
                            *objects = objects.checked_add(1).ok_or(Error::AddressOverflow)?;
                            aml_record(records, record_count, path, AmlNamespaceKind::Name)?;
                            offset = next;
                        }
                        Ok(_) => return Err(Error::InvalidLength),
                        Err(Error::InvalidSignature) => break,
                        Err(error) => return Err(error),
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }
    let mut objects = 0_usize;
    let mut records = [None; MAX_AML_NAMESPACE_RECORDS];
    let mut record_count = 0_usize;
    walk(
        bytes,
        0,
        bytes.len(),
        AmlPath::default(),
        &mut objects,
        &mut records,
        &mut record_count,
    )?;
    debug_assert_eq!(objects, record_count);
    Ok(objects)
}

fn decode_pci_root_resource_template(
    bytes: &[u8],
    output: &mut [PciRootResource; MAX_PCI_ROOT_RESOURCES],
) -> Result<usize, Error> {
    let mut offset = 0_usize;
    let mut count = 0_usize;
    let mut saw_end_tag = false;
    while offset < bytes.len() {
        let tag = bytes[offset];
        let item_len = if tag & 0x80 != 0 {
            let header_end = offset.checked_add(3).ok_or(Error::AddressOverflow)?;
            let header = bytes.get(offset..header_end).ok_or(Error::InvalidLength)?;
            3_usize
                .checked_add(usize::from(u16::from_le_bytes([header[1], header[2]])))
                .ok_or(Error::AddressOverflow)?
        } else {
            1_usize
                .checked_add(usize::from(tag & 0x07))
                .ok_or(Error::AddressOverflow)?
        };
        let end = offset.checked_add(item_len).ok_or(Error::AddressOverflow)?;
        let item = bytes.get(offset..end).ok_or(Error::InvalidLength)?;

        if tag & 0x80 == 0 && tag >> 3 == 0x0f {
            if item.len() != 2 || end != bytes.len() {
                return Err(Error::InvalidLength);
            }
            saw_end_tag = true;
            break;
        }

        if tag & 0x80 != 0 {
            if let Some(resource) = decode_address_space_resource(item)? {
                if count == output.len() {
                    return Err(Error::InvalidLength);
                }
                output[count] = resource;
                count += 1;
            }
        }
        offset = end;
    }
    if !saw_end_tag {
        return Err(Error::InvalidLength);
    }
    Ok(count)
}

fn decode_address_space_resource(bytes: &[u8]) -> Result<Option<PciRootResource>, Error> {
    if bytes.len() < 3 || bytes[0] & 0x80 == 0 {
        return Err(Error::InvalidLength);
    }
    let payload_len = usize::from(u16::from_le_bytes([bytes[1], bytes[2]]));
    if bytes.len() != payload_len.checked_add(3).ok_or(Error::AddressOverflow)? {
        return Err(Error::InvalidLength);
    }
    let width = match bytes[0] {
        0x88 if payload_len >= 13 => 2_usize,
        0x87 if payload_len >= 23 => 4_usize,
        0x8a if payload_len >= 43 => 8_usize,
        _ => return Ok(None),
    };
    let resource_type = bytes[3];
    if resource_type == 2 {
        return Ok(None);
    }
    let kind = match resource_type {
        0 => PciRootResourceKind::Memory,
        1 => PciRootResourceKind::Io,
        _ => return Ok(None),
    };
    if bytes[4] & 0xf0 != 0 {
        return Err(Error::InvalidLength);
    }
    let type_flags = bytes[5];
    // Memory-to-I/O and sparse/dense I/O translations require semantics that
    // WovenHat's BAR allocator does not yet implement.
    if type_flags & 0x20 != 0 {
        return Err(Error::InvalidLength);
    }
    fn field(bytes: &[u8], offset: usize, width: usize) -> Result<u64, Error> {
        let end = offset.checked_add(width).ok_or(Error::AddressOverflow)?;
        let source = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
        let mut value = 0_u64;
        for (shift, byte) in source.iter().copied().enumerate() {
            value |= u64::from(byte) << (shift * 8);
        }
        Ok(value)
    }
    let minimum = field(bytes, 6 + width, width)?;
    let maximum = field(bytes, 6 + width * 2, width)?;
    let translation_offset = field(bytes, 6 + width * 3, width)?;
    let length = field(bytes, 6 + width * 4, width)?;
    if length == 0 || minimum.checked_add(length).is_none() {
        return Err(Error::InvalidLength);
    }
    let inclusive_end = minimum.checked_add(length - 1).ok_or(Error::AddressOverflow)?;
    if inclusive_end > maximum {
        return Err(Error::InvalidLength);
    }
    Ok(Some(PciRootResource {
        kind,
        base: minimum,
        length,
        translation_offset,
        prefetchable: matches!(kind, PciRootResourceKind::Memory)
            && ((type_flags >> 1) & 0x3) == 0x3,
        address_width: (width * 8) as u8,
    }))
}

pub fn self_test() -> bool {
    let mut rsdp = [0_u8; RSDP_V2_LENGTH];
    rsdp[..8].copy_from_slice(b"RSD PTR ");
    rsdp[15] = 2;
    rsdp[20..24].copy_from_slice(&(RSDP_V2_LENGTH as u32).to_le_bytes());
    rsdp[24..32].copy_from_slice(&0x1234_5000_u64.to_le_bytes());
    rsdp[8] = 0_u8.wrapping_sub(
        rsdp[..RSDP_V1_LENGTH]
            .iter()
            .fold(0_u8, |sum, byte| sum.wrapping_add(*byte)),
    );
    rsdp[32] = 0_u8.wrapping_sub(rsdp.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)));
    let valid =
        validate_rsdp_v1(&rsdp[..RSDP_V1_LENGTH]).is_ok() && validate_rsdp_v2(&rsdp).is_ok();
    rsdp[9] ^= 1;
    let checksum_rejected =
        validate_rsdp_v1(&rsdp[..RSDP_V1_LENGTH]) == Err(Error::InvalidChecksum);

    let mut topology = Summary {
        local_apic_address: 0xfee0_0000,
        ..Summary::default()
    };
    let local_apic = [0_u8, 8, 0, 1, 1, 0, 0, 0];
    let io_apic = [1_u8, 12, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let interrupt_override = [2_u8, 10, 0, 1, 1, 0, 0, 0, 0, 0];
    let mut address_override = [0_u8; 12];
    address_override[0] = 5;
    address_override[1] = 12;
    address_override[4..12].copy_from_slice(&0xfee0_1000_u64.to_le_bytes());
    let topology_valid = update_madt_summary(&local_apic, &mut topology).is_ok()
        && update_madt_summary(&io_apic, &mut topology).is_ok()
        && update_madt_summary(&interrupt_override, &mut topology).is_ok()
        && update_madt_summary(&address_override, &mut topology).is_ok()
        && topology.enabled_processors == 1
        && topology.io_apics == 1
        && topology.interrupt_overrides == 1
        && topology.local_apic_address == 0xfee0_1000;
    let malformed_rejected =
        update_madt_summary(&[0, 7, 0, 0, 0, 0, 0], &mut topology) == Err(Error::InvalidLength);

    let mut srat = [0_u8; 16];
    srat[0] = 0;
    srat[1] = 16;
    srat[2] = 3;
    srat[3] = 0;
    srat[4] = 1;
    topology.processor_ids[0] = 0;
    topology.processor_count = 1;
    let srat_valid = update_srat_summary(&srat, &mut topology).is_ok()
        && topology.processor_domains[0] == 3
        && topology.numa_domains == 1;
    let mut memory_affinity = [0_u8; 40];
    memory_affinity[0] = 1;
    memory_affinity[1] = 40;
    memory_affinity[2..6].copy_from_slice(&3_u32.to_le_bytes());
    memory_affinity[8..16].copy_from_slice(&0x20_0000_u64.to_le_bytes());
    memory_affinity[16..24].copy_from_slice(&0x10_0000_u64.to_le_bytes());
    memory_affinity[28] = 1;
    let memory_affinity_valid = update_srat_summary(&memory_affinity, &mut topology).is_ok()
        && topology.memory_affinity_count == 1
        && topology.memory_affinities[0].domain == 3
        && topology.memory_affinities[0].base == 0x20_0000
        && topology.memory_affinities[0].length == 0x10_0000;

    let aml_scope = [0x10, 0x05, b'_', b'S', b'B', b'_'];
    let aml_scope_walk = aml_namespace_walk(&aml_scope) == Ok(1);
    let aml_device = [0x5b, 0x82, 0x05, b'P', b'C', b'I', b'0'];
    let aml_device_walk = aml_namespace_walk(&aml_device) == Ok(1);
    let aml_nested = [
        0x10, 0x0c, b'_', b'S', b'B', b'_', 0x5b, 0x82, 0x05, b'P', b'C', b'I', b'0',
    ];
    let aml_nested_walk = aml_namespace_walk(&aml_nested) == Ok(2);

    let aml_pkg_short = decode_aml_pkg_length(&[0x05], 0) == Ok((5, 1));
    let aml_pkg_multi = decode_aml_pkg_length(&[0x41, 0x02], 0) == Ok((33, 2));
    let aml_pkg_bounds = aml_package(&[0x04, 0xaa, 0xbb, 0xcc], 0)
        == Ok(AmlPackage { body_offset: 1, end_offset: 4 });
    let aml_name_valid = aml_name_seg(b"_CRS", 0)
        == Ok((*b"_CRS", 4));
    let aml_name_rejects_lower = aml_name_seg(b"_crs", 0) == Err(Error::InvalidSignature);

    let mut template = [0_u8; 44];
    template[0] = 0x87;
    template[1..3].copy_from_slice(&23_u16.to_le_bytes());
    template[3] = 0;
    template[5] = 0x06;
    template[10..14].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
    template[14..18].copy_from_slice(&0x8fff_ffff_u32.to_le_bytes());
    template[22..26].copy_from_slice(&0x1000_0000_u32.to_le_bytes());
    template[26] = 0x88;
    template[27..29].copy_from_slice(&13_u16.to_le_bytes());
    template[29] = 1;
    template[34..36].copy_from_slice(&0x1000_u16.to_le_bytes());
    template[36..38].copy_from_slice(&0x1fff_u16.to_le_bytes());
    template[40..42].copy_from_slice(&0x1000_u16.to_le_bytes());
    template[42] = 0x79;
    template[43] = 0;
    let mut decoded_resources = [PciRootResource::default(); MAX_PCI_ROOT_RESOURCES];
    let resource_template_valid =
        decode_pci_root_resource_template(&template, &mut decoded_resources).is_ok_and(|count| {
            count == 2
                && decoded_resources[0].kind == PciRootResourceKind::Memory
                && decoded_resources[1].kind == PciRootResourceKind::Io
        });
    let missing_end_tag_rejected =
        decode_pci_root_resource_template(&template[..42], &mut decoded_resources)
            == Err(Error::InvalidLength);

    let mut dword_memory = [0_u8; 26];
    dword_memory[0] = 0x87;
    dword_memory[1..3].copy_from_slice(&23_u16.to_le_bytes());
    dword_memory[3] = 0;
    dword_memory[5] = 0x06;
    dword_memory[10..14].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
    dword_memory[14..18].copy_from_slice(&0x8fff_ffff_u32.to_le_bytes());
    dword_memory[22..26].copy_from_slice(&0x1000_0000_u32.to_le_bytes());
    let dword_memory_valid = decode_address_space_resource(&dword_memory).is_ok_and(|resource| {
        resource.is_some_and(|resource| {
            resource.kind == PciRootResourceKind::Memory
                && resource.base == 0x8000_0000
                && resource.length == 0x1000_0000
                && resource.prefetchable
                && resource.address_width == 32
        })
    });
    let mut word_io = [0_u8; 16];
    word_io[0] = 0x88;
    word_io[1..3].copy_from_slice(&13_u16.to_le_bytes());
    word_io[3] = 1;
    word_io[8..10].copy_from_slice(&0x1000_u16.to_le_bytes());
    word_io[10..12].copy_from_slice(&0x1fff_u16.to_le_bytes());
    word_io[14..16].copy_from_slice(&0x1000_u16.to_le_bytes());
    let word_io_valid = decode_address_space_resource(&word_io).is_ok_and(|resource| {
        resource.is_some_and(|resource| {
            resource.kind == PciRootResourceKind::Io
                && resource.base == 0x1000
                && resource.length == 0x1000
                && !resource.prefetchable
                && resource.address_width == 16
        })
    });

    let mut mcfg_entry = [0_u8; MCFG_ALLOCATION_LENGTH];
    mcfg_entry[..8].copy_from_slice(&0xe000_0000_u64.to_le_bytes());
    mcfg_entry[10] = 0;
    mcfg_entry[11] = 0xff;
    let mcfg_valid = decode_mcfg_allocation(&mcfg_entry).is_ok_and(|allocation| {
        allocation.base_address == 0xe000_0000
            && allocation.segment_group == 0
            && allocation.start_bus == 0
            && allocation.end_bus == 0xff
    });
    mcfg_entry[10] = 2;
    mcfg_entry[11] = 1;
    let malformed_mcfg_rejected = decode_mcfg_allocation(&mcfg_entry) == Err(Error::InvalidLength);

    valid
        && checksum_rejected
        && topology_valid
        && malformed_rejected
        && srat_valid
        && memory_affinity_valid
        && aml_scope_walk
        && aml_device_walk
        && aml_nested_walk
        && aml_pkg_short
        && aml_pkg_multi
        && aml_pkg_bounds
        && aml_name_valid
        && aml_name_rejects_lower
        && resource_template_valid
        && missing_end_tag_rejected
        && dword_memory_valid
        && word_io_valid
        && mcfg_valid
        && malformed_mcfg_rejected
}
