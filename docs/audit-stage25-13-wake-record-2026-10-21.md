# Stage 25.13 audit - persistent firmware-wake record integration

Date: 2026-10-21

## Scope

This stage connects the completed preparation transaction to the low-memory
wake path. It does not issue S3 in automated acceptance and does not claim
physical firmware wake.

## Implementation

`PENDING_SUSPEND` is a single kernel-owned slot containing the complete private
preparation token. `enter_suspend` captures a fresh CPU context immediately
before PM1 request, rejects duplicate pending transitions, and stores the
record before writing sleep controls. Pre-transition failures consume the slot
and call the existing full rollback path.

`wake_entry` snapshots the validated context and invokes the Stage 25.12
non-returning restore primitive. The continuation consumes the pending token
and calls `abort_suspend`, restoring FACS vectors, releasing the trampoline,
re-onlining secondary CPUs, and resuming drivers. A missing record fails to a
parked diagnostic path rather than dereferencing arbitrary memory.

## Acceptance evidence

The guarded non-destructive gate passed on all required CPU counts:

- 1 CPU: `audit-artifacts/stage25.13-1cpu-1792577693918265200`
- 2 CPUs: `audit-artifacts/stage25.13-2cpu-1792577749038729500`
- 4 CPUs: `audit-artifacts/stage25.13-4cpu-1792577772792940000`

## Security and concurrency review

The record contains only fixed-size kernel-owned state and is protected by a
kernel spin lock. It cannot be populated from userspace, and duplicate
transitions are rejected. Wake restoration snapshots the context before
calling non-returning assembly, releasing the record lock before the context
switch. Rollback remains ordered and attempts all owned cleanup operations.

## Limits and prerequisites

The automated gate does not issue PM1 S3. Real firmware wake, testing on the
attached hardware, firmware-specific wake behavior, and physical suspend/
resume qualification remain required before production release.
