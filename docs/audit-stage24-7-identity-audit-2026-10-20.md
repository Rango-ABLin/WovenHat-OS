# Stage 24.7 — WovenIdentity audit actions

Date: 2026-10-20

## Scope

Identity session operations now enter the kernel security ledger with stable
action identifiers. Open, lock, unlock, elevation, and close each record the
calling task, session target where applicable, and allowed/denied outcome.
The public `libwoven::audit_read` decoder exposes these actions as IDs 26–30,
preserving the existing append-only ledger ABI.

## Acceptance

- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

The first 4-CPU attempt exposed an existing timing-sensitive migration-stress
slot exhaustion panic; its serial and QEMU logs remain in the normal
`target/shell-regression-4cpu` output. A clean rerun passed, and the final
acceptance logs are preserved in:

- `audit-artifacts/stage24.7-identity-audit-1cpu-1792531200000000000`
- `audit-artifacts/stage24.7-identity-audit-2cpu-1792531200000000000`
- `audit-artifacts/stage24.7-identity-audit-4cpu-1792531200000000000`

## Limits and next prerequisites

This slice audits valid identity syscall outcomes. User-buffer copy/parse
failures return before an identity operation is formed and are not yet
represented as separate malformed-request events. The identity subsystem still
needs durable WID2 runtime recovery, administration and PIN rotation, kernel
lockout/rate limiting, hardware-backed credentials, and physical USB boot and
power-loss qualification before a release claim is justified.
