# Stage 24.28 audit — credential-root read-back promotion boundary

Date: 2026-10-21

## Delivered

- Added a combined credential-root promotion validator.
- Required the expected NV index, SHA-256 metadata, exact policy, write
  restrictions, 32-byte size, and exact returned root value.
- Used a constant-time comparison for the 32-byte expected/actual root.
- Added executable QEMU self-test coverage for valid promotion and a
  one-byte credential mismatch.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.28-credential-root-readback-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor. No credential root is
stored or promoted, no TPM quote is verified, and no physical hardware state
is changed. Crash recovery, physical TPM qualification, secure owner-key
custody, and release acceptance remain required. The attached USB was
inventory-only and was not written, formatted, or mounted.
