# WovenFS v1 — on-disk format and crash-recovery contract

WovenFS is WovenHat's native filesystem. This document is the authority for its
on-disk layout and for the argument that it recovers from a crash at any point.

## Design constraint that makes recovery provable

**No operation depends on more than one sector being written atomically.**

A 512-byte sector write either lands or it does not. Every durability decision
in WovenFS is therefore reduced to a single-sector write, and every multi-sector
change is staged in a journal whose completion is itself decided by one sector.
This is what allows crash behaviour to be enumerated and tested exhaustively
rather than argued informally.

## Layout

All units are 512-byte sectors; one block is one sector.

| Region | Location | Purpose |
| --- | --- | --- |
| Superblock A | LBA 0 | Checkpoint slot 0 |
| Superblock B | LBA 1 | Checkpoint slot 1 |
| Journal | `journal_start`, `journal_blocks` | Write-ahead staging area |
| Block bitmap | `bitmap_start`, `bitmap_blocks` | One bit per block, 1 = allocated |
| Inode table | `inode_start`, `inode_count` | 128-byte inodes, 4 per sector |
| Data | `data_start` .. end | File and directory contents |

### Superblock (512 bytes, two slots)

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `WOVENFS1` |
| 8 | 2 | format version (1) |
| 12 | 8 | `sequence` — monotonic; the higher valid slot wins |
| 20 | 8 | `total_blocks` |
| 28 | 8 | `journal_start` |
| 36 | 4 | `journal_blocks` |
| 40 | 8 | `bitmap_start` |
| 48 | 4 | `bitmap_blocks` |
| 52 | 8 | `inode_start` |
| 60 | 4 | `inode_count` |
| 64 | 8 | `data_start` |
| 72 | 8 | `root_inode` |
| 80 | 8 | `checksum` over the rest of the sector |

**Checkpoint alternation.** A superblock is never overwritten in place. Each
commit writes the slot *not* currently authoritative, with `sequence + 1`. Mount
reads both slots, discards any whose magic, version or checksum fails or whose
geometry does not fit, and adopts the survivor with the highest sequence. A torn
superblock write therefore cannot destroy the filesystem: the other slot remains
intact and older.

**Geometry.** The checksum is unkeyed, so it shows only that the sector was
written whole, not that its fields are sane. Mount also requires, with no
arithmetic overflow:

- `journal_start >= 2` and `journal_blocks >= MAX_TXN_BLOCKS + 2`;
- journal, bitmap, inode table and data regions in that order without overlap
  (`journal_start + journal_blocks <= bitmap_start`, and so on up to
  `inode_start + ceil(inode_count / 4) <= data_start`);
- `bitmap_blocks` large enough for one bit per block of `total_blocks`;
- `inode_count > 0` and `1 <= root_inode <= inode_count`;
- `data_start < total_blocks <=` the device's sector count.

Every later bounds check (inode block pointers, bitmap and inode locations,
replay targets) is relative to this geometry.

### Inode (128 bytes, 4 per sector)

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | `mode` (0 = free) |
| 4 | 4 | `links` |
| 8 | 8 | `size` in bytes |
| 16 | 8 | `created` |
| 24 | 8 | `modified` |
| 32 | 8 | `data_checksum` |
| 40 | 64 | `direct[8]` block pointers |
| 104 | 2 | `kind` (1 = file, 2 = directory) |
| 120 | 8 | `checksum` over bytes 0..120 |

Eight direct blocks give a 4 KiB maximum file size. Bounded by construction, in
keeping with the rest of the kernel; indirect blocks are a later extension.

A direct pointer of 0 means "no block". Every other pointer must lie in
`data_start .. total_blocks`. An inode that breaks this, or a used inode whose
`kind` is not 1 or 2 or whose `size` is over 4 KiB, reads as corrupt even when
its checksum matches. This holds for free inodes too. The checksum catches bit
rot, not a mis-written or crafted image. Without the check, a write would put
file data over the journal or metadata, and an unlink would free a metadata
block.

An inode that names the same block in two slots also reads as corrupt. A
write that rewrites an existing block in place, and a shrink or unlink that
frees one, first requires that block's bitmap bit to be set. If the bit is
clear, the operation fails with `Corrupt` before anything is committed,
because the allocator may already have handed that block to another inode.
The bitmap records that a block is allocated, not which inode owns it, so a
second check covers two live inodes that name the same block. Before a write
commits, it fails with `Corrupt` if any other live inode names a block the
write rewrites in place, frees, or newly allocates. Unlink does the same
for the blocks it frees. The newly-allocated case catches a live inode that
names a block the bitmap marks free, which the allocator would otherwise hand
out. The scan reads every inode through the transaction view, so a write costs
O(`inode_count`) inode-table reads. Any unreadable or corrupt inode in the
table therefore fails the write closed. Create and unlink rewrite one
directory block in place to add or clear an entry. That block gets the same
two checks: its bitmap bit must be set, and no live inode other than the
directory may name it. Otherwise a create or unlink would overwrite the data
of a file it never touches. `check()` still runs the full whole-volume audit
in test and self-test builds.

