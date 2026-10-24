# Stage 28 ISO capability probe - 2026-10-22

## Scope

This slice adds a read-only, file-only capability probe at
`scripts/probe-installer-iso.py` with focused tests in
`tests/test_installer_iso_probe.py`. It does not generate an ISO, mount a
filesystem, access a physical disk, or alter the existing package-installer
slice.

The probe checks the exact prerequisites needed before an ISO implementation
could be attempted: a regular file-backed source image with protective MBR and
primary GPT signatures; an ISO builder executable (`xorriso`, `mkisofs`,
`genisoimage`, or `oscdimg`); and a file-backed EFI tree containing a non-empty
`EFI/BOOT/BOOTX64.EFI`.

It reports missing prerequisites as blockers and never reports ISO support. A
future builder must generate an artifact and boot that exact artifact in QEMU
before the capability can be accepted.

## Validation

```text
python -m unittest tests/test_installer_iso_probe.py tests/test_package_installer.py
python -m py_compile scripts/probe-installer-iso.py tests/test_installer_iso_probe.py
python scripts/probe-installer-iso.py --source-image target/release-validation/wovenhat-os-uefi.img
git diff --check
```

Results:

- 6 focused tests passed (3 probe, 3 unchanged installer tests).
- Python compilation checks passed.
- Diff whitespace check passed.
- The live probe exited with status 2 and reported these blockers:
  - the requested release-validation image path is not present;
  - no supported ISO builder is installed;
  - no EFI tree was supplied;
  - `EFI/BOOT/BOOTX64.EFI` therefore cannot be verified.

## Limits

No ISO was generated, and no QEMU ISO boot validation was run. ISO support is
not implemented or claimed. The existing Stage 28 file-backed GPT UEFI image
artifact remains the only accepted installer artifact boundary.
