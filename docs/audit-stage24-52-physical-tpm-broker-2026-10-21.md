# Stage 24.52 audit - physical TPM credential broker boundary

Date: 2026-10-21

## Delivered

- Added `HardwareCredentialBroker`, a kernel-owned configuration and execution
  boundary for physical TPM credential requests.
- Validated the CRB descriptor, active credential-root lease, signed TAN1
  attestation Name policy, ReadPublic key binding, PCR selection, signing
  handle, and bounded signing authorization value before installation.
- Added a one-request path that derives the exact account challenge, starts a
  fresh managed policy session, applies `PolicyPCR`, executes authenticated
  `TPM2_Quote`, verifies the quote, flushes the session on every success or
  failure path, and publishes only a move-only `CredentialDecision`.
- Added syscall 234 and libwoven request/consume wrappers. The account context
  is taken from the kernel owner/session binding; userspace cannot provide a
  replacement account name or receive TPM evidence.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with the signed-app
  build inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in, ten
  applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.52-physical-tpm-broker-20261021`.

## Limits and next prerequisites

The current QEMU firmware exposes no TPM2 ACPI device, so no broker is
installed and no physical credential login is claimed. Release provisioning
must supply the validated NV root, PCR policy, signed attestation Name policy,
attestation handle/public key, and signing authorization value. Cross-reboot
recovery, physical TPM testing, and hardware qualification remain open. The
attached USB device was not modified or used as a write target.
