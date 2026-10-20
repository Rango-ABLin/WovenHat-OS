# Stage 24.20 audit — signed PCR policy boundary

Date: 2026-10-21  
Owner: Codex  
Status: accepted as a policy-verification boundary

## Delivered

- Added fixed-size `TPP1` policy framing: version, signed length, PCR index,
  reserved bytes, expected SHA-256 digest, and Ed25519 signature.
- The caller supplies the trusted policy key; the envelope cannot select its
  own authority.
- Signature failure, malformed version/length/index/reserved fields, and PCR
  digest mismatch fail closed.
- Added QEMU-feature self-tests for valid policy, tampered signed data, and
  exact PCR matching.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.20-signed-pcr-policy-20261021`.
- Current QEMU has no TPM2 ACPI table; live evidence remains the fail-closed
  absence path plus codec self-tests, not a physical TPM transaction.

## Explicitly incomplete

No runtime release policy is provisioned. This stage does not verify a TPM
quote or attestation key/certificate, does not create or unseal a TPM NV
credential root, and does not bind WovenIdentity to PCR policy. Physical TPM
qualification and the attached USB disk remain untouched.
