# Stage 25.8 audit - CPU resume-context capture

Date: 2026-10-21

## Scope

This stage implements the next bounded suspend/resume prerequisite: capture
the current CPU state needed by a future wake path and retain it in the private
suspend preparation token. It does not perform a power transition.

## Implementation

`kernel/src/power.rs` adds `CpuResumeContext` with CR0, CR3, CR4, EFER,
RFLAGS, RSP, and GDT/IDT descriptor base and limit fields. The capture uses
privileged architectural reads and small x86_64 inline-assembly reads for
RFLAGS, RSP, SGDT, and SIDT. Validation requires long-mode paging, PAE, a
page-aligned CR3, canonical saved addresses, nonzero descriptor limits, and
the architectural RFLAGS bit.

`prepare_suspend` captures and validates this context after the existing
ACPI, wake-page, driver-quiescence, and wake-trampoline prerequisites. The
context is copied into kernel-owned storage and is not derived from or retained
as a userspace pointer. `abort_suspend` validates the token before teardown.
The `stage25-8-test` feature reports a boot self-test marker without changing
the normal boot path.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.8-1cpu-1792574752351773000`
- 2 CPUs: `audit-artifacts/stage25.8-2cpu-1792574817503473400`
- 4 CPUs: `audit-artifacts/stage25.8-4cpu-1792574865427387800`

The remaining host, kernel Clippy, and full test results are recorded in the
stage commit and final handoff after this audit is completed.

## Security and concurrency review

The context is private, fixed-size, `Copy` data. It contains no userspace
pointer and no firmware address is exposed. Capture occurs on the preparing
CPU before any future transition; this stage does not yet assert that other
CPUs are quiesced, so no multi-CPU resume claim is made beyond the boot
self-test. Existing driver rollback and wake-trampoline ownership remain
transactional.

## Limits and prerequisites

Register restoration, AP/secondary-CPU coordination, FACS vector installation,
wake-source policy, PM1 S3 writes, firmware wake, and physical suspend/resume
qualification remain required before exposing a suspend action or calling the
OS production-ready.
