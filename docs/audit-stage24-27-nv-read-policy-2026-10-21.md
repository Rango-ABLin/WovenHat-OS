# Stage 24.27 audit — policy-session NV read-back

Date: 2026-10-21

## Delivered

- Added bounded policy-session `NV_Read` framing for the fixed index, zero
  offset, and 32-byte credential-root value.
- Added fail-closed response parsing for the sessions tag, TPM response code,
  parameter size, exact data length, response nonce, attributes, HMAC, and
  total response size.
- Added executable QEMU self-test coverage for exact read-back contents and
  malformed parameter-size rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.27-nv-read-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and reads no device state. It does not create or write an NV
index, persist a credential, validate a TPM quote, or provide recovery.
Physical TPM provisioning, quote verification, and release qualification
remain required. The attached USB was inventory-only and was not written,
formatted, or mounted.
