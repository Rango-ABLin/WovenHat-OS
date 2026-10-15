# Autonomous progress log

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-15 — WovenFS: PowerShell QEMU harness; Stage 15.1/15.2 rerun

Branch: `auto/wovenhat-development` (base `4e9e236`). The working tree was
clean at the start, so there was no unfinished work to resume. This takes the
first open item from the previous entry: every WovenFS checkpoint so far
lacked QEMU evidence because this host has no Python interpreter.

### Change

- New `scripts/test-wovenfs-qemu.ps1 -Stage 15.1|15.2 [-Cpus 1|2|4]`. It is a
  port of `scripts/test-stage10-runtime.py --stage 15.1` (`q35`, boot disk
  only) and `scripts/test-wovenfs-device.py` (`pc`, blank scratch image as
  ATA primary master), with the same QEMU arguments. Pass rule: exit 33,
  stage marker, `[SMP] online=N expected=N`, and no `FAILED`/`TIMEOUT` stage
  line or panic. That is the runtime harness's rule; it is stricter than the
  device script's, which checked only exit code and marker. Raw physical
  drives are not supported here; `test-wovenfs-physical.ps1` (identity
  gated) still covers that. Reuses `.cargo-build`/`target`. Evidence
  (`serial.log`, `qemu.log`, `qemu.err`, `command.txt`, `result.txt`, scratch
  image) is kept under `audit-artifacts/` on pass and on failure. On timeout
  it kills only the QEMU process it started.
- `docs/stage-status.md`: Stage 15 revalidation note.
- No kernel or test source changed.

### Results (run on this host, 2026-10-15, QEMU 11.1.0, TCG)

| Command | Result |
|---|---|
| `test-wovenfs-qemu.ps1 -Stage 15.1 -Cpus 1` | PASS, exit 33 (`wovenfs-stage15.1-1cpu-20261015-180415-952`) |
| same, `-Cpus 2` | PASS, exit 33 (`…-2cpu-20261015-180607-949`) |
| same, `-Cpus 4` | PASS, exit 33 (`…-4cpu-20261015-180725-769`) |
| `test-wovenfs-qemu.ps1 -Stage 15.2 -Cpus 1` | PASS, exit 33, all 16 `[S15.2]` sub-checks PASS (`wovenfs-stage15.2-1cpu-20261015-180824-618`) |
| same, `-Cpus 2` | PASS, exit 33 (`…-2cpu-20261015-181013-975`) |
| same, `-Cpus 4` | PASS, exit 33 (`…-4cpu-20261015-181134-184`) |
| Negative control: `-Stage 15.2 -Cpus 1 -TimeoutSeconds 3` | FAILS as expected (script exit 1): timeout, both markers missing, `QEMU exit -1` (`…-1cpu-20261015-181254-100`). No QEMU process left running. |

Directory names are under `audit-artifacts/` (Git-ignored, kept locally).
Cargo build, Clippy and host tests were not rerun: nothing they compile
changed since `4e9e236`, whose entry below records them passing.

### Open / next

- The Python originals were not run (no interpreter), so the two harnesses'
  agreement is by code review, not side-by-side runs.
- Physical USB qualification (`test-wovenfs-physical.ps1`) needs elevation
  and authorization to erase the device; not rerun.
- Other Stage 15 gates (15.3–15.9) and earlier stages still go through
  `test-stage10-runtime.py`; they were not rerun in QEMU here.
- Cross-inode block aliasing and `unlink` ignoring `links` remain open.

## 2026-10-15 — WovenFS: commit checksum covers the journal descriptor

Branch: `auto/wovenhat-development` (base `346e4eb`). The working tree was
clean at the start, so there was no unfinished work to resume. This takes the
"uncovered journal descriptor" open item from the previous entries.

### Defect

The commit checksum covered only the staged data sectors, not the
descriptor's target list. A descriptor damaged after its commit landed still
looked committed. Replay checked only that each target was inside the
transactional regions (`bitmap_start..total_blocks`). So a changed target
there made replay write a staged block over the wrong bitmap, inode-table or
data block. Mount then returned success on a corrupted volume.

### Change

- `kernel/src/wfs.rs`: `staged_checksum` replaced by `journal_checksum`.
  It hashes the descriptor's used bytes (`[..24 + count * 8]`: magic, id,
  count, targets), then the data sectors, with the same FNV-1a chain. `commit`
  and `replay` both use it. The `journal_may_target` range check stays as an
  independent guard. Commit-sector layout and offsets are unchanged.
- `tests/wovenfs_crash.rs`: new `pending_transaction` helper (carries the
  existing replay-and-retire control) and `retarget_journal_block` (with
  optional commit reseal). New test
  `journal_replay_refuses_a_descriptor_changed_after_commit` covers in-range
  targets (bitmap, last inode-table block, last data block). For each one, a
  resealed control proves replay writes the target, and the unsealed
  retarget must leave the image byte-identical.
  `journal_replay_refuses_targets_outside_the_transactional_regions` now
  reseals, so it still isolates the range check rather than passing on the
  checksum.
