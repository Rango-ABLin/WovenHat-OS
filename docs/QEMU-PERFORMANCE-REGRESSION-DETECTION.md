# QEMU Performance Regression Detection

## Overview

The performance regression detection framework monitors kernel execution in QEMU,
establishes baselines, detects regressions, and tracks performance trends.
Catches slowdowns before reaching production.

## Components

### PerformanceMetric

Individual measurement (name, value, unit):
- syscall_latency: 1234 ticks
- boot_time: 2500 ms
- memory_alloc: 50 us

### PerformanceBenchmark

Complete benchmark run with up to 8 metrics and timestamp.

### PerformanceBaseline

Reference performance with regression detection:

```rust
baseline.check_regression(current, tolerance_percent) -> bool
baseline.regression_percent(current) -> u32
```

### RegressionDetector

Central detector managing multiple baselines with tolerance setting.

### PerformanceTrend

Track 16 samples with statistics:
- average(): Mean performance
- maximum(): Peak value
- minimum(): Best value
- is_trending_up(): Three recent samples increasing
- is_trending_down(): Three recent samples decreasing

### RegressionThreshold

Alert and fail levels:
- Alert: 5% regression (warning)
- Fail: 15% regression (failure)

## Metrics to Track

### Syscall Performance
- Entry latency (ticks)
- Exit latency (ticks)
- Total latency (ticks)
- Per-syscall breakdown

### Task Scheduling
- Context switch latency (ticks)
- Runqueue traversal (ticks)
- Scheduler overhead (%)
- Wake-up latency (ticks)

### Memory
- Allocation latency (us)
- Deallocation latency (us)
- Page fault handling (ticks)
- TLB shootdown (ticks)

### IPC
- Message send latency (ticks)
- Message receive latency (ticks)
- Endpoint creation (us)
- Endpoint destruction (us)

### Completion Ports
- Port creation (us)
- Operation association (ticks)
- Notification latency (ticks)
- Batch dequeue (ticks)

## Workflow

1. Establish baseline on known-good kernel
2. Run test suite on modified kernel
3. Compare each metric against baseline
4. Generate performance report
5. Alert on regressions, track improvements

## Regression Detection Example

```
Baseline: syscall_latency = 1234 ticks
Current:  syscall_latency = 1456 ticks
Regression: +17.9% (ALERT at 5%, FAIL at 15%)
Result: REGRESSION DETECTED
```

## Tolerance Thresholds

Recommended by metric type:
- Syscall latency: Alert 5%, Fail 15%
- Task switch: Alert 3%, Fail 10%
- Memory alloc: Alert 10%, Fail 25%
- Boot time: Alert 10%, Fail 30%
- IPC latency: Alert 5%, Fail 15%

## Trend Analysis

Three consecutive samples used for trend:
- Trending Up: Each sample higher (performance degrading)
- Trending Down: Each sample lower (performance improving)
- Stable: Consistent performance

## CI/CD Integration

Per-commit: Run perf tests and detect regressions
Nightly: Full performance suite with trend analysis
Weekly: Performance tracking and budget review

## Output Formats

### Text Report
Shows baseline, current, change, and status for each metric

### JSON Report
Machine-parseable results with timestamps and per-metric details

## Performance Budgets

Define acceptable windows:
- syscall_latency < 2000 ticks (16x baseline)
- boot_time < 5000 ms (2x baseline)
- task_switch < 200 ticks (2x baseline)

Violations trigger build failure.

## Best Practices

1. Run on consistent hardware (CPU, RAM, hypervisor)
2. Isolate from noise (disable turbo, frequency scaling)
3. Multiple runs average variance
4. Track trends over time
5. Alert early (5% catches issues)
6. Investigate and bisect fast

## Future Enhancements

1. Automatic bisection to find regressing commit
2. Flamegraph analysis for visual profiling
3. Per-function timing and hotspot identification
4. Statistical analysis with confidence intervals
5. Machine learning for trend prediction
