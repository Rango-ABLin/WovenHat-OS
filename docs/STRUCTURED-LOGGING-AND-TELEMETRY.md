# Structured Logging and Telemetry Infrastructure

## Overview

The WovenHat-OS kernel provides a zero-cost abstraction for structured logging and
performance telemetry. This infrastructure enables:

- Compile-time log level filtering (Error, Warn, Info, Debug, Trace)
- Structured key-value logging for easier analysis
- Performance telemetry hooks for syscall and IPC latency tracking
- Ring-buffered diagnostics for post-crash analysis
- Zero overhead when disabled (inlined away at compile time)

## Architecture

### Log Levels

```rust
pub enum LogLevel {
    Error = 0,   // Always enabled
    Warn = 1,    // Compile-time configurable
    Info = 2,    // Compile-time configurable
    Debug = 3,   // Debug builds only
    Trace = 4,   // Trace builds only
}
```

### Compile-Time Filtering

Log levels above the configured threshold are eliminated entirely by the compiler:

```rust
// With log level set to Info:
log_debug!("module", "This message is compiled away");
log_info!("module", "This message is included");
```

### Ring Buffer for Diagnostics

The kernel maintains a 64 KB ring buffer for diagnostic records:

- Survives kernel crashes for post-mortem analysis
- No performance overhead in normal operation
- Accessible via QEMU debugging or crash dumps

### Telemetry Metrics

Three categories of metrics are tracked:

#### Syscall Metrics
```rust
pub struct SyscallMetrics {
    pub total_syscalls: AtomicU64,
    pub total_latency: AtomicU64,
    pub max_latency: AtomicU64,
}
```

Measures:
- Total syscall count
- Sum of all latencies
- Maximum single syscall latency

#### Scheduling Metrics
```rust
pub struct SchedulingMetrics {
    pub context_switches: AtomicU64,
    pub total_run_time: AtomicU64,
    pub total_wait_time: AtomicU64,
}
```

Measures:
- Context switch frequency
- Total time spent in runnable state
- Total time spent waiting

#### IPC Metrics
```rust
pub struct IpcMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub total_latency: AtomicU64,
}
```

Measures:
- Message volume
- IPC latency accumulation

## Usage

### Logging

Use the compile-time macros for structured logging:

```rust
log_error!("file_io", "Failed to read sector");
log_warn!("memory", "Approaching 80% capacity");
log_info!("task", "Task 42 exited");
log_debug!("smp", "CPU 2 handling timer interrupt");
log_trace!("scheduler", "Selecting next task");
```

### Telemetry Recording

Record telemetry events for performance analysis:

```rust
// Create telemetry event
let event = TelemetryEvent {
    kind: "syscall:read",
    duration_ticks: end_ticks - start_ticks,
    cpu: cpu_id,
};

registry.telemetry(event);
```

### Metrics Access

Query metrics for monitoring:

```rust
let avg_latency = registry.syscalls.average_latency();
let ctx_switches = registry.scheduling.context_switches.load(Ordering::Relaxed);
let messages = registry.ipc.messages_sent.load(Ordering::Relaxed);
```

## Feature Flags

Enable specific log levels with Cargo features:

```toml
[features]
log-error = []      // Always on
log-warn = []       // Warn and above
log-info = []       // Info and above
log-debug = []      // Debug and above
log-trace = []      // Trace (verbose)
telemetry-verbose = [] // Detailed telemetry output
```

## Performance Characteristics

### Compile-Time Elimination

When a log level feature is not enabled:
- Macro call is inlined to nothing
- Zero instruction overhead
- No memory usage
- No runtime branch prediction pollution

### Runtime Overhead (When Enabled)

When enabled:
- Serial write: ~1000 CPU cycles per log message
- Ring buffer: ~50 CPU cycles per record
- Telemetry: ~20 CPU cycles per atomic operation

### Memory Usage

- Ring buffer: 64 KB (fixed, constant-time allocation)
- Metrics: 64 bytes per metric set
- No unbounded allocations

## Integration Points

### Syscall Handler Path

When implementing a syscall handler:

```rust
pub fn syscall_read(fd: u32, buffer: *mut u8, count: usize) -> Result<usize, i32> {
    let start = crate::timer::getticks();
    
    // ... syscall implementation ...
    
    let duration = crate::timer::getticks() - start;
    TELEMETRY.syscalls.record(duration);
    
    result
}
```

### Task Scheduling

In the scheduler:

```rust
fn context_switch(to_task: TaskId) {
    TELEMETRY.scheduling.record_context_switch();
}

fn accumulate_run_time(task: TaskId, ticks: u64) {
    TELEMETRY.scheduling.add_run_time(ticks);
}
```

## Best Practices

### Log Level Selection

- Error: Unexpected failures, resource errors
- Warn: Retries, resource pressure
- Info: Major transitions, milestone events
- Debug: Routine operations, state changes
- Trace: Every branch, all values (verbose)

### Telemetry Naming

Use hierarchical naming:

```rust
kind: "syscall:completion_port:wait"  // category:subsystem:operation
```

## Post-Crash Diagnostics

After a kernel panic or crash:

1. Boot into recovery mode (via QEMU debugger)
2. Access ring buffer at kernel symbol LOGGER.diagnostics.buffer
3. Parse records using the 40-byte structure
4. Extract last N records for analysis

## Testing

### Unit Tests

Test metric collection:

```bash
cargo test -p wovenhat-kernel structured_logging
cargo test -p wovenhat-kernel telemetry_hooks
```

### Integration with Acceptance Gates

Telemetry metrics are recorded throughout stage acceptance testing.

## Migration Guide

### From printf-style Logging

Before:
```rust
serial::write_line(format_args!("Task {} exited", id));
```

After:
```rust
log_info!("task", format_args!("Task {} exited", id));
```
