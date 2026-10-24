# Stage 25.11 audit - PM1 S3 request boundary

Date: 2026-10-21

## Scope

This stage adds the internal PM1 control-word request boundary after the
previous stages have prepared drivers, wake memory, vectors, CPU context, and
secondary-CPU ownership. It does not expose a suspend action to userspace.

## Implementation

`power::enter_suspend` validates the discovered ACPI mode, PM1a block, and
`_S3` types, then reads PM1a and optional PM1b. It preserves unrelated control
bits, replaces SLP_TYP with the bounded firmware values, and sets SLP_EN. The
request is issued only after the private `SuspendPreparation` token owns all
rollback resources.

On hardware where firmware accepts S3, the request does not return. If the
machine remains running, the path restores the FACS vectors, wake trampoline,
secondary CPUs, and drivers through `abort_suspend`, returning
`StillRunning`; a failed cleanup returns `RollbackFailed`.

The `stage25-11-test` self-test checks the pure control-word encoding and an
out-of-range type rejection. It does not access I/O ports or request S3.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.11-1cpu-1792576039149377100`
- 2 CPUs: `audit-artifacts/stage25.11-2cpu-1792576252034971100`
- 4 CPUs: `audit-artifacts/stage25.11-4cpu-1792576273864200600`

After a final fail-closed review of the PM1b error path, the exact committed
tree was rerun as:

- 1 CPU: `audit-artifacts/stage25.11-1cpu-1792576842603079000`
- 2 CPUs: `audit-artifacts/stage25.11-2cpu-1792576881588102400`
- 4 CPUs: `audit-artifacts/stage25.11-4cpu-1792576903007970400`

## Security and concurrency review

The transition remains private to the power subsystem; no untrusted process
can write PM1. The values come from bounded ACPI discovery, and unrelated PM1
bits are preserved. I/O writes occur with interrupts disabled. A returning
firmware path cannot silently abandon quiesced drivers or parked CPUs because
rollback is mandatory and failure is surfaced.

## Limits and prerequisites

This stage does not restore CPU registers, execute the wake-entry restoration
path, restart secondary CPUs after firmware wake, provide a suspend syscall or
policy, or establish physical hardware suspend/resume qualification. The
QEMU gate intentionally avoids issuing S3.
