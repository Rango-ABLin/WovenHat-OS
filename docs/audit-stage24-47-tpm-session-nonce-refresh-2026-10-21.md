# Stage 24.47 audit - TPM policy-session nonce refresh

Date: 2026-10-21

## Delivered

The TPM quote response decoder now validates the complete authorization area
and preserves the bounded response TPM nonce in `QuoteExecution`. Password
session responses retain their valid empty nonce behavior. Policy-session quote
execution rejects an empty refreshed nonce. The managed lease copies the new
nonce before marking the quote one-shot stale, preventing a later lifecycle
operation from treating the previous nonce as current authorization state.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers both empty password-session response handling
and non-empty policy-session nonce extraction. Serial evidence is preserved in
`audit-artifacts/stage24.47-tpm-session-nonce-refresh-20261021`.

## Limits

QEMU does not provide a live TPM session, so physical response-nonce behavior
and cross-reboot custody are not proven. Durable custody authorization,
provisioned-key lifecycle, credential decisions, recovery policy, and physical
qualification remain open. The attached USB device was not modified.
