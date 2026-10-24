# Stage 24.6 — live WovenIdentity runtime and WovenShell login

Date: 2026-10-20

## Scope

This increment imports the bounded salted account file into the kernel
identity authority before WovenShell starts, binds account-backed sign-in and
lock/unlock to owner-bound kernel sessions, applies the authenticated UID/GID
to the shell process, and releases all identity bindings when the owner exits.
The trusted WovenShell login manager verifies the PIN once, then uses the
capability-gated digest handoff; the kernel compares that digest against its
own authority record before creating or unlocking a session. The ordinary
password syscall path remains available for non-login-manager callers.

## Acceptance

- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- The same live production shell gate passed on 2 and 4 CPUs.

Final evidence is preserved in:

- `audit-artifacts/stage24.6-live-1cpu-1792530000000000000`
- `audit-artifacts/stage24.6-live-2cpu-1792530000000000000`
- `audit-artifacts/stage24.6-live-4cpu-1792530000000000000`

The serial logs prove account provisioning, kernel credential loading, shell
desktop readiness, `ada` sign-in, diagnostic-console handoff, and the SMP
input/migration checks. The QMP harness now waits for the shell's actual
desktop-ready marker before injecting login input.

## Limits and next prerequisites

This does not yet load the authenticated WID2 snapshot into the live runtime,
implement account administration or PIN rotation, add kernel lockout/rate
limiting, emit identity audit actions, or provide hardware-backed credential
storage. The digest handoff is restricted to the trusted system session's
`AccountManage` capability and must remain audited as the login-manager trust
boundary. Physical USB boot and power-loss qualification remain separate
release gates.
