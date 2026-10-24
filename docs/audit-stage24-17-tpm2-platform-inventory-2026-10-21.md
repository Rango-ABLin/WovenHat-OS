# Stage 24.17 audit — TPM2 platform discovery boundary

Date: 2026-10-21  
Owner: Codex  
Status: accepted as an integration boundary

## Delivered

- Added bounded `Tpm2Info` data to validated ACPI discovery.
- Validated the TPM2 signature, table length, non-zero control-area address,
  and retained platform class/start method metadata.
- Logged an explicit boot result. QEMU without a TPM reports:
  `[TPM] ACPI TPM2 unavailable; hardware credential root not provisioned`.
- Added ACPI self-test coverage for a valid TPM2 descriptor and zero-address
  rejection.
- Kept TPM table presence strictly informational. No random value, firmware
  address, or software measurement is promoted into trusted credentials.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.17-tpm2-platform-inventory-20261021`.
- Read-only host inventory found the attached USB as a separate 15.7-GB GPT
  disk; it was not written, formatted, mounted, or passed destructively to
  QEMU.

## Explicitly incomplete

This stage does not claim hardware-backed identity. The next implementation
must provide CRB transport, validate PCR/attestation evidence against a
provisioned policy, and implement sealed credential-root
provisioning/recovery. A TPM
query on the Windows host required Administrator elevation, so no physical
TPM result was asserted from the host. Physical qualification remains open.
