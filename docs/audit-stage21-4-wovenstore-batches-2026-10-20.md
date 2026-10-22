# Stage 21.4 WovenStore batch resolution audit — 2026-10-20

## Scope

This increment adds bounded dependency-ordered batch installation to the
WovenStore transaction core. It preserves the copy-on-write transaction
boundary and does not perform persistent storage I/O.

## Implemented contract

- At most 32 requests are accepted in one batch.
- Requests may be supplied in reverse dependency order.
- Missing dependencies are deferred across bounded passes.
- Cycles or absent dependencies fail with `MissingDependency`.
- Any non-dependency error aborts immediately without committing the working
  catalog.

## Validation

- `cargo test -p libwoven`: passed; 62 tests.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-4-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.4 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.4-1cpu-1792514391320435900`.
- Stage 21.4 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.4-2cpu-1792514406768911700`.
- Stage 21.4 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.4-4cpu-1792514420345269700`.

## Remaining acceptance prerequisites

Persistent WovenFS catalog/payload publication, crash and power-loss recovery,
package-service IPC authority, and physical USB qualification remain open.
No physical USB write was performed.
