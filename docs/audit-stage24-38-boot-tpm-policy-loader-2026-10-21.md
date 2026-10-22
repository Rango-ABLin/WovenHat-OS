# Stage 24.38 audit - boot-time TPM Name policy loader

Date: 2026-10-21

## Delivered

The release `system-apps` boot path now invokes
`tpm2::load_attestation_name_policy_at_boot` after the durable filesystem
mount attempt and before WovenShell startup. The loader reads the exact
bounded TAN1 record from WovenFS and verifies it with
`signed_app::TRUSTED_PUBLIC_KEY`, the compile-time application release trust
root. It reports a validated Name only on success.

The verified envelope is retained in an allocation-free kernel-owned slot for
later quote authorization. The slot is cleared on every load failure, so a
previously accepted policy cannot survive a failed replacement attempt.

Storage absence is reported as unavailable hardware credentials. Malformed
records, invalid policy metadata, and signature failures are rejected and
also leave hardware credentials disabled. The desktop boot remains available
when no physical TPM policy has been provisioned.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

The 1/2/4-CPU serial logs include the expected
`hardware credentials disabled` marker for the unprovisioned QEMU image.
Serial evidence is preserved in
`audit-artifacts/stage24.38-boot-tpm-policy-loader-20261021`.

## Limits

QEMU has no live physical TPM and therefore exercises the fail-closed missing
policy path, not successful physical provisioning. A production image still
requires authenticated policy provisioning, live TPM quote execution,
credential decision integration, recovery testing, and physical hardware
qualification. The attached USB device was inventory-only and remains
untouched.
