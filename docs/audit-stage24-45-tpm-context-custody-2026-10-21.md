# Stage 24.45 audit - durable TPM context custody boundary

Date: 2026-10-21

## Delivered

Opaque TPM-produced policy-session context blobs now have a fixed-size `TPS1`
WovenFS record containing version, commit sequence, context metadata, session
handle/nonce metadata, and a SHA-256 integrity digest. Persistence uses two
journal-backed paths and advances one copy per commit. Recovery validates both
copies, selects the unique newest valid sequence, and rejects ties or an
entirely invalid set.

This is custody only. The recovered snapshot is still passed to the bounded
TPM `ContextLoad` path, whose TPM response is required before a new managed
lease exists. A filesystem record cannot itself create session authority.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers record encoding, metadata bounds, sequence field,
and tamper-digest rejection. Serial evidence is preserved in
`audit-artifacts/stage24.45-tpm-context-custody-20261021`.

## Limits

This stage does not yet wire custody recovery into boot, establish a separate
secure-storage authorization policy, or prove cross-reboot behavior on a
physical TPM. Session nonce refresh, provisioned-key lifecycle, credential
decisions, recovery, and physical qualification remain open. The attached
USB device was inventory-only and remains untouched.
