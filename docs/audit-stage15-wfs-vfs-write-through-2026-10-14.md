# Stage 15 WovenFS VFS write-through audit — 2026-10-14

This increment closes the bounded software write-through boundary between the
task file authority and native WovenFS. It does not claim full POSIX filesystem
or physical flash qualification.

## Implemented boundary

- Production task file operations create, write, and unlink `/wfs/<name>` files
  through native WovenFS before publishing or removing the VFS name.
- The adapter accepts one validated flat root name only; nested paths,
  traversal, separators, NULs, and prefix-confusion paths are rejected.
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
  11 WovenFS crash-recovery tests.
- Stage 15.1 QEMU acceptance passed on 1, 2, and 4 CPUs (exit 33):
  `stage15.1-1cpu-1791981155975268700`,
  `stage15.1-2cpu-1791981222170305100`, and
  `stage15.1-4cpu-1791981275434721400`.

An earlier 1-CPU timeout is retained as
`stage15.1-1cpu-1791980850302510700`; the exact rerun passed.

## Limits and next prerequisite

The adapter is a bounded flat root-file slice. Rename, mkdir, directory
enumeration through `/wfs`, multi-directory trees, and mounted-system policy
are not claimed. Physical qualification remains incomplete: the inserted USB
flash disk was inventoried read-only, while the destructive harness requires
Administrator elevation and explicit authorization to erase it. No destructive
write was issued in this audit.

## Follow-up 2026-10-15: atomic create-with-contents

Defect: a write to an absent `/wfs/<name>` (both `write_vfs_path_at_with_publish`
at offset 0 and `write_ata_file`) committed `create` as one transaction and
the contents as a second. If the second failed (`NoSpace`, device error) the
caller reported failure while an empty file stayed durable on disk.

Fix: `Volume::create_file` stages the inode, directory entry, data blocks and
bitmap into one journaled transaction (at most 8 data sectors plus inode,
directory and bitmap sectors, within `MAX_TXN_BLOCKS = 32`). `create` and
`write_file` now delegate to shared `stage_create`/`stage_write` helpers;
existing-file writes are unchanged.

Validation: new host tests `create_file_is_atomic_across_every_crash_point`
and `create_file_out_of_space_leaves_no_file` pass (13/13 WovenFS crash
tests; 123 host tests total). Host and freestanding Clippy (default,
`stage11-5-test`, `stage15-1-test` … `stage15-9-test`) pass with
`-D warnings`. QEMU acceptance was **not run**: no QEMU binary is installed on
the host used for this checkpoint.
