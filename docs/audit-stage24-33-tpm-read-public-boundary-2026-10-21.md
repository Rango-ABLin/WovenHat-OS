# Stage 24.33 audit - bounded TPM ReadPublic attestation-key boundary

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now encodes `TPM2_ReadPublic` for a selected object
handle, executes it through the existing serialized CRB transport, and
decodes the exact no-session response. The public-area validator requires
SHA-256 naming, fixed-TPM/fixed-parent/sensitive-origin/restricted/signing
attributes, and rejects decrypt-only objects. RSA public areas are limited to
2048-4096-bit moduli and validated through RustCrypto; ECC public areas are
limited to valid NIST P-256 affine points. Name and qualified-name TPM2Bs are
bounded and trailing bytes are rejected.

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
`audit-artifacts/stage24.33-tpm-read-public-boundary-20261021`.

## Limits

The QEMU self-test uses a bounded synthetic `ReadPublic` response and does
not assert physical TPM execution. It does not yet bind the returned TPM2B
name to a provisioned attestation-key identity, persist the key, authorize a
credential decision, implement recovery, or qualify physical hardware. The
attached USB device was inventory-only and remains untouched.
