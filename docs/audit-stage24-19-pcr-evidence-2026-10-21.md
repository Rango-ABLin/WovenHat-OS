# Stage 24.19 audit — bounded PCR evidence read

Date: 2026-10-21  
Owner: Codex  
Status: accepted as an evidence increment

## Delivered

- Added SHA-256 `PCR_Read` command framing for one bounded PCR index.
- Added response validation for TPM error code, total response length, one
  SHA-256 selection, selected-PCR bit, one digest, and exactly 32 digest bytes.
- Added malformed-selection coverage to the QEMU-feature codec self-test.
- Boot reads PCR 7 only when a validated TPM2 descriptor is present and wipes
  the digest after logging a four-byte inventory prefix.
- Kept PCR output informational. It is not compared to a release policy and
  cannot authorize identity or volume credentials.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.19-pcr-evidence-20261021`.
- Current QEMU firmware has no TPM2 ACPI table, so live acceptance verifies
  the fail-closed absence path and codec self-test rather than a hardware PCR.

## Explicitly incomplete

PCR reads are not signed attestation. Production identity still requires a
provisioned expected PCR policy or verified TPM quote, attestation-key and
certificate validation, sealed NV credential-root provisioning, recovery and
rotation, and physical TPM qualification. The attached USB disk was not
modified.
