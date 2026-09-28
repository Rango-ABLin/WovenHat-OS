//! PCI MSI-X capability decoding and bounded table/PBA layout validation.
//!
//! Actual table writes require a validated BAR mapping owned by the driver.
//! This module deliberately separates capability/layout validation from MMIO
//! programming so configuration-space discovery can never dereference an
//! unowned device BAR.

use super::{Address, Device, read_config};

pub const PCI_CAP_ID_MSIX: u8 = 0x11;
pub const PCI_MSIX_ENABLE: u16 = 1 << 15;
pub const PCI_MSIX_FUNCTION_MASK: u16 = 1 << 14;
pub const MAX_MSIX_VECTORS: u16 = 2048;
pub const TABLE_ENTRY_BYTES: u64 = 16;

pub const VECTOR_CONTROL_MASKED: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableEntry {
    pub address_low: u32,
    pub address_high: u32,
    pub data: u32,
    pub vector_control: u32,
}

pub fn entry_offset(capability: Capability, index: u16) -> Result<u64, Error> {
    if index >= capability.table_size { return Err(Error::MalformedCapability); }
    u64::from(capability.table.offset)
        .checked_add(u64::from(index).checked_mul(TABLE_ENTRY_BYTES).ok_or(Error::Overflow)?)
        .ok_or(Error::Overflow)
}

