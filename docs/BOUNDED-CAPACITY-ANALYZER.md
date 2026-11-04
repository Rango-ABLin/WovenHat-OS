# Bounded-Object Capacity Analyzer

## Overview

The bounded capacity analyzer verifies that all kernel objects are bounded by
construction. This enforces: no unbounded growth, all capacities explicit.

## Core Principles

1. Explicit Bounds - Every object declares capacity
2. Runtime Tracking - All allocations checked against bounds
3. No Silent Failures - Allocations fail loudly when exceeded
4. Saturation Detection - Warns approaching capacity
5. Trend Analysis - Detects utilization trends

## Components

### BoundedObjectMetadata

Tracks individual object capacity:

```rust
pub struct BoundedObjectMetadata {
    pub capacity: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub allocation_failures: u32,
}
```

Provides:
- can_allocate(): Check before allocation
- record_allocation(): Track usage
- record_deallocation(): Release usage
- utilization_percent(): Get percent used
- is_saturated(): Check if near full (>90%)
- is_full(): Check if at capacity

### CapacityAnalyzer

Central tracking for bounded objects:

```rust
let mut analyzer = CapacityAnalyzer::new();
let obj1 = analyzer.register(1000)?;
analyzer.allocate(obj1, 500)?;
```

Supports up to 32 bounded objects. Methods:
- register(): Add new object type
- allocate(): Check and record allocation
- deallocate(): Release allocation
- total_capacity(): Sum all capacities
- total_usage(): Sum current usage
- total_peak(): Sum peak usage

### BoundednessValidator

Verifies bounded-by-construction property:

- validate(): All objects have explicit capacity
- verify_bounds(): Usage never exceeds capacity
- check_saturation(): No capacity exhaustion

### CapacityReport

Aggregated statistics across all objects:

```rust
let report = CapacityReport::from_analyzer(&analyzer);
report.utilization_percent()      // Current percent used
report.peak_utilization_percent() // Peak percent used
report.allocation_failures         // Count of failures
report.saturated_count             // Objects near capacity
```

### CapacityTrendAnalyzer

Detects capacity saturation trends:

```rust
let mut trend = CapacityTrendAnalyzer::new();
trend.snapshot(usage, capacity);
if trend.is_trending_high() {
    // Three recent snapshots above 70%
}
```

## Usage Patterns

### Per-Subsystem Tracking

Each kernel subsystem maintains its own analyzer:

```rust
pub struct PortSubsystem {
    analyzer: CapacityAnalyzer,
}

impl PortSubsystem {
    pub fn create_port(&mut self) -> Result<u64, i32> {
        let idx = self.analyzer.register(32)?;
        Ok(idx as u64)
    }
}
```

### Allocation Path

```rust
pub fn associate(subsys: &mut PortSubsystem, op: u64) -> Result<(), i32> {
    subsys.analyzer.allocate(0, 1)
        .map_err(|_| -28)?  // ENOSPC
    
    // Safe to associate
    perform_associate(op)
}
```

### Deallocation Path

```rust
pub fn disassociate(subsys: &mut PortSubsystem, op: u64) {
    subsys.analyzer.deallocate(0, 1);
    perform_disassociate(op);
}
```

### Saturation Monitoring

```rust
pub fn health_check(subsys: &PortSubsystem) {
    let report = CapacityReport::from_analyzer(&subsys.analyzer);
    
    if report.saturated_count > 0 {
        log_info!("health", "Objects near capacity");
    }
    
    if report.allocation_failures > 0 {
        log_error!("health", "Allocation failures!");
    }
}
```

## Bounded Object Types

Typical kernel objects with explicit capacity:

- CompletionPort: 32 ports, 32 associations per port
- AsyncOperation: 256 pending per subsystem
- ProcessDescriptor: 512 max
- TaskDescriptor: 4096 max
- MemoryRegion: Per-process limit
- FileHandle: 256 per process
- IpcMessage: Bounded queue per endpoint

## Performance

### Time Complexity
- register(): O(1)
- allocate(): O(1)
- deallocate(): O(1)
- find_saturated(): O(n) n<=32
- aggregate operations: O(n) n<=32

### Space Complexity
- Per analyzer: 512 bytes
- Per trend analyzer: 256 bytes
- Zero additional allocations

## Verification

### Static Checks

```rust
const CAPACITY: usize = 256;
// Compile-time check that capacity is constant
const _: [(); 1] = [(); CAPACITY];
```

### Runtime Checks

During operation:
- All allocations succeed or return error
- Peak usage never exceeds capacity
- Allocation failures are logged

### Integration with Tests

```bash
cargo test --features stage10-8-test
# Capacity analyzer verifies bounds during test
```

## Error Handling

Allocation failure returns `Err(())`:

```rust
pub fn try_allocate(capacity_analyzer: &mut CapacityAnalyzer) -> Result<(), i32> {
    capacity_analyzer.allocate(0, 1)
        .map_err(|_| {
            // Return error to caller
            // Log for debugging
            -28  // ENOSPC
        })
}
```

## Testing

Comprehensive tests for each auditor:

```bash
cargo test bounded_capacity_analyzer --lib
```

Tests verify:
- Allocation within bounds
- Overflow detection
- Peak tracking
- Saturation calculation
- Trend detection
- Report generation

## Bounded-by-Construction Pattern

Core design for all kernel subsystems:

1. Define capacity constant
2. Create analyzer instance
3. Register with analyzer
4. Check capacity before allocation
5. Verify no allocation failures
6. Monitor saturation

## Best Practices

1. Define all capacities as compile-time constants
2. Register all objects during subsystem init
3. Check capacity on every allocation path
4. Never allocate more than requested
5. Deallocate exactly what was allocated
6. Verify zero allocation failures at shutdown
7. Monitor saturation in health checks

