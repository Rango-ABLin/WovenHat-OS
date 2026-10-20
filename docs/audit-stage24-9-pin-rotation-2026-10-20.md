# Stage 24.9 — live PIN rotation consistency

Date: 2026-10-20

## Scope

PIN rotation now updates the live kernel identity authority and the bounded
account file as one guarded operation. The kernel verifies the old PIN,
generates a fresh salt, updates its in-memory credential, performs the
trusted account-file rewrite, and rolls the authority credential back if the
file transaction fails. WovenShell reloads its account cache after success so
future lock/unlock operations use the new credential.

## Acceptance

- `cargo test -p libwoven`: 86 passed, including atomic password-rotation coverage.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.9-pin-rotation-1cpu-1792532400000000000`
- `audit-artifacts/stage24.9-pin-rotation-2cpu-1792532400000000000`
- `audit-artifacts/stage24.9-pin-rotation-4cpu-1792532400000000000`

## Limits and next prerequisites

The hardware-demo fixture does not drive the PIN-change UI, so this slice's
rotation-specific proof is the bounded host policy test and reviewed kernel /
shell integration; the live gates prove no boot or SMP regression. Account
creation/deletion and group administration, lockout/rate limiting,
malformed-request telemetry, hardware-backed credentials, real release-key
provisioning, and physical USB/power-loss qualification remain open.
