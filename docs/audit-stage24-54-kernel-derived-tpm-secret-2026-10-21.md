# Stage 24.54 audit - kernel-derived TPM authorization secret

Date: 2026-10-21

## Delivered

- Extended `CredentialRootLease` with its validated NV index and policy
  binding; rotations now reject metadata changes.
- Added kernel-only attestation authorization derivation using a
  domain-separated credential-root derivation.
- Added provisioned broker installation checks for the signed `TCB1` policy,
  exact attestation Name, and matching root metadata.
- Wiped the derived authorization value before installation returns. No raw
  authorization secret is accepted from userspace or stored in WovenFS.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with signed-app inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.54-kernel-derived-tpm-secret-20261021`.

## Limits and next prerequisites

No physical TPM is available in the QEMU image, so the provisioning install
path has not been exercised against a live device. Physical TPM provisioning,
live credential login, cross-reboot recovery, and hardware qualification
remain open. The attached USB device was not modified or used as a write
target.
