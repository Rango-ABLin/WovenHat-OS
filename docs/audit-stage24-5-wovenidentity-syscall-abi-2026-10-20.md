# Stage 24.5 audit — WovenIdentity syscall ABI

Date: 2026-10-20

Added additive syscalls 219–223 for identity session open, lock, unlock,
elevation, and close. Kernel handlers copy bounded user buffers before parsing;
length-prefixed name/password frames reject truncation, empty fields, and
oversized values. Safe `libwoven::identity` wrappers use bounded stack-owned
request buffers and preserve the existing error sentinel ABI.

Validation:

- `cargo test -p libwoven --lib`: 84 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.5] WovenIdentity syscall ABI: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.5-1cpu-1792526996850760700`
- `audit-artifacts/stage24.5-2cpu-1792527013189914600`
- `audit-artifacts/stage24.5-4cpu-1792527027362429900`

Limits: this gate does not claim durable runtime loading, a live Ring-3 login
fixture, process-exit binding reclamation, audit actions, lockout/rate limiting,
or WovenShell migration.
