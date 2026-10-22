# Stage 24.26 audit — bounded TPM NV definition framing

Date: 2026-10-21

## Delivered

- Added sessions-tagged owner-authorized `NV_DefineSpace` framing.
- Bounded the caller-supplied owner password-session authorization to 32
  bytes and kept it transient in the command buffer.
- Encoded the fixed credential-root index, SHA-256 name algorithm, 32-byte
  PCR policy, exact NV attributes, and 32-byte data size.
- Added executable QEMU self-test coverage for command handles,
  authorization area, public metadata, and size fields.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.26-nv-define-space-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and creates no device state. It does not persist owner
authorization, write credential bytes, validate a TPM quote, or provide
recovery. Physical TPM testing, secure provisioning, quote verification, and
release qualification remain required. The attached USB was inventory-only
and was not written, formatted, or mounted.