- `docs/wovenfs-on-disk-format.md`: documents what the commit checksum covers
  and the compatibility consequence below.

### Compatibility limit (intentional, documented)

A transaction committed but not yet retired by a kernel built before this
change is discarded on first mount by this kernel. Its commit checksum does
not include the descriptor. The volume presents the consistent prior state
for that one transaction rather than the new one. Retired journals and all
other on-disk structures are unaffected. `FORMAT_VERSION` was not bumped.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New test, unfixed `wfs.rs`, as written | FAILS, but in the resealed control (`bitmap: a resealed descriptor must be replayed to its target`). The test reseals with the new checksum, which the old code rejects, so this run does not show the defect. |
| New test, unfixed `wfs.rs`, control temporarily unsealed (negative control) | FAILS as expected: `bitmap: inconsistent: Corrupt`. Old code replayed the unsealed retarget onto the bitmap and mounted a corrupt volume. Both files were restored afterwards and checked with `cmp`. |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 22 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 132 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: see below |

Correction to earlier entries: QEMU is installed here.
`C:\Program Files\qemu\qemu-system-x86_64.exe` reports version 11.1.0. It
is not on `PATH`, which is why `which` did not find it. The Stage 15.2 device
harness (`scripts/test-wovenfs-device.py`) is a Python script, though, and
this host has only the Microsoft Store `python` stub, no interpreter. So the
QEMU gate still cannot run here as written.

### Open / next

- Install Python, or port `scripts/test-wovenfs-device.py` to PowerShell,
  then run the Stage 15.1/15.2 QEMU acceptance on 1/2/4 CPUs. Until then
  none of the WovenFS checkpoints count as stage acceptance.
- FNV-1a detects corruption but is not a MAC. A deliberately crafted image
  can still reseal a descriptor. That is consistent with the rest of the
  format's checksums and is out of scope for this checkpoint.
- Cross-inode block aliasing and `unlink` ignoring `links` remain open.

## 2026-10-15 — WovenFS: read_file refuses directory inodes

Branch: `auto/wovenhat-development` (base `e0a8f47`). The working tree was
clean at the start, so there was no unfinished work to resume. This takes the
first open item from the previous entry: what `read_file` does when an entry
names a free inode or a directory.

### Defect

`read_file` checked only `is_used()`, not `kind`. A directory inode read as a
file whenever its `data_checksum` matched its blocks. Directories never
maintain `data_checksum` (format leaves it 0), so in practice a read failed
with `Corrupt`. A crafted or mis-written image that set it, however, made
`read_file` return the raw directory entry table as file contents. This
applied both to a direct call and to a directory entry retargeted at a
directory inode. The reachable kernel paths were `read_ata_file`, the
mount-time VFS import loop, and the read-modify-write in
`write_vfs_path_at_with_publish`. A free inode already returned `NotFound`.

### Change

- `kernel/src/wfs.rs`: `read_file` returns `NotADirectory` for any used
  inode whose kind is not `KIND_FILE`. That is the wrong-kind error
  `write_file` already uses; `task.rs` maps it to `FileError::NotFound`. No
  new error variant, so no ABI or mapping change.
- `tests/wovenfs_crash.rs`: `read_file_refuses_a_directory_inode` re-seals
  the root inode with a `data_checksum` matching its entry table. It then
  checks that a direct `read_file(1)` and a read through an entry retargeted
  at inode 1 both return `NotADirectory`, that an entry retargeted at a free
  inode returns `NotFound`, and (control) that `b.txt` still reads `bravo`.
- `docs/wovenfs-on-disk-format.md`: documents the kind rule for read/write.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New test with the `wfs.rs` fix stashed (negative control) | FAILS as expected: `the root directory must not read as a file`, `left: Ok(512)` |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 21 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 131 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

### Open / next

- `NotADirectory` is a misleading name for "is a directory". A dedicated
  variant would need a matching `task.rs` mapping. Left unchanged to keep
  the existing error surface.
- `unlink` still ignores `links` (no hard links yet). Cross-inode block
  aliasing and the uncovered journal descriptor checksum remain open. Both
  need design decisions.
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

## 2026-10-15 — WovenFS: validate directory entries before unlink

Branch: `auto/wovenhat-development` (base `3024bdb`). The working tree was
clean at the start, so there was no unfinished work to resume. The previous
entry's open items (cross-inode aliasing scan, descriptor checksum) both
need design decisions, so this checkpoint takes a separate, bounded gap:
directory entries, which carry no checksum, were trusted.

### Defects

- `unlink` freed whatever inode an entry named. An entry naming the root
  directory's own inode returned `Ok(())`, emptied the root inode and freed
  its directory blocks, losing the whole namespace. An entry sharing an inode
  with another entry freed it while the other entry kept pointing at it. An
  entry naming a free inode was "unlinked" as if valid.
- `DirEntry::matches` sliced `name[..name.len()]` after only checking
  `name_len == name.len()`. An entry with `name_len > 48`, looked up by a name
  of that length through the public `Volume::lookup`/`unlink`, panicked with
  an out-of-bounds slice. The kernel adapters cap names at 48 first, so this
  was not reachable through them.

