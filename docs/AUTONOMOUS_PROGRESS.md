# Autonomous progress log

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — Resume + validate uncommitted UEFI installer prototype

Branch: `auto/wovenhat-development` (base `8139336`). The working tree already
held an unfinished, uncommitted Stage 28 installer prototype (an earlier
session's work): `tools/woven-installer/` (safe-Rust UEFI app + host-testable
GPT/authority library), `scripts/build-installer.py`,
`scripts/test-installer-qemu.py`, `docs/installer.md`,
`docs/audit-installer-prototype-2026-10-25.md`, and edits to
`docs/stage-status.md` / `docs/architecture-and-codebase-guide.md`. Per the
resume rule, I did not discard or overwrite it; I inspected the diff,
re-validated what I could on this host, and recorded the results before
committing the checkpoint. This is **not** full Stage 28/29 acceptance.

### Inspection

- `tools/woven-installer/src/lib.rs` (223 lines): `#![no_std]` + `alloc`,
  host-testable device-path authority (`disk_path`, `source_disk`), `crc32`,
  and canonical GPT validation/relocation (`plan`). Three `#[cfg(test)]` unit
  tests.
- `tools/woven-installer/src/firmware.rs` (217 lines): UEFI-only
  (`cfg(target_os = "uefi")`), uses the `uefi` crate's safe API — scoped
  `open_protocol_exclusive` handles dropped before the next open,
  event-driven keyboard waits (`wait_for_event`), synchronous `BlockIO`,
  source-media exclusion, typed `ERASE N` confirmation, post-confirmation
  revalidation of path/media-id/geometry/alignment/writability, aligned
  (`repr(align(4096))`) write buffer, `flush_blocks`, and byte-for-byte
  readback compare.
- `grep` confirms **no `unsafe` and no `asm!`** anywhere in
  `tools/woven-installer/src/`.
- `scripts/build-installer.py` / `test-installer-qemu.py` contain no physical
  host-disk access (the only `physical` match is QEMU's
  `physical_block_size` device flag); QEMU targets are disposable file-backed
  disks.

### Results (independently re-run on this host, 2026-10-27)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo test --offline --manifest-path tools/woven-installer/Cargo.toml --lib` | COMPILES CLEAN, but test-binary **execution blocked** by Windows Application Control (`os error 4551`). Not re-confirmable by execution on this host. |
| `cargo clippy --offline --manifest-path tools/woven-installer/Cargo.toml --all-targets -- -D warnings` | PASS |
| `cargo clippy --offline --manifest-path tools/woven-installer/Cargo.toml --target x86_64-unknown-uefi --release -- -D warnings` (with `WOVENHAT_INSTALL_IMAGE` set to the retained `wovenhat-system.img`) | PASS |

The blocked test binary's hash (`woven_installer-0c566a530a0b06ed.exe`) is
identical to the one in the retained evidence log
`audit-artifacts/installer-20261025-final/installer-host-tests.log`, which
recorded **3 passed / 0 failed** on 2026-10-25. The same binary is now blocked
by host policy; this is the environmental Application Control block already
documented in the audit (also affects the main repo binary test executable). I
did not and must not modify Windows configuration, so I could not re-execute it
this session. **No independent execution-time PASS is claimed for the host
tests** — only a clean compile plus the retained prior-run evidence.

### Limits / open

- Host unit-test execution remains blocked by Windows Application Control
  (`os error 4551`); unresolved and outside this session's allowed changes.
- Full Stage 28 (graphical installer, accounts, timezone/keyboard/network
  setup, progress UX, durable system root, session startup) and Stage 29
  (physical hardware qualification) remain open. No physical disk was written.
- The 1/2/4-CPU installer QEMU matrix from the 2026-10-25 session is retained
  under `audit-artifacts/installer-qemu-20261025-final/` but was **not** re-run
  this session (one acceptance chain at a time; host-port/output constraints).
- This commit records the prototype and its validation state only; it does not
  close Stage 28.

## 2026-10-16 — WovenFS: create refuses a free inode that an entry still names

