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
const STT_TLS: u8 = 6;
const USER_ADDRESS_LIMIT: u64 = 0x0000_8000_0000_0000;
const PAGE_SIZE: u64 = 4096;
pub const R_X86_64_RELATIVE: u32 = 8;
pub const R_X86_64_64: u32 = 1;
pub const R_X86_64_GLOB_DAT: u32 = 6;
pub const R_X86_64_JUMP_SLOT: u32 = 7;
pub const R_X86_64_DTPMOD64: u32 = 16;
pub const R_X86_64_DTPOFF64: u32 = 17;
pub const R_X86_64_TPOFF64: u32 = 18;
pub const R_X86_64_GOTTPOFF: u32 = 22;
pub const R_X86_64_TPOFF32: u32 = 23;
pub const RELA_ENTRY_SIZE: usize = 24;
const REL_ENTRY_SIZE: usize = 16;
const MAX_RELATIVE_RELOCATIONS: usize = 4096;
const MAX_DYNAMIC_SYMBOLS: usize = 4096;
const MAX_DYNAMIC_STRING_TABLE: usize = 64 * 1024;
pub const MAX_LIFECYCLE_FUNCTIONS: usize = 4;
pub const MAX_NEEDED_LIBRARIES: usize = 4;

/// A bounded dynamic-linker scope. The main image is searched first, followed
/// by its direct `/lib` dependencies in declaration order. This is deliberately
/// a small, immutable scope: it cannot grow while relocations are being
/// applied, and it has no lazy-binding or versioned-symbol semantics.
pub const MAX_SCOPE_IMAGES: usize = MAX_NEEDED_LIBRARIES + 1;

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
    pub virtual_address: u64,
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
    pub rel_address: u64,
    pub rel_file_offset: usize,
    pub rel_size: usize,
    pub rel_entry_size: usize,
    pub plt_rela_address: u64,
    pub plt_rela_file_offset: usize,
    pub plt_rela_size: usize,
    pub plt_rel_address: u64,
    pub plt_rel_file_offset: usize,
    pub plt_rel_size: usize,
    pub plt_rel_entry_size: usize,
    pub strtab_address: u64,
    pub strtab_file_offset: usize,
    pub strtab_size: usize,
    pub symtab_address: u64,
    pub symtab_file_offset: usize,
    pub symtab_entry_size: usize,
    pub symbol_count: usize,
    pub hash_address: u64,
    pub hash_file_offset: usize,
    pub needed_offsets: [u64; MAX_NEEDED_LIBRARIES],
    pub needed_count: usize,
    pub soname_offset: Option<u64>,
    pub versym_address: u64,
    pub versym_file_offset: usize,
    pub verdef_address: u64,
    pub verdef_file_offset: usize,
    pub verdef_count: usize,
    pub verneed_address: u64,
    pub verneed_file_offset: usize,
    pub verneed_count: usize,
    pub init_address: u64,
    pub fini_address: u64,
    pub init_array_address: u64,
    pub init_array_count: usize,
    pub fini_array_address: u64,
    pub fini_array_count: usize,
}

#[derive(Clone, Copy)]
pub struct Lifecycle {
    pub init: [u64; MAX_LIFECYCLE_FUNCTIONS],
    pub init_count: usize,
    pub fini: [u64; MAX_LIFECYCLE_FUNCTIONS],
    pub fini_count: usize,
}

impl Lifecycle {
    const EMPTY: Self = Self {
        init: [0; MAX_LIFECYCLE_FUNCTIONS],
        init_count: 0,
        fini: [0; MAX_LIFECYCLE_FUNCTIONS],
        fini_count: 0,
    };
}

pub struct Image {
    pub entry: u64,
    segments: [Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    segment_count: usize,
    relro: Option<(u64, usize)>,
    tls: Option<TlsImage>,
    dynamic: Option<DynamicImage>,
    lifecycle: Lifecycle,
    load_bias: u64,
    // ET_EXEC retains absolute addresses; only ET_DYN accepts an ASLR bias.
    position_independent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Truncated,
    InvalidHeader,
    Unsupported,
    InvalidSegment,
    OverlappingSegments,
    InvalidEntry,
    TooManySegments,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScopeError {
    InvalidName,
    DuplicateDependency,
    Capacity,
    MissingDependency,
}

#[derive(Clone, Copy)]
struct ScopeImage<'a> {
    name: Option<&'a [u8]>,
    image: &'a Image,
    bytes: &'a [u8],
}

/// An immutable, bounded symbol scope for one executable and its direct
/// dependencies. The caller owns the image bytes for the scope's lifetime;
/// this prevents asynchronous use of borrowed file buffers.
pub struct SymbolScope<'a> {
    images: [Option<ScopeImage<'a>>; MAX_SCOPE_IMAGES],
    count: usize,
}

impl Image {
    pub const fn is_position_independent(&self) -> bool {
        self.position_independent
    }

    /// Choose executable placement without changing the library relocation API.
    pub fn relocated_for_execution(self, candidate_bias: u64) -> Option<Self> {
        let bias = if self.is_position_independent() {
            candidate_bias
        } else {
            0
        };
        self.relocated(bias)
    }

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

    pub const fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    pub const fn load_bias(&self) -> u64 {
        self.load_bias
    }

