# Stage 21.6 WovenStore native persistence audit — 2026-10-20

## Scope

This increment wires the bounded WSTC catalog record to the existing
capability-gated native WovenFS writer and direct readback path. It uses only a
disposable QEMU ATA image; no physical USB is written.

## Implemented contract

- Stage 21.6 reuses the audited Stage 15.2 WovenFS scratch-volume prerequisite.
- `UpdateState::authorized_writer` is the only path used to obtain the private
  WovenFS writer token, and it requires `Capability::StorageIo`.
- The probe creates `/wfs/packages`, writes `/wfs/packages/catalog.wstc`
  through the native journal, reads it back from the mounted volume, and
  decodes the checksum-verified record.
- The feature gate suppresses earlier unrelated FAT32/Stage 15.2 exit blocks
  while retaining the required native storage implementation.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-6-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.6 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.6-1cpu`.
- Stage 21.6 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.6-2cpu`.
- Stage 21.6 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.6-4cpu`.

## Remaining acceptance prerequisites

Atomic catalog-plus-payload publication, persistent install/update/removal
transactions, power-loss injection and recovery, package-service IPC and
authority ownership, and physical USB qualification remain unimplemented.
