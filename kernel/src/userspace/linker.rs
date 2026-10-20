//! Eager process loader. All file bytes are copied and verified before mapping;
//! unpublished address spaces are owned by Pending until commit or rollback.

use super::*;
use crate::elf::{self, Image, SymbolScope};
use alloc::{borrow::Cow, vec::Vec};

#[cfg(feature = "stage11-5-test")]
mod acceptance;

#[cfg(feature = "stage11-5-test")]
pub(super) fn shared_library_self_test() -> bool {
    acceptance::run()
}

#[cfg(feature = "stage11-compiler-test")]
pub(super) fn compiler_loader_self_test() -> bool {
    acceptance::compiler_run()
}

const LIBRARY_START: u64 = USER_REGION_START + 0x1000_0000;
const LIBRARY_STRIDE: u64 = 0x0100_0000;

struct LinkImage<'a> {
    name: Option<Vec<u8>>,
    bytes: Cow<'a, [u8]>,
    image: Image,
}

fn library_path(name: &[u8]) -> Option<alloc::string::String> {
    if name.is_empty()
        || name.len() > 128
        || name == b"."
        || name == b".."
        || !name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return None;
    }
    let mut path = alloc::string::String::from("/lib/");
    path.push_str(core::str::from_utf8(name).ok()?);
    Some(path)
}

fn prepare(bytes: &[u8], candidate_bias: u64) -> Option<Vec<LinkImage<'_>>> {
    let main = elf::parse(bytes).ok()?.relocated_for_execution(candidate_bias)?;
    let mut images = Vec::with_capacity(elf::MAX_SCOPE_IMAGES);
    images.push(LinkImage {
        name: None,
        bytes: Cow::Borrowed(bytes),
        image: main,
    });
    let mut cursor = 0;
    while cursor < images.len() {
        let count = images[cursor]
            .image
            .dynamic()
            .map_or(0, |dynamic| dynamic.needed_count);
        for index in 0..count {
            let name = images[cursor]
                .image
                .needed_name(&images[cursor].bytes, index)?
                .to_vec();
            if (0..index).any(|prior| {
                images[cursor]
                    .image
                    .needed_name(&images[cursor].bytes, prior)
                    == Some(name.as_slice())
            }) {
                return None;
            }
            let path = library_path(&name)?;
            if images
                .iter()
                .any(|image| image.name.as_deref() == Some(name.as_slice()))
            {
                continue;
            }
            if images.len() == elf::MAX_SCOPE_IMAGES {
                return None;
            }
            let stat = crate::vfs::stat(&path).ok()?;
            if stat.kind != crate::vfs::NodeKind::File || stat.writable {
                return None;
            }
            // Read one bounded snapshot. Signature verification covers exactly
            // the bytes mapped, even if the pathname is later replaced.
            let mut package = alloc::vec![0; crate::vfs::NODE_CAPACITY];
            let length = crate::vfs::read_all(&path, &mut package).ok()?;
            let payload = crate::signed_app::verify_package(package.get(..length)?)
                .ok()?
                .bytes()
                .to_vec();
            let image = elf::parse_shared(&payload).ok()?;
            if image.tls().is_some() {
                return None;
            }
            if image
                .dynamic()
                .is_some_and(|dynamic| dynamic.soname_offset.is_some())
                && image.soname(&payload) != Some(name.as_slice())
            {
                return None;
            }
            let original_start = image
                .segments()
                .map(|segment| segment.mapping_start)
                .min()?;
            let base = LIBRARY_START
                .checked_add((images.len() as u64 - 1) * LIBRARY_STRIDE)?
                .checked_add(aslr_pages(ASLR_IMAGE_SPAN) * ASLR_PAGE)?;
            let delta = base.checked_sub(original_start)?;
            let image = image.relocated(delta)?;
            if image.segments().any(|segment| {
                segment.mapping_start < base
                    || segment
                        .mapping_start
                        .checked_add(segment.mapping_size as u64)
                        .is_none_or(|end| end > base + LIBRARY_STRIDE - ASLR_IMAGE_SPAN)
            }) {
                return None;
            }
            images.push(LinkImage {
                name: Some(name),
                bytes: Cow::Owned(payload),
                image,
            });
        }
        cursor += 1;
    }
    Some(images)
}

