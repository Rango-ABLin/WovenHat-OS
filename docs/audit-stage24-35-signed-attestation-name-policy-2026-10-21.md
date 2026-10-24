# Stage 24.35 audit - signed TPM attestation-Name policy

Date: 2026-10-21

## Delivered

`verify_signed_attestation_name_policy` validates the fixed-size TAN1 policy
envelope: magic, version, signed-length field, SHA-256 Name algorithm, and a
34-byte TPM Name are checked before the Ed25519 signature is verified against
the caller-provisioned trust root. It returns only the signed Name; a record
found in persistent storage cannot create authority by itself.

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
`audit-artifacts/stage24.35-signed-attestation-name-policy-20261021`.

## Limits

The QEMU self-test verifies a deterministic signed record and rejects a
tampered signature. This stage does not persist the record in WovenFS or a
TPM NV index, retrieve the live TPM object, bind the verified Name to a
credential authorization transaction, implement recovery, or qualify physical
hardware. The attached USB device was inventory-only and remains untouched.
