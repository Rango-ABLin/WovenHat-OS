# Stage 10.8 Implementation Guide: Unified Completion Port / Wait-Many

## Overview

Stage 10.8 implements a unified asynchronous I/O completion mechanism - a per-process completion port that allows applications to wait for many async operations simultaneously. This stage provides the architectural foundation for everything that follows: GUI event loops, server event handling, and efficient multi-operation polling.

**Status**: Implemented and passing validation gate
**Feature gate**: `stage10-8-test`
**Syscalls**: 82-86
**Test marker**: `[S10.8] completion ports + batch/cancel/timeout/teardown/SMP: PASSED`

---

## Architecture

### Completion Port Model

```
Application (Ring 3)
        |
Multiple Async Operations (file read, TCP recv, timer, etc.)
        |
CompletionPort (per-process object)
        |
Ring 0: Event Queue + Waiter List + Consumer Claim
        |
Events published by operation producers
        |
Application polls/waits and dequeues batch
```

### Design Constraints (Bounded by Construction)

- **32 global ports**: Fixed table, generation-tagged handles
- **4 ports per process**: Bounded per-caller limit
- **32 ready/reserved slots per port**: Fixed queue capacity
- **BATCH = 8 events**: Maximum dequeue in one syscall
- **No unbounded allocation**: All sizes compile-time constants

### Handle Format (Generation-Tagged)

```
Bits 0-15:   Slot index (0-31)
Bits 16-31:  Tag (0xc001) - validates handle type
Bits 32-63:  Generation (1-u32::MAX)
```

Generation counter increments on each allocation; generation 0 is never used. Exhausted generations retire slots permanently rather than wrapping.

---

## Syscall Interface

### Syscall 82: CompletionPortCreate

**Parameters**: None
**Returns**: Port handle (u64) or u64::MAX on error

Creates a completion port owned by the current process.

### Syscall 83: CompletionPortClose

**Parameters**: rdi = port handle
**Returns**: 0 on success, u64::MAX on error

Closes an owned completion port. Wakes all waiters.

### Syscall 84: CompletionPortAssociate

**Parameters**: rdi = async op, rsi = port, rdx = cookie
**Returns**: 0 on success, u64::MAX on error

Associates an async operation with a port.

### Syscall 85: CompletionPortPoll

**Parameters**: rdi = port, rsi = destination, rdx = capacity (1-8)
**Returns**: Number of events, u64::MAX on error

Nonblocking poll of the port.

### Syscall 86: CompletionPortWait

**Parameters**: rdi = port, rsi = destination, rdx = options pointer
**Returns**: Number of events, u64::MAX on error

Blocks until at least one event is available or timeout expires.

---

## Implementation Details

### Key Module Files

- `kernel/src/completion_port.rs` — Per-process port management
- `kernel/src/completion_queue.rs` — Ring-buffer event queue
- `kernel/src/async_op.rs` — Operation lifecycle and association
- `kernel/src/syscall.rs` — Syscall 82-86 handlers

### Lock Ordering

```
Allocation Lock (rank 10)
    |
Operation Table Lock (rank 10)
    |
Port Table Lock (rank 20, per port)
    |
Scheduler (only at waiter wake time, outside port lock)
```

Critical: Never hold a port lock while calling the scheduler.

### Association Flow

When an operation is associated with a port:

1. Lock operation table (verify ownership, not already associated)
2. Lock port (reserve capacity)
3. Store port handle in operation
4. If operation is already complete, immediately publish event
5. Release locks in reverse order
6. Wake waiters (outside all locks)

### Completion Event Publishing

When an operation completes:

1. Lock operation table
2. Set operation state to Complete
3. If associated with a port, publish event to queue
4. Return waiter list and port waiters
5. Release operation table lock
6. Wake all waiters (outside locks)

---

## Testing

### Unit Tests (Host-side)

`tests/completion_queue.rs` tests:
- Queue reserve/publish/remove
- Sequence numbering
- Event encoding

### Integration Test (QEMU)

Ring 3 program (`kernel/src/stage10_8.S`) exercises:

1. Port creation (syscall 82)
2. Operation association (syscall 84)
3. Batch dequeue (syscall 85 with capacity 1-8)
4. Timeout wait (syscall 86 with deadline)
5. Cancellation (syscall 67 on associated ops)
6. Process teardown (exit with active ports)
7. SMP safety (multi-CPU execution)

**Test marker**: `[S10.8] completion ports + batch/cancel/timeout/teardown/SMP: PASSED`

---

## Bounded-by-Construction Proofs

### No unbounded growth

- 32 global ports, 4 per process, 32 events per port (all compile-time)
- No dynamic allocation
- Generation exhaustion retires slots

### Fail-closed

- Invalid handle → u64::MAX immediately
- Full port → rejects association
- Bad capacity → error
- Timeout overflow → error
- Copy failure → events stay queued for retry

### SMP safety

- Allocation lock guards global table
- Per-port IRQsafe lock
- Operation table → port lock ordering
- Waker notification outside all locks
- Consumer claim prevents concurrent dequeue

### Teardown safety

- Process teardown under scheduler lock
- All owned ports cleared deterministically
- No recursive scheduler acquisition
- Waiter list fully cleaned

---

## Integration for Stage 10.9+

Before depending on completion ports:

- [ ] Host unit tests pass
- [ ] Kernel builds with `-D warnings`
- [ ] QEMU 1/2/4 CPU gate passes
- [ ] Teardown tested (normal exit, termination)
- [ ] Port full behavior verified
- [ ] Timeout edge cases validated
- [ ] Canceled op behavior correct
- [ ] SMP contention tested
- [ ] No orphaned ports after exit
- [ ] FIFO event ordering preserved

---

## Building

```bash
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage10-8-test -- -D warnings
cargo test
python scripts/test-feature-qemu.py --feature stage10-8-test --name s10.8-ports --require "[S10.8]" --firmware path/to/ovmf.fd --cpus 1
```

---

## See Also

- `docs/completion-port-abi.md` — ABI specification
- `docs/master-development-roadmap.md` — Stage roadmap
- `kernel/src/completion_port.rs` — Implementation
- `kernel/src/async_op.rs` — Operation lifecycle