### Change

- `kernel/src/wfs.rs`:
  - `matches` uses `name.get(..len)`, so an oversized `name_len` never
    matches.
  - `unlink` returns `Corrupt` before staging anything when the entry names
    the directory itself, an inode past the table, an inode another entry in
    the directory also names, or an inode that is not a used file.
- `tests/wovenfs_crash.rs`:
  - `unlink_refuses_an_entry_that_does_not_name_one_live_file`: retargets
    `a.txt`'s entry at the root inode, at `b.txt`'s inode, at a free inode and
    past the table. Each `unlink` must return `Corrupt`, leave the image
    byte-identical and keep `b.txt` readable. A control unlinks the untouched
    entry and checks consistency.
  - `an_entry_name_length_past_the_name_field_does_not_match`: `name_len = 60`
    and a 60-byte lookup/unlink must return `NotFound`.
- `docs/wovenfs-on-disk-format.md`: documents the directory-entry rules and
  the remaining scope limit.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New tests against `HEAD`'s `wfs.rs` (negative control, before the fix) | Both FAIL as expected: `the directory's own inode: unlink must refuse the entry` (`left: Ok(())`), and a panic `range end index 60 out of range for slice of length 48` at `wfs.rs:287` |
| `cargo fmt --all -- --check` | PASS. The first check failed on the formatting of one new assertion; fixed with `cargo fmt --all`, then rerun |
| `cargo test --test wovenfs_crash` | PASS, 20 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 130 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

### Open / next

- `lookup` still returns an entry that names a free inode or a directory
  without validating it. `write_file` refuses unused and non-file inodes, but
  `read_file`'s behaviour on such an entry was not examined or tested in this
  checkpoint.
- `unlink` ignores `links`. Hard links are not implemented, so every file has
  `links = 1`. If hard links are added, unlink must decrement instead of
  freeing.
- Cross-inode block aliasing and the uncovered journal descriptor remain open
  from the previous entry. Both need design decisions.
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

## 2026-10-15 — WovenFS: refuse to reuse or free blocks the bitmap marks free

Branch: `auto/wovenhat-development` (base `0989924`). The working tree was
clean at the start, so there was no unfinished work to resume. This narrows
the "pointer aliasing" item left open by the previous entry. It does not
close it.

### Defect

The write path trusted inode pointers against the bitmap:

- `stage_write` rewrote an existing pointer's block in place without checking
  that the bitmap still marked it allocated. `stage_write` (shrink) and
  `unlink` cleared bitmap bits without checking that they were set. A
  pointer to a block the bitmap calls free is the step that leads to
  aliasing: the allocator gives that block to another file, and the first
  inode then overwrites or frees the other file's data.
- An inode naming the same block in two slots was accepted. A write staged
  that block twice with different bytes, and a shrink freed it while another
  slot still named it.

### Change

- `kernel/src/wfs.rs`:
  - `checked_inode` refuses duplicate non-zero pointers within one inode.
  - The new `stage_free_block` refuses (`Corrupt`) to free a block whose bit
    is already clear. Shrink and `unlink` use it.
  - `stage_write` refuses to rewrite in place an existing block whose bit is
    clear.
  - Every refusal happens while staging, so nothing reaches the journal or
    the disk.
- `tests/wovenfs_crash.rs`:
  - `a_pointer_at_a_block_the_bitmap_marks_free_is_not_reused_or_freed`
    redirects a pointer to a free data block, with the inode checksum
    re-sealed. `check()` confirms the bit is clear. A growing write, a
    shrinking write and `unlink` must each return `Corrupt` and leave the
    image byte-identical.
  - `an_inode_naming_one_block_twice_is_refused` makes `inode`, `read_file`,
    both writes and `unlink` return `Corrupt`, again with the image
    unchanged.
- `docs/wovenfs-on-disk-format.md`: documents both rules and the remaining
  gap.

### Results (run on this host, 2026-10-15)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| New tests against `HEAD`'s `wfs.rs` (negative control) | Both FAIL as expected: `in-place write: must refuse a block the bitmap marks free` (`left: Ok(())`), and the duplicate-pointer `inode()` assertion (`left: Ok(())`). The fixed file was restored afterwards |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 18 passed / 0 failed |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 128 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS. The first run failed with `clippy::type_complexity` in the new test; fixed with a type alias, then rerun |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| QEMU Stage 15.x on 1/2/4 CPUs | **NOT RUN**: `qemu-system-x86_64` is not installed on this host |

### Open / next

- Cross-inode aliasing, where two live inodes both name one block marked
  allocated, is still detected only by `check()` (test and self-test builds).
  Closing it needs an ownership scan, either at mount or before each
  in-place write or free. That costs inode-table reads and needs a design
  decision.
- The journal descriptor's target list is still not covered by any checksum
  (format change; needs a compatibility decision).
- QEMU Stage 15.1/15.2 acceptance on 1/2/4 CPUs must be rerun on a host with
  QEMU before this counts as stage acceptance.

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
