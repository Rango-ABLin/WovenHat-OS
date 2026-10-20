# Stage 24.10 — kernel identity lockout and rate limiting

Date: 2026-10-20

## Scope

The owner-bound kernel identity runtime now applies a bounded per-account
lockout policy to both plaintext and trusted-login-manager digest paths. Five
failed authentication attempts enter a five-second cooldown; successful
authentication clears the failure state. The fixed table is bounded to the
maximum identity user count, resets on authority installation, and fails closed
if its capacity is unexpectedly exhausted. WovenShell now sends a candidate
digest for every entered PIN so failed login-manager attempts reach the kernel
policy instead of being rejected only in userspace.

## Acceptance

- `cargo test -p libwoven`: 86 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed, including the lockout self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.10-lockout-1cpu-1792533000000000000`
- `audit-artifacts/stage24.10-lockout-2cpu-1792533000000000000`
- `audit-artifacts/stage24.10-lockout-4cpu-1792533000000000000`

The first 4-CPU attempt emitted the known migration-stress slot-capacity
panic; the unchanged rerun passed. The final passing logs are preserved above;
the failure is recorded here because the harness reuses its fixed target log
paths.

## Limits and next prerequisites

The lockout state is runtime-only and is reset after reboot or authority
reload; durable policy state and administrator-configurable thresholds remain
open. The hardware-demo gate does not inject five failed PINs through the UI,
so the dedicated proof is the Stage 24.5 kernel self-test. Account creation,
deletion, and group administration, malformed-request telemetry,
hardware-backed credentials, release-key provisioning, and physical
USB/power-loss qualification remain release prerequisites.
