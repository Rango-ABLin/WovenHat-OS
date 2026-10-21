# Stage 24.22 audit — credential-root NV policy validation

Date: 2026-10-21

## Delivered

- Added `validate_credential_root_nv` to validate the decoded public metadata
  of the reserved 32-byte credential-root NV object.
- Required the expected NV index, SHA-256 name algorithm, 32-byte data size,
  32-byte PCR policy, and policy-authorized writes.
- Rejected ordinary owner/object authorization writes and policy deletion.
- Added valid, owner-write, and policy-mismatch codec self-test coverage.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.22-credential-root-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage performs
no live TPM transaction. It does not create an NV index, start an authorization
session, read or write credential bytes, validate a TPM quote, or provide
recovery. Those operations require a physical TPM2 test matrix and remain
next-stage work. The attached USB was inventory-only and was not written,
formatted, or mounted.
