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
| 106 | 8 | `xattr_block` (0 = none) |
| 114 | 6 | spare, always zero |
| 120 | 8 | `checksum` over bytes 0..120 |

Eight direct blocks give a 4 KiB maximum file size. Bounded by construction, in
keeping with the rest of the kernel; indirect blocks are a later extension.

`xattr_block` was added in place of what was previously always-zero spare
space (bytes 106..120 were zeroed and never read). Every inode written before
this field existed already decodes as `xattr_block = 0`, so no format-version
change was needed and no reader/writer skew is possible — there is exactly
one copy of this code.

### Directory entry (64 bytes, 8 per block)

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | `inode` (0 = free slot) |
| 8 | 2 | `name_len` |
| 10 | 2 | `kind` |
| 16 | 48 | `name` |

Eight direct blocks per directory inode give 64 entries per directory.

### Extended-attribute block (512 bytes, one per inode)

Allocated from the same block bitmap as any other data block — there is no
separate on-disk region for it, and `xattr_block` in the owning inode is the
only reference.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `entry_count` |
| 2 | 502 | packed entries, back-to-back, no padding |
| 504 | 8 | `checksum` over bytes 0..504 |

Each packed entry is `[name_len: u8][value_len: u16][name bytes][value
bytes]`. Names are bounded by `MAX_XATTR_NAME` (48, the same bound as a
directory-entry name) and values by `MAX_XATTR_VALUE` (256); the number of
attributes an inode can hold is implicitly bounded by the 502-byte entries
region rather than a separate count limit.

`set_xattr`/`remove_xattr` always rebuild the whole entries region (decode,
drop or replace the named entry, re-encode) rather than splicing a
variable-length region in place — simple and bounded, at the cost of
rewriting the whole block on every mutation, which is one sector regardless.
Removing the last attribute frees the block back to the bitmap in the same
transaction, exactly like `write_file` freeing a direct block it no longer
needs; `unlink` frees a still-attached xattr block the same way.

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

### Bounded multi-file replacements

The native batch path stages file creation, directory entries, inode metadata,
allocation bitmaps and complete replacement contents in a single transaction.
Parent directories must already exist, duplicate paths are rejected, and each
file is limited to `MAX_FILE_SIZE`. The total number of distinct sectors must
fit `MAX_TXN_BLOCKS`; exhaustion or invalid input before commit leaves the
volume unchanged. An individual replacement has the exact supplied length,
including truncation of longer prior contents.

A device error during or after commit is an indeterminate outcome: the caller
must remount and inspect the recovered volume before deciding whether it took
effect. The batch API persists disk state; it does not publish replacement
bytes into already-cached VFS nodes. These guarantees depend on the block
device honoring flush ordering and the sector-write assumptions above; they do
not establish physical-media power-loss qualification.

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
3. Before any target writes, reject a committed descriptor with duplicate or
   out-of-range targets, or targets in the superblock or journal sectors.
   Transaction creation applies the same checks before writing the journal.
4. Otherwise the transaction is committed but possibly only partly applied, so
   every staged data sector is written to its target again. Replay is
   **idempotent** — applying it twice is identical to applying it once — so a
   crash during recovery is itself recoverable.

The commit checksum covers staged data, not descriptor target addresses. The
target checks protect reserved sectors and prevent duplicate or out-of-range
replay, but do not authenticate a descriptor or detect redirection between
otherwise valid targets.

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
