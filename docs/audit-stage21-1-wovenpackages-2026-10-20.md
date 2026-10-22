# Stage 21.1 WovenPackages bounded envelope audit — 2026-10-20

## Scope

This increment establishes the first bounded `WPKG` package envelope in
`libwoven/src/package.rs` and exercises it through a feature-gated kernel
self-test. It is intentionally a format/parser increment, not an installer or
package-service acceptance.

## Implemented contract

- Magic `WPKG`, version `1`, fixed 92-byte header.
- Manifest, application, resource, and 64-byte signature fields.
- Manifest version, capability bitmap, bounded package name, and up to 16
  bounded dependencies.
- Manifest maximum 4096 bytes, application maximum 64 MiB, resource maximum
  16 MiB, names maximum 48 bytes.
- Exact-length and overflow checks before borrowed payload slices are returned.
- Parser tests for valid payloads, truncation, bad names, and length mismatch.
- Kernel `stage21-1-test` validates a package in freestanding execution and
  rejects a mutated magic field.

The parser does not authenticate the signature. The future package service
must define and enforce the trust root and signature domain before install.

## Validation

- `cargo test --workspace`: passed; 59 `libwoven` unit tests plus all workspace
  integration tests passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-1-test -- -D warnings`: passed.
- `python scripts/test-stage21-1-packages.py --cpus 1`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-1cpu-1792513802229723100`.
- `python scripts/test-stage21-1-packages.py --cpus 2`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-2cpu-1792513815638396400`.
- `python scripts/test-stage21-1-packages.py --cpus 4`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-4cpu-1792513829341085500`.

## Remaining acceptance prerequisites

Package producer/signing tool, trust-root verification, persistent WovenStore
transactions, update/removal/rollback behavior, sandbox declaration
enforcement, service ownership/IPC, power-loss tests, and physical USB
qualification remain unimplemented. No physical USB write was performed.
