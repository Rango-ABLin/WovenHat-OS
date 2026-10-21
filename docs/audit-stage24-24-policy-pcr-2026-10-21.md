# Stage 24.24 audit — bounded TPM PolicyPCR transcript

Date: 2026-10-21

## Delivered

- Added bounded `PolicyPCR` request framing for one nonzero policy session,
  one PCR index, and one SHA-256 expected digest.
- Added exact response-size and TPM response-code validation for the policy
  command response.
- Added executable QEMU self-test coverage for the PCR selection, digest,
  session handle, successful response, and malformed response rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.24-policy-pcr-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so the command codec
is tested with synthetic responses and no live TPM transaction occurs. The
stage does not run `PolicyPCR` against a physical TPM, bind an authorization
session to NV write commands, create or write credentials, validate a quote,
or provide recovery. Physical TPM testing, secure provisioning, and release
qualification remain required. The attached USB was inventory-only and was
not written, formatted, or mounted.
