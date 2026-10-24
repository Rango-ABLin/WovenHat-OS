# Stage 24.23 audit — bounded TPM policy-session framing

Date: 2026-10-21

## Delivered

- Added bounded policy `StartAuthSession` request framing.
- Restricted the request to null TPM/bind handles, bounded caller nonce,
  empty encrypted salt, null symmetric protection, and SHA-256 policy hash.
- Added fail-closed response parsing for TPM status, response size, nonzero
  session handle, bounded TPM nonce, and exact response framing.
- Added executable QEMU self-test coverage for command and response fields.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.23-policy-session-20261021/`.

## Diagnostic record and limits

The first executable self-test exposed an offset assertion defect and exited
35. The field offsets were corrected, temporary diagnostics were removed, and
the rerun passed. This stage still performs no live TPM transaction on the
current QEMU platform, which has no TPM2 ACPI descriptor. It does not bind an
authorization session to an NV command, create or write an object, validate a
TPM quote, or provide recovery. The attached USB was inventory-only and was
not written, formatted, or mounted.
