# Stage 31 process-crash containment audit — 2026-10-22

Added a feature-gated Ring-3 probe that writes to an unmapped user address.
The kernel's user-fault path must terminate only that process, preserve the
kernel, and expose the signed fault status to the waiter.

Verification:

- `cargo check -p wovenhat-kernel --features stage31-process-crash-test --target x86_64-unknown-none`: passed.
- Warning-denying kernel Clippy for `stage31-process-crash-test`: passed.
- Stage 31 process-crash QEMU gate: passed on 1/2/4 CPUs, exit 33.
- Each serial log records a user page fault at `0x400000300000` and
  `[S31.PROCESS-CRASH] faulting process contained and reaped: PASSED status=-11`.

Evidence is retained under
`target/stage31-process-crash-{1,2,4}cpu`.

This covers isolated process-fault containment only. Driver crashes,
service restart, packet corruption, sudden shutdown, and physical hardware
faults remain separate Stage 31 scenarios.
