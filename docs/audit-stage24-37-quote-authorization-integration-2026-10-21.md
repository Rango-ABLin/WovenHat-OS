# Stage 24.37 audit - fail-closed TPM quote authorization integration

Date: 2026-10-21

## Delivered

`authorize_quote` accepts a structured request containing the signed TAN1
policy, trusted policy key, validated `ReadPublic` result, quote bytes,
signature bytes, expected nonce, PCR index, and expected PCR digest. It then
enforces this order:

1. Verify the signed expected TPM Name.
2. Bind the validated public key to that Name.
3. Validate the quote attestation and its nonce/PCR digest.
4. Decode the complete TPMT_SIGNATURE frame.
5. Verify RSA RSASSA/RSAPSS or ECDSA P-256 cryptographically according to the
   bound key type.

No key supplied independently of the validated public-area result can create
authority, and an identity mismatch fails before signature processing.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.37-quote-authorization-integration-20261021`.

## Limits

The QEMU self-test exercises the fail-closed policy/identity ordering; it does
not perform a live physical TPM quote or make a credential decision during
boot. Boot-time policy loading, live TPM transport qualification, recovery,
and physical hardware testing remain required. The attached USB device was
inventory-only and remains untouched.
