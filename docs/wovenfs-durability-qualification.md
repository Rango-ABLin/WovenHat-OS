# WovenFS durable storage qualification gate (2026-10-25)

## Scope and current evidence

WovenFS v1 already provides 512-byte sector journaling, dual superblock
checkpoints, bounded inodes and directory storage, plus exhaustive
write-failure and recovery tests in `tests/wovenfs_crash.rs`.
The new `tests/wovenfs_durable_recovery.rs` extends the model with a
volatile write cache and a device flush boundary.

These tests **do not** certify actual SSD/HDD/NVMe/AHCI hardware behavior,
cache-flush correctness, atomic sector writes, or production durability.

## Durability contract (mandatory for release)

1. A successful `write_sector` may be volatile. Only a successful `flush`
   establishes persistence of all preceding writes, including the journal
   descriptor, staged data, commit marker, and home-location checkpoint.
2. Journal data and descriptor must be durable **before** the commit sector
   can become durable. The committed record must be durable **before**
   any home block is allowed to become durable.
3. Do not retire the journal until all home writes and the next checkpoint
   have been flushed. A flush error is a transaction failure and must not
   be reported as an acknowledged durable success.
4. At mount, a corrupt or ambiguous journal/superblock must not be silently
   treated as a blank volume. Where safe recovery cannot be established,
   refuse the writable mount and preserve the media for diagnosis.
5. A block driver must implement real write-cache barriers and flushes (e.g.
   ATA FLUSH CACHE or NVMe FLUSH with proper ordering), and propagate
   timeout/reset/media failures instead of silently accepting writes.

## Run the host acceptance matrix

```bash
cargo test --test wovenfs_crash
cargo test --test wovenfs_durable_recovery
cargo fmt --all -- --check
```

## Remaining qualification work

- Simulate torn individual sector writes, reordered persisted sectors, bad
  checksums, silent data corruption, out-of-range LBAs, dropped flushes, and
  abrupt loss after every *write and flush*, not just acknowledged writes.
- Check write/append/truncate/unlink/rename/mkdir/remount and directory
  concurrency across the entire operation matrix; compare recovered results
  against an independent reference model.
- Establish stable baseline and crash test on 1, 2, and 4 vCPUs with distinct
  backing disk images and full reboot (not a reset of in-memory state only).
- Verify write barriers through ATA/AHCI/NVMe/VirtIO drivers, DMA completion,
  transport timeouts, and cache-enabled devices; collect emulator and physical
  media evidence.
- Implement versioned migrations, fsck repair/read-only fallback, space
  exhaustion and wear/error resilience, online scrub, quotas, snapshots,
  backup/restore, volume encryption, secure deletion limitations, and
  compatibility policy; preserve the v1 format until migrations are proven.
- Expand the direct-block-only 4 KiB file limit behind a new on-disk format
  version with crash-safe extent trees and independently verified recovery.
- Retain CI logs, random seeds, disk images, checksums, and test outcomes as
  evidence. Never claim production-grade storage based solely on RAM models.

Release criterion: every designated gate must pass on the exact candidate
commit, and physical power interruption testing must establish the declared
failure model. Until then, mark WovenFS as experimental.
