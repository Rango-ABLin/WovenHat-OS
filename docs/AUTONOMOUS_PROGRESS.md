# Autonomous progress log

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-15 — WovenFS: restrict journal replay targets

Branch: `auto/wovenhat-development` (base `897ec82`). The working tree was
clean at the start, so there was no unfinished work to resume. This closes the
second "Open / next" item of the previous entry.

### Defect

`Volume::replay` applied any committed descriptor target below
`total_blocks`, including superblock slots 0/1 and the journal sectors. The
commit checksum covers only the staged data, not the descriptor's target
list, so a retargeted descriptor still counted as committed. Replay would
then write a data or metadata sector over a checkpoint, or over the journal
sectors it was still reading.

### Change

- `kernel/src/wfs.rs`: `SuperBlock::journal_may_target(lba)` allows only
  `bitmap_start..total_blocks` (bitmap, inode table, data). Replay discards a
  transaction with any other target, the same way it handles a torn commit.
  `commit` refuses with `Corrupt` before writing to the journal if a staged
  target is outside that range. All current staging sites derive targets
  from the bitmap/inode/data geometry, so this check is defense in depth.
- `tests/wovenfs_crash.rs`:
  `journal_replay_refuses_targets_outside_the_transactional_regions` leaves a
  committed, unretired transaction in the journal. A control mount must
  replay it and retire the journal. For targets at slot 0, slot 1, the
  journal descriptor, the first payload sector, the last journal block and
  `total_blocks`, mount must stay consistent and leave the image
  byte-identical.
- `docs/wovenfs-on-disk-format.md`: documents the allowed target range.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New test with replay check reverted to `target >= total_blocks` (negative control) | FAILS as expected: `superblock slot 0: replay must discard the transaction without writing`; check restored afterwards |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 16 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 126 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

### Open / next

- The descriptor (block count and target list) is still not covered by any
  checksum. A bit flip in a target that stays within the allowed regions
  would replay committed data to the wrong block. Fixing this changes the
  commit sector format (for example, hashing the descriptor into the commit
  checksum) and needs a compatibility decision.
- Pointer aliasing (two inodes claiming one block) is still detected only by
  `check()` (test/self-test builds), not on the write path.
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

## 2026-10-15 — WovenFS: validate superblock geometry on mount

Branch: `auto/wovenhat-development` (base `073780e`). The working tree was
clean at the start, so there was no unfinished work to resume. This closes the
first "Open / next" item of the previous entry.

### Defect

`Volume::mount` adopted any superblock whose magic, version and checksum
matched. The checksum is unkeyed FNV-1a, so a crafted or mis-written image
with re-sealed checksum was accepted with any geometry: a data region
overlapping the inode table or journal, a journal smaller than one
transaction, a bitmap too small for `total_blocks`, `total_blocks` beyond the
device, zero inodes, or a root inode outside the table. The inode-pointer
check added in the previous checkpoint is relative to `data_start` /
`total_blocks`, so it was only as strong as this geometry.

### Change

- `kernel/src/wfs.rs`: `SuperBlock::geometry_fits(device_sectors)` checks,
  with overflow-checked arithmetic, that slots, journal, bitmap, inode table
  and data are ordered and non-overlapping; the journal holds a full
  transaction (`MAX_TXN_BLOCKS + 2`); the bitmap covers `total_blocks`;
  `inode_count > 0`; `root_inode` is in `1..=inode_count`; and
  `data_start < total_blocks <= device.sector_count()`. `mount` discards a
  slot that fails it, exactly like a failed checksum, so a valid older
  checkpoint is still used and with none left mount returns `Corrupt`
  (reported as `NotWovenFs` by `mount_ata_into_vfs`). Valid layouts
  produced by `format`/`plan` are unaffected.
- `tests/wovenfs_crash.rs`: `superblock_geometry_that_does_not_fit_is_refused`
  re-seals 11 bad geometries. With only slot 0, mount must return `Corrupt`
  and leave the image byte-identical. With a valid older slot 0, a bad newer
  slot 1 must be skipped and the volume must mount consistent from slot 0.
