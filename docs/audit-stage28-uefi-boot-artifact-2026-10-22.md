# Stage 28 UEFI boot artifact preparation audit — 2026-10-22

## Scope

This slice adds `scripts/prepare-uefi-boot-artifact.py` and
`tests/test_uefi_boot_artifact.py`. It consumes the verified disposable GPT
layout produced by the earlier Stage 28 layout slice, locates its single ESP,
and writes a bounded envelope containing the exact artifact path
`EFI/BOOT/BOOTX64.EFI`, byte length, SHA-256 digest, and payload bytes.

The source may be an explicit regular file or the deterministic test fixture.
Writes are limited to the ESP region, are followed by verification, and restore
the complete original ESP on injected pre-write, post-write, or post-verify
failure.

## Validation

- Stage 28 focused installer tests, including this slice
- Python compilation checks
- `git diff --check`

## Exact limits

- File-backed disposable images only; an explicit test-harness authorization is
  required.
- Artifact size is greater than zero and at most 4 MiB; the ESP is bounded by
  the prior layout metadata.
- The envelope is not a FAT filesystem directory entry and no mount is used.
- No physical/device path, symlink, partition API, filesystem formatter,
  firmware flash, bootloader installer, or hardware access is performed.
- The artifact is not parsed as PE/COFF and no signature policy is asserted.
- The result reports `bootability: not-qualified`; no OS-kernel bootability or
  QEMU boot claim is made by this slice.
