# Stage 22.3 WovenPackage Ring-3 broker audit — 2026-10-20

## Scope

Prove that the WovenPackage service boundary exists as a signed external
Ring-3 process and can publish, receive, dispatch, correlate, and reply over
the Stage 22 IPC ABI.

## Implementation

- Added `apps/woven-package`, a signed no-std service image.
- The service publishes `woven.package` with `Capability::Ipc`.
- The fixture discovers the service, sends a versioned `WPKI` query, validates
  the response request ID/status/value, and reaps the process.
- The receive ABI carries the sender PID; the broker uses the existing mailbox
  send path for the response.
- Service buffers are static bounded allocations, not user-stack allocations.
- Added `scripts/test-stage22-3-package-service.py` and feature-gated kernel
  acceptance `stage22-3-test`.

## Validation

- `cargo clippy --release --target x86_64-unknown-none -- -D warnings` in
  `apps/woven-package`: passed.
- `cargo test -p libwoven`: 66 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-3-test -- -D warnings`: passed.
- Stage 22.3 QEMU: passed on 1 CPU (exit 33), 2 CPUs (exit 33), and 4 CPUs
  (exit 33).
- Evidence:
  - `audit-artifacts/stage22.3-1cpu-1792518878417244900`
  - `audit-artifacts/stage22.3-2cpu-1792518903678229000`
  - `audit-artifacts/stage22.3-4cpu-1792518929724634300`

## Limits

The broker's query dispatch is an end-to-end protocol proof. Install, remove,
and rollback deliberately return a storage-authority status until a userspace
storage/package transaction API is available. Production system-app startup
wiring and physical USB qualification remain open. Earlier failed attempts,
including the preserved disk-full build failure and broker request-routing
failures, remain in their timestamped audit directories.
