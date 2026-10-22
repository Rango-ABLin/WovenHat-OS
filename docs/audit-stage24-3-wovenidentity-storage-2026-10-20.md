# Stage 24.3 audit — kernel-authorized WovenIdentity storage

Date: 2026-10-20

Implemented `kernel/src/identity_storage.rs`, a StorageIo-authorized adapter
that persists authenticated `WID1` policy snapshots inside two native WovenFS
`WID2` records. Each record carries a non-zero generation and wrapper checksum;
recovery selects the newest valid copy and validates the inner identity
snapshot. The adapter never writes sessions.

Validation:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-3-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.3] WovenIdentity durable storage: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.3-1cpu-1792526461019999200`
- `audit-artifacts/stage24.3-2cpu-1792526482958590800`
- `audit-artifacts/stage24.3-4cpu-1792526505203409200`

The failed pre-fix run is retained at
`audit-artifacts/stage24.3-1cpu-1792526354775283900` and showed the harness
had no IDE disk. The launcher now creates an explicit disposable 16 MiB image.

Limits: this stage does not claim user-facing persistence, key management,
credential rotation, lockout/rate limiting, or kernel task/session binding.