    /// Return one exact NUL-terminated dependency name from the validated
    /// dynamic string table.
    pub fn needed_name<'a>(&self, bytes: &'a [u8], index: usize) -> Option<&'a [u8]> {
        let dynamic = self.dynamic?;
        if index >= dynamic.needed_count {
            return None;
        }
        let offset = *dynamic.needed_offsets.get(index)?;
        dynamic_string(bytes, dynamic, offset)
    }

    pub fn soname<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        let dynamic = self.dynamic?;
        dynamic_string(bytes, dynamic, dynamic.soname_offset?)
    }

    /// Resolve supported eager relocations. Private/protected definitions bind
    /// locally; default-visible globals use executable-first interposition.
    pub fn relocation_symbol_value(
        &self,
        bytes: &[u8],
        index: usize,
        scope: &SymbolScope<'_>,
    ) -> Option<u64> {
        let dynamic = self.dynamic?;
        if index == 0 || index >= dynamic.symbol_count || dynamic.symtab_entry_size != 24 {
            return None;
        }
        let offset = dynamic
            .symtab_file_offset
            .checked_add(index.checked_mul(24)?)?;
        let symbol = bytes.get(offset..offset.checked_add(24)?)?;
        let binding = symbol[4] >> 4;
        let visibility = symbol[5] & 3;
        let section = u16::from_le_bytes(symbol[6..8].try_into().ok()?);
        if !matches!(binding, 0..=2) || !matches!(symbol[4] & 0xf, 0..=2) {
            return None;
        }
        let name = dynamic_string(
            bytes,
            dynamic,
            u64::from(u32::from_le_bytes(symbol[..4].try_into().ok()?)),
        )?;
        let (version, _) = self.symbol_version(bytes, index, section != 0)?;
        if section == 0 {
            if visibility != 0 || name.is_empty() || !matches!(binding, 1 | 2) {
                return None;
            }
            return if binding == 2 {
                Some(scope.resolve_versioned(name, version).unwrap_or(0))
            } else {
                scope.resolve_versioned(name, version)
            };
        }
        let local = self.defined_symbol_value(bytes, index)?;
        if matches!(binding, 1 | 2) && visibility == 0 {
            scope.resolve_versioned(name, version)
        } else {
            Some(local)
        }
    }

    /// Resolve a defined symbol for an image-local relocation. Visibility does
    /// not restrict references from the defining image. Unsupported TLS, IFUNC
    /// and reserved section encodings must never become ordinary addresses.
    pub fn defined_symbol_value(&self, bytes: &[u8], index: usize) -> Option<u64> {
        let dynamic = self.dynamic?;
        if index == 0 || index >= dynamic.symbol_count || dynamic.symtab_entry_size != 24 {
            return None;
        }
        let offset = dynamic
            .symtab_file_offset
            .checked_add(index.checked_mul(24)?)?;
        let symbol = bytes.get(offset..offset.checked_add(24)?)?;
        let section = u16::from_le_bytes(symbol[6..8].try_into().ok()?);
        if !matches!(symbol[4] & 0xf, 0..=2) || section == 0 || section >= 0xff00 {
            return None;
        }
        let name = u32::from_le_bytes(symbol[..4].try_into().ok()?);
        dynamic_string(bytes, dynamic, u64::from(name))?;
        let value =
            u64::from_le_bytes(symbol[8..16].try_into().ok()?).checked_add(self.load_bias)?;
        let size = u64::from_le_bytes(symbol[16..24].try_into().ok()?);
        self.contains_symbol(value, size).then_some(value)
    }

    /// Return a TLS symbol's offset within this image's PT_TLS block. TLS
    /// symbols are never exposed through the ordinary code/data resolver.
    pub fn defined_tls_symbol_value(&self, bytes: &[u8], index: usize) -> Option<u64> {
        let dynamic = self.dynamic?;
        let tls = self.tls?;
        if index == 0 || index >= dynamic.symbol_count || dynamic.symtab_entry_size != 24 {
            return None;
        }
        let offset = dynamic
            .symtab_file_offset
            .checked_add(index.checked_mul(24)?)?;
        let symbol = bytes.get(offset..offset.checked_add(24)?)?;
        if symbol[4] & 0xf != STT_TLS || u16::from_le_bytes(symbol[6..8].try_into().ok()?) == 0 {
            return None;
        }
        let value = u64::from_le_bytes(symbol[8..16].try_into().ok()?);
        let size = u64::from_le_bytes(symbol[16..24].try_into().ok()?);
        let end = value.checked_add(size)?;
        if value >= tls.virtual_address
            && end <= tls.virtual_address.checked_add(tls.memory_size as u64)?
        {
            return Some(value - tls.virtual_address);
        }
        (end <= tls.memory_size as u64).then_some(value)
    }

    pub fn tls_symbol_resolution(
        &self,
        bytes: &[u8],
        index: usize,
        scope: &SymbolScope<'_>,
    ) -> Option<(u64, Option<usize>)> {
        let dynamic = self.dynamic?;
        if index == 0 || index >= dynamic.symbol_count || dynamic.symtab_entry_size != 24 {
            return None;
        }
        let offset = dynamic
            .symtab_file_offset
            .checked_add(index.checked_mul(24)?)?;
        let symbol = bytes.get(offset..offset.checked_add(24)?)?;
        let section = u16::from_le_bytes(symbol[6..8].try_into().ok()?);
        if symbol[4] & 0xf == STT_TLS && section != 0 {
            return Some((self.defined_tls_symbol_value(bytes, index)?, None));
        }
        if section != 0 {
            return None;
        }
        let name_offset = u32::from_le_bytes(symbol[..4].try_into().ok()?);
        let name = dynamic_string(bytes, dynamic, u64::from(name_offset))?;
        let (version, _) = self.symbol_version(bytes, index, false)?;
        let (value, image_index) = scope.resolve_tls(name, version)?;
        Some((value, Some(image_index)))
    }

    fn symbol_version<'a>(
        &self,
        bytes: &'a [u8],
        index: usize,
        defined: bool,
    ) -> Option<(Option<&'a [u8]>, bool)> {
        let dynamic = self.dynamic?;
        if dynamic.versym_address == 0 {
            return Some((None, false));
        }
        let offset = dynamic
            .versym_file_offset
            .checked_add(index.checked_mul(2)?)?;
        let raw = u16::from_le_bytes(bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?);
        let version_index = raw & 0x7fff;
        if version_index <= 1 {
            return Some((None, raw & 0x8000 != 0));
        }
        let name = version_name_by_index(bytes, dynamic, self, version_index, defined)?;
        Some((Some(name), raw & 0x8000 != 0))
    }

    /// Resolve one defined global symbol within this image. Undefined symbols,
    /// local-only symbols and malformed names never escape the loader boundary.
    #[cfg(test)]
    pub fn defined_global_symbol(&self, bytes: &[u8], name: &[u8]) -> Option<u64> {
        self.defined_global_symbol_kind(bytes, name, None)
            .map(|(value, _)| value)
    }

    fn defined_global_symbol_kind(
        &self,
        bytes: &[u8],
        name: &[u8],
        requested_version: Option<&[u8]>,
    ) -> Option<(u64, bool)> {
        if name.is_empty() || name.contains(&0) {
            return None;
        }
        let dynamic = self.dynamic?;
        if dynamic.symbol_count == 0 || dynamic.symtab_entry_size != 24 {
            return None;
        }
        let mut weak = None;
        for index in 1..dynamic.symbol_count {
            let symbol_offset = dynamic
                .symtab_file_offset
                .checked_add(index.checked_mul(dynamic.symtab_entry_size)?)?;
            let symbol_end = symbol_offset.checked_add(24)?;
            let symbol = bytes.get(symbol_offset..symbol_end)?;
            let name_offset = u32::from_le_bytes(symbol[..4].try_into().ok()?) as u64;
            let binding = symbol[4] >> 4;
            let visibility = symbol[5] & 3;
            if !matches!(binding, 1 | 2) || !matches!(visibility, 0 | 3) {
                continue;
            }
            if dynamic_string(bytes, dynamic, name_offset)? != name {
                continue;
            }
            let (version, hidden) = self.symbol_version(bytes, index, true)?;
            if hidden || version != requested_version {
                continue;
            }
            if let Some(value) = self.defined_symbol_value(bytes, index) {
                if binding == 1 {
                    return Some((value, true));
                }
                weak = Some((value, false));
            }
        }
        weak
    }

    fn contains_symbol(&self, value: u64, size: u64) -> bool {
        self.segments().any(|segment| {
            value >= segment.virtual_address
                && value
                    < segment
                        .virtual_address
                        .saturating_add(segment.memory_size as u64)
                && value.checked_add(size).is_some_and(|end| {
                    end <= segment
                        .virtual_address
                        .saturating_add(segment.memory_size as u64)
                })
        })
    }

    fn contains_executable(&self, value: u64, size: u64) -> bool {
        self.segments().any(|segment| {
            segment.executable
                && value >= segment.virtual_address
                && value < segment.virtual_address + segment.memory_size as u64
                && value.checked_add(size).is_some_and(|end| {
                    end <= segment.virtual_address + segment.memory_size as u64
                })
        })
    }

    /// Relocate every load segment and the entry by one page-aligned delta.
    /// The parser's W^X, overlap and file-range checks remain in force, while
    /// this second pass rechecks user bounds after the randomized placement.
    pub fn relocated(mut self, delta: u64) -> Option<Self> {
        self.entry = self.entry.checked_add(delta)?;
        self.load_bias = delta;
        if let Some(tls) = self.tls.as_mut() {
            tls.virtual_address = tls.virtual_address.checked_add(delta)?;
            if tls.virtual_address.checked_add(tls.memory_size as u64)? >= USER_ADDRESS_LIMIT {
                return None;
            }
        }
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
        if let Some(dynamic) = self.dynamic.as_mut() {
            for address in [
                &mut dynamic.dynamic_address,
                &mut dynamic.rela_address,
                &mut dynamic.rel_address,
                &mut dynamic.plt_rela_address,
                &mut dynamic.plt_rel_address,
                &mut dynamic.strtab_address,
                &mut dynamic.symtab_address,
                &mut dynamic.hash_address,
                &mut dynamic.versym_address,
                &mut dynamic.verdef_address,
                &mut dynamic.verneed_address,
                &mut dynamic.init_address,
                &mut dynamic.fini_address,
                &mut dynamic.init_array_address,
                &mut dynamic.fini_array_address,
            ] {
                if *address != 0 {
                    *address = (*address).checked_add(delta)?;
                }
            }
        }
        for entry in self.lifecycle.init[..self.lifecycle.init_count].iter_mut() {
            *entry = entry.checked_add(delta)?;
        }
        for entry in self.lifecycle.fini[..self.lifecycle.fini_count].iter_mut() {
            *entry = entry.checked_add(delta)?;
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

impl<'a> SymbolScope<'a> {
    /// Start a scope with the main executable. Its name is intentionally absent
    /// because it is not a dependency selected by a DT_NEEDED string.
    pub fn new(main: &'a Image, bytes: &'a [u8]) -> Self {
        let mut images = [None; MAX_SCOPE_IMAGES];
        images[0] = Some(ScopeImage {
            name: None,
            image: main,
            bytes,
        });
        Self { images, count: 1 }
    }

    /// Add one direct dependency under its exact soname/path bytes.
    pub fn add_dependency(
        &mut self,
        name: &'a [u8],
        image: &'a Image,
        bytes: &'a [u8],
    ) -> Result<(), ScopeError> {
        if name.is_empty() || name.contains(&0) {
            return Err(ScopeError::InvalidName);
        }
        if self.count == MAX_SCOPE_IMAGES {
            return Err(ScopeError::Capacity);
        }
        if self.images[..self.count]
            .iter()
            .flatten()
            .any(|entry| entry.name.is_some_and(|existing| existing == name))
        {
            return Err(ScopeError::DuplicateDependency);
        }
        self.images[self.count] = Some(ScopeImage {
            name: Some(name),
            image,
            bytes,
        });
        self.count += 1;
        Ok(())
    }

    /// Require an exact one-to-one match between DT_NEEDED and the namespace.
    pub fn validate_dependencies(&self) -> Result<(), ScopeError> {
        let mut discovered = 1;
        for image_index in 0..self.count {
            if image_index >= discovered {
                return Err(ScopeError::MissingDependency);
            }
            let entry = self.images[image_index].ok_or(ScopeError::MissingDependency)?;
            let count = entry
                .image
                .dynamic()
                .map_or(0, |dynamic| dynamic.needed_count);
            for index in 0..count {
                let needed = entry
                    .image
                    .needed_name(entry.bytes, index)
                    .filter(|name| !name.is_empty())
                    .ok_or(ScopeError::InvalidName)?;
                if (0..index)
                    .any(|prior| entry.image.needed_name(entry.bytes, prior) == Some(needed))
                {
                    return Err(ScopeError::DuplicateDependency);
                }
                if self.images[1..discovered]
                    .iter()
                    .flatten()
                    .any(|prior| prior.name == Some(needed))
                {
                    continue;
                }
                if discovered >= self.count
                    || self.images[discovered].and_then(|next| next.name) != Some(needed)
                {
                    return Err(ScopeError::MissingDependency);
                }
                discovered += 1;
            }
        }
        if discovered == self.count {
            Ok(())
        } else {
            Err(ScopeError::MissingDependency)
        }
    }

    /// Resolve an exact exported global symbol in main-image-first scope.
    pub fn resolve(&self, name: &[u8]) -> Option<u64> {
        self.resolve_versioned(name, None)
    }

    fn resolve_versioned(&self, name: &[u8], version: Option<&[u8]>) -> Option<u64> {
        self.validate_dependencies().ok()?;
        self.images[..self.count]
            .iter()
            .flatten()
            .filter_map(|entry| {
                entry
                    .image
                    .defined_global_symbol_kind(entry.bytes, name, version)
            })
            .find_map(|(value, strong)| strong.then_some(value))
            .or_else(|| {
                self.images[..self.count]
                    .iter()
                    .flatten()
                    .filter_map(|entry| {
                        entry
                            .image
                            .defined_global_symbol_kind(entry.bytes, name, version)
                    })
                    .find_map(|(value, strong)| (!strong).then_some(value))
            })
    }

    fn resolve_tls(&self, name: &[u8], version: Option<&[u8]>) -> Option<(u64, usize)> {
        self.validate_dependencies().ok()?;
        for (image_index, entry) in self.images[..self.count].iter().flatten().enumerate() {
            let dynamic = entry.image.dynamic()?;
            for index in 1..dynamic.symbol_count {
                let offset = dynamic
                    .symtab_file_offset
                    .checked_add(index.checked_mul(dynamic.symtab_entry_size)?)?;
                let symbol = entry.bytes.get(offset..offset.checked_add(24)?)?;
                if symbol[4] >> 4 != 1 && symbol[4] >> 4 != 2 {
                    continue;
                }
                let name_offset = u32::from_le_bytes(symbol[..4].try_into().ok()?);
                if dynamic_string(entry.bytes, dynamic, u64::from(name_offset))? != name {
                    continue;
                }
                let (found_version, hidden) = entry.image.symbol_version(
                    entry.bytes,
                    index,
                    true,
                )?;
                if hidden || found_version != version {
                    continue;
                }
                if let Some(value) = entry.image.defined_tls_symbol_value(entry.bytes, index) {
                    return Some((value, image_index));
                }
            }
        }
        None
    }
}

fn dynamic_string(bytes: &[u8], dynamic: DynamicImage, offset: u64) -> Option<&[u8]> {
    let start = dynamic
        .strtab_file_offset
        .checked_add(usize::try_from(offset).ok()?)?;
    let end = dynamic
        .strtab_file_offset
        .checked_add(dynamic.strtab_size)?;
    let string = bytes.get(start..end)?;
    let length = string.iter().position(|byte| *byte == 0)?;
    Some(&string[..length])
}

fn version_file_offset(
    image: &Image,
    bytes: &[u8],
    address: u64,
    size: usize,
) -> Result<usize, Error> {
    let end = address
        .checked_add(size as u64)
        .ok_or(Error::InvalidSegment)?;
    let segment = image
        .segments()
        .find(|segment| {
            let file_end = segment
                .virtual_address
                .saturating_add(segment.file_size as u64);
            segment.virtual_address <= address && end <= file_end
        })
        .ok_or(Error::InvalidSegment)?;
    let offset = segment
        .file_offset
        .checked_add((address - segment.virtual_address) as usize)
        .ok_or(Error::InvalidSegment)?;
    if offset.checked_add(size).filter(|end| *end <= bytes.len()).is_none() {
        return Err(Error::Truncated);
    }
    Ok(offset)
}

fn validate_version_tables(bytes: &[u8], image: &mut Image) -> Result<(), Error> {
    let Some(mut dynamic) = image.dynamic else {
        return Ok(());
    };
    let has_versions = dynamic.versym_address != 0
        || dynamic.verdef_address != 0
        || dynamic.verdef_count != 0
        || dynamic.verneed_address != 0
        || dynamic.verneed_count != 0;
    if !has_versions {
        return Ok(());
    }
    if dynamic.symbol_count == 0 || dynamic.versym_address == 0 {
        return Err(Error::InvalidSegment);
    }
    let versym_size = dynamic
        .symbol_count
        .checked_mul(2)
        .ok_or(Error::InvalidSegment)?;
    dynamic.versym_file_offset =
        version_file_offset(image, bytes, dynamic.versym_address, versym_size)?;

    if dynamic.verdef_count == 0 {
        if dynamic.verdef_address != 0 {
            return Err(Error::InvalidSegment);
        }
    } else {
        if dynamic.verdef_address == 0 || dynamic.verdef_count > MAX_DYNAMIC_SYMBOLS {
            return Err(Error::InvalidSegment);
        }
        dynamic.verdef_file_offset =
            version_file_offset(image, bytes, dynamic.verdef_address, 20)?;
        let mut cursor = dynamic.verdef_address;
        for index in 0..dynamic.verdef_count {
            let offset = version_file_offset(image, bytes, cursor, 20)?;
            let version = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
            let version_index =
                u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap());
            let aux = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
            let next = u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap());
            if version != 1 || version_index < 2 || aux == 0 {
                return Err(Error::InvalidSegment);
            }
            let aux_address = cursor
                .checked_add(aux as u64)
                .ok_or(Error::InvalidSegment)?;
            let aux_offset = version_file_offset(image, bytes, aux_address, 8)?;
            let name = u32::from_le_bytes(bytes[aux_offset..aux_offset + 4].try_into().unwrap());
            if dynamic_string(bytes, dynamic, name as u64)
                .filter(|name| !name.is_empty())
                .is_none()
            {
                return Err(Error::InvalidSegment);
            }
            if next == 0 {
                if index + 1 != dynamic.verdef_count {
                    return Err(Error::InvalidSegment);
                }
            } else {
                cursor = cursor
                    .checked_add(next as u64)
                    .ok_or(Error::InvalidSegment)?;
            }
        }
    }

    if dynamic.verneed_count == 0 {
        if dynamic.verneed_address != 0 {
            return Err(Error::InvalidSegment);
        }
    } else {
        if dynamic.verneed_address == 0 || dynamic.verneed_count > MAX_DYNAMIC_SYMBOLS {
            return Err(Error::InvalidSegment);
        }
        dynamic.verneed_file_offset =
            version_file_offset(image, bytes, dynamic.verneed_address, 16)?;
        let mut cursor = dynamic.verneed_address;
        for index in 0..dynamic.verneed_count {
            let offset = version_file_offset(image, bytes, cursor, 16)?;
            let version = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
            let count = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap());
            let file = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            let aux = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            let next = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
            if version != 1 || count == 0 || aux == 0 {
                return Err(Error::InvalidSegment);
            }
            if dynamic_string(bytes, dynamic, file as u64)
                .filter(|name| !name.is_empty())
                .is_none()
            {
                return Err(Error::InvalidSegment);
            }
            let mut aux_cursor = cursor
                .checked_add(aux as u64)
                .ok_or(Error::InvalidSegment)?;
            for aux_index in 0..count {
                let aux_offset = version_file_offset(image, bytes, aux_cursor, 16)?;
                let version_index =
                    u16::from_le_bytes(bytes[aux_offset + 6..aux_offset + 8].try_into().unwrap());
                let name = u32::from_le_bytes(bytes[aux_offset + 8..aux_offset + 12].try_into().unwrap());
                if version_index < 2
                    || dynamic_string(bytes, dynamic, name as u64)
                        .filter(|name| !name.is_empty())
                        .is_none()
                {
                    return Err(Error::InvalidSegment);
                }
                let aux_next =
                    u32::from_le_bytes(bytes[aux_offset + 12..aux_offset + 16].try_into().unwrap());
                if aux_next == 0 && aux_index + 1 != count {
                    return Err(Error::InvalidSegment);
                }
                if aux_next != 0 {
                    aux_cursor = aux_cursor
                        .checked_add(aux_next as u64)
                        .ok_or(Error::InvalidSegment)?;
                }
            }
            if next == 0 {
                if index + 1 != dynamic.verneed_count {
                    return Err(Error::InvalidSegment);
                }
            } else {
                cursor = cursor
                    .checked_add(next as u64)
                    .ok_or(Error::InvalidSegment)?;
            }
        }
    }
    for index in 0..dynamic.symbol_count {
        let offset = dynamic
            .versym_file_offset
            .checked_add(index.checked_mul(2).ok_or(Error::InvalidSegment)?)
            .ok_or(Error::InvalidSegment)?;
        let version =
            u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) & 0x7fff;
        if version > 1 && !version_index_exists(bytes, dynamic, image, version)? {
            return Err(Error::InvalidSegment);
        }
    }
    image.dynamic = Some(dynamic);
    Ok(())
}

