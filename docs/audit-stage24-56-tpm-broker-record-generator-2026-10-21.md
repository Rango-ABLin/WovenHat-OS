# Stage 24.56 audit - signed TPM broker record generator

Date: 2026-10-21

## Delivered

- Added `tools/tpm-broker-config`, a standalone Rust release tool.
- It validates the reserved NV index, non-zero signing handle, PCR range,
  supported quote scheme, SHA-256 attestation Name, non-zero digest, and
  non-zero NV policy.
- It requires `WOVENHAT_TPM_POLICY_SIGNING_SEED_HEX` and
  `WOVENHAT_TPM_POLICY_TRUSTED_PUBLIC_KEY_HEX`, checks that they match, and
  emits exactly 182 bytes (`TCB1` signed bytes plus Ed25519 signature).
- It has no development-key fallback, TPM access, raw-disk access, or media
  write path.

## Verification

- `cargo clippy --manifest-path tools/tpm-broker-config/Cargo.toml -- -D warnings`:
  passed.
- Synthetic positive generation: passed, 182-byte output.
- Missing signing-key negative control: rejected with exit 2.
- Invalid NV-index negative control: rejected with exit 2.
- Exact-tree memory QEMU 1 CPU: passed, exit 33.
- Exact-tree signed system-app hardware demo 1/2/4 CPUs: passed, including
  sign-in, ten applications, desktop, PS/2, IOAPIC, and smptest.
- The existing Stage 24.55 read-only preflight remained unchanged; no TPM or
  USB state was modified.
- Evidence is preserved under
  `audit-artifacts/stage24.56-tpm-broker-record-generator-20261021`.

## Limits and next prerequisites

The generated record used synthetic policy values and the repository test
signing key; it is not a production credential artifact. A release operator
must use the dedicated production policy key, then validate the record against
the physical TPM's actual attestation Name/PCR/NV state. Physical provisioning,
live quote execution, reboot recovery, and hardware qualification remain open.
