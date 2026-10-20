# Stage 21.9 WovenPackage update/removal audit — 2026-10-20

## Scope

This increment closes the single-package update and removal publication gap
left by Stage 21.8. The catalog persists exact payload chunk counts, and the
intent record carries both the incoming and superseded generation.

## Implemented contract

- WSTC v2 records application and resource chunk counts per entry.
- Updates journal the old generation and retain it for the bounded rollback
  window; later transactions reclaim it when it is no longer active or
  rollback-capable.
- Removal writes a prepared intent, commits the catalog, and records the
  committed state before clearing the intent; payload cleanup is bounded by
  the rollback-generation policy.
- Recovery preserves the old generation when a removal did not reach catalog
  commit and cleans it when the catalog already reflects removal.
- Existing install and recovery behavior remains compatible with the new
  bounded record/journal layouts.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-9-test -- -D warnings`: passed.
- Stage 21.9 QEMU passed with exit 33 on 1/2/4 CPUs; evidence is under
  `audit-artifacts/stage21.9-{1,2,4}cpu-*`.
- Stage 21.6, 21.7, and 21.8 regression gates passed on 1/2/4 CPUs after
  these ABI and journal changes; newer timestamped evidence is retained under
  `audit-artifacts`.

## Remaining acceptance prerequisites

This is not full package-production acceptance. Multi-package recovery,
power-loss injection at every boundary, sandbox enforcement, userspace
service/IPC ownership, and physical USB qualification remain open.
No physical USB write was performed.
