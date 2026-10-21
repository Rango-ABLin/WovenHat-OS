# Stage 24.34 audit - TPM attestation-key Name binding

Date: 2026-10-21

## Delivered

The `TPM2_ReadPublic` decoder now recomputes the TPM Name as the SHA-256 hash
of the exact `TPMTPublic` bytes prefixed by the SHA-256 algorithm identifier,
then requires the returned Name TPM2B to match. The bounded qualified Name is
retained. `bind_attestation_public_name` compares that parsed identity with a
caller-provisioned 34-byte expected Name and returns the key only on an exact
match.

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
`audit-artifacts/stage24.34-tpm-name-binding-20261021`.

## Limits

The QEMU fixture covers recomputation, returned-Name validation, and expected
Name mismatch rejection using a synthetic public response. This stage does
not provision or persist an expected physical TPM object identity, execute a
live `ReadPublic` against hardware, bind quotes into credential authorization,
implement recovery, or qualify physical hardware. The attached USB device was
inventory-only and remains untouched.