Branch: `auto/wovenhat-development` (base `8c29d64`). The working tree was
clean at the start, so there was no unfinished work to resume.

### Defect

Directory blocks carry no checksum, so a stale or crafted entry can name a
free inode. `check()` flags that, but the write path did not: `create` and
`create_file` allocated the lowest free inode without looking at entries. If
an entry already named it, the new file got a second name. The stale name then
read (and could overwrite) the new file's data. `unlink` refuses duplicate
entries, so neither name could be removed.

### Change

- `stage_create` (used by `create` and `create_file`) returns `Corrupt`
  before committing anything if any entry in the target directory names the
  inode it just allocated. Failing closed was chosen over skipping to the next
  free inode, which would leave the dangling entry in place.
- Test `create_refuses_a_free_inode_that_an_entry_still_names`: a.txt's entry
  is patched to name the inode the next create would take. `check()` returns
  `Corrupt`; `create` and `create_file` return `Corrupt` and leave the image
  byte-identical; b.txt stays readable. Control: on the unpatched image the
  same `create_file` succeeds, takes that inode, and the volume passes
  `check()`.
- Docs: `wovenfs-on-disk-format.md` directory-entry section;
  `stage-status.md` Stage 15 note.

### Results (run on this host, 2026-10-16)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 28 passed / 0 failed |
| Negative control: same tests against `HEAD`'s `wfs.rs` (via `git show`; fixed copy restored and verified with `cmp`) | FAIL as expected: new test fails at `create must not give a second name to an inode` (`left: Ok(4)`, `right: Err(Corrupt)`); 27 others pass |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | First attempt: `LNK1104` on the `async_network` test exe (no such process was running). Rerun unchanged: PASS, 138 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.1 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.1-{1cpu-20261016-094711-960,2cpu-…-094900-760,4cpu-…-095051-871}`) |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.2 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.2-{1cpu-20261016-095218-883,2cpu-…-095341-744,4cpu-…-095507-927}`) |

After the QEMU runs, a source comment and the matching doc sentence were
corrected: they wrongly said `unlink` through either name would free the
other file, but `unlink` refuses duplicates. This was comment-only. After it,
`cargo fmt --check`, `cargo test --test wovenfs_crash` (28/0), all-targets
Clippy and kernel Clippy `--features stage15-2-test` were rerun and passed.
QEMU was not rerun for the comment change.

Evidence directories are under `audit-artifacts/` (Git-ignored, kept
locally). No QEMU process was left running.

### Limits / open

- The scan covers only the target directory. That is sufficient while the
  root is the only directory. Subdirectories would need a volume-wide check.
- `lookup` still returns an entry naming a free inode; `read_file` and
  `write_file` then fail with `NotFound` rather than `Corrupt`.
- `check()` records at most 64 blocks in its aliasing table. Beyond that,
  aliasing goes unchecked by the test oracle (the write-path checks are not
  affected).
- There is no repair (fsck) path. Stage 15.3–15.9 QEMU gates and physical USB
  qualification were not rerun.

## 2026-10-16 — WovenFS: unlink refuses a file whose link count is not 1

Branch: `auto/wovenhat-development` (base `59bfa08`). The working tree was
clean at the start, so there was no unfinished work to resume. This closes the
open item "`unlink` still ignores `links`" carried since the 2026-10-15
entries.

### Defect

WovenFS has no hard links: create writes `links = 1` and nothing changes it.
`unlink` never read the field. It freed the blocks of a file inode whose
checksummed `links` was 0, 2 or anything else. With any count other than 1,
another name may still reach the inode (or the image is crafted), so freeing
the blocks could destroy another file's data. `check()` did not flag the count
either.

### Change

- `unlink` returns `Corrupt` before staging anything if the file inode's
  `links != 1`. The comment records that adding hard links means
  decrementing the count instead of freeing.
- `check()` (test/self-test builds) flags a live file inode with `links != 1`.
  Directory inodes are not checked: the root is formatted with `links = 2`.
- Test `unlink_refuses_a_file_whose_link_count_is_not_one`: for `links` = 0, 2
  and `u32::MAX` (inode checksum re-sealed), `check()` returns `Corrupt`,
  `unlink` returns `Corrupt` and leaves the image byte-identical, and both
  files stay readable. Control: a rewritten file keeps `links = 1` and
  unlinks normally.
