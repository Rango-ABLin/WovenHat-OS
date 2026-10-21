# Stage 24.50 audit - hardware credential decision boundary

Date: 2026-10-21

## Delivered

`authorize_hardware_credential` binds a bounded identity context to the active
credential-root generation by deriving the expected TPM quote nonce. It then
requires the complete existing `authorize_quote` chain: signed TAN1 policy,
attestation-key Name binding, expected nonce/PCR evidence, and RSA/ECC
signature verification. Successful output is a generation-tagged
`CredentialDecision` containing attestation evidence only. Inactive or revoked
roots are rejected before quote authorization.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU self-test covers decision rejection for mismatched attestation data
and for a revoked root. Serial evidence is preserved in
`audit-artifacts/stage24.50-hardware-credential-decision-20261021`.

## Limits

QEMU has no live TPM, so a successful physical hardware credential decision is
not proven. The identity session/login path does not yet consume
`CredentialDecision`, and reboot recovery and physical qualification remain
open. The attached USB device was not modified.
