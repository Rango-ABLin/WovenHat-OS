# Stage 24.36 audit - durable signed TPM Name policy boundary

Date: 2026-10-21

## Delivered

The TPM2 module now exposes feature-gated durable APIs for the fixed TAN1
policy path `/wfs/tpm-attestation-name-policy`. `persist_attestation_name_policy`
verifies the complete Ed25519-signed envelope before invoking the accepted
journal-backed WovenFS writer. `read_attestation_name_policy` reads into a
caller-owned fixed buffer, rejects any length other than 106 bytes, and
revalidates the signature before returning the Name. Storage errors are
reported separately from policy and signature failures.

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
`audit-artifacts/stage24.36-durable-attestation-name-policy-20261021`.

## Limits

The QEMU run exercises the signed-policy validation path; lower-stage QEMU
exits before the physical WovenFS device path can exercise a TPM policy
record. No physical TAN1 record is provisioned, and no boot-time loader yet
requires one. Live TPM retrieval, quote authorization, recovery, and physical
hardware qualification remain required. The attached USB device was
inventory-only and remains untouched.
