# Stage 21.10 WovenPackage bounded rollback audit — 2026-10-20

## Scope

This increment adds one durable rollback generation to the package service.
Rollback is bounded so payload storage cannot grow without limit.

## Implemented contract

- Each package transaction persists the current catalog as the previous
  generation before mutation.
- The previous payload generation remains available while it is rollbackable.
- A checksummed WRBK intent is durable before catalog restoration.
- Recovery replays a partially completed rollback idempotently.
- The next transaction retires only previous generations that are no longer
  active or the current rollback target.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-10-test -- -D warnings`: passed.
- Stage 21.10 QEMU passed with exit 33 on 1/2/4 CPUs; evidence:
  `audit-artifacts/stage21.10-{1,2,4}cpu-*`.
- Stage 21.9 QEMU passed again on 1/2/4 CPUs after rollback integration.

## Remaining acceptance prerequisites

Multi-package rollback, power-loss injection at every boundary, sandbox
enforcement, userspace package-service IPC/ownership, and physical USB
qualification remain open. No physical USB write was performed.
