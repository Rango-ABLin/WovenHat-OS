# Stage 27.3 signed update-package producer audit — 2026-10-21

## Scope

This increment adds `tools/wupd-packager`, the release boundary for the
kernel's version-1 `WUPD` envelope. It emits the exact layout consumed by
`kernel::update::verify_package`: bounded generation and image length, SHA-256
image digest, and an Ed25519 signature covering a domain separator, all
immutable manifest fields, and the exact image bytes.

The producer rejects empty or oversized input, zero/non-numeric generations,
input/output aliasing, missing production signing material, and a trusted key
that does not match the signing seed.

## Verification

- `cargo test --offline --manifest-path tools/wupd-packager/Cargo.toml` passed:
  four tests, including layout, digest, signature coverage, and key parsing.
- `cargo clippy --offline --manifest-path tools/wupd-packager/Cargo.toml
  --all-targets -- -D warnings` passed.
- CLI boundary test passed for development packaging, missing production seed,
  mismatched production trust key, and matching production key packaging.
- `python scripts/test-stage27-1-update.py --cpus 1` passed.
- `python scripts/test-stage27-1-update.py --cpus 2` passed.
- `python scripts/test-stage27-1-update.py --cpus 4` passed.

The Stage 27.1 QEMU runs retain serial evidence under the corresponding
`audit-artifacts/stage27.1-*` directories. They revalidate the existing kernel
signature/activation/rollback boundary alongside the producer increment.

## Limits

This is a package-production boundary, not a complete update system. The
privileged long-lived update daemon, IPC/syscall protocol, image-slot storage,
bootloader handoff, power-loss injection and recovery, key rotation, and
physical USB qualification remain open. No physical disk was written.
