# Stage 24.43 audit - managed TPM policy-session lifecycle

Date: 2026-10-21

## Delivered

`PolicySessionLease` gives each started policy session a monotonic generation,
active state, and nonce-stale state. Managed PolicyPCR and quote operations
reject flushed or stale leases. A successful managed quote marks the lease
stale so it cannot silently reuse a nonce for another authority decision.
`flush_policy_session` executes bounded `TPM2_FlushContext`; only an exact
successful response invalidates the lease, while a failed cleanup remains
retryable.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers exact FlushContext framing and invalid-handle
rejection. Serial evidence is preserved in
`audit-artifacts/stage24.43-managed-policy-session-20261021`.

## Limits

This lease does not yet implement TPM context save/load across suspend or
reboot, nor does it expose refreshed session nonce state after every
authenticated command. Provisioned-key lifecycle, credential decisions,
recovery, and physical TPM qualification remain open. The attached USB
device was inventory-only and remains untouched.
