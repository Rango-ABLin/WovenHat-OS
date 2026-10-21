# Stage 24.21 audit — bounded TPM NV public metadata

Date: 2026-10-21

## Delivered

- Added fixed-size no-session `NV_ReadPublic` command framing.
- Added fail-closed parsing of the TPM response code, response size, NV
  public area, bounded authorization policy, data size, and trailing Name.
- Kept the boundary metadata-only: it does not read or write secret NV data,
  create an index, authorize a session, or promote any value to a credential
  root.
- Added a QEMU-feature codec self-test for the command and response shape.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.21-nv-public-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so no live NV command
was issued. A production credential root still requires a physical TPM2 test
matrix, authorization-session implementation, policy-bound sealed object or
NV provisioning, quote/attestation-key verification, rollback/recovery
handling, secure release-key custody, and hardware qualification. The
attached USB was inventory-only; it was not written, formatted, or mounted.
