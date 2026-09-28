//! Pure BAR sizing/encoding rules used by the Stage 13.2 PCI resource path.
//!
//! Hardware probing lives in pci.rs; this module keeps the mask arithmetic and
//! assignment encoding host-testable.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Io, Memory32, Memory64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub kind: Kind,
    pub size: u64,
    pub prefetchable: bool,
    pub low_flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { InvalidMask, UnsupportedMemoryType, AddressOverflow }

pub fn decode_probe(original_low: u32, mask_low: u32, mask_high: Option<u32>) -> Result<Probe, Error> {
    if original_low & 1 != 0 {
        let mask = u64::from(mask_low & !0x3);
        let size = (!mask & 0xffff_ffff).wrapping_add(1) & 0xffff_ffff;
        if size == 0 || !size.is_power_of_two() { return Err(Error::InvalidMask); }
        return Ok(Probe { kind: Kind::Io, size, prefetchable: false, low_flags: original_low & 0x3 });
    }
    let memory_type = (original_low >> 1) & 0x3;
    let prefetchable = original_low & 0x8 != 0;
    let low_flags = original_low & 0xf;
    match memory_type {
        0 => {
            let mask = u64::from(mask_low & !0xf);
            let size = (!mask & 0xffff_ffff).wrapping_add(1) & 0xffff_ffff;
            if size == 0 || !size.is_power_of_two() { return Err(Error::InvalidMask); }
            Ok(Probe { kind: Kind::Memory32, size, prefetchable, low_flags })
        }
        2 => {
            let high = mask_high.ok_or(Error::InvalidMask)?;
            let mask = (u64::from(high) << 32) | u64::from(mask_low & !0xf);
            let size = (!mask).wrapping_add(1);
            if size == 0 || !size.is_power_of_two() { return Err(Error::InvalidMask); }
            Ok(Probe { kind: Kind::Memory64, size, prefetchable, low_flags })
        }
        _ => Err(Error::UnsupportedMemoryType),
    }
}

#[cfg_attr(test, expect(dead_code, reason = "exercised by pci_bar integration tests, not assignment-only tests"))]
pub fn encode(probe: Probe, base: u64) -> Result<(u32, Option<u32>), Error> {
    if base & (probe.size - 1) != 0 { return Err(Error::AddressOverflow); }
    match probe.kind {
        Kind::Io => {
            let base = u32::try_from(base).map_err(|_| Error::AddressOverflow)?;
            Ok(((base & !0x3) | probe.low_flags, None))
        }
        Kind::Memory32 => {
            let base = u32::try_from(base).map_err(|_| Error::AddressOverflow)?;
            Ok(((base & !0xf) | probe.low_flags, None))
        }
        Kind::Memory64 => Ok((((base as u32) & !0xf) | probe.low_flags, Some((base >> 32) as u32))),
    }
}