- Docs: `wovenfs-on-disk-format.md` directory-entry section;
  `stage-status.md` Stage 15 note.

### Results (run on this host, 2026-10-16)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (after `cargo fmt --all` reflowed one call in the new test) |
| `cargo test --test wovenfs_crash` | PASS, 27 passed / 0 failed |
| Negative control 1: same tests against `HEAD`'s `wfs.rs` | FAIL as expected: new test fails at `links = 0: check() must flag the count` (`left: Ok(())`); 26 others pass |
| Negative control 2: fixed `wfs.rs` with only the `unlink` `links` condition reverted | FAIL as expected at `links = 0: unlink must not free a file another name may reach` (`left: Ok(())`); 26 others pass. The first attempt hit `LNK1104` and was rerun unchanged. The fixed copy was restored and verified identical with `cmp` after each control |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | First attempt: `LNK1104` on the `wovenfs_crash` exe (no test process was running). Rerun unchanged: PASS, 137 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.1 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.1-{1cpu-20261016-093024-482,2cpu-…-093220-378,4cpu-…-093353-444}`) |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.2 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.2-{1cpu-20261016-093520-941,2cpu-…-093711-439,4cpu-…-093902-128}`) |

Evidence directories are under `audit-artifacts/` (Git-ignored, kept
locally). No QEMU process was left running.

The first negative-control attempt replaced `wfs.rs` through a PowerShell
pipe, which mangled the file. That run is not evidence. The fixed file was
restored from a saved copy, and controls were redone with `git show` in bash.

### Limits / open

- Hard links remain unimplemented. Directory link counts are not validated.
- The O(`inode_count`) ownership scans on write and unlink are unchanged.
  There is still no repair (fsck) path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-16 — WovenFS: directory-entry rewrites check block ownership

Branch: `auto/wovenhat-development` (base `5722e0e`). The working tree was
clean at the start, so there was no unfinished work to resume. This closes the
open item "Directory-block aliasing is checked only when the writer is a file
inode" from the previous entry.

### Defect

`create`, `create_file` and `unlink` add or clear a directory entry by
rewriting one root directory block in place. That path checked neither the
block's bitmap bit nor whether another inode names the block. A file inode
that aliased a directory block therefore had its data overwritten by a create
or unlink that never touched the file. The previous checkpoint's scan caught
this only when the aliasing file itself was written or unlinked. A directory
block the bitmap marks free (one the allocator may already have handed out)
was also rewritten.

### Change

- `stage_dir_entry` now takes the directory's inode number. Before staging, it
  returns `Corrupt` if the entry's block is free in the bitmap, or if any live
  inode other than the directory names it (`refuse_shared_blocks`). Both
  callers (`stage_create`, `unlink`) pass `dir_inode`. The check runs before
  commit, so a refused operation leaves the image unchanged.
- Tests: `a_directory_block_named_by_a_file_is_not_rewritten` and
  `a_directory_block_the_bitmap_marks_free_is_not_rewritten`. Each checks
  that `create`, `create_file` and `unlink` of an unrelated sound file return
  `Corrupt` and leave the image byte-identical.
- Docs: `wovenfs-on-disk-format.md` integrity section; `stage-status.md`
  Stage 15 note.

### Results (run on this host, 2026-10-16)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (after `cargo fmt --all` reflowed one closure in the new test) |
| `cargo test --test wovenfs_crash` | PASS, 26 passed / 0 failed |
| Same tests against `HEAD`'s `wfs.rs` (negative control; fixed copy restored and verified identical with `cmp`) | FAIL as expected: both new tests fail at `create: must not rewrite a directory block …`; 24 others pass |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | PASS, 136 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.1 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.1-{1cpu-20261016-091005-893,2cpu-…-091207-188,4cpu-…-091355-462}`) |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.2 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.2-{1cpu-20261016-091543-323,2cpu-…-091745-424,4cpu-…-091935-496}`) |

