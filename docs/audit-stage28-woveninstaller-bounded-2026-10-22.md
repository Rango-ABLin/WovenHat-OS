# Stage 28 WovenInstaller bounded artifact slice — 2026-10-22

## Scope

This slice adds `scripts/package-installer.py`, a file-only artifact boundary
for the existing UEFI disk image emitted by `build.rs` and recorded by
`target/release-validation/validated-image.json`. It copies the image into a
new directory, verifies its digest, checks sector alignment plus the
protective-MBR/GPT signatures, and emits `installer-manifest.json` and
`SHA256SUMS`.

The packager does not enumerate, open, partition, format, mount, or write a
physical disk. Existing `scripts/write-live-usb.ps1` remains a separate,
destructive, administrator-only operation and is not called by this slice.

## Validation

Command:

```text
python -m unittest tests/test_package_installer.py
```

Result: 3 tests passed.

The test fixture is a synthetic file image and covers valid GPT-shape
acceptance, malformed-image rejection, digest-preserving copy, metadata
limits, and refusal to overwrite an existing output directory.

## Limits and next prerequisites

This is not full Stage 28 acceptance. It does not implement a graphical
installer, disk detection, partitioning, ISO generation, installation into a
target disk, rollback, or physical boot qualification. The metadata records
the ISO gap explicitly and identifies the artifact as suitable for QEMU and as
input to a separately reviewed USB writer.
