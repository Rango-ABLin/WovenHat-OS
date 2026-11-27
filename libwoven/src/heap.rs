//! Allocation metadata for a reclaiming native heap, without raw-pointer access.
//!
//! Sixteen 64 KiB regions mirror the existing kernel ABI. Adjacent regions can
//! service a single larger allocation. Bitmaps live outside application payloads;
//! freeing a span immediately makes it reusable without free-list fragmentation.
use core::alloc::Layout;
pub const REGION_BYTES: usize = 64 * 1024;
pub const MAX_REGIONS: usize = 16;
pub const GRANULE: usize = 16;
const UNITS: usize = REGION_BYTES / GRANULE;
const WORDS: usize = UNITS / 64;

#[derive(Clone, Copy)]
struct Region {
    base: usize,
    quarantined: bool,
    used: [u64; WORDS],
    starts: [u64; WORDS],
    ends: [u64; WORDS],
}
impl Region {
    const EMPTY: Self = Self {
        base: 0,
        quarantined: false,
        used: [0; WORDS],
        starts: [0; WORDS],
        ends: [0; WORDS],
    };
    fn empty(&self) -> bool {
        !self.quarantined && self.used.iter().all(|word| *word == 0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub regions: usize,
    pub quarantined_regions: usize,
    pub mapped_bytes: usize,
    pub live_allocations: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
    pub allocations: u64,
}
#[derive(Clone, Copy)]
struct Span {
    first: usize,
    start: usize,
    last: usize,
    end: usize,
}

pub struct Heap {
    regions: [Region; MAX_REGIONS],
    count: usize,
    live: usize,
    bytes: usize,
    peak: usize,
    allocations: u64,
}
impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}
impl Heap {
    pub const fn new() -> Self {
        Self {
            regions: [Region::EMPTY; MAX_REGIONS],
            count: 0,
            live: 0,
            bytes: 0,
            peak: 0,
            allocations: 0,
        }
    }
    pub fn stats(&self) -> Stats {
        Stats {
            regions: self.count,
            quarantined_regions: self.regions[..self.count]
                .iter()
                .filter(|r| r.quarantined)
                .count(),
            mapped_bytes: self.count * REGION_BYTES,
            live_allocations: self.live,
            live_bytes: self.bytes,
            peak_bytes: self.peak,
            allocations: self.allocations,
        }
    }
    pub fn region_base(&self, index: usize) -> Option<usize> {
        (index < self.count).then(|| self.regions[index].base)
    }
    pub fn region_empty(&self, base: usize) -> bool {
        self.regions[..self.count]
            .iter()
            .any(|region| region.base == base && region.empty())
    }
    /// Admit an exclusive, page-aligned range. This registers integer metadata
    /// only: the caller must arrange actual mapping before returning pointers.
    pub fn add_region(&mut self, base: usize) -> bool {
        let Some(end) = base.checked_add(REGION_BYTES) else {
            return false;
        };
        if self.count == MAX_REGIONS || base == 0 || !base.is_multiple_of(4096) {
            return false;
        }
        if self.regions[..self.count]
            .iter()
            .any(|r| base < r.base + REGION_BYTES && r.base < end)
        {
            return false;
        }
        let index = self.regions[..self.count].partition_point(|region| region.base < base);
        self.regions.copy_within(index..self.count, index + 1);
        self.regions[index] = Region {
            base,
            ..Region::EMPTY
        };
        self.count += 1;
        true
    }
    /// An unsuccessful unmap can have removed some pages. Never reuse or bridge
    /// that range; retain its descriptor accounting until process teardown.
    pub fn quarantine_region(&mut self, base: usize) -> bool {
        let Some(region) = self.regions[..self.count]
            .iter_mut()
            .find(|r| r.base == base)
        else {
            return false;
        };
        if !region.empty() {
            return false;
        }
        region.quarantined = true;
        true
    }
    /// Remove only an idle region, after its backing mapping is released.
    pub fn remove_region(&mut self, base: usize) -> bool {
        let Some(index) = self.regions[..self.count]
            .iter()
            .position(|region| region.base == base)
        else {
            return false;
        };
        if !self.regions[index].empty() {
            return false;
        }
        self.regions.copy_within(index + 1..self.count, index);
        self.count -= 1;
        self.regions[self.count] = Region::EMPTY;
        true
    }
    pub fn can_fit(layout: Layout) -> bool {
        layout.size() != 0
            && layout.size() <= REGION_BYTES * MAX_REGIONS
            && layout.align() <= REGION_BYTES * MAX_REGIONS
    }
    pub fn allocate(&mut self, layout: Layout) -> Option<usize> {
        if !Self::can_fit(layout) {
            return None;
        }
        let units = layout.size().checked_add(GRANULE - 1)? / GRANULE;
        let mut run = 0;
        let mut start = 0;
        let mut next = 0;
        for region in &self.regions[..self.count] {
            if region.quarantined {
                run = 0;
                next = 0;
                continue;
            }
            if region.base != next {
                run = 0;
            }
            for unit in 0..UNITS {
                let address = region.base + unit * GRANULE;
                if bit(&region.used, unit) {
                    run = 0;
                    continue;
                }
                if run == 0 {
                    if address & (layout.align() - 1) != 0 {
                        continue;
                    }
                    start = address;
                }
                run += 1;
                if run == units {
                    let span = self.span(start, units)?;
                    self.mark(span, true);
                    set(&mut self.regions[span.first].starts, span.start, true);
                    set(&mut self.regions[span.last].ends, span.end, true);
                    self.live += 1;
                    self.bytes += units * GRANULE;
                    self.peak = self.peak.max(self.bytes);
                    self.allocations = self.allocations.saturating_add(1);
                    return Some(start);
                }
            }
            next = region.base + REGION_BYTES;
        }
        None
    }
    /// Layout must describe the original allocation (GlobalAlloc contract).
    /// Span boundaries additionally reject interior, overlapping and double frees.
    pub fn deallocate(&mut self, address: usize, layout: Layout) -> bool {
        if !Self::can_fit(layout) || address & (layout.align() - 1) != 0 {
            return false;
        }
        let Some(size) = layout.size().checked_add(GRANULE - 1) else {
            return false;
        };
        let units = size / GRANULE;
        let Some(span) = self.span(address, units) else {
            return false;
        };
        if !bit(&self.regions[span.first].starts, span.start)
            || !bit(&self.regions[span.last].ends, span.end)
        {
            return false;
        }
        for index in span.first..=span.last {
            let first = if index == span.first { span.start } else { 0 };
            let last = if index == span.last {
                span.end
            } else {
                UNITS - 1
            };
            for unit in first..=last {
                if !bit(&self.regions[index].used, unit)
                    || (bit(&self.regions[index].starts, unit)
                        && (index != span.first || unit != span.start))
                    || (bit(&self.regions[index].ends, unit)
                        && (index != span.last || unit != span.end))
                {
                    return false;
                }
            }
        }
        self.mark(span, false);
        set(&mut self.regions[span.first].starts, span.start, false);
        set(&mut self.regions[span.last].ends, span.end, false);
        self.live -= 1;
        self.bytes -= units * GRANULE;
        true
    }
    fn span(&self, address: usize, units: usize) -> Option<Span> {
        if units == 0 || !address.is_multiple_of(GRANULE) {
            return None;
        }
        let first = self.regions[..self.count]
            .iter()
            .position(|r| !r.quarantined && address >= r.base && address < r.base + REGION_BYTES)?;
        let start = (address - self.regions[first].base) / GRANULE;
        let mut remaining = units;
        let mut index = first;
        let mut offset = start;
        loop {
            let available = UNITS - offset;
            if remaining <= available {
                return Some(Span {
                    first,
                    start,
                    last: index,
                    end: offset + remaining - 1,
                });
            }
            remaining -= available;
            let next = index + 1;
            if next >= self.count
                || self.regions[next].quarantined
                || self.regions[next].base != self.regions[index].base + REGION_BYTES
            {
                return None;
            }
            index = next;
            offset = 0;
        }
    }
    fn mark(&mut self, span: Span, value: bool) {
        for index in span.first..=span.last {
            let first = if index == span.first { span.start } else { 0 };
            let last = if index == span.last {
                span.end
            } else {
                UNITS - 1
            };
            for unit in first..=last {
                set(&mut self.regions[index].used, unit, value);
            }
        }
    }
}
fn bit(words: &[u64; WORDS], unit: usize) -> bool {
    words[unit / 64] & (1u64 << (unit % 64)) != 0
}
fn set(words: &mut [u64; WORDS], unit: usize, value: bool) {
    let mask = 1u64 << (unit % 64);
    if value {
        words[unit / 64] |= mask;
    } else {
        words[unit / 64] &= !mask;
    }
}

/// Access the native allocator directly for fallible OS-backed allocation and
/// precise runtime accounting, without Rust allocation-intrinsic assumptions.
/// All GlobalAlloc unsafe ownership/layout contracts apply to its methods.
#[cfg(all(feature = "reclaiming-allocator", target_os = "none", not(test)))]
pub fn runtime_allocator() -> &'static (impl core::alloc::GlobalAlloc + Sync) {
    &crate::USER_HEAP
}

