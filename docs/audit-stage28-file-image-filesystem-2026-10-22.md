# Stage 28 disposable file-image filesystem formatter audit — 2026-10-22

## Scope

This slice adds a formatter/verifier for the one `data` partition produced by
`scripts/build-file-image-layout.py`. It is intentionally limited to regular
file images created by the test harness. It does not format physical disks,
mount filesystems, install bootloaders, generate installation media, or qualify
hardware.

## Implemented boundary

- Refuses symlinks, non-regular files, device paths, protected identities, and
  missing harness authorization.
- Revalidates the prior layout metadata and requires exactly one data region.
- Writes only that region, leaving the protective MBR, GPT metadata, and ESP
  bytes untouched.
- Emits deterministic FAT32-compatible metadata: BPB, backup boot sector,
  FSInfo and backup FSInfo, two matching FATs, reserved entries, root cluster,
  and label `WOVENHAT`.
- Verifies sector geometry, FAT/root bounds, signatures, copies, label, and
  byte-for-byte deterministic output.
- Restores the original selected region after injected failures before write,
  after write, and after verification.

## Validation

The focused Stage 28 chain passed:

```text
34 tests: OK
python -m py_compile: OK
git diff --check: OK
```

The chain includes the new filesystem tests and the existing layout, package,
ISO-probe, inventory, plan, executor, and UI-model tests. No physical disk or
mount operation was attempted.

## Limits and prerequisites

The result is not a production filesystem formatter or installer. Real
partitioning, filesystem creation on approved hardware, bootloader writes,
power-loss recovery, ISO/USB boot qualification, and hardware testing remain
separate release-certification prerequisites.
