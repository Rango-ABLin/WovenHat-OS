//! PCI-to-PCI bridge resource-window encoding for Stage 13.2.
//!
//! Windows are inclusive in PCI config space. The API accepts base + size and
//! validates the granularity required by conventional type-1 bridge headers.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window { pub base: u64, pub size: u64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Empty, Overflow, Misaligned, AddressWidth }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IoEncoding { pub low: u32, pub upper: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryEncoding { pub value: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefetchEncoding { pub low: u32, pub base_upper: u32, pub limit_upper: u32 }

fn inclusive_limit(window: Window) -> Result<u64, Error> {
    if window.size == 0 { return Err(Error::Empty); }
    window.base.checked_add(window.size - 1).ok_or(Error::Overflow)
}

pub fn encode_io(window: Window) -> Result<IoEncoding, Error> {
    let limit = inclusive_limit(window)?;
    if window.base & 0xfff != 0 || window.size & 0xfff != 0 { return Err(Error::Misaligned); }
    if limit > u64::from(u32::MAX) { return Err(Error::AddressWidth); }
    let base = window.base as u32;
    let limit = limit as u32;
    // 32-bit I/O window: low nibbles carry 0x1 type, upper 16 bits live at 0x30.
    let low = ((base >> 8) & 0x0000_00f0) | ((limit >> 8) & 0x0000_f000) | 0x0000_0101;
    let upper = ((base >> 16) & 0xffff) | (limit & 0xffff_0000);
    Ok(IoEncoding { low, upper })
}

pub fn encode_memory(window: Window) -> Result<MemoryEncoding, Error> {
    let limit = inclusive_limit(window)?;
    if window.base & 0xfffff != 0 || window.size & 0xfffff != 0 { return Err(Error::Misaligned); }
    if limit > u64::from(u32::MAX) { return Err(Error::AddressWidth); }
    let base = window.base as u32;
    let limit = limit as u32;
    Ok(MemoryEncoding { value: ((base >> 16) & 0x0000_fff0) | (limit & 0xfff0_0000) })
}

pub fn encode_prefetch(window: Window) -> Result<PrefetchEncoding, Error> {
    let limit = inclusive_limit(window)?;
    if window.base & 0xfffff != 0 || window.size & 0xfffff != 0 { return Err(Error::Misaligned); }
    let low = (((window.base as u32) >> 16) & 0x0000_fff0)
        | ((limit as u32) & 0xfff0_0000)
        | 0x0001_0001;
    Ok(PrefetchEncoding {
        low,
        base_upper: (window.base >> 32) as u32,
        limit_upper: (limit >> 32) as u32,
    })
}


#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Windows {
    pub io: Option<Window>,
    pub memory: Option<Window>,
    pub prefetch: Option<Window>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub io_low: u32,
    pub memory: u32,
    pub prefetch_low: u32,
    pub prefetch_base_upper: u32,
    pub prefetch_limit_upper: u32,
    pub io_upper: u32,
}

pub fn encode_windows(windows: Windows) -> Result<Registers, Error> {
    // A disabled bridge window is encoded with base > limit. All-zero base/
    // limit pairs can describe a real low-address aperture on some bridges and
    // therefore must not be used as the generic disabled representation.
    let mut registers = Registers {
        io_low: 0x0000_00f1,
        memory: 0x0000_fff0,
        prefetch_low: 0x0001_fff1,
        prefetch_base_upper: 0,
        prefetch_limit_upper: 0,
        io_upper: 0,
    };
    if let Some(window) = windows.io {
        let encoded = encode_io(window)?;
        registers.io_low = encoded.low;
        registers.io_upper = encoded.upper;
    }
    if let Some(window) = windows.memory {
        registers.memory = encode_memory(window)?.value;
    }
    if let Some(window) = windows.prefetch {
        let encoded = encode_prefetch(window)?;
        registers.prefetch_low = encoded.low;
        registers.prefetch_base_upper = encoded.base_upper;
        registers.prefetch_limit_upper = encoded.limit_upper;
    }
    Ok(registers)
}