#[cfg(all(feature = "reclaiming-allocator", target_os = "none", not(test)))]
pub fn runtime_stats() -> Stats {
    crate::USER_HEAP.stats()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }
    #[test]
    fn reuses_and_implicitly_coalesces_freed_spans() {
        let mut heap = Heap::new();
        assert!(heap.add_region(0x100000));
        let a = heap.allocate(layout(1024, 16)).unwrap();
        let b = heap.allocate(layout(1024, 16)).unwrap();
        assert!(heap.deallocate(a, layout(1024, 16)));
        assert!(heap.deallocate(b, layout(1024, 16)));
        assert_eq!(heap.allocate(layout(REGION_BYTES, 4096)), Some(0x100000));
        assert_eq!(heap.stats().live_allocations, 1);
    }
    #[test]
    fn combines_adjacent_regions_but_does_not_bridge_gaps() {
        let mut heap = Heap::new();
        assert!(heap.add_region(0x120000));
        assert!(heap.add_region(0x100000));
        assert!(heap.allocate(layout(2 * REGION_BYTES, 16)).is_none());
        assert!(heap.add_region(0x110000));
        let address = heap.allocate(layout(3 * REGION_BYTES, 65536)).unwrap();
        assert_eq!(address, 0x100000);
        assert!(!heap.remove_region(0x110000));
        assert!(heap.deallocate(address, layout(3 * REGION_BYTES, 65536)));
        assert!(heap.remove_region(0x110000));
        assert_eq!(heap.stats().live_bytes, 0);
    }
    #[test]
    fn enforces_all_requested_alignments_and_layout_bounds() {
        let mut heap = Heap::new();
        assert!(heap.add_region(0x101000));
        assert!(heap.add_region(0x111000));
        for align in [1, 2, 8, 16, 32, 64, 256, 4096, 65536] {
            let spec = layout(19, align);
            let address = heap.allocate(spec).unwrap();
            assert_eq!(address % align, 0);
            assert!(heap.deallocate(address, spec));
        }
        assert!(heap.allocate(layout(0, 16)).is_none());
        assert!(
            heap.allocate(layout(MAX_REGIONS * REGION_BYTES + 1, 16))
                .is_none()
        );
    }
    #[test]
    fn rejects_bad_free_boundaries_without_mutating_live_payload_metadata() {
        let mut heap = Heap::new();
        assert!(heap.add_region(0x100000));
        let a = heap.allocate(layout(32, 16)).unwrap();
        let b = heap.allocate(layout(32, 16)).unwrap();
        let before = heap.stats();
        assert!(!heap.deallocate(a, layout(64, 16)));
        assert!(!heap.deallocate(a + 16, layout(16, 16)));
        assert!(!heap.deallocate(a, layout(16, 16)));
        assert_eq!(heap.stats(), before);
        assert!(heap.deallocate(a, layout(32, 16)));
        assert!(!heap.deallocate(a, layout(32, 16)));
        assert!(heap.deallocate(b, layout(32, 16)));
    }
    #[test]
    fn mixed_fragmentation_workload_preserves_nonoverlap_and_full_recovery() {
        let mut heap = Heap::new();
        for index in 0..4 {
            assert!(heap.add_region(0x100000 + index * REGION_BYTES));
        }
        let mut live = [None; 128];
        let mut seed = 0x12345678u64;
        for _ in 0..2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let index = seed as usize % live.len();
            if let Some((address, spec)) = live[index].take() {
                assert!(heap.deallocate(address, spec));
            } else {
                let spec = layout(
                    1 + ((seed >> 16) as usize % 6000),
                    1 << ((seed >> 32) as usize % 13),
                );
                if let Some(address) = heap.allocate(spec) {
                    for (other, other_spec) in live.iter().flatten() {
                        assert!(
                            address + spec.size().div_ceil(GRANULE) * GRANULE <= *other
                                || *other + other_spec.size().div_ceil(GRANULE) * GRANULE
                                    <= address
                        );
                    }
                    live[index] = Some((address, spec));
                }
            }
        }
        for (address, spec) in live.into_iter().flatten() {
            assert!(heap.deallocate(address, spec));
        }
        assert_eq!(heap.stats().live_bytes, 0);
        assert_eq!(heap.allocate(layout(4 * REGION_BYTES, 16)), Some(0x100000));
    }

    #[test]
    fn refuses_overlap_overflow_and_region_exhaustion() {
        let mut heap = Heap::new();
        assert!(!heap.add_region(0));
        assert!(!heap.add_region(1));
        assert!(!heap.add_region(usize::MAX & !4095));
        for index in 0..MAX_REGIONS {
            assert!(heap.add_region(0x100000 + index * REGION_BYTES));
        }
        assert!(!heap.add_region(0x100000));
        assert!(!heap.add_region(0x900000));
        assert!(
            heap.allocate(layout(MAX_REGIONS * REGION_BYTES, 16))
                .is_some()
        );
        assert!(heap.allocate(layout(1, 1)).is_none());
    }
}
