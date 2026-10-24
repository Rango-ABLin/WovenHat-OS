# Stage 22.1 userspace service IPC ABI audit — 2026-10-20

## Scope

Expose the existing capability- and owner-aware IPC service registry to
userspace so the WovenPackage service can later run outside the kernel.

## Implementation

- Added syscall numbers 211–214 for endpoint creation, service publish,
  unpublish, and discovery.
- Added bounded service-name copy-in using `MAX_IPC_SERVICE_NAME` before IPC
  registry access.
- Required `Capability::Ipc` for every operation; endpoint handles are created
  for the current process and service handles remain process-local.
- Added typed `libwoven` wrappers and a dedicated Stage 22.1 QEMU marker.
- Added handle-addressed message send/receive syscalls 215–216 and typed
  `libwoven` wrappers in the Stage 22.2 follow-on.
- Corrected the Stage 22.1 feature graph to inherit the native WovenFS
  prerequisite and suppressed duplicate inherited self-test exits.

## Validation

- `cargo test -p libwoven`: 63 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-1-test -- -D warnings`: passed.
- QEMU acceptance: passed on 1 CPU (exit 33), 2 CPUs (exit 33), and 4 CPUs
  (exit 33).
- Accepted evidence:
  - `audit-artifacts/stage21.22-1cpu-1792517099255354900`
  - `audit-artifacts/stage21.22-2cpu-1792517111947242900`
  - `audit-artifacts/stage21.22-4cpu-1792517125292213600`

Stage 22.2 transport evidence:

- `audit-artifacts/stage21.22-1cpu-1792517297224254700`
- `audit-artifacts/stage21.22-2cpu-1792517311818527300`
- `audit-artifacts/stage21.22-4cpu-1792517324885076600`

The receive ABI was subsequently hardened to prefix each copied message with
the sender process ID, which is required for a broker to route responses using
the existing mailbox syscall. The corrected ABI passed the affected matrix:

- `audit-artifacts/stage21.22-1cpu-1792517632981988900`
- `audit-artifacts/stage21.22-2cpu-1792517647927733400`
- `audit-artifacts/stage21.22-4cpu-1792517661629792600`

## Failed attempt retained

The first 1-CPU attempt is preserved at
`audit-artifacts/stage21.22-1cpu-1792516996924333800`. It reached the
inherited FAT32 mutation probe and exited before `[S22.1]` because the new
feature did not yet inherit `stage15-2-test`. The feature graph and guards were
fixed; the rerun passed. This was a harness/configuration defect, not a
runtime IPC failure.

## Limits and next prerequisites

This gate validates the ABI surface and the existing kernel registry probe; it
does not yet exercise a real Ring-3 package broker/client. The transport is
validated through the existing kernel endpoint probe; next work must add the
versioned broker/client lifecycle and protocol, then repeat the same host,
Clippy, and 1/2/4-CPU acceptance discipline. No physical USB write was
performed.
