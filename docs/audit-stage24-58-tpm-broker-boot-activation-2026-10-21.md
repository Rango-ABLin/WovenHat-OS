# Stage 24.58 audit — physical TPM broker boot activation

Date: 2026-10-21  
Owner: Codex  
Repository: `phase2/userspace-platform`

## Delivered

- Added the system-app boot activation path for the signed TPM broker.
- Revalidated the signed broker configuration and attestation Name policy
  after durable storage mount.
- Read and Name-validated the configured attestation public area directly from
  the TPM, generated a fresh TPM caller nonce, and invoked the bounded
  PCR-policy credential-root read from Stage 24.57.
- Installed the kernel-owned broker only after all checks succeed; transient
  nonce and policy material are wiped, and all errors leave the broker absent.
- Preserved a usable desktop when no TPM or signed records are available.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- Exact-tree memory acceptance passed on 1 CPU.
- Exact-tree signed system-app acceptance passed on 1, 2, and 4 CPUs.
- All QEMU boots logged the expected fail-closed marker:
  `[TPM] broker activation unavailable; no live TPM; hardware credentials disabled`.
- Evidence copies are in
  `audit-artifacts/stage24.58-tpm-broker-boot-activation-20261021`.

## Limits and next gate

The host and QEMU environment expose no live TPM and no `swtpm` backend, so
the successful activation branch, physical credential login, reboot recovery,
and hardware qualification remain unverified. The attached USB was not
opened, written, or modified.
