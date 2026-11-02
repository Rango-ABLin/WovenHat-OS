# Memory Safety Audit Tooling Framework

## Overview

The WovenHat-OS kernel provides comprehensive audit tooling for verifying memory
safety properties at runtime. This framework helps detect violations of:

- Userspace pointer validation requirements
- Async data pinning before use
- Complete owner resource teardown
- Handle generation tracking
- Bounded object capacity
- Lock ordering constraints
- Cross-CPU race conditions

## Architecture

### Core Audit Results

```rust
pub enum AuditResult {
    Pass,
    Fail(AuditError),
    Warn(AuditWarning),
}
```

All auditors return standardized results with categorized errors and warnings.

## Audit Components

### 1. Userspace Pointer Validator

Verifies that all pointers passed from userspace are:
- In the valid userspace address range (0x400000 - 0x80000000 typical)
- Properly validated before use
- Not crossing kernel/userspace boundaries

```rust
let mut validator = UserspacePointerValidator::new();
let ptr = /* from syscall argument */;
match validator.validate(ptr, length) {
    AuditResult::Pass => { /* safe to use */ },
    AuditResult::Fail(AuditError::UnvalidatedUserPointer) => { /* error */ },
    _ => {}
}
```

### 2. Async Data Pin Tracker

Ensures data is pinned before async use:

```rust
let mut tracker = AsyncDataPinTracker::new();
tracker.pin_for_async();
if !tracker.is_pinned() {
    return AuditResult::Fail(AuditError::AsyncDataNotPinned);
}
```

### 3. Owner Teardown Tracker

Verifies complete cleanup of owner resources:

```rust
let mut tracker = OwnerTeardownTracker::new();
tracker.record_allocation();  // +1
tracker.record_cleanup();     // -1
assert_eq!(tracker.audit(), AuditResult::Pass);
```

### 4. Generation Tracker

Tracks handle generations for stale handle detection:

```rust
let gen = GenerationTracker::new();
let handle = gen.allocate_handle();  // Includes generation tag
// Later, verify generation matches before using handle
```

### 5. Capacity Auditor

Enforces bounded object capacity:

```rust
let mut auditor = CapacityAuditor::new(1000);  // 1000 max
auditor.try_allocate(500)?;  // Succeeds
auditor.try_allocate(600)?;  // Fails: would exceed 1000
```

### 6. Lock Ordering Auditor

Detects lock ordering violations:

```rust
let mut auditor = LockOrderingAuditor::new();
let alloc_lock = LockRank(10);
let port_lock = LockRank(20);

auditor.verify_acquire(alloc_lock)?;  // OK: lowest rank
auditor.verify_acquire(port_lock)?;   // OK: higher rank
```

Lock ranks must increase monotonically. Acquiring rank 10 after rank 20 is rejected.

## Memory Safety Properties

### Userspace Pointer Safety

All pointers from userspace must be:

1. **Validated** - Address in 0x400000 - 0x80000000
2. **Bounded** - Length doesn't overflow or extend into kernel space
3. **Recorded** - Tracked by validator for later checks

```rust
pub fn read_from_user(ptr: *const u8, len: usize) -> Result<()> {
    validator.validate(ptr, len)?;  // Audit check
    // Safe to use ptr now
}
```

### Async Data Pinning

Before spawning async tasks using user-provided data:

1. **Pin the data** - Record with tracker
2. **Verify pinning** - Audit ensures it's pinned
3. **Never move** - Compiler enforces (Rust safety)

### Owner Teardown Completeness

When an owner (process) exits:

1. **Record all allocations** - As resources are allocated
2. **Record all cleanups** - As resources are freed
3. **Verify balance** - Audit ensures allocated == freed

## Performance

All audit operations are O(1) or O(n) where n is small (typically <= 16):

- UserspacePointerValidator: O(n) where n <= 16 ranges
- CapacityAuditor: O(1) atomic operations
- LockOrderingAuditor: O(n) where n <= 8 held locks

No allocations or lock acquisitions within auditors.

## Integration Patterns

### Syscall Entry

```rust
pub fn handle_read(fd: u32, buf: *mut u8, count: usize) -> Result<usize> {
    let mut validator = UserspacePointerValidator::new();
    validator.validate(buf, count)?;  // Audit: userspace validation
    
    // Safe to use buf now
    read_file(fd, buf, count)
}
```

### Async Operation Setup

```rust
pub fn spawn_async_operation(data: &[u8]) -> Result<()> {
    let mut tracker = AsyncDataPinTracker::new();
    tracker.pin_for_async();
    
    spawn_task(|| {
        // Safe: data is pinned during task lifetime
    });
    
    Ok(())
}
```

### Resource Cleanup

```rust
pub fn process_exit(pid: u32) {
    let mut teardown = OwnerTeardownTracker::new();
    
    for resource in process.resources.iter() {
        teardown.record_allocation();
    }
    
    // Free all resources
    for resource in process.resources.iter() {
        free(resource);
        teardown.record_cleanup();
    }
    
    assert_eq!(teardown.audit(), AuditResult::Pass);
}
```

### Lock Acquisition

```rust
fn acquire_locks() {
    let mut order = LockOrderingAuditor::new();
    let alloc_rank = LockRank(10);
    let port_rank = LockRank(20);
    
    order.verify_acquire(alloc_rank)?;  // First: rank 10
    order.verify_acquire(port_rank)?;   // Then: rank 20
    // Verify order compliance before deadlock occurs
}
```

## Error Handling

Each auditor reports specific errors:

- **UnvalidatedUserPointer**: Pointer not in userspace range
- **AsyncDataNotPinned**: Data not pinned before async use
- **IncompleteOwnerTeardown**: Resource allocation/cleanup mismatch
- **GenerationNotTracked**: Handle generation exhausted
- **UnboundedCapacity**: Capacity allocation exceeded
- **LockOrderingViolation**: Lock acquired out of order

## Testing

All auditors include comprehensive unit tests:

```bash
cargo test memory_safety_audit --lib
```

Tests verify:
- Validation of in-range and out-of-range pointers
- Teardown tracking with allocations/cleanups
- Capacity overflow detection
- Lock ordering violation detection
- Boundary conditions (0, max, overflow)

## Best Practices

1. **Use auditors at system boundaries** - User input, async spawning, cleanup
2. **Check audit results immediately** - Don't defer error handling
3. **Record allocation/cleanup pairs** - Keep counts synchronized
4. **Define lock ranks in advance** - Document lock ordering rules
5. **Audit early in critical paths** - Fail fast before corruption

## Future Enhancements

Potential improvements:

1. **Per-CPU audit counters** - Aggregate audit statistics
2. **Audit logging** - Record audit failures for analysis
3. **Audit replay** - Reproduce issues with audit trace
4. **Cross-CPU race detection** - Hardware breakpoints on audit state
5. **Symbolic execution** - Verify audit properties statically
