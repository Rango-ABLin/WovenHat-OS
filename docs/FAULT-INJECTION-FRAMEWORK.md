# Deterministic Fault Injection Framework

## Overview

The deterministic fault injection framework enables systematic testing of kernel
robustness by injecting faults in a predictable, repeatable manner. All injection
points are seeded with deterministic RNG for reproducible test scenarios.

## Design Principles

1. **Deterministic** - Same seed produces identical fault sequences
2. **Localized** - Faults injected at specific kernel points only
3. **Recoverable** - Tests validate graceful fault handling
4. **Measurable** - Coverage tracking for fault scenarios
5. **Transparent** - No impact when fault injection disabled

## Architecture

### Fault Kinds

Eight categories of injectable faults:

- AllocationFailure: Memory allocation returns null
- TimeoutExpired: Operation exceeds deadline
- DataCorruption: Data read from storage is corrupt
- CpuNotReady: CPU online/offline at unexpected time
- LockContentionHigh: Lock acquisition always contested
- MemoryPressure: System memory pressure escalates
- InterruptDropped: Hardware interrupt is missed
- DeadlineExceeded: Time-based operation runs out of time

### DeterministicFaultInjector

Central fault injection controller:

```rust
let mut injector = DeterministicFaultInjector::new(seed);

// Register fault points
let alloc_fault = injector.register_fault_point(
    FaultKind::AllocationFailure
)?;

// Enable fault after N iterations
injector.enable_fault(alloc_fault, 100)?;

// Check if should inject
if injector.should_inject(alloc_fault) {
    return Err(-12); // ENOMEM
}
```

### FaultInjectionTestCase

Predefined test scenarios:

```rust
let mut test = FaultInjectionTestCase::new(
    "scenario_1",
    0x1234_5678
);

test.add_fault(fault_id_1, 50)?;   // Trigger at 50
test.add_fault(fault_id_2, 200)?;  // Trigger at 200
test.run(&mut injector)?;
```

### FaultCoverage

Measures fault recovery effectiveness:

```rust
let mut coverage = FaultCoverage::new();
coverage.faults_triggered += 1;

if recovered {
    coverage.record_recovery();
} else {
    coverage.record_failure();
}

coverage.print_report();
// Output: Triggered: 1, Recovered: 1, Fatal: 0, Rate: 100%
```

## Fault Injection Points

Typical locations for fault injection:

### Memory Allocation

```rust
pub fn allocate(size: usize) -> Result<*mut u8, i32> {
    if injector.should_inject(ALLOC_FAULT_ID) {
        return Err(-12); // ENOMEM
    }
    
    unsafe { buddy_alloc(size) }
}
```

### Syscall Entry

```rust
pub fn syscall_handler(num: u32) -> i64 {
    if injector.should_inject(SYSCALL_FAULT_ID) {
        return -22; // EINVAL
    }
    
    handle_syscall(num)
}
```

### Async Operation

```rust
pub fn spawn_operation(op: Operation) -> Result<u64, i32> {
    if injector.should_inject(ASYNC_FAULT_ID) {
        return Err(-110); // ETIMEDOUT
    }
    
    spawn_async(op)
}
```

### Lock Acquisition

```rust
pub fn acquire_lock(lock: &Mutex) {
    if injector.should_inject(LOCK_FAULT_ID) {
        // Simulate high contention
        for _ in 0..1000 {
            spin();
        }
    }
    
    lock.acquire();
}
```

### I/O Operation

```rust
pub fn read_sector(lba: u64, buf: &mut [u8]) -> Result<(), i32> {
    if injector.should_inject(CORRUPTION_FAULT_ID) {
        // Corrupt data in buffer
        buf[0] ^= 0xff;
    }
    
    disk_read(lba, buf)
}
```

## Test Scenarios

### Scenario 1: Allocation Stress

Test allocation failure handling:

