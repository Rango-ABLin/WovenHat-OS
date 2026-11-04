//! Bounded-Object Capacity Analyzer

#![allow(dead_code, unused)]

#[derive(Debug, Clone, Copy)]
pub struct BoundedObjectMetadata {
    pub capacity: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub allocation_failures: u32,
}

impl BoundedObjectMetadata {
    pub const fn new(capacity: usize) -> Self {
        BoundedObjectMetadata {
            capacity,
            current_usage: 0,
            peak_usage: 0,
            allocation_failures: 0,
        }
    }

    pub fn can_allocate(&self, amount: usize) -> bool {
        self.current_usage.saturating_add(amount) <= self.capacity
    }

    pub fn record_allocation(&mut self, amount: usize) -> Result<(), ()> {
        let new_usage = self.current_usage.saturating_add(amount);
        if new_usage > self.capacity {
            self.allocation_failures += 1;
            return Err(());
        }
        self.current_usage = new_usage;
        if self.current_usage > self.peak_usage {
            self.peak_usage = self.current_usage;
        }
        Ok(())
    }

    pub fn record_deallocation(&mut self, amount: usize) {
        self.current_usage = self.current_usage.saturating_sub(amount);
    }

    pub fn utilization_percent(&self) -> u32 {
        ((self.current_usage as u64 * 100) / self.capacity as u64) as u32
    }

    pub fn is_saturated(&self) -> bool {
        self.utilization_percent() >= 90
    }

    pub fn is_full(&self) -> bool {
        self.current_usage >= self.capacity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedObjectType {
    CompletionPort,
    AsyncOperation,
    ProcessDescriptor,
    TaskDescriptor,
    MemoryRegion,
    FileHandle,
    IpcMessage,
    DmaBuffer,
}

pub struct CapacityAnalyzer {
    objects: [BoundedObjectMetadata; 32],
    object_count: usize,
}

impl CapacityAnalyzer {
    pub const fn new() -> Self {
        CapacityAnalyzer {
            objects: [BoundedObjectMetadata::new(0); 32],
            object_count: 0,
        }
    }

    pub fn register(&mut self, capacity: usize) -> Result<usize, ()> {
        if self.object_count >= 32 {
            return Err(());
        }
        let idx = self.object_count;
        self.objects[idx] = BoundedObjectMetadata::new(capacity);
        self.object_count += 1;
        Ok(idx)
    }

    pub fn allocate(&mut self, obj_idx: usize, amount: usize) -> Result<(), ()> {
        if obj_idx >= self.object_count {
            return Err(());
        }
        self.objects[obj_idx].record_allocation(amount)
    }

    pub fn deallocate(&mut self, obj_idx: usize, amount: usize) {
        if obj_idx < self.object_count {
            self.objects[obj_idx].record_deallocation(amount);
        }
    }

    pub fn metadata(&self, obj_idx: usize) -> Option<BoundedObjectMetadata> {
        if obj_idx < self.object_count {
            Some(self.objects[obj_idx])
        } else {
            None
        }
    }

    pub fn find_saturated(&self) -> usize {
        self.objects[..self.object_count]
            .iter()
            .filter(|m| m.is_saturated())
            .count()
    }

    pub fn total_capacity(&self) -> usize {
        self.objects[..self.object_count]
            .iter()
            .map(|m| m.capacity)
            .sum()
    }

    pub fn total_usage(&self) -> usize {
        self.objects[..self.object_count]
            .iter()
            .map(|m| m.current_usage)
            .sum()
    }

    pub fn total_peak(&self) -> usize {
        self.objects[..self.object_count]
            .iter()
            .map(|m| m.peak_usage)
            .sum()
    }

    pub fn total_failures(&self) -> u32 {
        self.objects[..self.object_count]
            .iter()
            .map(|m| m.allocation_failures)
            .sum()
    }
}

pub struct BoundednessValidator;

impl BoundednessValidator {
    pub fn validate(analyzer: &CapacityAnalyzer) -> Result<(), &'static str> {
        if analyzer.object_count == 0 {
            return Err("No bounded objects");
        }

        for metadata in analyzer.objects[..analyzer.object_count].iter() {
            if metadata.capacity == 0 {
                return Err("Zero capacity object");
            }
            if metadata.allocation_failures > 0 {
                return Err("Allocation failures detected");
            }
        }

        Ok(())
    }

    pub fn verify_bounds(meta: BoundedObjectMetadata) -> Result<(), &'static str> {
        if meta.current_usage > meta.capacity {
            return Err("Usage exceeds capacity");
        }
        if meta.peak_usage > meta.capacity {
            return Err("Peak exceeded capacity");
        }
        Ok(())
    }

