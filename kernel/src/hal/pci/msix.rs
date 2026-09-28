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
}
