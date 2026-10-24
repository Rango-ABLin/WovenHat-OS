//! Checked x86-64 Variant-II static TLS layout. No pointers or allocations.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub image_offset: usize,
    pub thread_pointer_offset: usize,
    pub mapping_size: usize,
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|v| v & !(alignment - 1))
}

impl Layout {
    pub fn new(
        file_size: usize,
        memory_size: usize,
        alignment: u64,
        virtual_address: u64,
        capacity: usize,
    ) -> Option<Self> {
        if memory_size == 0
            || file_size > memory_size
            || !alignment.is_power_of_two()
            || alignment > 4096
        {
            return None;
        }
        let alignment = usize::try_from(alignment).ok()?;
        let congruence = usize::try_from(virtual_address % alignment as u64).ok()?;
        // TP is aligned; the start of the TLS image must retain p_vaddr's
        // congruence. All terms in this modular sum are at most 4095.
        let padding = (alignment - (congruence + memory_size % alignment) % alignment) % alignment;
        let span = memory_size.checked_add(padding)?;
        let thread_pointer_offset = align_up(span, alignment.max(8))?;
        let image_offset = thread_pointer_offset.checked_sub(span)?;
        // FS:0 is a mapped eight-byte self pointer, not the following guard.
        let mapping_size = align_up(thread_pointer_offset.checked_add(8)?, 4096)?;
        if mapping_size > capacity {
            return None;
        }
        Some(Self {
            image_offset,
            thread_pointer_offset,
            mapping_size,
        })
    }
}
