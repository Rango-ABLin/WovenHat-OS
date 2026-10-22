# Stage 24.25 audit — policy-session NV write framing

Date: 2026-10-21

## Delivered

- Added sessions-tagged `NV_Write` framing for a bounded 32-byte
  credential-root payload.
- Bound the auth handle and NV index to the same caller-selected index.
- Enforced a nonzero policy-session handle, zero NV offset, bounded caller
  nonce, continue-session attribute, and empty HMAC for the policy-session
  command shape.
- Added response validation for TPM status, sessions tag, zero parameter
  area, bounded response nonce/HMAC, and exact response length.
- Added executable QEMU self-test coverage for valid and malformed frames.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.25-nv-write-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and writes no device state. It does not create an NV index,
establish owner authorization, persist a credential, validate a TPM quote, or
provide recovery. Physical TPM testing, secure provisioning, quote
verification, and release qualification remain required. The attached USB
was inventory-only and was not written, formatted, or mounted.
