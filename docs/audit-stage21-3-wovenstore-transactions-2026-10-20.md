# Stage 21.3 WovenStore transaction audit — 2026-10-20

## Scope

This increment adds the bounded metadata transaction core in
`libwoven/src/store.rs`. It is intentionally storage-independent; the caller
must authenticate WPKG before submitting a manifest and must persist a
successful commit through the future package service.

## Implemented contract

- Fixed catalog capacity of 32 installed entries.
- Explicit allowed-capability mask for sandbox declarations.
- Dependency presence checks before installation.
- Monotonic version updates; stale/equal versions are rejected.
- Dependent-protected removal.
- Copy-on-write transaction working state with explicit commit or rollback.
- Bounded metadata and application digest storage; package payloads are not
  copied into the catalog.

## Validation

- `cargo test -p libwoven`: passed; 61 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-3-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.3 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.3-1cpu-1792514233459105300`.
- Stage 21.3 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.3-2cpu-1792514246539222000`.
- Stage 21.3 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.3-4cpu-1792514260377814200`.

## Remaining acceptance prerequisites

Authenticated package-service integration, persistent WovenFS catalog/payload
publication, crash/power-loss recovery, dependency ordering across a package
batch, IPC authority, and physical USB qualification remain unimplemented.
No physical USB write was performed.
