# Stage 25.9 audit - transactional FACS wake-vector installation

Date: 2026-10-21

## Scope

This stage closes the next bounded prerequisite between wake-trampoline
staging and a future S3 transition: ownership of the ACPI FACS waking-vector
mutation. It does not request a power transition.

## Implementation

`kernel/src/hal/acpi.rs` adds a private `FacsWakeVector` owner token and
`install_facs_wake_vector`/`restore_facs_wake_vector`. Discovery already
validates the FACS signature, minimum length, and physical memory range. The
install path uses that validated direct map, copies the existing 32-bit and
64-bit vector values, and writes the reserved low wake-page address to both
forms with volatile stores while interrupts are disabled. A small lock
serializes the firmware-field mutation. The token restores the exact previous
values on abort.

`power::prepare_suspend` now stages the trampoline and then installs the
vector. Any vector failure releases the trampoline and resumes drivers.
`abort_suspend` restores the vector before releasing the trampoline and
resuming drivers. The byte-level self-test rejects targets that cannot fit in
the 32-bit field and verifies installation and rollback without modifying
firmware memory.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.9-1cpu-1792575149733404100`
- 2 CPUs: `audit-artifacts/stage25.9-2cpu-1792575203237815100`
- 4 CPUs: `audit-artifacts/stage25.9-4cpu-1792575220341503800`

## Security and concurrency review

Only firmware memory already validated by ACPI discovery is written. The
private token owns the previous values, preventing a caller from supplying an
arbitrary restore address or userspace pointer. The mutation is serialized and
interrupt-safe. Preparation remains private, so no untrusted process can
request a firmware vector write. Driver and trampoline rollback remains
failure-ordered and explicit.

## Limits and prerequisites

The OS still does not expose suspend, coordinate or stop secondary CPUs,
restore CPU registers, write PM1 S3 controls, perform firmware wake, or claim
physical suspend/resume qualification. Those must be implemented and tested
before production release.