    pub fn check_saturation(meta: BoundedObjectMetadata) -> Result<(), &'static str> {
        if meta.is_full() {
            Err("Capacity exhausted")
        } else if meta.is_saturated() {
            Err("Near saturation")
        } else {
            Ok(())
        }
    }
}

pub struct CapacityReport {
    pub total_capacity: usize,
    pub total_usage: usize,
    pub peak_usage: usize,
    pub allocation_failures: u32,
    pub saturated_count: usize,
}

impl CapacityReport {
    pub fn from_analyzer(analyzer: &CapacityAnalyzer) -> Self {
        CapacityReport {
            total_capacity: analyzer.total_capacity(),
            total_usage: analyzer.total_usage(),
            peak_usage: analyzer.total_peak(),
            allocation_failures: analyzer.total_failures(),
            saturated_count: analyzer.find_saturated(),
        }
    }

    pub fn utilization_percent(&self) -> u32 {
        if self.total_capacity == 0 {
            return 0;
        }
        ((self.total_usage as u64 * 100) / self.total_capacity as u64) as u32
    }

    pub fn peak_utilization_percent(&self) -> u32 {
        if self.total_capacity == 0 {
            return 0;
        }
        ((self.peak_usage as u64 * 100) / self.total_capacity as u64) as u32
    }
}

pub struct CapacityTrendAnalyzer {
    snapshots: [(usize, usize); 16],
    snapshot_count: usize,
}

impl CapacityTrendAnalyzer {
    pub const fn new() -> Self {
        CapacityTrendAnalyzer {
            snapshots: [(0, 0); 16],
            snapshot_count: 0,
        }
    }

    pub fn snapshot(&mut self, usage: usize, capacity: usize) {
        if self.snapshot_count < 16 {
            self.snapshots[self.snapshot_count] = (usage, capacity);
            self.snapshot_count += 1;
        } else {
            for i in 0..15 {
                self.snapshots[i] = self.snapshots[i + 1];
            }
            self.snapshots[15] = (usage, capacity);
        }
    }

    pub fn is_trending_high(&self) -> bool {
        if self.snapshot_count < 3 {
            return false;
        }

        let recent = &self.snapshots[self.snapshot_count - 3..self.snapshot_count];
        recent
            .iter()
            .all(|(usage, capacity)| (*usage as u64 * 100) / (*capacity as u64) > 70)
    }

    pub fn average_utilization(&self) -> u32 {
        if self.snapshot_count == 0 {
            return 0;
        }

        let sum: u64 = self.snapshots[..self.snapshot_count]
            .iter()
            .map(|(usage, capacity)| {
                if *capacity == 0 {
                    0
                } else {
                    (*usage as u64 * 100) / (*capacity as u64)
                }
            })
            .sum();

        (sum / self.snapshot_count as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let m = BoundedObjectMetadata::new(100);
        assert_eq!(m.capacity, 100);
        assert_eq!(m.current_usage, 0);
    }

    #[test]
    fn test_alloc() {
        let mut m = BoundedObjectMetadata::new(100);
        assert!(m.record_allocation(50).is_ok());
        assert_eq!(m.current_usage, 50);
    }

    #[test]
    fn test_overflow() {
        let mut m = BoundedObjectMetadata::new(100);
        let _ = m.record_allocation(100);
        assert!(m.record_allocation(1).is_err());
    }

    #[test]
    fn test_analyzer() {
        let mut a = CapacityAnalyzer::new();
        let o1 = a.register(1000).unwrap();
        let o2 = a.register(500).unwrap();

        let _ = a.allocate(o1, 500);
        let _ = a.allocate(o2, 250);

        assert_eq!(a.total_usage(), 750);
    }

    #[test]
    fn test_trend() {
        let mut t = CapacityTrendAnalyzer::new();
        t.snapshot(70, 100);
        t.snapshot(75, 100);
        t.snapshot(80, 100);

        assert!(t.is_trending_high());
    }
}
