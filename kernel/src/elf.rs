use crate::config::MAX_ELF_SEGMENTS as MAX_LOAD_SEGMENTS;

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_TLS: u32 = 7;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const ELF_MACHINE_X86_64: u16 = 0x3e;
const USER_ADDRESS_LIMIT: u64 = 0x0000_8000_0000_0000;
const PAGE_SIZE: u64 = 4096;
const R_X86_64_RELATIVE: u32 = 8;
const RELA_ENTRY_SIZE: usize = 24;
const MAX_RELATIVE_RELOCATIONS: usize = 4096;

#[derive(Clone, Copy)]
pub struct LoadSegment {
    pub file_offset: usize,
    pub file_size: usize,
    pub virtual_address: u64,
    pub memory_size: usize,
    pub mapping_start: u64,
    pub mapping_size: usize,
    pub writable: bool,
    pub executable: bool,
}

#[derive(Clone, Copy)]
pub struct TlsImage {
    pub file_offset: usize,
    pub file_size: usize,
    pub memory_size: usize,
    pub alignment: u64,
}

#[derive(Clone, Copy)]
pub struct DynamicImage {
    pub dynamic_address: u64,
    pub dynamic_size: usize,
    pub rela_address: u64,
    pub rela_file_offset: usize,
    pub rela_size: usize,
    pub rela_entry_size: usize,
}

pub struct Image {
    pub entry: u64,
    segments: [Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    segment_count: usize,
    relro: Option<(u64, usize)>,
    tls: Option<TlsImage>,
    dynamic: Option<DynamicImage>,
    load_bias: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Truncated,
    InvalidHeader,
    Unsupported,
    InvalidSegment,
    OverlappingSegments,
    InvalidEntry,
    TooManySegments,
}

impl Image {
    pub fn segments(&self) -> impl Iterator<Item = LoadSegment> + '_ {
        self.segments[..self.segment_count]
            .iter()
            .filter_map(|segment| *segment)
    }

    pub const fn segment_count(&self) -> usize {
        self.segment_count
    }

    /// Page-aligned GNU RELRO range, if the image declares one.
    pub const fn relro(&self) -> Option<(u64, usize)> {
        self.relro
    }

    pub const fn tls(&self) -> Option<TlsImage> {
        self.tls
    }

    pub const fn dynamic(&self) -> Option<DynamicImage> {
        self.dynamic
    }

    pub const fn load_bias(&self) -> u64 {
        self.load_bias
    }

    /// Relocate every load segment and the entry by one page-aligned delta.
    /// The parser's W^X, overlap and file-range checks remain in force, while
    /// this second pass rechecks user bounds after the randomized placement.
    pub fn relocated(mut self, delta: u64) -> Option<Self> {
        self.entry = self.entry.checked_add(delta)?;
        self.load_bias = delta;
        if let Some((start, size)) = self.relro.as_mut() {
            *start = start.checked_add(delta)?;
            let end = start.checked_add(*size as u64)?;
            if end > USER_ADDRESS_LIMIT {
                return None;
            }
        }
        for segment in self.segments[..self.segment_count].iter_mut().flatten() {
            segment.virtual_address = segment.virtual_address.checked_add(delta)?;
            segment.mapping_start = segment.mapping_start.checked_add(delta)?;
            let memory_end = segment
                .virtual_address
                .checked_add(segment.memory_size as u64)?;
            let mapping_end = segment
                .mapping_start
                .checked_add(segment.mapping_size as u64)?;
            if segment.mapping_start < USER_ADDRESS_LIMIT
                && memory_end < USER_ADDRESS_LIMIT
                && mapping_end <= USER_ADDRESS_LIMIT
            {
                continue;
            }
            return None;
        }
        for (index, segment) in self.segments[..self.segment_count]
            .iter()
            .flatten()
            .enumerate()
        {
            let end = segment
                .mapping_start
                .checked_add(segment.mapping_size as u64)?;
            for other in self.segments[..index].iter().flatten() {
                let other_end = other.mapping_start.checked_add(other.mapping_size as u64)?;
                if segment.mapping_start < other_end && other.mapping_start < end {
                    return None;
                }
            }
        }
        Some(self)
    }
}

