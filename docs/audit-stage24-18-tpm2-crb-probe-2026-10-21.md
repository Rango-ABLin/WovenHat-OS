# Stage 24.18 audit — bounded TPM2 CRB probe

Date: 2026-10-21  
Owner: Codex  
Status: accepted as a transport increment

## Delivered

- Added a serialized TPM2 CRB `GetRandom` transaction over the validated ACPI
  control area.
- Validated CRB interface type, command/response capacity and address ranges,
  fatal status, TPM response code, response size, and bounded output length.
- Used a fixed poll-count timeout because the probe runs before timer IRQs are
  enabled; it cannot spin forever on a wedged device.
- Wiped probe output immediately after inventory logging. Random output is not
  persisted and is explicitly not treated as a stable credential root.
- Kept absent TPM hardware fail-closed. Current QEMU firmware has no TPM2
  table, so the live gate exercises the absence path rather than claiming a
  hardware transaction.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.18-tpm2-crb-probe-20261021`.

## Explicitly incomplete

This is not hardware-backed identity. Production credentials still require
authenticated PCR/attestation policy, sealed TPM NV/root provisioning,
recovery/rotation, and physical TPM qualification. The host TPM query required
Administrator elevation; no TPM presence was asserted from Windows. The USB
flash disk was not modified.