```rust
let seed = 0x_dead_beef;
let mut injector = DeterministicFaultInjector::new(seed);

let alloc_id = injector.register_fault_point(
    FaultKind::AllocationFailure
)?;

// Fail every 10th allocation
injector.enable_fault(alloc_id, 10)?;

// Run test suite
run_tests();

// Verify recovery rate
let stats = FaultInjectionStats::from_injector(&injector);
assert!(stats.total_faults_triggered > 0);
```

### Scenario 2: Timeout Handling

Test timeout tolerance:

```rust
let seed = 0x_cafe_babe;
let mut injector = DeterministicFaultInjector::new(seed);

let timeout_id = injector.register_fault_point(
    FaultKind::TimeoutExpired
)?;

// Timeout at 500ms mark
injector.enable_fault(timeout_id, 500)?;

// Run timeout-sensitive code
run_io_operation();
```

### Scenario 3: Data Corruption

Test data integrity handling:

```rust
let seed = 0x_1337_cafe;
let mut injector = DeterministicFaultInjector::new(seed);

let corrupt_id = injector.register_fault_point(
    FaultKind::DataCorruption
)?;

// Corrupt at operation 20
injector.enable_fault(corrupt_id, 20)?;

// Run storage operations
run_storage_test();
```

### Scenario 4: Combined Faults

Test resilience to multiple simultaneous faults:

```rust
let seed = 0x_cafe_dead;
let mut injector = DeterministicFaultInjector::new(seed);

let alloc_id = injector.register_fault_point(
    FaultKind::AllocationFailure
)?;
let timeout_id = injector.register_fault_point(
    FaultKind::TimeoutExpired
)?;

// Both faults active
injector.enable_fault(alloc_id, 50)?;
injector.enable_fault(timeout_id, 100)?;

run_stress_test();
```

## Reproducibility

All test runs are reproducible using seed:

```
Test Run 1: seed=0x1234_5678
  Fault 0: triggered at 100
  Fault 1: triggered at 250
  Result: PASS

Test Run 2: seed=0x1234_5678
  Fault 0: triggered at 100  (identical)
  Fault 1: triggered at 250  (identical)
  Result: PASS

Test Run 3: seed=0x_abcd_ef00
  Fault 0: triggered at 173
  Fault 1: triggered at 287
  Result: PASS
```

Seeds saved with test results enable exact reproduction.

## Coverage Metrics

Track recovery rate for each fault kind:

| Fault Type | Triggered | Recovered | Fatal | Rate |
|------------|-----------|-----------|-------|------|
| AllocationFailure | 245 | 245 | 0 | 100% |
| TimeoutExpired | 89 | 87 | 2 | 97% |
| DataCorruption | 56 | 56 | 0 | 100% |
| LockContention | 134 | 134 | 0 | 100% |
| **Total** | **524** | **522** | **2** | **99.6%** |

Fatal faults indicate uncaught error paths.

## Integration with Acceptance Gates

Fault injection runs during stage testing:

```bash
# Run with fault injection enabled
cargo test --features stage10-8-test,fault-injection

# Specific fault scenario
FAULT_SEED=0x1234_5678 cargo test fault_scenario_1
```

## Performance Impact

Fault injection overhead:

- No injections enabled: 0% overhead
- Injections enabled: ~1% overhead (RNG check only)
- Fault triggered: Full error path cost

## Best Practices

1. **Start Simple**: Test single fault type first
2. **Iterate Seeds**: Try multiple random seeds
3. **Measure Coverage**: Track recovery rate per fault
4. **Reproduce Failures**: Save seed for any failure
5. **Document Scenarios**: Name and document each test
6. **Validate Recovery**: Verify system state after fault

## Future Enhancements

Potential improvements:

1. **Probability Distribution**: Weight fault frequency
2. **Cascading Faults**: Inject secondary faults
3. **Recovery Verification**: Automated state validation
4. **Fault Fingerprinting**: Identify uncaught error patterns
5. **Adaptive Injection**: Increase frequency of weak areas