pub fn parse(bytes: &[u8]) -> Result<Image, Error> {
    if bytes.len() < ELF_HEADER_SIZE {
        return Err(Error::Truncated);
    }
    let elf_type = read_u16(bytes, 16)?;
    if &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(elf_type, 2 | 3)
        || read_u16(bytes, 18)? != ELF_MACHINE_X86_64
        || read_u32(bytes, 20)? != 1
        || read_u16(bytes, 52)? as usize != ELF_HEADER_SIZE
    {
        return Err(Error::InvalidHeader);
    }

    let entry = read_u64(bytes, 24)?;
    let program_offset = usize::try_from(read_u64(bytes, 32)?).map_err(|_| Error::Unsupported)?;
    let program_entry_size = read_u16(bytes, 54)? as usize;
    let program_count = read_u16(bytes, 56)? as usize;
    if program_entry_size < PROGRAM_HEADER_SIZE || program_count == 0 {
        return Err(Error::Unsupported);
    }
    let table_size = program_entry_size
        .checked_mul(program_count)
        .ok_or(Error::Truncated)?;
    if program_offset
        .checked_add(table_size)
        .filter(|end| *end <= bytes.len())
        .is_none()
    {
        return Err(Error::Truncated);
    }

    let mut image = Image {
        entry,
        segments: [None; MAX_LOAD_SEGMENTS],
        segment_count: 0,
        relro: None,
        tls: None,
        dynamic: None,
        load_bias: 0,
    };
    let mut entry_is_executable = false;

    for index in 0..program_count {
        let offset = program_offset + index * program_entry_size;
        let program_type = read_u32(bytes, offset)?;
        if program_type == PT_INTERP {
            return Err(Error::Unsupported);
        }
        if program_type == PT_DYNAMIC {
            if elf_type != 3 {
                return Err(Error::Unsupported);
            }
            if image.dynamic.is_some() {
                return Err(Error::Unsupported);
            }
            let file_offset = usize::try_from(read_u64(bytes, offset + 8)?)
                .map_err(|_| Error::InvalidSegment)?;
            let virtual_address = read_u64(bytes, offset + 16)?;
            let file_size = usize::try_from(read_u64(bytes, offset + 32)?)
                .map_err(|_| Error::InvalidSegment)?;
            let memory_size = usize::try_from(read_u64(bytes, offset + 40)?)
                .map_err(|_| Error::InvalidSegment)?;
            let file_end = file_offset
                .checked_add(file_size)
                .filter(|end| *end <= bytes.len())
                .ok_or(Error::Truncated)?;
            if file_size == 0
                || file_size > memory_size
                || virtual_address
                    .checked_add(memory_size as u64)
                    .filter(|end| *end < USER_ADDRESS_LIMIT)
                    .is_none()
                || file_size % 16 != 0
            {
                return Err(Error::InvalidSegment);
            }
            let mut rela_address = None;
            let mut rela_size = 0usize;
            let mut rela_entry_size = 0usize;
            let mut plt_rela_size = 0usize;
            let dynamic_bytes = &bytes[file_offset..file_end];
            let mut terminated = false;
            let (dynamic_entries, _) = dynamic_bytes.as_chunks::<16>();
            for entry in dynamic_entries {
                let tag = i64::from_le_bytes(entry[..8].try_into().unwrap());
                let value = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                match tag {
                    0 => {
                        terminated = true;
                        break;
                    }
                    1 | 14 | 15 | 16 | 17 | 18 | 19 | 22 | 23 | 29 => {
                        // Dependencies, symbol-based relocation tables, and
                        // text-relocation requests need a dynamic linker or
                        // shared-library authority that the kernel does not
                        // provide.
                        if value != 0 || matches!(tag, 1 | 14 | 15 | 16 | 17 | 18 | 19 | 22 | 23 | 29)
                        {
                            return Err(Error::Unsupported);
                        }
                    }
                    7 => rela_address = Some(value),
                    8 => rela_size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?,
                    9 => {
                        rela_entry_size = usize::try_from(value)
                            .map_err(|_| Error::InvalidSegment)?;
                    }
                    2 => plt_rela_size = usize::try_from(value)
                        .map_err(|_| Error::InvalidSegment)?,
                    // Common metadata tags do not affect relocation of a
                    // symbol-free static PIE and are intentionally ignored.
                    3 | 4 | 5 | 6 | 10 | 11 | 30 | 0x6fff_fff0 | 0x6fff_fff9
                    | 0x6fff_fffb | 0x6fff_fffe | 0x6fff_ffff => {}
                    _ => return Err(Error::Unsupported),
                }
            }
            if !terminated || plt_rela_size != 0 {
                return Err(Error::Unsupported);
            }
            if rela_size == 0 {
                if rela_address.is_some() || rela_entry_size != 0 {
                    return Err(Error::InvalidSegment);
                }
            } else {
                if rela_entry_size != RELA_ENTRY_SIZE
                    || !rela_size.is_multiple_of(rela_entry_size)
                    || rela_size / rela_entry_size > MAX_RELATIVE_RELOCATIONS
                {
                    return Err(Error::InvalidSegment);
                }
                let rela_address = rela_address.ok_or(Error::InvalidSegment)?;
                image.dynamic = Some(DynamicImage {
                    dynamic_address: virtual_address,
                    dynamic_size: file_size,
                    rela_address,
                    rela_file_offset: 0,
                    rela_size,
                    rela_entry_size,
                });
                // Resolve the virtual relocation table to its file-backed
                // bytes after all PT_LOAD headers have been collected.
                let _ = (rela_address, virtual_address);
            }
            if image.dynamic.is_none() {
                image.dynamic = Some(DynamicImage {
                    dynamic_address: virtual_address,
                    dynamic_size: file_size,
                    rela_address: 0,
                    rela_file_offset: 0,
                    rela_size: 0,
                    rela_entry_size: 0,
                });
            }
            continue;
        }
        if program_type == PT_TLS {
            if image.tls.is_some() {
                return Err(Error::Unsupported);
            }
            let file_offset =
                usize::try_from(read_u64(bytes, offset + 8)?).map_err(|_| Error::InvalidSegment)?;
            let file_size =
                usize::try_from(read_u64(bytes, offset + 32)?).map_err(|_| Error::InvalidSegment)?;
            let memory_size =
                usize::try_from(read_u64(bytes, offset + 40)?).map_err(|_| Error::InvalidSegment)?;
            let alignment = read_u64(bytes, offset + 48)?;
            if memory_size == 0
                || file_size > memory_size
                || !alignment.is_power_of_two()
                || alignment > PAGE_SIZE
                || file_offset
                    .checked_add(file_size)
                    .filter(|end| *end <= bytes.len())
                    .is_none()
            {
                return Err(Error::InvalidSegment);
            }
            image.tls = Some(TlsImage {
                file_offset,
                file_size,
                memory_size,
                alignment,
            });
            continue;
        }
        if program_type == PT_GNU_RELRO {
            if image.relro.is_some() {
                return Err(Error::Unsupported);
            }
            let virtual_address = read_u64(bytes, offset + 16)?;
            let memory_size =
                usize::try_from(read_u64(bytes, offset + 40)?).map_err(|_| Error::InvalidSegment)?;
            if memory_size == 0 {
                return Err(Error::InvalidSegment);
            }
            let end = virtual_address
                .checked_add(memory_size as u64)
                .filter(|end| *end <= USER_ADDRESS_LIMIT)
                .and_then(align_up)
                .ok_or(Error::InvalidSegment)?;
            let start = align_down(virtual_address);
            let size = usize::try_from(end - start).map_err(|_| Error::InvalidSegment)?;
            image.relro = Some((start, size));
            continue;
        }
        if program_type != PT_LOAD {
            continue;
        }
        if image.segment_count == MAX_LOAD_SEGMENTS {
            return Err(Error::TooManySegments);
        }

        let flags = read_u32(bytes, offset + 4)?;
        let file_offset =
            usize::try_from(read_u64(bytes, offset + 8)?).map_err(|_| Error::InvalidSegment)?;
        let virtual_address = read_u64(bytes, offset + 16)?;
        let file_size =
            usize::try_from(read_u64(bytes, offset + 32)?).map_err(|_| Error::InvalidSegment)?;
        let memory_size =
            usize::try_from(read_u64(bytes, offset + 40)?).map_err(|_| Error::InvalidSegment)?;
        let alignment = read_u64(bytes, offset + 48)?;
        let writable = flags & PF_WRITE != 0;
        let executable = flags & PF_EXECUTE != 0;

        if memory_size == 0
            || file_size > memory_size
            || writable && executable
            || alignment == 0
            || !alignment.is_power_of_two()
            || virtual_address % alignment != file_offset as u64 % alignment
        {
            return Err(Error::InvalidSegment);
        }
        let file_end = file_offset
            .checked_add(file_size)
            .filter(|end| *end <= bytes.len())
            .ok_or(Error::Truncated)?;
        let memory_end = virtual_address
            .checked_add(memory_size as u64)
            .filter(|end| *end < USER_ADDRESS_LIMIT)
            .ok_or(Error::InvalidSegment)?;
        let mapping_start = align_down(virtual_address);
        let mapping_end = align_up(memory_end).ok_or(Error::InvalidSegment)?;
        let mapping_size =
            usize::try_from(mapping_end - mapping_start).map_err(|_| Error::InvalidSegment)?;

        for existing in image.segments() {
            let existing_end = existing.mapping_start + existing.mapping_size as u64;
            if mapping_start < existing_end && existing.mapping_start < mapping_end {
                return Err(Error::OverlappingSegments);
            }
        }

        if executable && entry >= virtual_address && entry < memory_end {
            entry_is_executable = true;
        }
        let _ = file_end;
        image.segments[image.segment_count] = Some(LoadSegment {
            file_offset,
            file_size,
            virtual_address,
            memory_size,
            mapping_start,
            mapping_size,
            writable,
            executable,
        });
        image.segment_count += 1;
    }

    if image.segment_count == 0 || !entry_is_executable {
        return Err(Error::InvalidEntry);
    }
    if let Some(mut dynamic) = image.dynamic {
        let dynamic_end = dynamic
            .dynamic_address
            .checked_add(dynamic.dynamic_size as u64)
            .ok_or(Error::InvalidSegment)?;
        let dynamic_contained = image.segments().any(|segment| {
            let segment_end = segment
                .virtual_address
                .saturating_add(segment.memory_size as u64);
            let file_end = segment
                .virtual_address
                .saturating_add(segment.file_size as u64);
            segment.virtual_address <= dynamic.dynamic_address
                && dynamic_end <= file_end
                && dynamic_end <= segment_end
        });
        if !dynamic_contained {
            return Err(Error::InvalidSegment);
        }
        if dynamic.rela_size != 0 {
            let rela_end = dynamic
                .rela_address
                .checked_add(dynamic.rela_size as u64)
                .ok_or(Error::InvalidSegment)?;
            let Some(segment) = image.segments().find(|segment| {
                let segment_file_end = segment
                    .virtual_address
                    .saturating_add(segment.file_size as u64);
                segment.virtual_address <= dynamic.rela_address
                    && rela_end <= segment_file_end
            }) else {
                return Err(Error::InvalidSegment);
            };
            dynamic.rela_file_offset = segment
                .file_offset
                .checked_add((dynamic.rela_address - segment.virtual_address) as usize)
                .ok_or(Error::InvalidSegment)?;
            let rela_end = dynamic
                .rela_file_offset
                .checked_add(dynamic.rela_size)
                .filter(|end| *end <= bytes.len())
                .ok_or(Error::Truncated)?;
            let (rela_entries, _) =
                bytes[dynamic.rela_file_offset..rela_end].as_chunks::<RELA_ENTRY_SIZE>();
            for entry in rela_entries {
                let target = u64::from_le_bytes(entry[..8].try_into().unwrap());
                let info = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                let relocation_type = info as u32;
                let symbol = info >> 32;
                if relocation_type != R_X86_64_RELATIVE || symbol != 0 {
                    return Err(Error::Unsupported);
                }
                let target_end = target.checked_add(8).ok_or(Error::InvalidSegment)?;
                let writable_target = image.segments().any(|segment| {
                    let end = segment
                        .virtual_address
                        .saturating_add(segment.memory_size as u64);
                    segment.writable
                        && !segment.executable
                        && segment.virtual_address <= target
                        && target_end <= end
                });
                if !writable_target {
                    return Err(Error::InvalidSegment);
                }
            }
        }
        image.dynamic = Some(dynamic);
    }
    if let Some((relro_start, relro_size)) = image.relro {
        let relro_end = relro_start
            .checked_add(relro_size as u64)
            .ok_or(Error::InvalidSegment)?;
        let covered_by_writable_data = image.segments().any(|segment| {
            let segment_end = segment.mapping_start + segment.mapping_size as u64;
            !segment.executable
                && segment.writable
                && segment.mapping_start <= relro_start
                && relro_end <= segment_end
        });
        if !covered_by_writable_data {
            return Err(Error::InvalidSegment);
        }
    }
    Ok(image)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let value = bytes.get(offset..offset + 2).ok_or(Error::Truncated)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let value = bytes.get(offset..offset + 4).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let value = bytes.get(offset..offset + 8).ok_or(Error::Truncated)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

const fn align_down(value: u64) -> u64 {
    value & !(PAGE_SIZE - 1)
}

fn align_up(value: u64) -> Option<u64> {
    value.checked_add(PAGE_SIZE - 1).map(align_down)
}
