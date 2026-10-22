# Stage 28 disposable file-backed installation executor audit — 2026-10-22

This slice adds `scripts/execute-file-install.py` and focused host coverage in
`tests/test_file_install_executor.py`. It consumes the JSON emitted by the
validated installer-plan boundary and requires an explicitly supplied regular,
non-symlink image file. The plan target identity is bound to that image's
pre-install SHA-256 as `file-image:<digest>`.

The executor writes a deterministic bounded mock filesystem payload containing
an execution manifest and fixed test files. It stages both a backup and a new
image in the target directory, fsyncs them, and uses `os.replace` for atomic
replacement. Verification failure and injected failure before or after replace
restore the original image and clean temporary files.

The executor rejects device-looking paths, protected target identities,
symlinks, mismatched image digests, and out-of-bound images. It contains no
physical-device access, `\\.\PhysicalDrive` support, mount, format, partition,
or bootloader command. This is not real installation, does not create a
bootable OS image, and is not Stage 28 acceptance or release certification.