fn version_index_exists(
    bytes: &[u8],
    dynamic: DynamicImage,
    image: &Image,
    wanted: u16,
) -> Result<bool, Error> {
    if dynamic.verdef_count != 0 {
        let mut cursor = dynamic.verdef_address;
        for index in 0..dynamic.verdef_count {
            let offset = version_file_offset(image, bytes, cursor, 20)?;
            let value = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap());
            if value == wanted {
                return Ok(true);
            }
            let next = u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap());
            if next == 0 {
                if index + 1 != dynamic.verdef_count {
                    return Err(Error::InvalidSegment);
                }
            } else {
                cursor = cursor
                    .checked_add(next as u64)
                    .ok_or(Error::InvalidSegment)?;
            }
        }
    }
    if dynamic.verneed_count != 0 {
        let mut cursor = dynamic.verneed_address;
        for index in 0..dynamic.verneed_count {
            let offset = version_file_offset(image, bytes, cursor, 16)?;
            let count = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap());
            let aux = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            let mut aux_cursor = cursor
                .checked_add(aux as u64)
                .ok_or(Error::InvalidSegment)?;
            for _ in 0..count {
                let aux_offset = version_file_offset(image, bytes, aux_cursor, 16)?;
                let value =
                    u16::from_le_bytes(bytes[aux_offset + 6..aux_offset + 8].try_into().unwrap());
                if value == wanted {
                    return Ok(true);
                }
                let next = u32::from_le_bytes(bytes[aux_offset + 12..aux_offset + 16].try_into().unwrap());
                if next != 0 {
                    aux_cursor = aux_cursor
                        .checked_add(next as u64)
                        .ok_or(Error::InvalidSegment)?;
                }
            }
            let next = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
            if next == 0 {
                if index + 1 != dynamic.verneed_count {
                    return Err(Error::InvalidSegment);
                }
            } else {
                cursor = cursor
                    .checked_add(next as u64)
                    .ok_or(Error::InvalidSegment)?;
            }
        }
    }
    Ok(false)
}

