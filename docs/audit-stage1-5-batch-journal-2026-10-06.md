# Stage 1–5 bounded batch data-journal audit — 2026-10-06

## Scope

The FAT32 persistence layer now has a separate `WDJ2` batch journal for up to
four file replacements. The existing `WDJ1` single-file format and WMD1/WMD2
ABIs are unchanged. A prepared batch records each path, prior existence, prior
bytes, intended length, and intended checksum. Metadata intents are published
before data and finalized after all data writes; mount recovery removes stale
metadata intents when rolling data back. A committed batch is retired only
after all new images match their recorded lengths and checksums; otherwise
recovery restores the prior images.

The new `persist_paths_atomic` API rejects empty, duplicate, malformed, over-
capacity, and over-four-entry batches. Journal records have bounded parsing,
path confinement, version checks, and a whole-record checksum.

## Evidence

- `cargo check -p wovenhat-kernel --target x86_64-unknown-none` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` — passed.
- `cargo test -p libwoven` — passed.
- `python scripts/test-storage-qemu.py --cpus 1` — passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 2` — passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 4` — passed, exit 33.
- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\RUN-STAGE10.7.ps1` — passed, including host regressions, warning-denying Clippy, host tests, the 1/2/4-CPU memory matrix, live networking, and isolated Stage 10.4–10.7 preservation boots.

The workspace-wide `cargo test --workspace --all-targets` remains unsuitable
for this `no_std` kernel because it attempts to link the kernel's panic and
allocation handlers against the host `std` test harness; that pre-existing
failure was preserved and not suppressed.

## Limits

This increment implements the bounded multi-file data rollback/replay
boundary. The live disposable FAT32 mutation test exercises a two-file
`persist_paths_atomic` commit and readback on the QEMU storage path. Its
recovery subtest simulates a prepared partial write, a committed partial
write, and a fully committed batch before invoking recovery and verifying
on-disk bytes. The live commit also verifies a WMD2 inode metadata record
(`uid=1001`, `gid=1002`, `mode=0640`) after the batch completes. The kernel
self-test additionally injects pending WMD1/WMD2 metadata intents into the
prepared and partial-commit recovery cases and verifies that rollback removes
them. The kernel
self-test covers WDJ2 encoding, parsing, checksum rejection, and capacity
bounds. It does not establish
physical storage/DMA qualification, unclean-shutdown hardware evidence,
arbitrary-size transactions, metadata transactions, or full Stage 1–5
production completion.
