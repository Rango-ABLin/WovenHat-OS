//! Bounded ELF image admission plan for dynamic-loader integration.

pub const MAX_LOAD_SEGMENTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub virtual_address: u64,
    pub file_offset: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub flags: u32,
    pub alignment: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImagePlan {
    pub entry: u64,
    pub load_bias: u64,
    pub load_start: u64,
    pub load_end: u64,
    pub segments: [Option<Segment>; MAX_LOAD_SEGMENTS],
    pub segment_count: usize,
    pub has_dynamic: bool,
    pub tls: Option<Segment>,
    pub dynamic: Option<DynamicPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicPlan {
    pub rela: Option<(u64, u64)>,
    pub rela_entry_size: u64,
    pub symtab: Option<u64>,
    pub strtab: Option<u64>,
    pub strtab_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rela {
    pub offset: u64,
    pub info: u64,
    pub addend: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Unsupported,
    Bounds,
    TooManySegments,
    Overflow,
}

impl ImagePlan {
    /// Admit an ELF64 little-endian image at a caller-selected load bias.
    /// This only produces a checked plan; it does not map or execute bytes.
    pub fn parse(image: &[u8], load_bias: u64) -> Result<Self, Error> {
        if image.len() < 64 || &image[..4] != b"\x7fELF" || image[4] != 2 || image[5] != 1 {
            return Err(Error::Invalid);
        }
        let entry = read_u64(image, 24)?.checked_add(load_bias).ok_or(Error::Overflow)?;
        let phoff = usize::try_from(read_u64(image, 32)?).map_err(|_| Error::Overflow)?;
        let phentsize = usize::from(read_u16(image, 54)?);
        let phnum = usize::from(read_u16(image, 56)?);
        if phentsize < 56 || phnum > 64 { return Err(Error::Unsupported); }
        let mut plan = Self {
            entry,
            load_bias,
            load_start: u64::MAX,
            load_end: 0,
            segments: [None; MAX_LOAD_SEGMENTS],
            segment_count: 0,
            has_dynamic: false,
            tls: None,
            dynamic: None,
        };
        for index in 0..phnum {
            let header = phoff.checked_add(index.checked_mul(phentsize).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            if header.checked_add(56).ok_or(Error::Overflow)? > image.len() { return Err(Error::Bounds); }
            let kind = read_u32(image, header)?;
            let flags = read_u32(image, header + 4)?;
            let offset = read_u64(image, header + 8)?;
            let virtual_address = read_u64(image, header + 16)?;
            let file_size = read_u64(image, header + 32)?;
            let memory_size = read_u64(image, header + 40)?;
            let alignment = read_u64(image, header + 48)?;
            let end = offset.checked_add(file_size).ok_or(Error::Overflow)?;
            if end > image.len() as u64 || file_size > memory_size { return Err(Error::Bounds); }
            let segment = Segment { virtual_address: virtual_address.checked_add(load_bias).ok_or(Error::Overflow)?, file_offset: offset, file_size, memory_size, flags, alignment };
            match kind {
                1 => {
                    if plan.segment_count == MAX_LOAD_SEGMENTS { return Err(Error::TooManySegments); }
                    let segment_end = segment.virtual_address.checked_add(memory_size).ok_or(Error::Overflow)?;
                    plan.load_start = plan.load_start.min(segment.virtual_address);
                    plan.load_end = plan.load_end.max(segment_end);
                    plan.segments[plan.segment_count] = Some(segment);
                    plan.segment_count += 1;
                }
                2 => {
                    plan.has_dynamic = true;
                    plan.dynamic = Some(parse_dynamic(image, offset, file_size)?);
                }
                7 => plan.tls = Some(segment),
                _ => {}
            }
        }
        if plan.segment_count == 0 || plan.load_start >= plan.load_end || plan.entry < plan.load_start || plan.entry >= plan.load_end {
            return Err(Error::Invalid);
        }
        Ok(plan)
    }

    pub fn file_offset_for_address(&self, address: u64) -> Option<u64> {
        self.segments.iter().flatten().find_map(|segment| {
            let end = segment.virtual_address.checked_add(segment.file_size)?;
            (address >= segment.virtual_address && address < end)
                .then_some(segment.file_offset + address - segment.virtual_address)
        })
    }

    /// Read one RELA entry from the admitted dynamic table.
    pub fn rela_at(&self, image: &[u8], index: u64) -> Result<Rela, Error> {
        let dynamic = self.dynamic.ok_or(Error::Unsupported)?;
        let (address, size) = dynamic.rela.ok_or(Error::Unsupported)?;
        let byte_offset = index.checked_mul(dynamic.rela_entry_size).ok_or(Error::Overflow)?;
        if byte_offset.checked_add(24).ok_or(Error::Overflow)? > size {
            return Err(Error::Bounds);
        }
        let file_address = address.checked_add(byte_offset).ok_or(Error::Overflow)?
            .checked_add(self.load_bias).ok_or(Error::Overflow)?;
        let file_offset = usize::try_from(self.file_offset_for_address(file_address).ok_or(Error::Bounds)?)
            .map_err(|_| Error::Overflow)?;
        Ok(Rela {
            offset: read_u64(image, file_offset)?,
            info: read_u64(image, file_offset + 8)?,
            addend: read_u64(image, file_offset + 16)? as i64,
        })
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> { bytes.get(offset..offset + 2).and_then(|b| b.try_into().ok()).map(u16::from_le_bytes).ok_or(Error::Bounds) }
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> { bytes.get(offset..offset + 4).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes).ok_or(Error::Bounds) }
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> { bytes.get(offset..offset + 8).and_then(|b| b.try_into().ok()).map(u64::from_le_bytes).ok_or(Error::Bounds) }

fn parse_dynamic(image: &[u8], offset: u64, size: u64) -> Result<DynamicPlan, Error> {
    let start = usize::try_from(offset).map_err(|_| Error::Overflow)?;
    let end = usize::try_from(offset.checked_add(size).ok_or(Error::Overflow)?).map_err(|_| Error::Overflow)?;
    if end > image.len() || !size.is_multiple_of(16) { return Err(Error::Bounds); }
    let mut plan = DynamicPlan { rela: None, rela_entry_size: 24, symtab: None, strtab: None, strtab_size: 0 };
    let mut cursor = start;
    while cursor < end {
        let tag = read_u64(image, cursor)?;
        let value = read_u64(image, cursor + 8)?;
        cursor += 16;
        match tag {
            0 => break,
            5 => plan.strtab = Some(value),
            6 => plan.symtab = Some(value),
            10 => plan.strtab_size = value,
            7 => plan.rela = Some((value, 0)),
            8 => if let Some((address, _)) = plan.rela { plan.rela = Some((address, value)); },
            9 => plan.rela_entry_size = value,
            _ => {}
        }
    }
    if plan.rela_entry_size != 24 { return Err(Error::Unsupported); }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::ImagePlan;

    #[test]
    fn admits_dynamic_tls_image_with_bias() {
        let mut image = vec![0u8; 0x200];
        image[..4].copy_from_slice(b"\x7fELF"); image[4] = 2; image[5] = 1;
        image[24..32].copy_from_slice(&0x1000u64.to_le_bytes());
        image[32..40].copy_from_slice(&64u64.to_le_bytes());
        image[54..56].copy_from_slice(&56u16.to_le_bytes()); image[56..58].copy_from_slice(&3u16.to_le_bytes());
        image[64..68].copy_from_slice(&1u32.to_le_bytes()); image[72..80].copy_from_slice(&0x100u64.to_le_bytes()); image[80..88].copy_from_slice(&0x1000u64.to_le_bytes()); image[96..104].copy_from_slice(&4u64.to_le_bytes()); image[104..112].copy_from_slice(&0x100u64.to_le_bytes());
        image[120..124].copy_from_slice(&7u32.to_le_bytes()); image[128..136].copy_from_slice(&0x180u64.to_le_bytes()); image[160..168].copy_from_slice(&4u64.to_le_bytes()); image[168..176].copy_from_slice(&8u64.to_le_bytes());
        image[176..180].copy_from_slice(&2u32.to_le_bytes());
        let plan = ImagePlan::parse(&image, 0x4000).unwrap();
        assert_eq!(plan.entry, 0x5000); assert!(plan.has_dynamic); assert!(plan.tls.is_some());
    }
}
