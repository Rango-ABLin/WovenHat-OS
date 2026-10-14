# Stage 15 WovenFS VFS write-through audit — 2026-10-14

This increment closes the bounded software write-through boundary between the
task file authority and native WovenFS. It does not claim full POSIX filesystem
or physical flash qualification.

## Implemented boundary

- Production task file operations create, write, and unlink `/wfs/<name>` files
  through native WovenFS before publishing or removing the VFS name.
- The adapter resolves validated nested absolute paths below `/wfs`; traversal,
  empty components, separators inside names, NULs, and prefix-confusion paths
  are rejected.
- Native directory creation, empty-directory removal, and file/directory rename
  update parent entries in the same bounded journal transaction as the inode
  and allocation bitmap changes. Destination replacement is refused rather
  than performed partially.
- ATA mount publication now recursively imports validated WovenFS directories
  and files below `/wfs`, with bounded depth and visited-inode tracking so
  malformed cycles or namespace growth fail closed. Imported files remain
  read-only in the VFS.
- Directory rename rejects a destination below the moved directory using a
  bounded, cycle-detecting ancestry walk. Same-path rename is an explicit
  validated no-op; traversal capacity exhaustion fails closed.
- `task::write_current` commits the native journaled transaction before it
  refreshes the VFS cache and descriptor offset. Direct generic VFS mutation
  remains read-only for `/wfs`, preventing a capability bypass.
- Native create/write/unlink operations serialize their complete
  read-modify-write transaction with a rank-5 preemption-aware mutex. The ATA
  device lock is rank 10; no IRQ-disabled guard is held over device I/O.

## Validation

- `cargo clippy --all-targets -- -D warnings` — passed.
- Freestanding Clippy passed for `stage11-5-test`, `stage15-1-test`, and
  `stage15-2-test` with `-D warnings`.
- `cargo test -p wovenhat-os --tests --no-fail-fast` — passed, including all
  12 WovenFS crash-recovery tests.
- Stage 15.1 QEMU acceptance passed on 1, 2, and 4 CPUs (exit 33):
  `stage15.1-1cpu-1791981155975268700`,
  `stage15.1-2cpu-1791981222170305100`, and
  `stage15.1-4cpu-1791981275434721400`.
- Fresh Stage 15.1 evidence after nested-directory integration:
  `stage15.1-1cpu-1792003097975720200`,
  `stage15.1-2cpu-1792003179899219100`, and
  `stage15.1-4cpu-1792003230834435200`.
- The durable-first publication hardening was rechecked on 1/2/4 CPUs:
  `stage15.1-1cpu-1792006921025775400`,
  `stage15.1-2cpu-1792006991018191200`, and
  `stage15.1-4cpu-1792007054767355600`.
- Fresh scratch-device Stage 15.2 evidence passed on 1, 2, and 4 CPUs with
  exit 33 under `target/wovenfs-device-scratch-{1,2,4}cpu`.
- Cycle-guard hardening rerun passed Stage 15.1 on 1, 2, and 4 CPUs with exit
  33: `stage15.1-1cpu-1792014304834644400`,
  `stage15.1-2cpu-1792014406693740800`, and
  `stage15.1-4cpu-1792014471416201400`.
- Recursive-import Stage 15.2 scratch-device rerun passed on 1, 2, and 4 CPUs
  with exit 33 under `target/wovenfs-device-scratch-{1,2,4}cpu`; the fixture
  creates and reads `/docs/note.txt` through the mounted VFS.

An earlier 1-CPU timeout is retained as
`stage15.1-1cpu-1791980850302510700`; the exact rerun passed.

## Limits and next prerequisite

Mounted-system policy, including whether WovenFS replaces the FAT32 system
volume, remains open. The recursive importer is intentionally bounded to
16 path levels and 256 visited inodes and skips files larger than the VFS
payload capacity. Physical qualification remains incomplete: the inserted USB
flash disk was inventoried read-only, while the destructive harness requires
Administrator elevation and explicit authorization to erase it. No destructive
write was issued in this audit.