fn version_name_by_index<'a>(
    bytes: &'a [u8],
    dynamic: DynamicImage,
    image: &Image,
    wanted: u16,
    defined: bool,
) -> Option<&'a [u8]> {
    if defined {
        let mut cursor = dynamic.verdef_address;
        for _ in 0..dynamic.verdef_count {
            let offset = version_file_offset(image, bytes, cursor, 20).ok()?;
            let index = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().ok()?);
            if index == wanted {
                let aux = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().ok()?);
                let aux_offset = version_file_offset(
                    image,
                    bytes,
                    cursor.checked_add(aux as u64)?,
                    8,
                )
                .ok()?;
                let name = u32::from_le_bytes(bytes[aux_offset..aux_offset + 4].try_into().ok()?);
                return dynamic_string(bytes, dynamic, name as u64);
            }
            let next = u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().ok()?);
            cursor = cursor.checked_add(next as u64)?;
        }
    } else {
        let mut cursor = dynamic.verneed_address;
        for _ in 0..dynamic.verneed_count {
            let offset = version_file_offset(image, bytes, cursor, 16).ok()?;
            let count = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().ok()?);
            let aux = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().ok()?);
            let mut aux_cursor = cursor.checked_add(aux as u64)?;
            for _ in 0..count {
                let aux_offset = version_file_offset(image, bytes, aux_cursor, 16).ok()?;
                let index =
                    u16::from_le_bytes(bytes[aux_offset + 6..aux_offset + 8].try_into().ok()?);
                if index == wanted {
                    let name =
                        u32::from_le_bytes(bytes[aux_offset + 8..aux_offset + 12].try_into().ok()?);
                    return dynamic_string(bytes, dynamic, name as u64);
                }
                let next =
                    u32::from_le_bytes(bytes[aux_offset + 12..aux_offset + 16].try_into().ok()?);
                aux_cursor = aux_cursor.checked_add(next as u64)?;
            }
            let next = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().ok()?);
            cursor = cursor.checked_add(next as u64)?;
        }
    }
    None
}

