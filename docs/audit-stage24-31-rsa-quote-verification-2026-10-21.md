# Stage 24.31 audit - bounded TPM RSA quote verification

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now verifies SHA-256 TPM quote signatures for RSASSA
PKCS#1 v1.5 and RSASSA-PSS. The bounded `QuoteRsaPublicKey` accepts 2048,
3072, and 4096-bit modulus shapes, validates the RSA key through RustCrypto,
and applies TPM's exponent-zero-as-65537 rule. Verification covers the exact
`TPMS_ATTEST` byte slice and returns a dedicated policy-signature failure for
cryptographic mismatch.

The TPM self-test now runs after heap initialization. This is required because
the no-std RSA implementation uses bounded allocation internally; the earlier
placement caused a real allocator failure before the heap existed.

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
`audit-artifacts/stage24.31-rsa-quote-verification-20261021`.

## Limits

The fixed self-test fixture verifies a valid 2048-bit RSASSA signature and
rejects a one-bit tamper. This stage does not implement ECC/P-256 quote
verification, live TPM public-key retrieval and attestation-key validation,
credential authorization integration, crash recovery, or physical TPM
qualification. The attached USB device was inventory-only and remains
untouched.