### Directory entry (64 bytes, 8 per block)

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | `inode` (0 = free slot) |
| 8 | 2 | `name_len` |
| 10 | 2 | `kind` |
| 16 | 48 | `name` |

Eight direct blocks per directory inode give 64 entries per directory.

Directory blocks carry no checksum. A `name_len` past the 48-byte name field
never matches a lookup. `unlink` frees nothing unless the entry names a live
file inode inside the table, other than the directory itself, and no other
entry in the directory names the same inode. Otherwise it fails with
`Corrupt` before anything is committed. Without this, an entry naming the
directory would free the directory's own blocks, and a duplicate entry would
be left pointing at whatever file next reuses the inode number. The duplicate
scan covers only the directory being changed. That is sufficient while the
root is the only directory.

WovenFS has no hard links. Create writes `links = 1` on a file inode and
nothing changes it, so `unlink` also requires `links == 1`. Any other count
fails with `Corrupt` before anything is committed. Freeing the blocks of an
inode that another name might still reach would destroy that file's data.
`check()` flags such an inode too. Adding hard links would change `unlink` to
decrement the count and free only at zero.

Create allocates the lowest free inode. If any entry in the directory already
names that inode (a stale or crafted entry, since directory blocks carry no
checksum), create and `create_file` fail with `Corrupt` before anything is
committed. Otherwise the new file would get two names: the old name would
read and overwrite the new file's data, and `unlink` would refuse both names
as duplicates, so neither could be removed. Skipping to the next free inode was rejected because it would
leave the dangling entry in place. Like the duplicate scan, this covers only
the target directory, which is sufficient while the root is the only one.

`read_file` and `write_file` accept only a used inode of kind 1 (file). A
directory inode fails with `NotADirectory`, the existing wrong-kind error,
and a free inode fails with `NotFound`. A directory's `data_checksum` is
never maintained, so the content checksum alone does not stop a read. A
crafted value would otherwise return the raw entry table as file contents.

## Journal

A transaction is a contiguous run of journal sectors:

```
[ descriptor ] [ data 0 ] [ data 1 ] ... [ data n-1 ] [ commit ]
```

**Descriptor** — magic `WFSJDESC`, transaction id, block count, and the target
LBA for each staged data block (up to `MAX_TXN_BLOCKS`).

**Data sectors** — the exact bytes to be written to each target, in descriptor
order.

**Commit** — magic `WFSJCOMM`, the same transaction id, and a checksum computed
over the descriptor's used bytes (magic, id, count and the `count` target LBAs),
then every staged data sector in order. Written last, and written alone.

The descriptor is covered so that a descriptor damaged after its commit landed
is discarded rather than replayed. Without it, a changed target inside the
transactional regions would still look committed and replay would write a
staged block over the wrong bitmap, inode or data block. A commit sector
written before this rule covered only the data, so a transaction committed but
not yet retired by an older kernel is discarded on first mount by this one:
the volume presents the prior state, not the new one, for that transaction.

### Write path

1. Stage the descriptor, then every data sector, into the journal.
2. Write the commit sector. **This single sector write is the point at which the
   transaction becomes real.**
3. Apply each data sector to its target LBA.
4. Write the next superblock checkpoint, retiring the journal.

### Recovery

On mount, after choosing the authoritative superblock:

1. Read the journal descriptor. If its magic or id is invalid, there is no
   pending transaction; mount proceeds.
2. Read the commit sector. If it is missing, its id does not match the
   descriptor, or its checksum does not match the descriptor and staged data, the transaction
   was never committed. It is discarded and the filesystem is left exactly as it
   was before the transaction began.
   A transaction may only target the bitmap, inode table and data regions
   (`bitmap_start <= target < total_blocks`). The superblock slots are written
   only by the checkpoint and the journal only by the commit path. A
   descriptor naming any other target is discarded the same way, even if
   its commit checksum matches. `commit` refuses such a transaction with
   `Corrupt` before it writes to the journal.
3. Otherwise the transaction is committed but possibly only partly applied, so
   every staged data sector is written to its target again. Replay is
   **idempotent** — applying it twice is identical to applying it once — so a
   crash during recovery is itself recoverable.

### The resulting guarantee

For a crash at any single point in a metadata operation, the filesystem mounts
and presents either the complete prior state or the complete new state, never a
partial one. Each of the three failure windows is closed by a different
mechanism:

| Crash window | Outcome | Mechanism |
| --- | --- | --- |
| During journal staging | Prior state | No valid commit sector |
| After commit, during apply | New state | Idempotent replay |
| During superblock checkpoint | Prior or new, both consistent | Slot alternation + checksum |

## How this is verified

`tests/wovenfs_crash.rs` wraps a RAM disk in a device that fails every write
after the *n*th. For an operation performing *W* writes, the test runs the
operation once per *n* in `0..=W`, then mounts the resulting image and asserts:

- the superblock is valid and the filesystem mounts;
- the bitmap, inode table and directory are mutually consistent;
- the operation is observed as either entirely absent or entirely present;
- mounting twice produces identical state, proving replay idempotence.

This enumerates every crash point rather than sampling, so recovery is
demonstrated for the whole operation rather than asserted in general.
