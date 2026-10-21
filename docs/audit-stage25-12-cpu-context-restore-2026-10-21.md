# Stage 25.12 audit - CPU resume-context restoration primitive

Date: 2026-10-21

## Scope

This stage implements the architectural CPU-state restoration primitive needed
after a future firmware wake. It validates and restores a live kernel context;
it does not itself enter S3 or claim that firmware has resumed the machine.

## Implementation

`CpuResumeContext` is now `#[repr(C)]` and is restored by a documented
non-returning assembly boundary. The sequence disables interrupts, reloads
CR0, CR4, CR3, and EFER, reconstructs packed descriptor pointers for the saved
GDT and IDT, loads both tables, switches to the saved RSP, restores RFLAGS,
and jumps directly to a kernel-owned continuation. The caller validates the
context before entering this unsafe boundary.

The `stage25-12-test` continuation records the success marker and exits the
QEMU test device. It exercises the actual control-register and descriptor
restore instructions using the current live context, rather than only testing
a byte encoder.

## Acceptance evidence

The exact feature and marker passed on all required CPU counts:

- 1 CPU: `audit-artifacts/stage25.12-1cpu-1792577216406089600`
- 2 CPUs: `audit-artifacts/stage25.12-2cpu-1792577279770004200`
- 4 CPUs: `audit-artifacts/stage25.12-4cpu-1792577302737476600`

The exact final tree, including the warning-denying default-feature annotation
pass, was rerun as:

- 1 CPU: `audit-artifacts/stage25.12-1cpu-1792577520657008000`
- 2 CPUs: `audit-artifacts/stage25.12-2cpu-1792577557169550100`
- 4 CPUs: `audit-artifacts/stage25.12-4cpu-1792577578206893300`

## Security and concurrency review

The restore target and continuation are kernel-owned; no userspace pointer or
firmware-provided address is accepted. The context is validated before the
assembly boundary, and interrupts remain disabled until the saved flags are
restored. The sequence does not allocate, lock, or schedule while architectural
state is transient.

## Limits and prerequisites

This stage does not create or consume a persistent firmware-wake record, wire
the low-memory wake entry to a stored suspend token, restore drivers or
secondary CPUs after a real S3 wake, or qualify physical hardware behavior.
