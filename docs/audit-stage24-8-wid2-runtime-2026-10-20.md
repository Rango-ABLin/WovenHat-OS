# Stage 24.8 — WID2 runtime recovery

Date: 2026-10-20

## Scope

The system-apps identity startup now attempts to recover the newest valid
authenticated WID2 dual-copy snapshot directly from native WovenFS before
starting WovenShell. Snapshot decoding is bounded and revalidates the
libwoven authority structure; if no valid durable copy is available, boot
falls back to the existing bounded `/etc/wovenhat/accounts` bootstrap. Durable
storage remains read-only from this startup path and session state is never
loaded from disk.

## Acceptance

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-3-test -- -D warnings`: passed.
- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

The QEMU image has no ATA WovenFS disk, so the live logs prove the bounded
account-file fallback and fail-closed no-durable-copy behavior. The existing
Stage 24.3 durable WID2 self-test remains the native dual-copy write/recovery
proof. Evidence is preserved in:

- `audit-artifacts/stage24.8-wid2-runtime-1cpu-1792531800000000000`
- `audit-artifacts/stage24.8-wid2-runtime-2cpu-1792531800000000000`
- `audit-artifacts/stage24.8-wid2-runtime-4cpu-1792531800000000000`

The 4-CPU acceptance needed one rerun after the known migration-stress slot
capacity flake; the final rerun passed and the failed diagnostic remains in
the ordinary `target/shell-regression-4cpu` output.

## Limits and next prerequisites

There is not yet a hardware-backed credential root, account administration or
PIN rotation flow, lockout/rate limiting, malformed-request audit telemetry,
or power-loss/physical-device qualification. A production image must also
provision a real trusted application key instead of the deterministic test key
used by this acceptance harness.
