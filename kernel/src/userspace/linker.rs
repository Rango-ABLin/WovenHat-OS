//! Eager process loader. All file bytes are copied and verified before mapping;
//! unpublished address spaces are owned by Pending until commit or rollback.

use super::*;
use crate::elf::{self, Image, SymbolScope};
use alloc::{borrow::Cow, vec::Vec};

#[cfg(feature = "stage11-5-test")]
mod acceptance;

#[cfg(feature = "stage11-5-test")]
#[inline(never)]
pub(super) fn shared_library_self_test() -> bool {
    acceptance::run()
}

#[cfg(feature = "stage11-compiler-test")]
#[inline(never)]
pub(super) fn compiler_loader_self_test() -> bool {
    acceptance::compiler_run()
}

const LIBRARY_START: u64 = USER_REGION_START + 0x1000_0000;
const LIBRARY_STRIDE: u64 = 0x0100_0000;
const LIFECYCLE_TRAMPOLINE: u64 = USER_REGION_START + 0x0d_0000;
const LAZY_TRAMPOLINE_BASE: u64 = LIFECYCLE_TRAMPOLINE + 0x100;
const LAZY_TRAMPOLINE_STRIDE: u64 = 32;

struct LinkImage<'a> {
    name: Option<Vec<u8>>,
    bytes: Cow<'a, [u8]>,
    image: Image,
}

#[derive(Clone, Copy)]
struct TlsPlan {
    layout: tls_layout::Layout,
    template_size: usize,
    total: usize,
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|v| v & !(alignment - 1))
}

