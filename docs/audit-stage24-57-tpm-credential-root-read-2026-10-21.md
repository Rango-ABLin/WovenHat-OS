# Stage 24.57 audit — physical TPM credential-root read boundary

Date: 2026-10-21  
Owner: Codex  
Repository: `phase2/userspace-platform`

## Delivered

- Added bounded `NV_ReadPublic` access for the configured credential-root
  index.
- Added authenticated policy-session `NV_Read` framing and cpHash/HMAC
  derivation for a single bounded 32-byte read at offset zero.
- Added the kernel promotion path: validate signed NV metadata, start a
  managed policy session, apply the configured PCR policy, read the root,
  refresh the TPM nonce, flush the session on every path, and wipe transient
  root/authentication state.
- Kept the root and TPM authorization material out of userspace and durable
  storage.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel Clippy passed with `system-apps` and `stage24-5-test` feature gates.
- Exact-tree memory acceptance passed on 1 CPU.
- Exact-tree signed system-app acceptance passed on 1, 2, and 4 CPUs,
  including desktop readiness, PS/2/IOAPIC, SMP migration, and acknowledged
  TLB-shootdown checks.
- Evidence copies are in
  `audit-artifacts/stage24.57-tpm-credential-root-read-20261021`.

## Limits and next gate

The test machine exposes no accessible TPM and has no `swtpm` backend. QEMU
therefore exercises the fail-closed no-TPM path, not a live NV read or PCR
policy execution. Physical TPM provisioning, live read/quote qualification,
reboot persistence, and hardware validation remain mandatory before any
production-release claim. The attached USB was not opened, written, or
modified.
