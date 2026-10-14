# Stage 1–5 immediate batch-rollback hardening — 2026-10-13

## Scope

`persist_paths_atomic` now treats the on-volume WDJ2 record as an active
transaction, not only as reboot evidence. After the prepared record is durable,
any later write, metadata, commit-record, or journal-retirement error invokes
the same generation/checksum-aware recovery path before returning the original
error. The file-page cache is invalidated before recovery so subsequent VFS
reads cannot retain pages from the failed attempt. Recovery supports direct
FAT32 devices and FAT32 partitions discovered through the existing MBR/GPT
paths. A committed batch is retained when all new images match; an incomplete
or prepared batch restores prior bytes and removes stale metadata intents.

The recovery result is deliberately not substituted for the original write
error. If rollback itself fails, the operation still fails closed and the
durable WDJ2 record remains available for the next mount attempt.

## Evidence

- `cargo clippy -p wovenhat-os --tests -- -D warnings` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` — passed.
- `cargo test -p wovenhat-os --test wovenfs_crash` — 11 passed.
- `python scripts/test-storage-qemu.py --cpus 1 --timeout 180` — passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 2 --timeout 180` — passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 4 --timeout 180` — passed, exit 33.

Serial evidence is retained at:

- `target/storage-regression-1-debug/serial.log`
- `target/storage-regression-2-debug/serial.log`
- `target/storage-regression-4-debug/serial.log`

## Limits

This closes a runtime-recovery defect in the bounded WDJ2 transaction. It does
not close the documented Stage 1–5 gaps: metadata transactions beyond the
bounded sidecars, arbitrary-size/full-volume rollback, physical storage/DMA
qualification, or unclean-shutdown testing on physical media.
