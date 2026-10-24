# Stage 28 disposable file-image layout audit - 2026-10-22

## Scope

This slice adds `scripts/build-file-image-layout.py`, a file-only test
boundary that prepares a disposable image from a validated installer plan.
The image must already be an explicitly supplied regular file, and the caller
must provide the explicit `--test-harness-created` authorization. Symlinks,
device/physical-drive paths, protected identities, missing files, and invalid
capacities are rejected.

## Layout and verification

The builder writes a protective MBR, primary and backup GPT-like headers and
entry arrays, an 8 MiB 1 MiB-aligned ESP region, and 1 MiB-aligned plan data
regions. Canonical metadata records the deterministic disk identity, regions,
capacity, and file-only mode; its SHA-256 is checked during verification.
Verification also checks signatures, metadata integrity, capacity, bounds,
alignment, and overlap after sorting regions.

## Validation

The focused three-test suite covers deterministic build/signature output,
authorization and protected-target refusal, metadata checksum tampering, and
overlap tampering. Python compilation and `git diff --check` are required.

## Limits

This is disposable-image preparation, not production installation. It does
not invoke partitioning APIs, open physical media, mount or format a
filesystem, write a bootloader, access hardware, or claim ISO/USB support.