Evidence directories are under `audit-artifacts/` (Git-ignored, kept
locally). No QEMU process was left running.

### Limits / open

- Cost: `unlink` now scans the inode table twice (once for the file's blocks
  and once for the directory block), and `create_file` scans it twice. Both
  are O(`inode_count`). Folding the scans into one is possible but was not
  needed at current volume sizes.
- Only the root directory exists, so directory-to-directory aliasing is not
  reachable yet. The check covers it once subdirectories exist.
- `unlink` still ignores `links`. There is still no repair (fsck) path.
  Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-16 — WovenFS: refuse blocks shared by two live inodes

Branch: `auto/wovenhat-development` (base `a720b9e`).

Resumed uncommitted work found in `kernel/src/wfs.rs` and
`tests/wovenfs_crash.rs`. I reviewed and validated that diff before
committing. No new work was started. This closes the open item "Cross-inode
block aliasing" from the previous entry.

### Defect

The bitmap records that a block is allocated, not which inode owns it. Two
live inodes naming one allocated block passed every per-inode check. A write
to one owner overwrote the other's data in place, and a shrink or unlink
freed a block the other still named. A live inode naming a block the bitmap
marks free was also missed: the allocator could give that block to a new or
growing file. Only `check()` (test/self-test builds) detected aliasing.

### Change

- `Volume::refuse_shared_blocks(txn, owner, blocks)` scans every inode
  through the transaction view and returns `Corrupt` if a live inode other
  than `owner` names any of `blocks`.
- `stage_write` (used by `write_file` and `create_file`) checks the union of
  the old and new pointers. That covers blocks rewritten in place, freed, or
  newly allocated. `unlink` checks the blocks it is about to free. Both run
  before commit, so a refused operation leaves the image unchanged.
- Tests: `a_block_named_by_two_inodes_is_not_rewritten_or_freed` (in-place
  write, shrink and unlink of either owner, image byte-identical after
  refusal); `a_free_block_named_by_a_live_inode_is_not_allocated`
  (`create_file` and a growing write).
- Docs: `wovenfs-on-disk-format.md` integrity section; `stage-status.md`
  Stage 15 note.

### Results (run on this host, 2026-10-16)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`,
`CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test wovenfs_crash` | PASS, 24 passed / 0 failed |
| Same tests against `HEAD`'s `wfs.rs` (negative control, working copy restored afterwards) | FAIL as expected: both new tests fail (`create_file` returned `Ok(4)`; in-place write of b returned `Ok(())`), 22 others pass |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | First attempt: `LNK1104` (Windows could not open the `wovenfs_crash` test exe for linking; host file lock). Rerun: PASS, 134 passed / 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` (default) | PASS |
| same, `--features stage11-5-test` | PASS |
| same, `--features stage15-1-test` … `stage15-9-test` (each) | PASS |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.1 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.1-{1cpu-20261016-085119-836,2cpu-…-085249-104,4cpu-…-085443-831}`) |
| `scripts/test-wovenfs-qemu.ps1 -Stage 15.2 -Cpus 1/2/4` | PASS, exit 33 each (`wovenfs-stage15.2-{1cpu-20261016-085640-839,2cpu-…-085840-224,4cpu-…-090034-650}`) |

`audit-artifacts/wovenfs-stage15.1-1cpu-20261016-085103-375` is from a first
QEMU batch I stopped early because its tool timeout was too short. It has no
result and is not evidence. No QEMU process was left running.

### Limits / open

- Cost: every write and unlink now reads the whole inode table,
  O(`inode_count`) reads. That is acceptable at current volume sizes. An
  in-memory owner map would be needed for larger tables.
- A corrupt or unreadable inode anywhere in the table now fails every write
  and unlink closed, not just operations that touch it. This is deliberate:
  ownership cannot be proven otherwise. There is still no repair (fsck) path.
- Directory-block aliasing is checked only when the writer is a file inode.
  `create` stages directory entries without this scan. A file aliasing a
  directory block is still refused when that file is written or unlinked.
- `unlink` still ignores `links`. Stage 15.3–15.9 and physical USB
  qualification were not rerun.

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
