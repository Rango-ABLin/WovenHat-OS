# Stage 28 disposable-image acceptance harness — 2026-10-22

## Scope

`test-stage28-disposable-acceptance.py` composes the existing validated-plan
validator, file-image layout builder, FAT32-compatible formatter, EFI artifact
preparer, and file-backed executor. It creates a new 64 MiB regular temporary
image and never opens a device path, mounts a filesystem, invokes a partition
tool, or accesses physical media.

The executor is run successfully and with both atomic-replacement failure
injections on a separate disposable copy. Its existing contract replaces the
whole file with a deterministic mock filesystem payload, so it is not mixed
with the final partitioned-image bytes. The final image is assembled by the
layout, formatter, and EFI stages and is independently re-verified.

## Acceptance checks

- Planner emits a valid canonical GPT/FAT32 file-only plan.
- Executor success produces a verified mock manifest digest.
- Executor `before-replace` and `after-replace` failures restore the source
  byte-for-byte.
- Layout builder verifies protective MBR, primary/backup GPT metadata, bounds,
  alignment, and metadata digest.
- Formatter verifies deterministic FAT32-compatible data-region metadata and
  all injected rollback cases.
- EFI preparation verifies the exact `EFI/BOOT/BOOTX64.EFI` envelope, payload
  digest, and all injected rollback cases.
- A deterministic acceptance manifest records the GPT, data, EFI, executor,
  and manifest digests.

## Limits

This is disposable file-image acceptance only. It is not physical installation,
filesystem mounting, bootloader installation, ISO generation, firmware boot
qualification, hardware qualification, or release certification. The EFI
artifact remains `bootability: not-qualified`.