pub fn parse(bytes: &[u8]) -> Result<Image, Error> {
    parse_image(bytes, false)
}

/// Shared objects may have no entry point, but must be ET_DYN. All segment,
/// metadata, relocation-target and W^X checks are shared with executables.
pub fn parse_shared(bytes: &[u8]) -> Result<Image, Error> {
    parse_image(bytes, true)
}

fn parse_image(bytes: &[u8], shared: bool) -> Result<Image, Error> {
    if bytes.len() < ELF_HEADER_SIZE {
        return Err(Error::Truncated);
    }
    let elf_type = read_u16(bytes, 16)?;
    if &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(elf_type, 2 | 3)
        || (shared && elf_type != 3)
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
        lifecycle: Lifecycle::EMPTY,
        load_bias: 0,
        position_independent: elf_type == 3,
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
            let file_offset =
                usize::try_from(read_u64(bytes, offset + 8)?).map_err(|_| Error::InvalidSegment)?;
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
            let mut rel_address = None;
            let mut rel_size = 0usize;
            let mut rel_entry_size = 0usize;
            let mut plt_rela_size = 0usize;
            let mut plt_rela_address = None;
            let mut plt_rel_size = 0usize;
            let mut plt_address = None;
            let mut plt_kind = None;
            let mut strtab_address = 0;
            let mut strtab_size = 0usize;
            let mut symtab_address = 0;
            let mut symtab_entry_size = 0usize;
            let mut hash_address = 0;
            let mut needed_offsets = [0; MAX_NEEDED_LIBRARIES];
            let mut needed_count = 0;
            let mut soname_offset = None;
            let mut versym_address = None;
            let mut verdef_address = None;
            let mut verdef_count = 0usize;
            let mut verneed_address = None;
            let mut verneed_count = 0usize;
            let mut init_address = None;
            let mut fini_address = None;
            let mut init_array_address = None;
            let mut init_array_count = 0usize;
            let mut fini_array_address = None;
            let mut fini_array_count = 0usize;
            let dynamic_bytes = &bytes[file_offset..file_end];
            let mut terminated = false;
            let mut debug_seen = false;
            let (dynamic_entries, _) = dynamic_bytes.as_chunks::<16>();
            for entry in dynamic_entries {
                let tag = i64::from_le_bytes(entry[..8].try_into().unwrap());
                let value = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                match tag {
                    0 => {
                        terminated = true;
                        break;
                    }
                    1 => {
                        if needed_count == MAX_NEEDED_LIBRARIES {
                            return Err(Error::Unsupported);
                        }
                        needed_offsets[needed_count] = value;
                        needed_count += 1;
                    }
                    14 => {
                        if soname_offset.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    21 => {
                        // DT_DEBUG is debugger rendezvous metadata, not an
                        // import or relocation. Accept only the inert null
                        // placeholder emitted by LLD; never follow a supplied
                        // address or advertise an unimplemented debugger ABI.
                        if value != 0 {
                            return Err(Error::Unsupported);
                        }
                        if debug_seen {
                            return Err(Error::InvalidSegment);
                        }
                        debug_seen = true;
                    }
                    15 | 16 | 22 | 29 => {
                        // Dependencies, symbol-based relocation tables, and
                        // text-relocation requests need a dynamic linker or
                        // shared-library authority that the kernel does not
                        // provide.
                        if value != 0
                            || matches!(tag, 1 | 14 | 15 | 16 | 17 | 18 | 19 | 22 | 23 | 29)
                        {
                            return Err(Error::Unsupported);
                        }
                    }
                    4 => hash_address = value,
                    5 => strtab_address = value,
                    6 => symtab_address = value,
                    7 => rela_address = Some(value),
                    8 => rela_size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?,
                    9 => {
                        rela_entry_size =
                            usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                    }
                    17 => {
                        if rel_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    18 => rel_size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?,
                    19 => {
                        rel_entry_size =
                            usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                    }
                    10 => {
                        strtab_size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?
                    }
                    11 => {
                        symtab_entry_size =
                            usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                    }
                    2 => {
                        let size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                        if plt_rela_size != 0 || plt_rel_size != 0 {
                            return Err(Error::InvalidSegment);
                        }
                        // DT_PLTREL is allowed to precede DT_PLTRELSZ.
                        match plt_kind {
                            Some(7) => plt_rela_size = size,
                            Some(17) => plt_rel_size = size,
                            Some(_) => return Err(Error::Unsupported),
                            None => {
                                plt_rela_size = size;
                            }
                        }
                    }
                    20 => {
                        if plt_kind.replace(value).is_some() || !matches!(value, 7 | 17) {
                            return Err(Error::Unsupported);
                        }
                        // DT_PLTRELSZ may precede DT_PLTREL.
                        if value == 17 && plt_rela_size != 0 {
                            plt_rel_size = plt_rela_size;
                            plt_rela_size = 0;
                        } else if value == 7 && plt_rel_size != 0 {
                            plt_rela_size = plt_rel_size;
                            plt_rel_size = 0;
                        }
                    }
                    23 => {
                        if plt_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    12 => {
                        if init_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    13 => {
                        if fini_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    25 => {
                        if init_array_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    26 => {
                        if fini_array_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    27 => {
                        let size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                        if !size.is_multiple_of(8) {
                            return Err(Error::InvalidSegment);
                        }
                        init_array_count = size / 8;
                    }
                    28 => {
                        let size = usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                        if !size.is_multiple_of(8) {
                            return Err(Error::InvalidSegment);
                        }
                        fini_array_count = size / 8;
                    }
                    // Common metadata tags do not affect relocation of a
                    // symbol-free static PIE and are intentionally ignored.
                    0x6fff_fff0 => {
                        if versym_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    0x6fff_fffc => {
                        if verdef_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    0x6fff_fffd => {
                        verdef_count = usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                    }
                    0x6fff_fffe => {
                        if verneed_address.replace(value).is_some() {
                            return Err(Error::InvalidSegment);
                        }
                    }
                    0x6fff_ffff => {
                        verneed_count = usize::try_from(value).map_err(|_| Error::InvalidSegment)?;
                    }
                    3 | 30 | 0x6fff_fff9 | 0x6fff_fffb => {}
                    _ => return Err(Error::Unsupported),
                }
            }
            if !terminated {
                return Err(Error::Unsupported);
            }
            if plt_rela_size == 0 && plt_rel_size == 0 {
                if plt_address.is_some() || plt_kind.is_some() {
                    return Err(Error::InvalidSegment);
                }
            } else {
                let plt_address = plt_address.ok_or(Error::InvalidSegment)?;
                let plt_kind = plt_kind.ok_or(Error::InvalidSegment)?;
                let entry_size = if plt_kind == 7 {
                    RELA_ENTRY_SIZE
                } else {
                    REL_ENTRY_SIZE
                };
                let size = if plt_kind == 7 {
                    plt_rela_size
                } else {
                    plt_rel_size
                };
                if size == 0
                    || !size.is_multiple_of(entry_size)
                    || size / entry_size > MAX_RELATIVE_RELOCATIONS
                {
                    return Err(Error::InvalidSegment);
                }
                if plt_kind == 7 {
                    plt_rela_address = Some(plt_address);
                } else {
                    // Keep the common DT_JMPREL address in the REL field.
                    // The final image validation resolves it file-backed.
                    plt_rela_address = None;
                }
            }
            if rela_size == 0 {
                if rela_address.is_some()
                    || (rela_entry_size != 0 && plt_rela_size == 0 && plt_rel_size == 0)
                {
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
                    rel_address: rel_address.unwrap_or(0),
                    rel_file_offset: 0,
                    rel_size,
                    rel_entry_size,
                    plt_rela_address: plt_rela_address.unwrap_or(0),
                    plt_rela_file_offset: 0,
                    plt_rela_size,
                    plt_rel_address: if plt_kind == Some(17) {
                        plt_address.unwrap_or(0)
                    } else {
                        0
                    },
                    plt_rel_file_offset: 0,
                    plt_rel_size,
                    plt_rel_entry_size: if plt_rel_size != 0 { REL_ENTRY_SIZE } else { 0 },
                    strtab_address,
                    strtab_file_offset: 0,
                    strtab_size,
                    symtab_address,
                    symtab_file_offset: 0,
                    symtab_entry_size,
                    symbol_count: 0,
                    hash_address,
                    hash_file_offset: 0,
                    needed_offsets,
                    needed_count,
                    soname_offset,
                    versym_address: versym_address.unwrap_or(0),
                    versym_file_offset: 0,
                    verdef_address: verdef_address.unwrap_or(0),
                    verdef_file_offset: 0,
                    verdef_count,
                    verneed_address: verneed_address.unwrap_or(0),
                    verneed_file_offset: 0,
                    verneed_count,
                    init_address: init_address.unwrap_or(0),
                    fini_address: fini_address.unwrap_or(0),
                    init_array_address: init_array_address.unwrap_or(0),
                    init_array_count,
                    fini_array_address: fini_array_address.unwrap_or(0),
                    fini_array_count,
                });
                // Resolve the virtual relocation table to its file-backed
                // bytes after all PT_LOAD headers have been collected.
                let _ = (rela_address, virtual_address);
            }
            if rel_size == 0 {
                if rel_address.is_some() || rel_entry_size != 0 {
                    return Err(Error::InvalidSegment);
                }
            } else if rel_address.is_none()
                || rel_entry_size != REL_ENTRY_SIZE
                || !rel_size.is_multiple_of(rel_entry_size)
                || rel_size / rel_entry_size > MAX_RELATIVE_RELOCATIONS
            {
                return Err(Error::InvalidSegment);
            }
            if image.dynamic.is_none() {
                image.dynamic = Some(DynamicImage {
                    dynamic_address: virtual_address,
                    dynamic_size: file_size,
                    rela_address: 0,
                    rela_file_offset: 0,
                    rela_size: 0,
                    rela_entry_size: 0,
                    rel_address: rel_address.unwrap_or(0),
                    rel_file_offset: 0,
                    rel_size,
                    rel_entry_size,
                    plt_rela_address: plt_rela_address.unwrap_or(0),
                    plt_rela_file_offset: 0,
                    plt_rela_size,
                    plt_rel_address: if plt_kind == Some(17) {
                        plt_address.unwrap_or(0)
                    } else {
                        0
                    },
                    plt_rel_file_offset: 0,
                    plt_rel_size,
                    plt_rel_entry_size: if plt_rel_size != 0 { REL_ENTRY_SIZE } else { 0 },
                    strtab_address,
                    strtab_file_offset: 0,
                    strtab_size,
                    symtab_address,
                    symtab_file_offset: 0,
                    symtab_entry_size,
                    symbol_count: 0,
                    hash_address,
                    hash_file_offset: 0,
                    needed_offsets,
                    needed_count,
                    soname_offset,
                    versym_address: versym_address.unwrap_or(0),
                    versym_file_offset: 0,
                    verdef_address: verdef_address.unwrap_or(0),
                    verdef_file_offset: 0,
                    verdef_count,
                    verneed_address: verneed_address.unwrap_or(0),
                    verneed_file_offset: 0,
                    verneed_count,
                    init_address: init_address.unwrap_or(0),
                    fini_address: fini_address.unwrap_or(0),
                    init_array_address: init_array_address.unwrap_or(0),
                    init_array_count,
                    fini_array_address: fini_array_address.unwrap_or(0),
                    fini_array_count,
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
            let file_size = usize::try_from(read_u64(bytes, offset + 32)?)
                .map_err(|_| Error::InvalidSegment)?;
            let memory_size = usize::try_from(read_u64(bytes, offset + 40)?)
                .map_err(|_| Error::InvalidSegment)?;
            let alignment = read_u64(bytes, offset + 48)?;
            let virtual_address = read_u64(bytes, offset + 16)?;
            if memory_size == 0
                || file_size > memory_size
                || !alignment.is_power_of_two()
                || alignment > PAGE_SIZE
                || virtual_address % alignment != file_offset as u64 % alignment
                || virtual_address
                    .checked_add(memory_size as u64)
                    .is_none_or(|end| end >= USER_ADDRESS_LIMIT)
                || file_offset
                    .checked_add(file_size)
                    .filter(|end| *end <= bytes.len())
                    .is_none()
            {
                return Err(Error::InvalidSegment);
            }
            image.tls = Some(TlsImage {
                file_offset,
                virtual_address,
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
            let memory_size = usize::try_from(read_u64(bytes, offset + 40)?)
                .map_err(|_| Error::InvalidSegment)?;
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

    if image.segment_count == 0 || (!entry_is_executable && !(shared && entry == 0)) {
        return Err(Error::InvalidEntry);
    }
    if let Some(tls) = image.tls.filter(|tls| tls.file_size != 0) {
        let contained = image.segments().any(|segment| {
            tls.file_offset
                .checked_sub(segment.file_offset)
                .is_some_and(|offset| {
                    offset
                        .checked_add(tls.file_size)
                        .is_some_and(|end| end <= segment.file_size)
                        && segment.virtual_address.checked_add(offset as u64)
                            == Some(tls.virtual_address)
                })
        });
        if !contained {
            return Err(Error::InvalidSegment);
        }
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
        if (dynamic.needed_count != 0 || dynamic.soname_offset.is_some())
            && (dynamic.strtab_address == 0 || dynamic.symtab_address == 0)
        {
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
                segment.virtual_address <= dynamic.rela_address && rela_end <= segment_file_end
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
                if !matches!(
                    relocation_type,
                    R_X86_64_RELATIVE
                        | R_X86_64_64
                        | R_X86_64_GLOB_DAT
                        | R_X86_64_JUMP_SLOT
                        | R_X86_64_DTPMOD64
                        | R_X86_64_DTPOFF64
                        | R_X86_64_TPOFF64
                        | R_X86_64_GOTTPOFF
                        | R_X86_64_TPOFF32
                ) {
                    return Err(Error::Unsupported);
                }
                if relocation_type == R_X86_64_RELATIVE && symbol != 0 {
                    return Err(Error::InvalidSegment);
                }
                if relocation_type != R_X86_64_RELATIVE && symbol == 0 {
                    return Err(Error::InvalidSegment);
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
        if dynamic.plt_rela_size != 0 {
            let rela_end = dynamic
                .plt_rela_address
                .checked_add(dynamic.plt_rela_size as u64)
                .ok_or(Error::InvalidSegment)?;
            let Some(segment) = image.segments().find(|segment| {
                let segment_file_end = segment
                    .virtual_address
                    .saturating_add(segment.file_size as u64);
                segment.virtual_address <= dynamic.plt_rela_address && rela_end <= segment_file_end
            }) else {
                return Err(Error::InvalidSegment);
            };
            dynamic.plt_rela_file_offset = segment
                .file_offset
                .checked_add((dynamic.plt_rela_address - segment.virtual_address) as usize)
                .ok_or(Error::InvalidSegment)?;
            let rela_end = dynamic
                .plt_rela_file_offset
                .checked_add(dynamic.plt_rela_size)
                .filter(|end| *end <= bytes.len())
                .ok_or(Error::Truncated)?;
            let (rela_entries, _) =
                bytes[dynamic.plt_rela_file_offset..rela_end].as_chunks::<RELA_ENTRY_SIZE>();
            for entry in rela_entries {
                let target = u64::from_le_bytes(entry[..8].try_into().unwrap());
                let info = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                let relocation_type = info as u32;
                let symbol = info >> 32;
                if !matches!(
                    relocation_type,
                    R_X86_64_RELATIVE
                        | R_X86_64_64
                        | R_X86_64_GLOB_DAT
                        | R_X86_64_JUMP_SLOT
                        | R_X86_64_DTPMOD64
                        | R_X86_64_DTPOFF64
                        | R_X86_64_TPOFF64
                        | R_X86_64_GOTTPOFF
                        | R_X86_64_TPOFF32
                ) {
                    return Err(Error::Unsupported);
                }
                if relocation_type == R_X86_64_RELATIVE && symbol != 0 {
                    return Err(Error::InvalidSegment);
                }
                if relocation_type != R_X86_64_RELATIVE && symbol == 0 {
                    return Err(Error::InvalidSegment);
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
        if dynamic.plt_rel_size != 0 {
            let rel_end = dynamic
                .plt_rel_address
                .checked_add(dynamic.plt_rel_size as u64)
                .ok_or(Error::InvalidSegment)?;
            let Some(segment) = image.segments().find(|segment| {
                let segment_file_end = segment
                    .virtual_address
                    .saturating_add(segment.file_size as u64);
                segment.virtual_address <= dynamic.plt_rel_address && rel_end <= segment_file_end
            }) else {
                return Err(Error::InvalidSegment);
            };
            dynamic.plt_rel_file_offset = segment
                .file_offset
                .checked_add((dynamic.plt_rel_address - segment.virtual_address) as usize)
                .ok_or(Error::InvalidSegment)?;
            let rel_end = dynamic
                .plt_rel_file_offset
                .checked_add(dynamic.plt_rel_size)
                .filter(|end| *end <= bytes.len())
                .ok_or(Error::Truncated)?;
            let (rel_entries, _) =
                bytes[dynamic.plt_rel_file_offset..rel_end].as_chunks::<REL_ENTRY_SIZE>();
            for entry in rel_entries {
                let target = u64::from_le_bytes(entry[..8].try_into().unwrap());
                let info = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                let relocation_type = info as u32;
                let symbol = info >> 32;
                if !matches!(
                    relocation_type,
                    R_X86_64_RELATIVE
                        | R_X86_64_64
                        | R_X86_64_GLOB_DAT
                        | R_X86_64_JUMP_SLOT
                        | R_X86_64_DTPMOD64
                        | R_X86_64_DTPOFF64
                        | R_X86_64_TPOFF64
                        | R_X86_64_GOTTPOFF
                        | R_X86_64_TPOFF32
                ) {
                    return Err(Error::Unsupported);
                }
                if relocation_type == R_X86_64_RELATIVE && symbol != 0 {
                    return Err(Error::InvalidSegment);
                }
                if relocation_type != R_X86_64_RELATIVE && symbol == 0 {
                    return Err(Error::InvalidSegment);
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
        if dynamic.rel_size != 0 {
            let rel_end = dynamic
                .rel_address
                .checked_add(dynamic.rel_size as u64)
                .ok_or(Error::InvalidSegment)?;
            let Some(segment) = image.segments().find(|segment| {
                let segment_file_end = segment
                    .virtual_address
                    .saturating_add(segment.file_size as u64);
                segment.virtual_address <= dynamic.rel_address && rel_end <= segment_file_end
            }) else {
                return Err(Error::InvalidSegment);
            };
            dynamic.rel_file_offset = segment
                .file_offset
                .checked_add((dynamic.rel_address - segment.virtual_address) as usize)
                .ok_or(Error::InvalidSegment)?;
            let rel_end = dynamic
                .rel_file_offset
                .checked_add(dynamic.rel_size)
                .filter(|end| *end <= bytes.len())
                .ok_or(Error::Truncated)?;
            let (rel_entries, _) =
                bytes[dynamic.rel_file_offset..rel_end].as_chunks::<REL_ENTRY_SIZE>();
            for entry in rel_entries {
                let target = u64::from_le_bytes(entry[..8].try_into().unwrap());
                let info = u64::from_le_bytes(entry[8..16].try_into().unwrap());
                let relocation_type = info as u32;
                let symbol = info >> 32;
                if !matches!(
                    relocation_type,
                    R_X86_64_RELATIVE
                        | R_X86_64_64
                        | R_X86_64_GLOB_DAT
                        | R_X86_64_JUMP_SLOT
                        | R_X86_64_DTPMOD64
                        | R_X86_64_DTPOFF64
                        | R_X86_64_TPOFF64
                        | R_X86_64_GOTTPOFF
                        | R_X86_64_TPOFF32
                ) {
                    return Err(Error::Unsupported);
                }
                if relocation_type == R_X86_64_RELATIVE && symbol != 0 {
                    return Err(Error::InvalidSegment);
                }
                if relocation_type != R_X86_64_RELATIVE && symbol == 0 {
                    return Err(Error::InvalidSegment);
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
        if dynamic.strtab_address != 0 || dynamic.symtab_address != 0 {
            if dynamic.strtab_address == 0
                || dynamic.symtab_address == 0
                || dynamic.strtab_size == 0
                || dynamic.strtab_size > MAX_DYNAMIC_STRING_TABLE
                || dynamic.symtab_entry_size != 24
            {
                return Err(Error::InvalidSegment);
            }
            let strtab = image
                .segments()
                .find(|segment| {
                    let file_end = segment
                        .virtual_address
                        .saturating_add(segment.file_size as u64);
                    dynamic.strtab_address >= segment.virtual_address
                        && dynamic
                            .strtab_address
                            .checked_add(dynamic.strtab_size as u64)
                            .is_some_and(|end| end <= file_end)
                })
                .ok_or(Error::InvalidSegment)?;
            dynamic.strtab_file_offset = strtab
                .file_offset
                .checked_add((dynamic.strtab_address - strtab.virtual_address) as usize)
                .ok_or(Error::InvalidSegment)?;
            if dynamic
                .strtab_file_offset
                .checked_add(dynamic.strtab_size)
                .filter(|end| *end <= bytes.len())
                .is_none()
            {
                return Err(Error::Truncated);
            }
            if dynamic.symtab_address != 0 {
                if dynamic.hash_address == 0 {
                    return Err(Error::InvalidSegment);
                }
                let hash_segment = image
                    .segments()
                    .find(|segment| {
                        let file_end = segment
                            .virtual_address
                            .saturating_add(segment.file_size as u64);
                        dynamic.hash_address >= segment.virtual_address
                            && dynamic
                                .hash_address
                                .checked_add(8)
                                .is_some_and(|end| end <= file_end)
                    })
                    .ok_or(Error::InvalidSegment)?;
                dynamic.hash_file_offset = hash_segment
                    .file_offset
                    .checked_add((dynamic.hash_address - hash_segment.virtual_address) as usize)
                    .ok_or(Error::InvalidSegment)?;
                let hash_end = dynamic
                    .hash_file_offset
                    .checked_add(8)
                    .filter(|end| *end <= bytes.len())
                    .ok_or(Error::Truncated)?;
                let symbol_count = read_u32(bytes, dynamic.hash_file_offset + 4)? as usize;
                if symbol_count == 0 || symbol_count > MAX_DYNAMIC_SYMBOLS {
                    return Err(Error::InvalidSegment);
                }
                let _ = hash_end;
                let symtab_bytes = symbol_count
                    .checked_mul(dynamic.symtab_entry_size)
                    .ok_or(Error::InvalidSegment)?;
                let symtab_segment = image
                    .segments()
                    .find(|segment| {
                        let file_end = segment
                            .virtual_address
                            .saturating_add(segment.file_size as u64);
                        dynamic.symtab_address >= segment.virtual_address
                            && dynamic
                                .symtab_address
                                .checked_add(symtab_bytes as u64)
                                .is_some_and(|end| end <= file_end)
                    })
                    .ok_or(Error::InvalidSegment)?;
                dynamic.symtab_file_offset = symtab_segment
                    .file_offset
                    .checked_add((dynamic.symtab_address - symtab_segment.virtual_address) as usize)
                    .ok_or(Error::InvalidSegment)?;
                if dynamic
                    .symtab_file_offset
                    .checked_add(symtab_bytes)
                    .filter(|end| *end <= bytes.len())
                    .is_none()
                {
                    return Err(Error::Truncated);
                }
                dynamic.symbol_count = symbol_count;
            }
            for offset in dynamic.needed_offsets[..dynamic.needed_count]
                .iter()
                .copied()
                .chain(dynamic.soname_offset)
            {
                let name_start = dynamic
                    .strtab_file_offset
                    .checked_add(usize::try_from(offset).map_err(|_| Error::InvalidSegment)?)
                    .ok_or(Error::InvalidSegment)?;
                let name_end = dynamic
                    .strtab_file_offset
                    .checked_add(dynamic.strtab_size)
                    .ok_or(Error::InvalidSegment)?;
                if name_start >= name_end
                    || bytes
                        .get(name_start..name_end)
                        .and_then(|slice| slice.iter().position(|byte| *byte == 0))
                        .is_none()
                {
                    return Err(Error::InvalidSegment);
                }
            }
            if dynamic.rela_size != 0 && dynamic.symtab_address == 0 {
                return Err(Error::InvalidSegment);
            }
        }
        image.dynamic = Some(dynamic);
        validate_version_tables(bytes, &mut image)?;
        image.lifecycle = validate_lifecycle(bytes, &mut image)?;
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

fn validate_lifecycle(bytes: &[u8], image: &mut Image) -> Result<Lifecycle, Error> {
    let dynamic = image.dynamic.ok_or(Error::InvalidSegment)?;
    let mut lifecycle = Lifecycle::EMPTY;
    if dynamic.init_address != 0 {
        if !image.contains_executable(dynamic.init_address, 0) {
            return Err(Error::InvalidSegment);
        }
        lifecycle.init[0] = dynamic.init_address;
        lifecycle.init_count = 1;
    }
    if dynamic.init_array_count != 0 {
        if dynamic.init_array_address == 0
            || dynamic.init_array_count > MAX_LIFECYCLE_FUNCTIONS - lifecycle.init_count
        {
            return Err(Error::InvalidSegment);
        }
        let (offset, _) = lifecycle_table_file_range(
            bytes,
            image,
            dynamic.init_array_address,
            dynamic.init_array_count,
        )?;
        for index in 0..dynamic.init_array_count {
            let entry = read_u64(bytes, offset + index * 8)?;
            if entry != 0 {
                if !image.contains_executable(entry, 0) {
                    return Err(Error::InvalidSegment);
                }
                lifecycle.init[lifecycle.init_count] = entry;
                lifecycle.init_count += 1;
            }
        }
    } else if dynamic.init_array_address != 0 {
        return Err(Error::InvalidSegment);
    }
    let mut fini_count = 0;
    if dynamic.fini_array_count != 0 {
        if dynamic.fini_array_address == 0
            || dynamic.fini_array_count > MAX_LIFECYCLE_FUNCTIONS
        {
            return Err(Error::InvalidSegment);
        }
        let (offset, _) = lifecycle_table_file_range(
            bytes,
            image,
            dynamic.fini_array_address,
            dynamic.fini_array_count,
        )?;
        for index in (0..dynamic.fini_array_count).rev() {
            let entry = read_u64(bytes, offset + index * 8)?;
            if entry != 0 {
                if !image.contains_executable(entry, 0) {
                    return Err(Error::InvalidSegment);
                }
                if fini_count == MAX_LIFECYCLE_FUNCTIONS {
                    return Err(Error::InvalidSegment);
                }
                lifecycle.fini[fini_count] = entry;
                fini_count += 1;
            }
        }
    } else if dynamic.fini_array_address != 0 {
        return Err(Error::InvalidSegment);
    }
    if dynamic.fini_address != 0 {
        if fini_count == MAX_LIFECYCLE_FUNCTIONS
            || !image.contains_executable(dynamic.fini_address, 0)
        {
            return Err(Error::InvalidSegment);
        }
        lifecycle.fini[fini_count] = dynamic.fini_address;
        fini_count += 1;
    }
    lifecycle.fini_count = fini_count;
    Ok(lifecycle)
}

fn lifecycle_table_file_range(
    bytes: &[u8],
    image: &Image,
    address: u64,
    count: usize,
) -> Result<(usize, usize), Error> {
    let size = count.checked_mul(8).ok_or(Error::InvalidSegment)?;
    let end = address
        .checked_add(size as u64)
        .ok_or(Error::InvalidSegment)?;
    let segment = image
        .segments()
        .find(|segment| {
            let file_end = segment.virtual_address.saturating_add(segment.file_size as u64);
            segment.virtual_address <= address && end <= file_end
        })
        .ok_or(Error::InvalidSegment)?;
    let offset = segment
        .file_offset
        .checked_add((address - segment.virtual_address) as usize)
        .ok_or(Error::InvalidSegment)?;
    if offset.checked_add(size).filter(|end| *end <= bytes.len()).is_none() {
        return Err(Error::Truncated);
    }
    Ok((offset, size))
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
