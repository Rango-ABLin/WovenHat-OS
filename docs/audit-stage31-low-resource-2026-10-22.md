# Stage 31 low-resource filesystem evidence — 2026-10-22

Added `full_data_region_rejects_write_without_partial_commit` to
`tests/wovenfs_crash.rs`. The test formats a 48-sector volume with eight
inodes, fills the available data region, attempts a two-sector write when only
one block remains, and requires `Error::NoSpace`. It then remounts the image,
verifies the previously committed file byte-for-byte, confirms the failed file
has size zero, and runs the filesystem consistency check.

Command:

```text
cargo test --test wovenfs_crash
```

Result: PASS, 16 tests. This is deterministic host coverage of WovenFS
disk-full atomicity and consistency. It does not yet qualify whole-kernel low
RAM behavior, block-device failure injection, or physical media exhaustion.