#[inline(never)]
fn plan_tls(images: &[LinkImage<'_>]) -> Option<Option<TlsPlan>> {
    let main_tls = images.first()?.image.tls();
    let mut library_prefix = 0usize;
    for image in images.iter().skip(1) {
        let Some(tls) = image.image.tls() else {
            continue;
        };
        let alignment = usize::try_from(tls.alignment).ok()?;
        library_prefix = align_up(library_prefix, alignment.max(8))?;
        library_prefix = library_prefix.checked_add(tls.memory_size)?;
    }
    let (main_image_offset, main_thread_pointer_offset) = if let Some(tls) = main_tls {
        let main_layout = tls_layout::Layout::new(
            tls.file_size,
            tls.memory_size,
            tls.alignment,
            tls.virtual_address,
            USER_MMAP_MAX_SIZE,
        )?;
        (
            library_prefix.checked_add(main_layout.image_offset)?,
            library_prefix.checked_add(main_layout.thread_pointer_offset)?,
        )
    } else {
        (library_prefix, align_up(library_prefix, 8)?)
    };
    if main_tls.is_none() && library_prefix == 0 {
        return Some(None);
    }
    let mapping_size = align_up(main_thread_pointer_offset.checked_add(8)?, 4096)?;
    if mapping_size > USER_MMAP_MAX_SIZE {
        return None;
    }
    let layout = tls_layout::Layout {
        image_offset: main_image_offset,
        thread_pointer_offset: main_thread_pointer_offset,
        mapping_size,
    };
    Some(Some(TlsPlan {
        layout,
        template_size: mapping_size,
        total: mapping_size.checked_mul(2)?,
    }))
}

#[inline(never)]
fn tls_image_offset(images: &[LinkImage<'_>], index: usize) -> Option<usize> {
    if index == 0 {
        let tls = images.first()?.image.tls()?;
        let main_layout = tls_layout::Layout::new(
            tls.file_size,
            tls.memory_size,
            tls.alignment,
            tls.virtual_address,
            USER_MMAP_MAX_SIZE,
        )?;
        let mut prefix = 0usize;
        for image in images.iter().skip(1) {
            let Some(tls) = image.image.tls() else {
                continue;
            };
            prefix = align_up(prefix, usize::try_from(tls.alignment).ok()?.max(8))?;
            prefix = prefix.checked_add(tls.memory_size)?;
        }
        return prefix.checked_add(main_layout.image_offset);
    }
    let mut offset = 0usize;
    for (image_index, image) in images.iter().enumerate().skip(1) {
        let Some(tls) = image.image.tls() else {
            continue;
        };
        offset = align_up(offset, usize::try_from(tls.alignment).ok()?.max(8))?;
        if image_index == index {
            return Some(offset);
        }
        offset = offset.checked_add(tls.memory_size)?;
    }
    None
}

#[inline(never)]
fn tls_tpoff(images: &[LinkImage<'_>], index: usize, plan: &TlsPlan) -> Option<i64> {
    i64::try_from(tls_image_offset(images, index)?)
        .ok()?
        .checked_sub(i64::try_from(plan.layout.thread_pointer_offset).ok()?)
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
    let main = elf::parse(bytes)
        .ok()?
        .relocated_for_execution(candidate_bias)?;
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

/// Map and populate verified image segments into an unpublished address space.
/// Keeping this operation on `Pending` preserves rollback if any segment fails.
fn map_linked_segments(
    pending: &mut Pending,
    images: &[LinkImage<'_>],
    stack_guard: u64,
    mmap_base: u64,
) -> Option<()> {
    let page_table = pending.space.paging();
    let mmap_end = mmap_base.checked_add(MAX_ANONYMOUS_MAPPINGS as u64 * USER_MMAP_STRIDE)?;
    for (index, linked) in images.iter().enumerate() {
        for segment in linked.image.segments() {
            let end = segment.mapping_start.checked_add(segment.mapping_size as u64)?;
            if segment.mapping_start < USER_REGION_START
                || (index == 0 && end > stack_guard)
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
    (pending.space.mapping_count < MAX_IMAGE_MAPPINGS).then_some(())
}

fn relocate_linked_images(
    images: &[LinkImage<'_>],
    scope: &SymbolScope<'_>,
    page_table: paging::AddressSpace,
    space: &mut AddressSpace,
    tls_plan: Option<&TlsPlan>,
) -> Option<()> {
    for (image_index, linked) in images.iter().enumerate() {
        relocate(
            linked,
            image_index,
            scope,
            page_table,
            space,
            tls_plan,
            images,
        )?;
    }
    Some(())
}

fn protect_linked_images(
    images: &[LinkImage<'_>],
    page_table: paging::AddressSpace,
) -> Option<()> {
    for linked in images {
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
    Some(())
}

fn map_initial_tls(
    pending: &mut Pending,
    plan: TlsPlan,
    images: &[LinkImage<'_>],
    stack_guard: u64,
    mmap_end: u64,
) -> Option<u64> {
    let page_table = pending.space.paging();
    let layout = plan.layout;
    let template_size = plan.template_size;
    let template_start = stack_guard.checked_sub(plan.total as u64)?;
    let start = template_start.checked_add(template_size as u64)?;
    if template_start < mmap_end
        || pending.space.mappings[..pending.space.mapping_count]
            .iter()
            .any(|mapping| {
                template_start < mapping.start() + mapping.size() as u64
                    && mapping.start() < stack_guard
            })
    {
        return None;
    }
    if template_size != 0 {
        paging::map_user_range_in_inactive(page_table, template_start, template_size, true, false)
            .ok()?;
        pending.template_mapped = Some((template_start, template_size));
        paging::zero_user_range_in(page_table, template_start, template_size).ok()?;
        for (index, image) in images.iter().enumerate() {
            let Some(tls) = image.image.tls() else { continue };
            let image_offset = tls_image_offset(images, index)?;
            copy_tls_bytes(
                page_table,
                tls.virtual_address,
                template_start.checked_add(image_offset as u64)?,
                tls.file_size,
            )?;
        }
        paging::protect_user_range_in_inactive(page_table, template_start, template_size, false, false)
            .ok()?;
    }
    paging::map_user_range_in_inactive(page_table, start, layout.mapping_size, true, false)
        .ok()?;
    let initial = InitialTls {
        start,
        layout,
        template: pending.template_mapped,
        template_size,
        module_tpoff: core::array::from_fn(|index| tls_tpoff(images, index, &plan).unwrap_or(0)),
        module_size: core::array::from_fn(|index| {
            images
                .get(index)
                .and_then(|image| image.image.tls())
                .map(|tls| tls.memory_size)
                .unwrap_or(0)
        }),
        module_valid: core::array::from_fn(|index| {
            images.get(index).and_then(|image| image.image.tls()).is_some()
        }),
    };
    pending.space.tls = Some(initial);
    initialize_tls(page_table, start, initial)
}

fn collect_lifecycle(images: &[LinkImage<'_>]) -> Option<(Vec<u64>, Vec<u64>)> {
    let capacity = elf::MAX_SCOPE_IMAGES * elf::MAX_LIFECYCLE_FUNCTIONS;
    let mut init = Vec::new();
    let mut fini = Vec::new();
    for image in images {
        let lifecycle = image.image.lifecycle();
        if init.len() + lifecycle.init_count > capacity { return None; }
        init.extend_from_slice(&lifecycle.init[..lifecycle.init_count]);
    }
    for image in images.iter().rev() {
        let lifecycle = image.image.lifecycle();
        if fini.len() + lifecycle.fini_count > capacity { return None; }
        fini.extend_from_slice(&lifecycle.fini[..lifecycle.fini_count]);
    }
    Some((init, fini))
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
    let (init_functions, fini_functions) = collect_lifecycle(&images)?;
    let mut scope = SymbolScope::new(&main.image, &main.bytes);
    for image in &images[1..] {
        scope
            .add_dependency(image.name.as_deref()?, &image.image, &image.bytes)
            .ok()?;
    }
    scope.validate_dependencies().ok()?;
    let tls_reservation = plan_tls(&images)?;
    let tls_size = tls_reservation.map_or(0, |plan| plan.total as u64);
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
            lifecycle_trampoline: 0,
            lazy_bindings: [LazyBinding::EMPTY; MAX_LAZY_BINDINGS],
            lazy_binding_count: 0,
        },
        stack_mapped: false,
        template_mapped: None,
        committed: false,
    };
    for (index, linked) in images.iter().enumerate() {
        if index != 0 {
            pending.space.library_relro[index - 1] = linked.image.relro();
        }
    }
    map_linked_segments(&mut pending, &images, stack.guard_base, mmap_base)?;
    let mut image_end = LIFECYCLE_TRAMPOLINE;
    for mapping in &pending.space.mappings[..pending.space.mapping_count] {
        image_end = image_end.max(mapping.start().checked_add(mapping.size() as u64)?);
    }
    let lifecycle_trampoline = if LIFECYCLE_TRAMPOLINE < image_end {
        image_end.checked_add(4095).map(|address| address & !4095)?
    } else {
        LIFECYCLE_TRAMPOLINE
    };
    if lifecycle_trampoline < image_end || lifecycle_trampoline.checked_add(4096)? > mmap_base {
        return None;
    }
    let lazy_trampoline_base = lifecycle_trampoline.checked_add(0x100)?;
    let trampoline = UserMapping::new(lifecycle_trampoline, 4096, false, true)?;
    paging::map_user_range_in_inactive(page_table, lifecycle_trampoline, 4096, true, false).ok()?;
    paging::zero_user_range_in(page_table, lifecycle_trampoline, 4096).ok()?;
    // The exit continuation preserves the status in callee-saved R12 while
    // destructors run, then re-enters the normal exit syscall path.
    let trampoline_code = [
        0x4c, 0x89, 0xe7, // mov rdi,r12
        0xb8, 0x03, 0, 0, 0, // mov eax, SYS_EXIT
        0xcd, 0x80, // int 0x80
        0x0f, 0x0b, // ud2 if the syscall unexpectedly returns
    ];
    paging::write_user_bytes(page_table, lifecycle_trampoline, &trampoline_code).ok()?;
    pending.space.mappings[pending.space.mapping_count] = trampoline;
    pending.space.mapping_count += 1;
    pending.space.lifecycle_trampoline = lifecycle_trampoline;
    // No writable executable page exists during relocation: pages are RW/NX
    // until every fixup is applied, then transitioned to their final W^X mode.
    relocate_linked_images(
        &images,
        &scope,
        page_table,
        &mut pending.space,
        tls_reservation.as_ref(),
    )?;
    for (index, binding) in pending.space.lazy_bindings[..pending.space.lazy_binding_count]
        .iter()
        .copied()
        .enumerate()
    {
        let trampoline = lazy_trampoline_base
            .checked_add((index as u64).checked_mul(LAZY_TRAMPOLINE_STRIDE)?)?;
        let mut code = [0u8; 24];
        code[0..2].copy_from_slice(&[0x49, 0xbb]); // mov r11, slot
        code[2..10].copy_from_slice(&binding.slot.to_le_bytes());
        code[10..15].copy_from_slice(&[0xb8, 137, 0, 0, 0]); // SYS_LAZY_BIND
        code[15..17].copy_from_slice(&[0xcd, 0x80]);
        code[17..19].copy_from_slice(&[0xff, 0xe0]); // jmp rax
        paging::write_user_bytes(page_table, trampoline, &code).ok()?;
    }
    paging::protect_user_range_in_inactive(page_table, lifecycle_trampoline, 4096, false, true)
        .ok()?;
    protect_linked_images(&images, page_table)?;
    let tls_base = if let Some(plan) = tls_reservation {
        map_initial_tls(&mut pending, plan, &images, stack.guard_base, mmap_end)?
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
        &init_functions,
    )?;
    setup_destructor_stack(
        page_table,
        stack,
        pending.space.lifecycle_trampoline,
        &fini_functions,
    )?;
    pending.space.destructors[..fini_functions.len()].copy_from_slice(&fini_functions);
    pending.space.destructor_count = fini_functions.len();
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
    let chain_size = init_functions
        .len()
        .checked_mul(core::mem::size_of::<u64>())?;
    let chain_start = stack_top.checked_sub(chain_size as u64)?;
    if chain_start < stack.base {
        return None;
    }
    for (index, function) in init_functions.iter().copied().enumerate() {
        let next = init_functions.get(index + 1).copied().unwrap_or(main_entry);
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

fn setup_destructor_stack(
    page_table: paging::AddressSpace,
    stack: UserStack,
    trampoline: u64,
    fini_functions: &[u64],
) -> Option<u64> {
    if fini_functions.is_empty() {
        return Some(0);
    }
    let chain_size = fini_functions
        .len()
        .checked_mul(core::mem::size_of::<u64>())?;
    let chain_start = stack
        .base
        .checked_add((UserStack::SIZE / 2) as u64)?
        .checked_sub(chain_size as u64)?;
    for (index, function) in fini_functions.iter().copied().enumerate() {
        let next = fini_functions.get(index + 1).copied().unwrap_or(trampoline);
        paging::write_user_bytes(
            page_table,
            chain_start + index as u64 * 8,
            &next.to_le_bytes(),
        )
        .ok()?;
        let _ = function;
    }
    Some(chain_start)
}

#[inline(never)]
fn relocate(
    linked: &LinkImage<'_>,
    image_index: usize,
    scope: &SymbolScope<'_>,
    page_table: paging::AddressSpace,
    space: &mut AddressSpace,
    tls_plan: Option<&TlsPlan>,
    images: &[LinkImage<'_>],
) -> Option<()> {
    let Some(dynamic) = linked.image.dynamic() else {
        return Some(());
    };
    for (offset, size, is_plt) in [
        (dynamic.rela_file_offset, dynamic.rela_size, false),
        (dynamic.plt_rela_file_offset, dynamic.plt_rela_size, true),
    ] {
        if size == 0 {
            continue;
        }
        let end = offset.checked_add(size)?;
        for entry in linked.bytes.get(offset..end)?.chunks_exact(if is_plt {
            elf::RELA_ENTRY_SIZE
        } else {
            dynamic.rela_entry_size
        }) {
            let target = u64::from_le_bytes(entry[..8].try_into().ok()?)
                .checked_add(linked.image.load_bias())?;
            let info = u64::from_le_bytes(entry[8..16].try_into().ok()?);
            let addend = i64::from_le_bytes(entry[16..24].try_into().ok()?);
            let value = match info as u32 {
                elf::R_X86_64_RELATIVE if info >> 32 == 0 => linked.image.load_bias(),
                elf::R_X86_64_64 | elf::R_X86_64_GLOB_DAT | elf::R_X86_64_JUMP_SLOT => linked
                    .image
                    .relocation_symbol_value(&linked.bytes, (info >> 32) as usize, scope)?,
                elf::R_X86_64_DTPMOD64
                | elf::R_X86_64_DTPOFF64
                | elf::R_X86_64_TPOFF64
                | elf::R_X86_64_GOTTPOFF
                | elf::R_X86_64_TPOFF32 => tls_relocation_value(
                    linked,
                    image_index,
                    (info >> 32) as usize,
                    info as u32,
                    scope,
                    tls_plan,
                    images,
                )?,
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            let value = if is_plt
                && info as u32 == elf::R_X86_64_JUMP_SLOT
                && lazy_slot_allowed(*space, target)
            {
                register_lazy_binding(space, target, value)?
            } else {
                value
            };
            write_relocation_value(page_table, target, value, info as u32)?;
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
                elf::R_X86_64_DTPMOD64
                | elf::R_X86_64_DTPOFF64
                | elf::R_X86_64_TPOFF64
                | elf::R_X86_64_GOTTPOFF
                | elf::R_X86_64_TPOFF32 => tls_relocation_value(
                    linked,
                    image_index,
                    (info >> 32) as usize,
                    info as u32,
                    scope,
                    tls_plan,
                    images,
                )?,
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            let value =
                if info as u32 == elf::R_X86_64_JUMP_SLOT && lazy_slot_allowed(*space, target) {
                    register_lazy_binding(space, target, value)?
                } else {
                    value
                };
            write_relocation_value(page_table, target, value, info as u32)?;
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
                elf::R_X86_64_DTPMOD64
                | elf::R_X86_64_DTPOFF64
                | elf::R_X86_64_TPOFF64
                | elf::R_X86_64_GOTTPOFF
                | elf::R_X86_64_TPOFF32 => tls_relocation_value(
                    linked,
                    image_index,
                    (info >> 32) as usize,
                    info as u32,
                    scope,
                    tls_plan,
                    images,
                )?,
                _ => return None,
            };
            let value = if addend >= 0 {
                value.checked_add(addend as u64)?
            } else {
                value.checked_sub(addend.unsigned_abs())?
            };
            let value =
                if info as u32 == elf::R_X86_64_JUMP_SLOT && lazy_slot_allowed(*space, target) {
                    register_lazy_binding(space, target, value)?
                } else {
                    value
                };
            write_relocation_value(page_table, target, value, info as u32)?;
        }
    }
    Some(())
}

#[inline(never)]
fn tls_relocation_value(
    linked: &LinkImage<'_>,
    image_index: usize,
    symbol_index: usize,
    relocation_type: u32,
    scope: &SymbolScope<'_>,
    tls_plan: Option<&TlsPlan>,
    images: &[LinkImage<'_>],
) -> Option<u64> {
    let (symbol_offset, provider) =
        linked
            .image
            .tls_symbol_resolution(&linked.bytes, symbol_index, scope)?;
    let module_index = provider.unwrap_or(image_index);
    let plan = tls_plan?;
    let tpoff = tls_tpoff(images, module_index, plan)?;
    match relocation_type {
        elf::R_X86_64_DTPMOD64 => Some((module_index + 1) as u64),
        elf::R_X86_64_DTPOFF64 => Some(symbol_offset),
        elf::R_X86_64_TPOFF64 | elf::R_X86_64_GOTTPOFF | elf::R_X86_64_TPOFF32 => {
            let value = tpoff.checked_add(i64::try_from(symbol_offset).ok()?)?;
            Some(value as u64)
        }
        _ => None,
    }
}

fn write_relocation_value(
    page_table: paging::AddressSpace,
    target: u64,
    value: u64,
    relocation_type: u32,
) -> Option<()> {
    if relocation_type == elf::R_X86_64_TPOFF32 {
        let value = i32::try_from(value as i64).ok()?;
        paging::write_user_bytes(page_table, target, &value.to_le_bytes()).ok()
    } else {
        paging::write_user_bytes(page_table, target, &value.to_le_bytes()).ok()
    }
}

fn lazy_slot_allowed(space: AddressSpace, slot: u64) -> bool {
    space.mappings[..space.mapping_count].iter().any(|mapping| {
        mapping.writable
            && slot >= mapping.start()
            && slot
                .checked_add(8)
                .is_some_and(|end| end <= mapping.start() + mapping.size() as u64)
            && !space
                .relro
                .into_iter()
                .chain(space.library_relro.into_iter().flatten())
                .any(|(start, size)| slot < start + size as u64 && start < slot + 8)
    })
}

fn register_lazy_binding(space: &mut AddressSpace, slot: u64, target: u64) -> Option<u64> {
    if space.lazy_binding_count == MAX_LAZY_BINDINGS {
        return None;
    }
    let index = space.lazy_binding_count;
    space.lazy_bindings[index] = LazyBinding { slot, target };
    space.lazy_binding_count += 1;
    LAZY_TRAMPOLINE_BASE.checked_add((index as u64).checked_mul(LAZY_TRAMPOLINE_STRIDE)?)
}
