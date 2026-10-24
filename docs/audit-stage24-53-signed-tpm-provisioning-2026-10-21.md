# Stage 24.53 audit - signed TPM broker provisioning record

Date: 2026-10-21

## Delivered

- Added the fixed-size signed `TCB1` broker-policy envelope.
- Authenticated and bounded the reserved credential-root NV index,
  attestation signing handle, quote scheme, PCR selection, expected PCR
  digest, attestation-object Name, and NV policy.
- Added WovenFS durable persistence and boot-time read/revalidation APIs.
- Kept authorization secrets out of the durable record; a valid file cannot
  by itself install a broker, create a root, or enable hardware login.
- Added QEMU self-test coverage for malformed-record rejection.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with signed-app inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.53-signed-tpm-provisioning-20261021`.

## Limits and next prerequisites

The current QEMU firmware exposes no TPM2 device and has no provisioning
record. Physical TPM provisioning and secure authorization-secret injection
remain required before installing the broker. Live credential login,
cross-reboot recovery, and hardware qualification remain open. The attached
USB device was not modified or used as a write target.