struct Pending {
    space: AddressSpace,
    stack_mapped: bool,
    template_mapped: Option<(u64, usize)>,
    committed: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut ranges = [(0, 0); MAX_IMAGE_MAPPINGS + 3];
        let mut count = 0;
        if self.stack_mapped {
            ranges[count] = (self.space.stack_base, UserStack::SIZE);
            count += 1;
        }
        if let Some(tls) = self.space.tls {
            ranges[count] = tls.range();
            count += 1;
        }
        if let Some(template) = self.template_mapped {
            ranges[count] = template;
            count += 1;
        }
        for mapping in self.space.mappings[..self.space.mapping_count].iter().rev() {
            ranges[count] = (mapping.start(), mapping.size());
            count += 1;
        }
        if count == 0 {
            assert!(
                paging::discard_empty_user_address_space(self.space.paging),
                "empty loader rollback failed"
            );
        } else {
            assert!(
                paging::destroy_user_address_space(self.space.paging, &ranges[..count]).is_ok(),
                "loader rollback failed"
            );
        }
    }
}

pub(super) fn load(bytes: &[u8], args: &[&str]) -> Option<UserProgram> {
    load_with_candidate_bias(bytes, args, aslr_image_delta())
}

pub(super) fn load_with_candidate_bias(
    bytes: &[u8],
    args: &[&str],
    candidate_bias: u64,
) -> Option<UserProgram> {
    load_with_geometry(
        bytes,
        args,
        candidate_bias,
        aslr_stack_base()?,
        aslr_mmap_base()?,
    )
}

