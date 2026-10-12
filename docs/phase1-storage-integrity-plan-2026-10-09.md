# Phase 1 — Storage integrity and honesty plan

Derived from [the architectural audit](audit-architectural-recovery-2026-10-09.md) §D-1
and §H. This plan records the design decisions and the evidence behind them before any
code changes, so the reasoning is reviewable separately from the diff.

## 1.1 Bring the WDJ2 batch rollback journal onto the main line

**Finding.** `master`'s `storage.rs` is 2,382 lines and carries only the single-file
`WDJ1` data journal. Commit `097f0d6` *"Stage 1-5: add bounded batch rollback journal"* —
reachable only from `origin/local-stage14-6` — adds 596 lines implementing a versioned
`WDJ2` four-file batch journal with prepared/committed recovery, per-file previous-bytes
rollback, and checksum validation before retirement. It is real, gate-tested work that
has never reached `master`.

**Cherry-pick is safe.** `kernel/src/storage.rs` is byte-identical at `f752e7e` (the
commit's parent) and at `b81a7771` (current `master`); `git log f752e7e..b81a7771 --
kernel/src/storage.rs` is empty. The commit touches only `storage.rs` and four documents,
so it cannot disturb the Stage 14.5 firewall work that caused the two branches to
diverge. Expected conflicts are confined to `docs/stage-status.md`,
`docs/architecture-and-codebase-guide.md` and `docs/stage1-12-production-gap-audit.md`,
all of which have moved on `master`.

**Action.** Cherry-pick `097f0d6`, resolve the three document conflicts by keeping
`master`'s content and adding the WDJ2 sections, then re-run the Stage 1–5 gate on
1/2/4 CPUs.

## 1.2 Correct `journal.rs` rather than delete it

**Finding.** `kernel/src/journal.rs` declares itself a *"Bounded write-ahead intent
journal for crash-safe metadata updates"* and is wired into the live write path —
`storage.rs:1274` `recover()`, `:1311` `begin()`, `:1397` `commit()`. But `LOG` is a
`static Mutex<[Option<Entry>; 32]>` held in RAM. Nothing reaches a device, so after a real
crash the table is empty, `recover()` returns 0, and no intent is ever replayed. Its own
`recover()` body makes the absence of replay explicit: the committed branch evaluates
`let _ = (x.path_hash, x.checksum);` and discards the record.

Within a single boot it has **no behavioural effect either**. `persist_path` discards both
results (`let _replayed_intents = ...`, `let _ = commit(token)`), so the module currently
changes nothing except timing.

**Why not delete it.** Deletion is the instinctive answer and it is wrong here. The Stage
1–5 acceptance gate requires the serial marker `[S1-5] storage journal: PASSED`
(`scripts/test-stage10-runtime.py:92`), produced by
`journal::structural_self_test() && storage::self_test()` at `main.rs:3342`. Removing the
module would mean editing an accepted gate's criteria, which `AGENTS.md` prohibits:
*"Treat test failures as defects to diagnose, not reasons to weaken assertions, disable
tests, suppress warnings or increase timeouts."* The defect is a false claim, not the
module's existence.

**Action.** Keep the module and the gate; remove the falsehood.

1. Rewrite the module documentation to state precisely what it is — a bounded,
   same-boot, in-memory intent tracker — and what it explicitly is **not**: durable,
   write-ahead, or crash-safe. Name the on-volume `WDJ` journal in `storage.rs` as the
   mechanism that actually provides crash safety, so the next reader cannot mistake one
   for the other.
2. Remove the `let _ = (x.path_hash, x.checksum);` dead pattern from `recover()` and make
   the committed-entry retirement explicit.
3. In `storage.rs`, replace the discarded `let _replayed_intents` and `let _ = commit(..)`
   bindings with a comment stating why the counts are advisory, so the write path no
   longer *looks* like it is relying on journal durability it does not have.

This leaves the gate intact, keeps the module honest, and removes the trap.

## 1.3 Fix path-hash collision handling

**Finding.** `wovenfs.rs` and `journal.rs` both identify records by a 64-bit FNV-1a hash
of the path and match on that hash **alone** — `wovenfs.rs:41` (`record`), `:69`
(`metadata`), `:78` (`set_xattr`), `:94` (`xattr`), and the `path_hash` field in
`journal.rs`. A collision between two distinct paths silently returns or overwrites the
other file's record. In `wovenfs`'s case the record includes `mode`, i.e. permission bits.

This is currently unexploitable because `wovenfs` has no production callers, but it is a
latent trap for whoever wires it up, and `journal.rs` *is* on the write path.

**Rejected approach.** Storing the full path costs `MAX_PATH_SIZE` (512 bytes) per entry:
16 KB static for `journal.rs`'s 32 entries and 32 KB for `wovenfs.rs`'s 64. That is a
large static cost for modules of no current behavioural value.

**Action.** Identify a record by a composite key: the existing FNV-1a hash, a second
independent hash over the same bytes with a different offset basis, and the path length.
A false match then requires two independent 64-bit hashes *and* the length to collide,
which is not a practical concern, at a cost of 16 bytes per entry — 512 bytes and 1 KB
respectively. Add a self-test asserting that two paths are distinguished.

## 1.4 Resolve the WovenFS question

**Finding.** `wovenfs.rs` is 104 lines: an in-memory, 64-entry metadata side table with no
on-disk format, no inodes, no directories, no block allocation and no data blocks,
volatile across reboot, whose only caller is its own self-test (`main.rs:3351`).
`snapshots.rs` is 53 lines holding eight `(id, generation, checksum)` triples, with
`restore()` returning that triple and restoring no data; its only caller is likewise its
own self-test (`main.rs:3369`). `State::len` in `wovenfs.rs` is incremented at line 60 and
never read or decremented, and there is no `remove`, so the table can only fill — after
which `record()` returns `false` and metadata silently stops being recorded.

`docs/stage1-12-production-gap-audit.md` already concedes the gap: Stage 12's open item is
"Native WovenFS, journaling/COW disk replay, full data rollback."

**This is a product decision, not a defect to patch.** Two coherent options:

- **(a) Build a native filesystem.** On-disk superblock and format, inode allocation,
  directory structure, extent or block allocator, a real write-ahead journal with replay,
  copy-on-write snapshots that reference blocks, and `fsck`. This is a multi-month
  subsystem and should be its own stage sequence with its own gates.
- **(b) Keep FAT32 as the filesystem and reclassify.** Retain `wovenfs.rs` honestly as
  what it is — a bounded metadata/xattr side table — rename its documentation accordingly,
  and stop describing Stage 12.2/12.4 as filesystem and snapshot subsystems in the status
  documents.

**Action now (neither option foreclosed).** Fix the two concrete defects — the
collision key in 1.3 and the unreachable `len`/missing `remove` — and correct the module
and status documentation so it describes what the code does. Record the (a)/(b) choice as
an explicit open decision for the project owner rather than making it implicitly through a
patch.

## Acceptance for Phase 1

1. Warning-denying Clippy passes for all 41 kernel feature configurations, `libwoven`,
   and the host with `--all-targets`.
2. The Stage 1–5 gate passes on 1/2/4 CPUs with `[S1-5] storage journal: PASSED`,
   including WDJ2 batch commit and the prepared, partial-commit and complete-commit
   recovery exercises.
3. Stage 12.1–12.5 gates pass on 1/2/4 CPUs.
4. The new collision self-tests pass.
5. No module documentation claims durability, crash safety, or copy-on-write behaviour
   that the implementation does not provide.
