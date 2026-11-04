# Multi-Agent Coordination Protocol

Version 1.0 | Date: 2026-11-02 | Status: Phase 1 Task 1.3

## Overview

The WovenHat-OS multi-agent coordination system enables two or more agents 
(Claude Code, Codex, etc.) to work in parallel on the same codebase without conflicts.

## Key Components

### 1. CoordinationDashboard
- 8 concurrent agents, 32 tasks max
- Agent states: Idle, Working, Waiting, Blocked, Complete, Error
- Task states: Queued, Assigned, InProgress, Completed, Failed, Blocked

### 2. CollisionDetector
- Exclusive file access control
- try_acquire(file): Lock file
- release(file): Unlock file
- is_in_use(file): Check lock status

### 3. WorkloadBalancer
- Least-busy agent assignment
- Per-agent load tracking
- Fair distribution algorithm

### 4. CriticalFileLockRegistry
Protected files (12+):
- kernel/src/memory_safety_audit.rs
- kernel/src/telemetry_hooks.rs
- kernel/src/structured_logging.rs
- kernel/src/bounded_capacity_analyzer.rs
- kernel/src/deterministic_fault_injection.rs
- kernel/src/compiler_fuzzing.rs
- kernel/src/qemu_perf_regression.rs
- kernel/src/agent_coordination.rs
- docs/agent-coordination.md
- docs/stage-status.md
- Cargo.toml
- kernel/Cargo.toml

## Task Execution Workflow

1. Agent registers -> Status: Idle
2. Task created with optional dependencies (up to 4)
3. Task assigned to agent -> Status: Assigned
4. File lock acquired (collision detection)
5. All dependencies checked (must be Completed)
6. Task execution -> Status: InProgress
7. Files released after work done
8. Task marked completed -> Status: Completed

## Dependency Resolution

- all_dependencies_complete(): Verify prerequisites
- can_start_task(): Check readiness
- find_next_ready_task(): Priority-ordered selection

## Deadlock Detection

detect_deadlock() returns true when:
- Idle agents exist
- No executable tasks available
- Queued tasks have unsatisfiable dependencies

## Priority Scheduling

Higher priority value = earlier execution
- Critical: 200+
- High: 150-199
- Normal: 50-149
- Low: 1-49

## Health Monitoring

- Agent heartbeat: every 30 seconds
- Timeout: 2 minutes (120 seconds)
- Task timeout: 2 hours default
- Error recovery: Task reassignment to healthy agent

## Testing

Unit Tests:
- Collision detection
- Dependency resolution
- Workload balancing
- Task state transitions
- Deadlock detection

Integration Tests:
- Sequential execution (file lock blocking)
- Parallel independent tasks
- 4-dependency chains
- Agent failure and recovery

## Configuration

Enable coordination via feature flags:

```toml
[features]
stage_11 = ["agent-coordination"]
```

Compile:
```bash
cargo build -p wovenhat-kernel --target x86_64-unknown-none --features stage_11
```

---

Document Version: 1.0
Last Updated: 2026-11-02
Next Review: 2026-11-16 (Task 1.3 completion)
