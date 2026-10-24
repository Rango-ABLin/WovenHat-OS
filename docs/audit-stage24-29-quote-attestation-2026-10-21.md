# Stage 24.29 audit — bounded TPM quote attestation parsing

Date: 2026-10-21

## Delivered

- Added bounded parsing of the TPM `TPMS_ATTEST` quote structure.
- Validated generated-value magic, quote type, qualified-signer bound,
  qualifying nonce, clock metadata, one SHA-256 PCR selection, and exact
  32-byte PCR digest.
- Bound the parsed nonce and digest to caller-supplied expectations.
- Added executable QEMU self-test coverage for valid evidence and nonce
  mismatch rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.29-quote-attestation-20261021/`.

## Diagnostic record and limits

The first executable run exposed a self-test fixture shadowing bug that copied
an 8-byte NV nonce into a 16-byte quote slot and caused a kernel panic. The
fixture was corrected to use an explicitly named quote nonce, and the rerun
passed. No signature or TPM attestation key was verified; the current QEMU
platform has no TPM2 ACPI descriptor and no physical state was changed. The
attached USB was inventory-only and was not written, formatted, or mounted.