// One graph-loader path for production draws and forced-draw CPU regressions.
// Explicit geometry is private to the userspace module, not a userspace ABI.
pub(super) fn load_with_geometry(
    bytes: &[u8],
    args: &[&str],
    candidate_bias: u64,
    stack_base: u64,
    mmap_base: u64,
) -> Option<UserProgram> {
    let images = prepare(bytes, candidate_bias)?;
    let main = images.first()?;
    let mut init_functions = [0u64; elf::MAX_SCOPE_IMAGES * elf::MAX_LIFECYCLE_FUNCTIONS];
    let mut init_count = 0usize;
    let mut fini_functions = [0u64; elf::MAX_SCOPE_IMAGES * elf::MAX_LIFECYCLE_FUNCTIONS];
    let mut fini_count = 0usize;
    for image in &images {
        let lifecycle = image.image.lifecycle();
        for function in lifecycle.init[..lifecycle.init_count].iter().copied() {
            *init_functions.get_mut(init_count)? = function;
            init_count += 1;
        }
    }
    for image in images.iter().rev() {
        let lifecycle = image.image.lifecycle();
        for function in lifecycle.fini[..lifecycle.fini_count].iter().copied() {
            *fini_functions.get_mut(fini_count)? = function;
            fini_count += 1;
        }
    }
    let mut scope = SymbolScope::new(&main.image, &main.bytes);
    for image in &images[1..] {
        scope
            .add_dependency(image.name.as_deref()?, &image.image, &image.bytes)
            .ok()?;
    }
    scope.validate_dependencies().ok()?;
    let tls_reservation = match main.image.tls() {
        Some(tls) => {
            let layout = tls_layout::Layout::new(
                tls.file_size,
                tls.memory_size,
                tls.alignment,
                tls.virtual_address,
                USER_MMAP_MAX_SIZE,
            )?;
            let template_size = usize::try_from(page_align_up(tls.file_size as u64)?).ok()?;
            let total = layout.mapping_size.checked_add(template_size)?;
            Some((layout, template_size, total))
        }
        None => None,
    };
    let tls_size = tls_reservation.map_or(0, |(_, _, size)| size as u64);
    let stack_base = plan_stack_base(stack_base, mmap_base, tls_size)?;
    let stack = UserStack::new(stack_base);
    let mmap_end = mmap_base.checked_add(MAX_ANONYMOUS_MAPPINGS as u64 * USER_MMAP_STRIDE)?;
    let page_table = paging::create_user_address_space(main.image.entry)?;
    let mut pending = Pending {
        space: AddressSpace {
            paging: page_table,
            stack_base,
            mmap_base,
            mappings: [UserMapping::EMPTY; MAX_IMAGE_MAPPINGS],
            mapping_count: 0,
            relro: main.image.relro(),
            library_relro: [None; elf::MAX_NEEDED_LIBRARIES],
            tls: None,
            destructors: [0; elf::MAX_SCOPE_IMAGES * elf::MAX_LIFECYCLE_FUNCTIONS],
            destructor_count: 0,
            destructor_pending: false,
        },
        stack_mapped: false,
        template_mapped: None,
        committed: false,
    };
    for (index, linked) in images.iter().enumerate() {
        if index != 0 {
            pending.space.library_relro[index - 1] = linked.image.relro();
        }
        for segment in linked.image.segments() {
            let end = segment
                .mapping_start
                .checked_add(segment.mapping_size as u64)?;
            if segment.mapping_start < USER_REGION_START
                || (index == 0 && end > stack.guard_base)
                || (segment.mapping_start < mmap_end && mmap_base < end)
                || pending.space.mappings[..pending.space.mapping_count]
                    .iter()
                    .any(|mapping| {
                        segment.mapping_start < mapping.start() + mapping.size() as u64
                            && mapping.start() < end
                    })
                || pending.space.mapping_count == MAX_IMAGE_MAPPINGS
            {
                return None;
            }
            let mapping = UserMapping::new(
                segment.mapping_start,
                segment.mapping_size,
                segment.writable,
                segment.executable,
            )?;
            paging::map_user_range_in_inactive(
                page_table,
                segment.mapping_start,
                segment.mapping_size,
                true,
                false,
            )
            .ok()?;
            pending.space.mappings[pending.space.mapping_count] = mapping;
            pending.space.mapping_count += 1;
            paging::zero_user_range_in(page_table, segment.mapping_start, segment.mapping_size)
                .ok()?;
            let file_end = segment.file_offset.checked_add(segment.file_size)?;
            paging::write_user_bytes(
                page_table,
                segment.virtual_address,
                linked.bytes.get(segment.file_offset..file_end)?,
            )
            .ok()?;
        }
    }
    // No writable executable page exists during relocation: pages are RW/NX
    // until every fixup is applied, then transitioned to their final W^X mode.
    for linked in &images {
        relocate(linked, &scope, page_table)?;
    }
    for linked in &images {
        for segment in linked.image.segments() {
            paging::protect_user_range_in_inactive(
                page_table,
                segment.mapping_start,
                segment.mapping_size,
                segment.writable,
                segment.executable,
            )
            .ok()?;
        }
        if let Some((start, size)) = linked.image.relro() {
            paging::protect_user_range_in_inactive(page_table, start, size, false, false).ok()?;
        }
    }
    let tls_base = if let Some(tls) = main.image.tls() {
        let (layout, template_size, total) = tls_reservation?;
        let template_start = stack.guard_base.checked_sub(total as u64)?;
        let start = template_start.checked_add(template_size as u64)?;
        if template_start < mmap_end
            || pending.space.mappings[..pending.space.mapping_count]
                .iter()
                .any(|mapping| {
                    template_start < mapping.start() + mapping.size() as u64
                        && mapping.start() < stack.guard_base
                })
        {
            return None;
        }
        if template_size != 0 {
            paging::map_user_range_in_inactive(
                page_table,
                template_start,
                template_size,
                true,
                false,
            )
            .ok()?;
            pending.template_mapped = Some((template_start, template_size));
            paging::zero_user_range_in(page_table, template_start, template_size).ok()?;
            // Capture post-relocation initialization bytes from the validated
            // PT_LOAD source, not raw ELF or the mutable first thread's TLS.
            copy_tls_bytes(
                page_table,
                tls.virtual_address,
                template_start,
                tls.file_size,
            )?;
            paging::protect_user_range_in_inactive(
                page_table,
                template_start,
                template_size,
                false,
                false,
            )
            .ok()?;
        }
        paging::map_user_range_in_inactive(page_table, start, layout.mapping_size, true, false)
            .ok()?;
        let initial = InitialTls {
            start,
            layout,
            template: pending.template_mapped,
            file_size: tls.file_size,
        };
        pending.space.tls = Some(initial);
        initialize_tls(page_table, start, initial)?
    } else {
        0
    };
    paging::map_user_range_in_inactive(page_table, stack_base, stack.size, true, false).ok()?;
    pending.stack_mapped = true;
    paging::zero_user_range_in(page_table, stack_base, stack.size).ok()?;
    if !paging::user_range_is_unmapped_in(page_table, stack.guard_base, UserStack::GUARD_SIZE)
        || !paging::user_range_has_protection_in(page_table, stack.base, stack.size, true, false)
    {
        return None;
    }
    let stack_top = setup_argv_stack(page_table, stack, args)?;
    let (initial_entry, initial_stack) = setup_lifecycle_stack(
        page_table,
        stack,
        stack_top,
        main.image.entry,
        &init_functions[..init_count],
    )?;
    pending.space.destructors[..fini_count].copy_from_slice(&fini_functions[..fini_count]);
    pending.space.destructor_count = fini_count;
    let program = UserProgram {
        image: UserImage {
            entry: main.image.entry,
            stack_top: initial_stack,
            image_size: bytes.len() as u64,
            load_segments: pending.space.mapping_count,
        },
        initial_entry,
        stack,
        address_space: pending.space,
        tls_base,
    };
    pending.committed = true;
    Some(program)
}

