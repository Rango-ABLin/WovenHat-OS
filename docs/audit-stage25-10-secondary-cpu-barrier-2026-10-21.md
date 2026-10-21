# Stage 25.10 audit - secondary-CPU suspend barrier

Date: 2026-10-21

## Scope

This stage adds the CPU-coordination prerequisite needed before a future BSP
PM1 S3 write. It does not enter a firmware sleep state.

## Implementation

`kernel/src/smp.rs` adds a private `CpuSuspendBarrier`. The BSP snapshots the
online mask, requests each online secondary CPU to evacuate scheduler-owned
work, and waits for the existing CPU-owned hotplug checkpoint to park it. The
barrier records the exact completed set. A failed request immediately attempts
to restore the completed prefix. Rollback attempts every parked CPU in reverse
order and verifies that the original online mask is restored.

`power::prepare_suspend` now takes this barrier after driver quiescence,
wake-trampoline staging, CPU-context capture, and FACS-vector installation.
Failure unwinds the vector, trampoline, CPU barrier, and driver plan. Abort
restores the vector and all parked CPUs before completing driver rollback.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.10-1cpu-1792575579462473700`
- 2 CPUs: `audit-artifacts/stage25.10-2cpu-1792575623913254100`
- 4 CPUs: `audit-artifacts/stage25.10-4cpu-1792575642628049000`

## Security and concurrency review

Only the BSP can create the barrier. Scheduler evacuation and CPU ownership
changes use the previously accepted hotplug checkpoint and lock ordering;
the new power path does not take a scheduler lock directly or sleep under an
IRQ lock. Partial failure is bounded and attempts every cleanup operation.
The private token prevents an unrelated caller from re-onlining or altering a
different CPU set.

## Limits and prerequisites

The barrier parks secondary CPUs but does not save their architectural
registers, restore them after firmware wake, write PM1 S3 controls, or provide
firmware wake handling. Those remain required before a suspend action or a
production hardware claim.
