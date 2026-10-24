# Stage 24.51 audit - identity hardware-decision consumption

Date: 2026-10-21

## Delivered

- Added a move-only `CredentialDecision` handoff from the TPM boundary into a
  bounded one-entry kernel pending slot.
- Added syscall 233, `IdentityConsumeHardware`, which consumes a pending
  decision only for the exact owner and session.
- Bound consumption to the active account name context used to derive the TPM
  challenge, the credential-root generation, and a one-shot hardware binding.
- Rejected absent, locked, mismatched-owner/session, mismatched-context, and
  duplicate consumption paths fail-closed; `IdentityHardware` records the
  result.
- Added the broker-facing `authorize_and_publish_hardware_credential` entry
  point. TPM quote material remains kernel-only and is not copied to
  userspace.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed.
- Memory QEMU, 1 CPU: passed, exit 33.
- Signed system-app hardware demo, 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.51-identity-hardware-consumption-20261021`.

## Limits and next prerequisites

No live physical TPM broker has published a decision yet, so this stage does
not claim physical credential login, reboot recovery, or release readiness.
Physical TPM qualification and real hardware testing remain required. The
attached USB device was not modified or used as a write target.