- `docs/wovenfs-on-disk-format.md`: documents the geometry invariant.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New test with the `geometry_fits` check disabled (negative control) | FAILS as expected at the "mount must refuse the geometry" assertion; check restored afterwards |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 15 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 125 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

### Open / next

- Pointer aliasing (two inodes claiming one block) is still detected only by
  `check()` (test/self-test builds), not on the write path.
- Journal replay accepts any target below `total_blocks`, including the
  superblock slots. Staged targets are not yet restricted to the regions a
  transaction may legitimately write.
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

## 2026-10-15 — WovenFS: refuse inode block pointers outside the data region

Branch: `auto/wovenhat-development` (base `f04a994`). The working tree was
clean at the start, so there was no unfinished work to resume.

### Defect

`Volume::inode` / `inode_staged` trusted any inode whose checksum matched.
The checksum is an unkeyed FNV-1a, so it catches bit rot but not a
mis-written or crafted image. A direct pointer at superblock slot 1, the
journal, the bitmap, the inode table, or past the end of the device was used
as given:

- `write_file` reused the block in place and wrote file data over metadata.
  The negative control below shows this returning `Ok(())`.
- `unlink` cleared the bitmap bit of a metadata block.

### Change

- `kernel/src/wfs.rs`: new `checked_inode`, used by both inode readers. It
  returns `Corrupt` if any non-zero direct pointer lies outside
  `data_start..total_blocks`, and for a used inode whose `kind` is not
  file/dir or whose `size` exceeds `MAX_FILE_SIZE`. Free inodes are checked
  too, because a stale directory entry can reach one through `unlink`.
- `tests/wovenfs_crash.rs`: `inode_block_pointers_outside_the_data_region_are_refused`
  re-seals an inode's checksum with `direct[1]` aimed at each metadata region
  and past the end. It requires `read_file`, `write_file` and `unlink` to
  return `Corrupt` and leave the image byte-identical.
- `docs/wovenfs-on-disk-format.md`: documents the pointer invariant.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New test against unfixed `wfs.rs` (negative control) | FAILS as expected: `superblock slot 1: write must not reuse a metadata block`, `left: Ok(())` |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 14 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 124 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

Note: the first test build failed once with `LNK1104` (the output exe was
locked; no test process was running). It succeeded on retry with nothing
changed.

### Open / next

- Superblock geometry is still not validated on mount. A checksummed
  superblock whose `data_start` overlaps the inode table or journal, or
  whose `total_blocks` exceeds the device, is accepted. That is the natural
  next checkpoint, since this check depends on that geometry.
- Pointer aliasing (two inodes claiming one block) is still detected only by
  `check()` (test/self-test builds), not on the write path.
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

## 2026-10-15 — WovenFS: atomic create-with-contents

Branch: `auto/wovenhat-development` (base `0a87401`).

Resumed uncommitted work found in `kernel/src/wfs.rs` and
`tests/wovenfs_crash.rs`; reviewed the diff and validated it before
committing. No new work was started.

### Defect

Writing to an absent `/wfs/<name>` committed an empty file in one transaction
and its contents in a second. A failed second transaction (`NoSpace`, device
error) returned an error while leaving an empty file on disk. Affected
`write_vfs_path_at_with_publish` (offset 0) and `write_ata_file`.

### Change

- `Volume::create_file(dir, name, mode, data, now)` stages create and write
  into one journaled transaction.
- `create` / `write_file` refactored onto shared `stage_create` /
  `stage_write`; `stage_create` now reads the directory inode through the
  transaction view (`inode_staged`).
- Both ATA/VFS write paths use `create_file` for absent files.
- Tests: crash at every write point of `create_file` leaves either no file or
  the full contents; out-of-data-blocks `create_file` leaves no file and a
  consistent volume (and documents the old create-then-write orphan).

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 13 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 123 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU `scripts/test-wovenfs-device.py` / Stage 15.x on 1/2/4 CPUs | **NOT RUN** — no `qemu-system-x86_64` on this host |

### Open / next

- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this is counted as stage acceptance.
- Existing-file writes that grow a file still fail atomically (single
  transaction) — unchanged.
