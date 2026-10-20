# Stage 24.4 audit — owner-bound kernel identity seam

Date: 2026-10-20

Implemented the kernel identity runtime seam. It binds each identity session
to an owning process ID in a fixed table, validates ownership for lock,
unlock, elevation, and close, and applies credentials only through a crate-only
task API after releasing the identity lock. If no live process can receive the
credentials, the session and binding are cleaned up and the operation fails.

Validation:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-4-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.4] WovenIdentity task binding: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.4-1cpu-1792526710240384400`
- `audit-artifacts/stage24.4-2cpu-1792526744522828500`
- `audit-artifacts/stage24.4-4cpu-1792526778145524000`

Limits: userspace syscall framing, durable runtime loading, process-exit
binding reclamation, audit actions, lockout/rate limiting, and WovenShell
integration remain unimplemented.