fn setup_lifecycle_stack(
    page_table: paging::AddressSpace,
    stack: UserStack,
    stack_top: u64,
    main_entry: u64,
    init_functions: &[u64],
) -> Option<(u64, u64)> {
    if init_functions.is_empty() {
        return Some((main_entry, stack_top));
    }
    let chain_size = init_functions.len().checked_mul(core::mem::size_of::<u64>())?;
    let chain_start = stack_top.checked_sub(chain_size as u64)?;
    if chain_start < stack.base {
        return None;
    }
    for (index, function) in init_functions.iter().copied().enumerate() {
        let next = init_functions
            .get(index + 1)
            .copied()
            .unwrap_or(main_entry);
        paging::write_user_bytes(
            page_table,
            chain_start + index as u64 * 8,
            &next.to_le_bytes(),
        )
        .ok()?;
        let _ = function;
    }
    Some((init_functions[0], chain_start))
}

fn relocate(
    linked: &LinkImage<'_>,
    scope: &SymbolScope<'_>,
    page_table: paging::AddressSpace,
) -> Option<()> {
    let Some(dynamic) = linked.image.dynamic() else {
        return Some(());
    };
    for (offset, size) in [
        (dynamic.rela_file_offset, dynamic.rela_size),
        (dynamic.plt_rela_file_offset, dynamic.plt_rela_size),
    ] {
        if size == 0 {
            continue;
        }
        let end = offset.checked_add(size)?;
        for entry in linked
            .bytes
            .get(offset..end)?
            .chunks_exact(dynamic.rela_entry_size)
        {
            let target = u64::from_le_bytes(entry[..8].try_into().ok()?)
                .checked_add(linked.image.load_bias())?;
            let info = u64::from_le_bytes(entry[8..16].try_into().ok()?);
            let addend = i64::from_le_bytes(entry[16..24].try_into().ok()?);
            let value = match info as u32 {
                elf::R_X86_64_RELATIVE if info >> 32 == 0 => linked.image.load_bias(),
                elf::R_X86_64_64 | elf::R_X86_64_GLOB_DAT | elf::R_X86_64_JUMP_SLOT => {
                    linked.image.relocation_symbol_value(
                        &linked.bytes,
                        (info >> 32) as usize,
                        scope,
                    )?
                }
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            paging::write_user_bytes(page_table, target, &value.to_le_bytes()).ok()?;
        }
    }
    if dynamic.rel_size != 0 {
        let end = dynamic.rel_file_offset.checked_add(dynamic.rel_size)?;
        for entry in linked
            .bytes
            .get(dynamic.rel_file_offset..end)?
            .chunks_exact(dynamic.rel_entry_size)
        {
            let target = u64::from_le_bytes(entry[..8].try_into().ok()?)
                .checked_add(linked.image.load_bias())?;
            let info = u64::from_le_bytes(entry[8..16].try_into().ok()?);
            let mut addend_bytes = [0; 8];
            paging::read_user_bytes_in(page_table, target, &mut addend_bytes).ok()?;
            let addend = i64::from_le_bytes(addend_bytes);
            let value = match info as u32 {
                elf::R_X86_64_RELATIVE if info >> 32 == 0 => linked.image.load_bias(),
                elf::R_X86_64_64 | elf::R_X86_64_GLOB_DAT | elf::R_X86_64_JUMP_SLOT => linked
                    .image
                    .relocation_symbol_value(&linked.bytes, (info >> 32) as usize, scope)?,
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            paging::write_user_bytes(page_table, target, &value.to_le_bytes()).ok()?;
        }
    }
    if dynamic.plt_rel_size != 0 {
        let end = dynamic
            .plt_rel_file_offset
            .checked_add(dynamic.plt_rel_size)?;
        for entry in linked
            .bytes
            .get(dynamic.plt_rel_file_offset..end)?
            .chunks_exact(dynamic.plt_rel_entry_size)
        {
            let target = u64::from_le_bytes(entry[..8].try_into().ok()?)
                .checked_add(linked.image.load_bias())?;
            let info = u64::from_le_bytes(entry[8..16].try_into().ok()?);
            let mut addend_bytes = [0; 8];
            paging::read_user_bytes_in(page_table, target, &mut addend_bytes).ok()?;
            let addend = i64::from_le_bytes(addend_bytes);
            let value = match info as u32 {
                elf::R_X86_64_RELATIVE if info >> 32 == 0 => linked.image.load_bias(),
                elf::R_X86_64_64 | elf::R_X86_64_GLOB_DAT | elf::R_X86_64_JUMP_SLOT => linked
                    .image
                    .relocation_symbol_value(&linked.bytes, (info >> 32) as usize, scope)?,
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            paging::write_user_bytes(page_table, target, &value.to_le_bytes()).ok()?;
        }
    }
    Some(())
}
