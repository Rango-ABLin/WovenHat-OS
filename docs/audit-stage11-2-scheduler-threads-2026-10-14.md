# Stage 11.2 scheduler-backed thread increment — 2026-10-14

This audit records the production-safety increment built on the earlier
generation-safe thread registry. It does not close the complete Stage 11
roadmap: shared libraries, symbol resolution, dynamic TLS, and hardware
qualification remain open.

## Implemented boundary

- Scheduler task control blocks now carry stable process ownership and an
  optional generation-safe thread ID.
- `ThreadCreate` validates the entry against an executable mapping, creates a
  private NX stack, copies the process's bounded initial-exec TLS image, and
  schedules the new task in the owner's address space.
- `ThreadExit` latches the exit status in the TCB and publishes registry
  termination only after the scheduler has switched away from the exiting
  thread. This prevents a joiner from unmapping a stack that is still in use.
- `ThreadJoin` is owner-checked and retains exited records/resources until the
  owner joins them; join then unmaps the private stack/TLS mappings before the
  TCB and generation slot are recycled.
- `ThreadSelf`, `ThreadSetTls`, and `ThreadGetTls` expose the bounded ABI
  required by the current initial-exec TLS model.
- Kernel-task identity remains compatible with existing sandbox and scheduler
  self-tests; user-task identity is process-stable for shared-address-space
  lookup.
- A joiner now registers with the thread record before blocking. Thread exit
  wakes that task through the scheduler's latched event path after safe
  hand-off, eliminating userspace polling and closing the registration/exit
  race.
- Process-wide termination now retires ready/blocked siblings immediately,
  requests scheduler-safe termination for siblings executing on another CPU,
  releases per-task async ownership, and permits all resulting TCBs to be
  reclaimed after the address space remains alive through parent reaping.

## Validation

The following gates passed from the repository root with project-local Cargo
build directories:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-2-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test   (all host integration/unit suites in the Stage 11.2 launcher)
```

The launcher also passed the Ring-3 create/exit/join acceptance on all target
CPU counts, with exit code 33 and retained serial logs:

```text
audit-artifacts/stage11.2-1cpu-1791975128090915200
audit-artifacts/stage11.2-2cpu-1791975197249865000
audit-artifacts/stage11.2-4cpu-1791975247102041500

The event-driven join follow-up reran the same gate and retained these newer
serial artifacts:

```text
audit-artifacts/stage11.2-1cpu-1791975427938656800
audit-artifacts/stage11.2-2cpu-1791975491088108800
audit-artifacts/stage11.2-4cpu-1791975537755343700

The process-teardown follow-up reran the same gate and retained these newer
serial artifacts:

```text
audit-artifacts/stage11.2-1cpu-1791975759656374100
audit-artifacts/stage11.2-2cpu-1791975829411200100
audit-artifacts/stage11.2-4cpu-1791975876876280500

The registry self-test now directly covers waiter registration, termination,
waiter hand-off, and join status. The resulting regression gate retained:

```text
audit-artifacts/stage11.2-1cpu-1791976051779252200
audit-artifacts/stage11.2-2cpu-1791976117458262700
audit-artifacts/stage11.2-4cpu-1791976165640693800
```
```
```
```

The image emitted the existing lower-stage markers and the new
`[S11.2] scheduler-backed thread: PASSED` marker in each serial log.

## Explicit remaining limits

This increment is not a claim that all Stage 11 requirements are complete.
Join currently exposes a bounded nonblocking retry result rather than a
fully event-driven wait queue. Process-wide sibling cancellation and teardown
of unjoined thread records still need a dedicated lifecycle pass. Shared
libraries, symbol-based relocations, dynamic TLS/module IDs, and physical
hardware qualification remain fail-closed or unqualified.

## 2026-10-15 sibling-cancellation follow-up

The scheduler-backed thread boundary now includes append-only syscall 136,
`ThreadCancel`. A process may cancel an owned sibling with an explicit exit
status. Ready, blocked, and sleeping targets are retired while the scheduler
lock protects selection metadata; running and switching targets carry the
request to the existing post-switch finalizer. Both paths transition the
generation-tagged registry exactly once, wake the registered join waiter, and
leave the thread record/resources available for `ThreadJoin` to unmap and
reclaim. Canceling the current thread, a foreign handle, an exited handle, or
an invalid generation is rejected.

The Stage 11.2 Ring-3 image now exercises normal create/exit/join followed by
create/cancel/join with status 23. The focused gate passed on 1, 2, and 4
CPUs; serial evidence is retained at:

```text
audit-artifacts/stage11.2-1cpu-1792053325191002600
audit-artifacts/stage11.2-2cpu-1792053407390931300
audit-artifacts/stage11.2-4cpu-1792053457047808100
```

Additional validation passed:

```text
cargo check -p wovenhat-os --all-targets
cargo clippy -p wovenhat-os --all-targets -- -D warnings
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-2-test -- -D warnings
cargo test -p wovenhat-os --tests --no-fail-fast
```

This closes sibling cancellation for the bounded scheduler-backed thread
increment. It does not claim process-wide teardown of unjoined thread records,
shared-library loading, cross-image symbol resolution, dynamic TLS/module IDs,
or physical hardware qualification.
