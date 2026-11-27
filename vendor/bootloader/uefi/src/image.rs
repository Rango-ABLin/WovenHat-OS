//! Bounded inspection before the upstream ELF mapper consumes a kernel.
//! This validates structure, not authenticity; signature policy is separate.

use core::ops::Range;

pub const MAX_KERNEL_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    TooLarge,
    Truncated,
    UnsupportedFormat,
    Malformed,
    MissingBootContract,
    UnsupportedBootContract,
}

pub fn validate_size(size: usize) -> Result<(), Error> {
    match size {
        0 => Err(Error::Empty),
        n if n > MAX_KERNEL_BYTES => Err(Error::TooLarge),
        _ => Ok(()),
    }
}

fn range(bytes: &[u8], offset: u64, size: u64) -> Result<Range<usize>, Error> {
    let end = offset.checked_add(size).ok_or(Error::Malformed)?;
    if end > bytes.len() as u64 {
        return Err(Error::Truncated);
    }
    Ok(offset as usize..end as usize)
}

fn number<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::Malformed)?)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(number(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(number(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(number(bytes, offset)?))
}

pub fn contract_range(bytes: &[u8]) -> Result<Range<usize>, Error> {
    validate_size(bytes.len())?;
    if bytes.len() < 64 {
        return Err(Error::Truncated);
    }
    if &bytes[..7] != b"\x7fELF\x02\x01\x01"
        || !matches!(u16_at(bytes, 16)?, 2 | 3)
        || u16_at(bytes, 18)? != 62
        || u32_at(bytes, 20)? != 1
    {
        return Err(Error::UnsupportedFormat);
    }
    if u16_at(bytes, 52)? != 64 || u16_at(bytes, 54)? != 56 {
        return Err(Error::Malformed);
    }
    let programs = range(bytes, u64_at(bytes, 32)?, u16_at(bytes, 56)? as u64 * 56)?;
    let entry = u64_at(bytes, 24)?;
    let mut executable_entry = false;
    for p in bytes[programs].as_chunks::<56>().0 {
        if u32_at(p, 0)? != 1 {
            continue;
        }
        let offset = u64_at(p, 8)?;
        let address = u64_at(p, 16)?;
        let file_size = u64_at(p, 32)?;
        let memory_size = u64_at(p, 40)?;
        range(bytes, offset, file_size)?;
        if file_size > memory_size || address.checked_add(memory_size).is_none() {
            return Err(Error::Malformed);
        }
        let alignment = u64_at(p, 48)?;
        if alignment > 1
            && (!alignment.is_power_of_two() || address % alignment != offset % alignment)
        {
            return Err(Error::Malformed);
        }
        executable_entry |=
            u32_at(p, 4)? & 1 != 0 && entry >= address && entry - address < file_size;
    }
    if !executable_entry {
        return Err(Error::Malformed);
    }
    let count = u16_at(bytes, 60)? as usize;
    if count == 0 {
        return Err(Error::MissingBootContract);
    }
    if u16_at(bytes, 58)? != 64 {
        return Err(Error::Malformed);
    }
    let sections = range(bytes, u64_at(bytes, 40)?, count as u64 * 64)?;
    let headers = &bytes[sections];
    let names_index = u16_at(bytes, 62)? as usize;
    if names_index >= count {
        return Err(Error::Malformed);
    }
    let names_header = &headers[names_index * 64..][..64];
    if u32_at(names_header, 4)? != 3 {
        return Err(Error::Malformed);
    }
    let names = &bytes[range(bytes, u64_at(names_header, 24)?, u64_at(names_header, 32)?)?];
    let mut contract = None;
    for section in headers.as_chunks::<64>().0 {
        let kind = u32_at(section, 4)?;
        let data = if kind != 8 {
            Some(range(bytes, u64_at(section, 24)?, u64_at(section, 32)?)?)
        } else {
            None
        };
        let start = u32_at(section, 0)? as usize;
        let name = names.get(start..).ok_or(Error::Malformed)?;
        let end = name.iter().position(|b| *b == 0).ok_or(Error::Malformed)?;
        if &name[..end] == b".bootloader-config" {
            if contract.is_some() || kind != 1 {
                return Err(Error::Malformed);
            }
            contract = data;
        }
    }
    contract.ok_or(Error::MissingBootContract)
}

pub fn validate(bytes: &[u8]) -> Result<bootloader_api::BootloaderConfig, Error> {
    let contract = contract_range(bytes)?;
    bootloader_api::BootloaderConfig::deserialize(&bytes[contract])
        .map_err(|_| Error::UnsupportedBootContract)
}