pub fn masked_entry(destination_apic_id: u32, vector: u8) -> Result<TableEntry, Error> {
    let message = super::MsiMessage::fixed(destination_apic_id, vector)
        .map_err(|_| Error::MalformedCapability)?;
    Ok(TableEntry {
        address_low: message.address_low,
        address_high: message.address_high,
        data: u32::from(message.data),
        vector_control: VECTOR_CONTROL_MASKED,
    })
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirOffset {
    pub bir: u8,
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capability {
    pub offset: u16,
    pub control: u16,
    pub table_size: u16,
    pub table: BirOffset,
    pub pba: BirOffset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NoCapability,
    MalformedCapability,
    ConfigRead,
    InvalidBir,
    Overflow,
    InvalidLease,
    ConfigWrite,
    VerifyFailed,
    MmioMap,
}

pub fn decode_bir_offset(raw: u32) -> Result<BirOffset, Error> {
    let bir = (raw & 0x7) as u8;
    if bir > 5 { return Err(Error::InvalidBir); }
    Ok(BirOffset { bir, offset: raw & !0x7 })
}

pub fn decode_capability(offset: u16, header: u32, table: u32, pba: u32) -> Result<Capability, Error> {
    if header as u8 != PCI_CAP_ID_MSIX { return Err(Error::MalformedCapability); }
    let control = (header >> 16) as u16;
    let table_size = (control & 0x07ff) + 1;
    if table_size == 0 || table_size > MAX_MSIX_VECTORS { return Err(Error::MalformedCapability); }
    Ok(Capability {
        offset,
        control,
        table_size,
        table: decode_bir_offset(table)?,
        pba: decode_bir_offset(pba)?,
    })
}

pub fn capability(device: Device) -> Result<Capability, Error> {
    if !device.capabilities.msix || device.capabilities.msix_offset == 0 {
        return Err(Error::NoCapability);
    }
    let address = Address {
        segment: device.segment, bus: device.bus,
        device: device.device, function: device.function,
    };
    let base = device.capabilities.msix_offset;
    let header = read_config(address, base).ok_or(Error::ConfigRead)?;
    let table = read_config(address, base + 4).ok_or(Error::ConfigRead)?;
    let pba = read_config(address, base + 8).ok_or(Error::ConfigRead)?;
    decode_capability(base, header, table, pba)
}

pub fn table_span(capability: Capability) -> Result<(u64, u64), Error> {
    let bytes = u64::from(capability.table_size)
        .checked_mul(TABLE_ENTRY_BYTES).ok_or(Error::Overflow)?;
    let end = u64::from(capability.table.offset)
        .checked_add(bytes).ok_or(Error::Overflow)?;
    Ok((u64::from(capability.table.offset), end))
}

pub fn pba_span(capability: Capability) -> Result<(u64, u64), Error> {
    let qwords = u64::from(capability.table_size).div_ceil(64);
    let bytes = qwords.checked_mul(8).ok_or(Error::Overflow)?;
    let end = u64::from(capability.pba.offset).checked_add(bytes).ok_or(Error::Overflow)?;
    Ok((u64::from(capability.pba.offset), end))
}




fn map_table_entry(table_base: u64, offset: u64) -> Result<*mut u32, Error> {
    let physical = table_base.checked_add(offset).ok_or(Error::Overflow)?;
    let page = physical & !0xfff;
    let within = physical & 0xfff;
    // A 16-byte MSI-X entry may straddle a 4 KiB boundary. Map both pages
    // before returning a pointer into WovenHat's fixed MMIO virtual window.
    let first = crate::paging::map_mmio(page).map_err(|_| Error::MmioMap)?;
    if within > 0xff0 {
        crate::paging::map_mmio(page.checked_add(0x1000).ok_or(Error::Overflow)?)
            .map_err(|_| Error::MmioMap)?;
    }
    let virtual_address = first.checked_add(within).ok_or(Error::Overflow)?;
    Ok(virtual_address as *mut u32)
}

/// Program one MSI-X table entry while it remains masked, then verify every
/// dword using volatile MMIO. The caller must have already validated the BAR
/// lease and function-masked MSI-X.
pub fn program_masked_entry(
    table_base: u64,
    capability: Capability,
    index: u16,
    entry: TableEntry,
) -> Result<(), Error> {
    let offset = entry_offset(capability, index)?
        .checked_sub(u64::from(capability.table.offset))
        .ok_or(Error::Overflow)?;
    let pointer = map_table_entry(table_base, offset)?;
    // SAFETY: table_base came from a current owner-bound BAR lease; the entry
    // is bounds-checked against the MSI-X capability; map_mmio created
    // uncached writable NX mappings for all bytes in this 16-byte record.
    unsafe {
        pointer.add(3).write_volatile(VECTOR_CONTROL_MASKED);
        pointer.write_volatile(entry.address_low);
        pointer.add(1).write_volatile(entry.address_high);
        pointer.add(2).write_volatile(entry.data);
        if pointer.read_volatile() != entry.address_low
            || pointer.add(1).read_volatile() != entry.address_high
            || pointer.add(2).read_volatile() != entry.data
            || pointer.add(3).read_volatile() & VECTOR_CONTROL_MASKED == 0
        {
            return Err(Error::VerifyFailed);
        }
    }
    Ok(())
}

fn device_for(function: super::topology::FunctionHandle) -> Option<Device> {
    let snapshot = super::TOPOLOGY.lock().snapshot(function).ok()?;
    super::INVENTORY.lock().devices.iter().flatten().find(|device| {
        device.segment == snapshot.address.segment
            && device.bus == snapshot.address.bus
            && device.device == snapshot.address.device
            && device.function == snapshot.address.function
    }).copied()
}

/// Validate that an MSI-X table belongs to the BAR authorized by a
/// generation-safe MMIO lease. This is the mandatory gate before table MMIO.
pub fn validate_table_lease(
    lease: super::topology::MmioLease,
    owner: u32,
    capability: Capability,
) -> Result<u64, Error> {
    if !super::validate_bar_lease(lease, owner) || lease.bar != capability.table.bir {
        return Err(Error::InvalidLease);
    }
    let (start, _) = table_span(capability)?;
    lease.base.checked_add(start).ok_or(Error::Overflow)
}

/// Set Function Mask while preserving the capability ID/next pointer and all
/// read-only control bits. MSI-X remains disabled until table programming has
/// completed and been verified.
pub fn mask_function(device: Device) -> Result<Capability, Error> {
    let capability = capability(device)?;
    let address = Address {
        segment: device.segment, bus: device.bus,
        device: device.device, function: device.function,
    };
    let _guard = super::CONFIG_LOCK.lock();
    let header = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    let mut control = (header >> 16) as u16;
    control |= PCI_MSIX_FUNCTION_MASK;
    control &= !PCI_MSIX_ENABLE;
    let updated = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !super::write_config_unlocked(address, capability.offset, updated) {
        return Err(Error::ConfigWrite);
    }
    let verify = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    if (verify >> 16) as u16 != control { return Err(Error::VerifyFailed); }
    Ok(Capability { control, ..capability })
}


/// Enable MSI-X while keeping Function Mask asserted. The selected table
/// entry is unmasked only after the capability enable write is verified.
pub fn enable_function_masked(device: Device) -> Result<Capability, Error> {
    let capability = capability(device)?;
    let address = Address {
        segment: device.segment, bus: device.bus,
        device: device.device, function: device.function,
    };
    let _guard = super::CONFIG_LOCK.lock();
    let header = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    let mut control = (header >> 16) as u16;
    control |= PCI_MSIX_ENABLE | PCI_MSIX_FUNCTION_MASK;
    let updated = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !super::write_config_unlocked(address, capability.offset, updated) {
        return Err(Error::ConfigWrite);
    }
    let verify = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    if (verify >> 16) as u16 != control { return Err(Error::VerifyFailed); }
    Ok(Capability { control, ..capability })
}

pub fn unmask_entry(table_base: u64, capability: Capability, index: u16) -> Result<(), Error> {
    let offset = entry_offset(capability, index)?
        .checked_sub(u64::from(capability.table.offset)).ok_or(Error::Overflow)?;
    let pointer = map_table_entry(table_base, offset)?;
    // SAFETY: same validated mapping and bounded entry contract as
    // program_masked_entry; only the vector-control dword is changed.
    unsafe {
        pointer.add(3).write_volatile(0);
        if pointer.add(3).read_volatile() & VECTOR_CONTROL_MASKED != 0 {
            return Err(Error::VerifyFailed);
        }
    }
    Ok(())
}

pub fn disable_and_mask_function(device: Device) -> Result<(), Error> {
    let capability = capability(device)?;
    let address = Address {
        segment: device.segment, bus: device.bus,
        device: device.device, function: device.function,
    };
    let _guard = super::CONFIG_LOCK.lock();
    let header = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    let mut control = (header >> 16) as u16;
    control |= PCI_MSIX_FUNCTION_MASK;
    control &= !PCI_MSIX_ENABLE;
    let updated = (header & 0x0000_ffff) | (u32::from(control) << 16);
    if !super::write_config_unlocked(address, capability.offset, updated) {
        return Err(Error::ConfigWrite);
    }
    let verify = super::read_config_unlocked(address, capability.offset).ok_or(Error::ConfigRead)?;
    if (verify >> 16) as u16 != control { return Err(Error::VerifyFailed); }
    Ok(())
}

#[expect(dead_code)]
pub fn validate_owned_table(
    function: super::topology::FunctionHandle,
    lease: super::topology::MmioLease,
    owner: u32,
) -> Result<(Capability, u64), Error> {
    if lease.function != function {
        return Err(Error::InvalidLease);
    }
    let device = device_for(function).ok_or(Error::InvalidLease)?;
    // Validate the BAR lease against the discovered capability before making
    // any configuration-space mutation.
    let discovered = capability(device)?;
    let base = validate_table_lease(lease, owner, discovered)?;
    // Once authority and BIR are proven, quiesce MSI-X before any table MMIO.
    // This makes ConfigWrite/VerifyFailed part of the real preparation path.
    let masked = mask_function(device)?;
    Ok((masked, base))
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsixLease {
    pub function: super::topology::FunctionHandle,
    pub bar: super::topology::MmioLease,
    pub vector: super::vector::Lease,
    pub entry: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    Msix(Error),
    Vector(super::vector::Error),
}

impl From<Error> for LifecycleError {
    fn from(error: Error) -> Self { Self::Msix(error) }
}
impl From<super::vector::Error> for LifecycleError {
    fn from(error: super::vector::Error) -> Self { Self::Vector(error) }
}

/// Complete one-vector MSI-X activation transaction. Vector ownership is
/// retained on every failure after the device could have observed the vector.
#[expect(dead_code)]
pub fn enable_owned(
    function: super::topology::FunctionHandle,
    bar: super::topology::MmioLease,
    owner: u32,
    destination_apic_id: u32,
    entry_index: u16,
) -> Result<MsixLease, LifecycleError> {
    let device = device_for(function).ok_or(Error::InvalidLease)?;
    let (masked_capability, table_base) = validate_owned_table(function, bar, owner)?;
    let vector = super::msi::allocate_vector(owner)?;
    let entry = match masked_entry(destination_apic_id, vector.vector) {
        Ok(entry) => entry,
        Err(error) => {
            let _ = super::msi::release_vector(vector, owner);
            return Err(error.into());
        }
    };
    if let Err(error) = program_masked_entry(table_base, masked_capability, entry_index, entry) {
        let _ = super::msi::release_vector(vector, owner);
        return Err(error.into());
    }
    // From this point onward the table contains the vector. On any failure,
    // keep the vector reserved rather than risk delivery to a future owner.
    let enabled = enable_function_masked(device)?;
    unmask_entry(table_base, enabled, entry_index)?;
    Ok(MsixLease { function, bar, vector, entry: entry_index })
}

/// Quiesce the function before recycling its interrupt vector. Failure keeps
/// the lease live and the vector unavailable for reuse.
#[expect(dead_code)]
pub fn disable_owned(lease: MsixLease, owner: u32) -> Result<(), LifecycleError> {
    super::msi::validate_vector(lease.vector, owner)?;
    if lease.bar.function != lease.function || !super::validate_bar_lease(lease.bar, owner) {
        return Err(Error::InvalidLease.into());
    }
    let device = device_for(lease.function).ok_or(Error::InvalidLease)?;
    disable_and_mask_function(device)?;
    super::msi::release_vector(lease.vector, owner)?;
    Ok(())
}

/// Pure Stage 13.2 acceptance coverage for MSI-X capability decoding and
/// table/PBA bounds. Hardware MMIO programming remains gated on BAR ownership.
pub fn stage13_2_msix_self_test() -> bool {
    // Exercise the real device-facing capability path as well as the pure
    // decoder. A synthetic device with no MSI-X capability must fail closed
    // before any configuration-space access.
    let absent = Device {
        capabilities: super::Capabilities::default(),
        ..Device::default()
    };
    if capability(absent) != Err(Error::NoCapability) {
        return false;
    }

    let header = u32::from(PCI_CAP_ID_MSIX) | (3_u32 << 16); // four vectors
    let Ok(capability) = decode_capability(0x60, header, 0x0000_2000, 0x0000_3000) else {
        return false;
    };
    if capability.offset != 0x60
        || capability.control & (PCI_MSIX_ENABLE | PCI_MSIX_FUNCTION_MASK) != 0
        || capability.table_size != 4
        || capability.table.bir != 0
        || capability.pba.bir != 0
    {
        return false;
    }
    if table_span(capability) != Ok((0x2000, 0x2040))
        || pba_span(capability) != Ok((0x3000, 0x3008))
    {
        return false;
    }
    decode_bir_offset(0x0000_1006) == Err(Error::InvalidBir)
        && capability.table_size <= MAX_MSIX_VECTORS
        && TABLE_ENTRY_BYTES == 16
        && entry_offset(capability, 3) == Ok(0x2030)
        && entry_offset(capability, 4) == Err(Error::MalformedCapability)
        && masked_entry(0x2a, 0x90) == Ok(TableEntry {
            address_low: 0xfee2_a000,
            address_high: 0,
            data: 0x90,
            vector_control: VECTOR_CONTROL_MASKED,
        })
}
