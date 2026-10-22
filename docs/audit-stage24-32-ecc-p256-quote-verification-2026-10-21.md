# Stage 24.32 audit - bounded ECC/P-256 quote verification

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now verifies TPM ECDSA/P-256 SHA-256 quote signatures
against an uncompressed SEC1 public key assembled from bounded 32-byte affine
coordinates. TPM `r` and `s` values must each be exactly 32 bytes, matching
the P-256 field width. The verifier validates that the point is on the curve
and returns fail-closed policy-signature errors for cryptographic mismatch.

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
`audit-artifacts/stage24.32-ecc-p256-quote-verification-20261021`.

## Limits

The deterministic self-test verifies a valid P-256 signature and rejects a
tampered component. This stage does not retrieve an attestation key from a
physical TPM, validate its TPM object attributes/name, bind it to a persistent
credential policy, implement recovery, or qualify physical hardware. The
attached USB device was inventory-only and remains untouched.
