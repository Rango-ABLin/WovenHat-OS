# Autonomous progress log

## 2026-10-28 — WovenFS check(): refuse free inode with nonzero fields

Branch: `auto/wovenhat-development` (base `69f69984`). Working tree was clean.

### Defect / gap

`check()` immediately `continue`s when `!node.is_used()` (mode == 0) without
verifying that the remaining inode fields are zero.

`Inode::empty()` zeroes every field — `mode`, `links`, `size`, `created`,
`modified`, `data_checksum`, `kind`, and all `direct[]` entries.  `format()`
zeroes inode slots directly (no encode call); `stage_unlink` writes
`Inode::empty()` to free an occupied slot.  No runtime path can therefore leave
a non-zero `links`, `size`, `data_checksum`, `kind`, or `direct[i]` in a free
inode slot.

A crafted image can reseal the structural checksum after patching any of those
fields while leaving `mode == 0`.  `Inode::decode()` accepts the image
(checksum valid), returns the inode with the non-zero field, and `check()`
silently continues — wrong.

### Change

**`kernel/src/wfs.rs`** `check()` — replaced the bare `continue` on
`!node.is_used()` with an explicit guard before continuing:

```rust
if !node.is_used() {
    // `Inode::empty()` zeroes every field. ... no runtime path can leave
    // non-zero links, size, data_checksum, kind, or any direct entry in a
    // free inode (mode == 0). ...
    if node.links != 0
        || node.size != 0
        || node.data_checksum != 0
        || node.kind != 0
        || node.direct.iter().any(|&b| b != 0)
    {
        return Err(Error::Corrupt);
    }
    continue;
}
```

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_a_free_inode_with_nonzero_fields`:
1. Formats a fresh volume (inode 2 is the first free slot).
2. For each of five cases — `links = 1`, `size = 1`, non-zero `data_checksum`,
   `kind = 1`, `direct[0] = 1` — forks the image, patches that field via the
   existing `reseal_inode_*` helpers (resealing the structural checksum so
   `mount()` accepts the image), and asserts `check()` returns `Err(Corrupt)`
   and does not modify the image.
3. Control: the unpatched volume passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **60 passed / 0 failed** (was 59; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The guard covers `links`, `size`, `data_checksum`, `kind`, and `direct[]`;
  `created` and `modified` timestamps are not checked (they do not affect
  structural invariants and no runtime path leaves them non-zero in a free
  inode, but this guard does not verify that explicitly).
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse free directory entry with nonzero fields

Branch: `auto/wovenhat-development` (base `d9f4a826`). Working tree was clean.

### Defect / gap

`check()` already verifies that the 4-byte reserved padding (raw entry bytes
12..16) is zero for every directory entry, live or free (added in the previous
session). But the check covered only those 4 bytes.

A free directory entry (inode == 0, bytes 0..8 all zero) must have ALL remaining
fields zeroed:

- `DirEntry::encode` calls `raw[..DIR_ENTRY_SIZE].fill(0)` before writing any
  field, so a freshly encoded free entry is 64 zero bytes.
- `DirEntry::empty()` returns all-zero fields.
- `stage_dir_entry` is always called with `DirEntry::empty()` when freeing a
  slot, so no runtime path ever leaves non-zero bytes in the `name_len` (bytes
  8..10), `kind` (bytes 10..12), or `name` (bytes 16..64) fields of a free
  slot.

`check()` only validated bytes 12..16 for free entries. A crafted image with a
non-zero `name_len`, `kind`, or any `name` byte in a free slot passed all
existing guards and `check()` returned `Ok(())` — wrong.

### Change

**`kernel/src/wfs.rs`** `check()` — in the existing raw-block pass, after the
reserved-padding check (bytes 12..16), added a second guard inside the per-entry
loop:

```rust
if sector[at..at + 8].iter().all(|&b| b == 0)
    && (sector[at + 8..at + 12].iter().any(|&b| b != 0)
        || sector[at + 16..at + DIR_ENTRY_SIZE].iter().any(|&b| b != 0))
{
    return Err(Error::Corrupt);
}
```

If bytes 0..8 are all zero (free entry) and any byte in 8..12 (`name_len` +
`kind`) or 16..64 (`name`) is non-zero, `check()` returns `Err(Corrupt)`.
Together with the existing padding check, this means every non-inode byte in a
free slot is verified.

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_a_free_directory_entry_with_nonzero_fields`:
1. Formats a fresh volume (all directory entries are free, all bytes zero).
2. Reads `root.direct[0]` to find the directory block.
3. For each of six cases — `name_len` lo/hi (offsets 8, 9), `kind` lo/hi
   (offsets 10, 11), first name byte (offset 16), last name byte (offset 63) —
   forks the image, patches entry 0 at that offset to a non-zero value, and
   asserts `check()` returns `Err(Corrupt)` and does not modify the image.
4. Control: the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **59 passed / 0 failed** (was 58; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The new guard covers `name_len`, `kind`, and `name` in free slots; the
  padding bytes (12..16) were already covered by the prior session's check.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse live inode with unknown kind

Branch: `auto/wovenhat-development` (base `78e92220`). Working tree was clean.

### Defect / gap

Only two inode kind values are defined: `KIND_FILE` (1) and `KIND_DIR` (2).
`stage_create` allocates only `KIND_FILE` inodes; `format()` writes exactly one
`KIND_DIR` inode (the root). Any other kind value is corruption no runtime path
can produce.

`check()` has guards gated on `kind == KIND_FILE` or `kind == KIND_DIR`, but no
guard that rejects an unknown kind outright. A live inode with, e.g., `kind = 3`
slips through every existing check silently:

- The non-root `KIND_DIR` guard fires only for kind == 2.
- All `KIND_FILE`-specific checks (links, size, data_checksum, block allocation,
  data checksum verification) are gated on `kind == KIND_FILE`.
- The orphan-inode scan at the end of `check()` skips non-`KIND_FILE` inodes:
  `if !node.is_used() || node.kind != KIND_FILE { continue; }`.
- The bitmap→inode scan passes because no directory entry names the phantom inode,
  yet its `direct` slots can all be zero (so no allocated blocks to check).
- Result: `check()` returns `Ok(())` on the corrupt image — wrong.

### Change

**`kernel/src/wfs.rs`** `check()` — at the top of the per-inode loop, right after
the `!node.is_used()` continue and before the non-root `KIND_DIR` check, added:

```rust
// Only KIND_FILE (1) and KIND_DIR (2) are defined.  `stage_create`
// allocates only KIND_FILE inodes; `format()` writes exactly one
// KIND_DIR inode (the root).  Any other value is corruption that no
// runtime path can produce.  All downstream guards are gated on
// `node.kind == KIND_FILE` or `node.kind == KIND_DIR`, so an unknown
// kind slips through every check — the KIND_FILE guards are skipped,
// the orphan-inode scan at the end skips non-KIND_FILE inodes, and
// the bitmap→inode check sees no named blocks to verify.
if !matches!(node.kind, KIND_FILE | KIND_DIR) {
    return Err(Error::Corrupt);
}
```

**`tests/wovenfs_crash.rs`** — added test `check_refuses_a_live_inode_with_unknown_kind`:
1. Formats a fresh volume (`formatted()`).
2. For each of four unknown kind values (0, 3, 0xFF, 0xFFFF):
   a. Forks the image.
   b. Calls `reseal_inode_mode(&image, 2, 0o100_644)` — inode 2 is the first free slot.
   c. Calls `reseal_inode_kind(&image, 2, kind)` — sets the unknown kind and reseals.
   d. Asserts `check()` returns `Err(Corrupt)`.
   e. Asserts `check()` does not modify the image.
3. Control: the unpatched volume passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **58 passed / 0 failed** (was 57; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

Commit: `d9f4a826`

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- Unknown kind is now caught for all non-root inode slots. The root inode (KIND_DIR)
  is the only live inode that is not KIND_FILE; the existing `root.kind != KIND_DIR`
  guard covers it.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse directory entry with non-zero reserved padding bytes

Branch: `auto/wovenhat-development` (base `26751c10`). Working tree was clean.

### Defect / gap

`DirEntry::encode` writes an on-disk layout of 64 bytes per entry:
- bytes 0..8: `inode`
- bytes 8..10: `name_len`
- bytes 10..12: `kind`
- bytes 12..16: **4-byte reserved padding** (always zeroed by `fill(0)`)
- bytes 16..64: `name` (48 bytes)

`DirEntry::decode` does not capture bytes 12..16 — they are dropped.
`check()` worked entirely with decoded `DirEntry` structs and therefore
had no way to observe those 4 bytes.  A crafted image with any non-zero
byte at offsets 12..16 passed all existing guards and `check()` returned
`Ok(())` — wrong.

This was explicitly noted as an open item in the previous session's entry.

### Change

**`kernel/src/wfs.rs`** `check()` — after the per-live-entry validation
loop, added a raw-block pass:

```rust
// The DirEntry on-disk layout has a 4-byte reserved gap at raw bytes
// 12..16, between `kind` (10..12) and `name` (16..64).
// `DirEntry::encode` calls `raw[..DIR_ENTRY_SIZE].fill(0)` before
// writing any field, so those 4 bytes are always zero on a
// correctly-written filesystem.  The decoded `DirEntry` struct drops
// those bytes; `check()` must inspect the raw block directly.
for block in root.direct.iter().copied().filter(|&b| b != 0) {
    let sector = self.read_block_staged(&txn, block)?;
    for index in 0..DIR_ENTRIES_PER_BLOCK {
        let at = index * DIR_ENTRY_SIZE;
        if sector[at + 12..at + 16].iter().any(|&b| b != 0) {
            return Err(Error::Corrupt);
        }
    }
}
```

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_a_directory_entry_with_nonzero_reserved_padding`:
1. Formats a fresh volume with `a.txt` and `b.txt`.
2. For each of the 4 padding byte offsets (12, 13, 14, 15 within the entry):
   a. Forks the image.
   b. Patches that byte to a non-zero value via `patch_dir_entry`.
   c. Asserts `check()` returns `Err(Corrupt)`.
   d. Asserts `check()` does not modify the image.
   e. Asserts `b.txt` remains readable.
3. Control: the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **57 passed / 0 failed** (was 56; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

Commit: `f6aa692b`

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- Padding is verified for all entries (live and free) in the directory block
  — `encode()` zeroes the full 64-byte slot unconditionally, so non-zero
  padding in a free slot is also corruption.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse directory entry with non-zero bytes beyond name_len

Branch: `auto/wovenhat-development` (base `f81d6001`). Working tree was clean.

### Defect / gap

`stage_create` builds every new `DirEntry` via `DirEntry::empty()`, which
zeroes the full 48-byte `name` array, then copies only `name[..name.len()]`
bytes.  `DirEntry::encode` writes the entire `self.name` array (all 48 bytes)
to disk — including the trailing zeros from `empty()`.  Therefore
`entry.name[name_len..]` is always zero on a correctly-written filesystem.

`check()` validated `entry.name[..name_len]` for null bytes, `/`, `\\`,
`.`/`..`, and non-UTF-8, but did not verify that the bytes beyond `name_len`
are zero.  A crafted image with `0xFF` (or any non-zero byte) at a position
past `name_len` passed all existing guards and `check()` returned `Ok(())` —
wrong.

`matches()` only compares `name[..name_len]` against the caller-supplied key,
so the crafted entry is still fully reachable at runtime.  The deviation is
only detectable by `check()`.

### Change

**`kernel/src/wfs.rs`** `check()` — after the existing UTF-8 guard, added:

```rust
// `stage_create` builds every new entry via `DirEntry::empty()`,
// which zeroes the full `MAX_NAME`-byte name array, then copies
// exactly `name.len()` bytes into `name[..name_len]`.  The bytes
// at `name[name_len..]` are therefore always zero on a
// correctly-written filesystem.  `DirEntry::encode` copies the
// full `self.name` array (including those trailing zeros) to disk,
// so no runtime write ever plants a non-zero byte past `name_len`.
// A crafted entry with non-zero bytes beyond `name_len` is
// corruption that the runtime cannot have produced.
if entry.name[name_len..].iter().any(|&b| b != 0) {
    return Err(Error::Corrupt);
}
```

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_a_directory_entry_with_nonzero_bytes_beyond_name_len`:
1. Formats a fresh volume with `a.txt` and `b.txt`.
2. For each of three positions beyond `name_len` (`name[5]` = offset 21,
   `name[15]` = offset 31, `name[47]` = offset 63):
   a. Forks the image.
   b. Patches that raw byte in a.txt's directory entry to a non-zero value.
   c. Asserts `check()` returns `Err(Corrupt)`.
   d. Asserts `check()` does not modify the image.
   e. Asserts `b.txt` remains readable.
3. Control: the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **56 passed / 0 failed** (was 55; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The guard covers trailing bytes in the `name` field only; the 4-byte
  padding gap between `kind` and `name` (raw entry bytes 12..16) is always
  zero from `encode()` but is not yet explicitly verified by `check()`.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse directory entry with non-UTF-8 name

Branch: `auto/wovenhat-development` (base `393cb925`). Working tree was clean.

### Defect / gap

`stage_create` accepts file names only through `checked_native_name(name: &str)`.
Rust's type system guarantees every `&str` is valid UTF-8, so no runtime path
can store a name whose bytes are not valid UTF-8.

An entry with invalid UTF-8 bytes in its name is permanently unreachable:
`DirEntry::matches` compares caller-supplied `&str` bytes against the stored
bytes directly.  A crafted byte sequence (e.g. `0xFF`) can never equal any
valid UTF-8 string, so `lookup` and `unlink` can never find the entry.
Neither the inode slot nor its data blocks can be freed through the normal
path; the file is a permanent space leak invisible to the user.

`check()` already validates `name_len`, null bytes, `/`, `\\`, and `.`/`..`,
but did not call `core::str::from_utf8` on the stored bytes.  A crafted image
with any non-UTF-8 byte in the name field (e.g. `0xFF`, `0xC0`, `0x80`)
passed all existing guards and `check()` returned `Ok(())` — wrong.

### Change

**`kernel/src/wfs.rs`** `check()` — after the existing dot/dotdot guard,
added:

```rust
// `stage_create` accepts names only as `&str`, which Rust guarantees
// to be valid UTF-8.  No runtime path can therefore store a name
// whose bytes are not valid UTF-8.  An entry with a non-UTF-8 name
// is permanently unreachable: every lookup converts the caller-
// supplied name through `&str`, so the stored byte sequence can
// never compare equal to any valid lookup argument.  The file and
// its data blocks cannot be freed through any normal path either.
if core::str::from_utf8(&entry.name[..name_len]).is_err() {
    return Err(Error::Corrupt);
}
```

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_a_directory_entry_whose_name_is_not_valid_utf8`:
1. Formats a fresh volume with two files (`a.txt`, `b.txt`).
2. For each of three invalid UTF-8 byte values (`0xFF`, `0xC0`, `0x80`):
   a. Forks the image.
   b. Patches the first name byte of `a.txt`'s directory entry to the invalid byte.
   c. Asserts `check()` returns `Err(Corrupt)`.
   d. Asserts `check()` does not modify the image.
   e. Asserts `b.txt` remains readable.
3. Control: the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **55 passed / 0 failed** (was 54; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The guard covers names at creation time; existing data blocks are not affected.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse KIND_FILE inode with size above MAX_FILE_SIZE

Branch: `auto/wovenhat-development` (base `0382eb7b`). Working tree was clean.

### Defect / gap

`check()` did not verify that a `KIND_FILE` inode's `size` field is within
`MAX_FILE_SIZE` (`DIRECT_BLOCKS * SECTOR_SIZE = 4096`).

`stage_write` and `create_file` both enforce `data.len() <= MAX_FILE_SIZE`, so
no runtime path can produce a file inode with a larger size.  Without an
explicit guard the per-inode block-count check in `check()` computes:

```rust
let needed = (node.size as usize).div_ceil(SECTOR_SIZE);
```

When `size > MAX_FILE_SIZE`, `needed > DIRECT_BLOCKS` (e.g. 9 for size 4097).
The block-count loop only covers `slot in 0..DIRECT_BLOCKS`, so with all 8
slots non-zero and `needed = 9`, every `(true) != (slot < 9)` = false — the
loop passes silently.  The data-checksum loop that follows then runs:

```rust
for slot in 0..needed {
    let block = node.direct[slot];   // slot 8 is out of bounds!
```

Indexing `node.direct[8]` on an `[u64; 8]` array panics in both debug and
release builds.  So a crafted image with `size = MAX_FILE_SIZE + 1` and all 8
direct blocks set causes `check()` to **panic** rather than return `Corrupt`.

### Change

**`kernel/src/wfs.rs`** `check()` — before the block-count check, added:

```rust
if node.kind == KIND_FILE && node.size > MAX_FILE_SIZE as u64 {
    return Err(Error::Corrupt);
}
```

An eight-line comment explains that `stage_write`/`create_file` enforce the
bound, so any larger value is corruption, and that without this guard the
data-checksum loop would index past the eight-element `direct` array.

**`tests/wovenfs_crash.rs`** — added test
`check_refuses_file_inode_with_size_above_max_file_size`:
1. Formats a fresh volume.
2. Creates a file with exactly `MAX_FILE_SIZE` bytes (filling all 8 direct
   blocks and setting the correct data checksum).
3. Forks the image and patches the inode's `size` to `MAX_FILE_SIZE + 1` via
   `reseal_inode_size`.  All 8 blocks remain non-zero and the data checksum
   is unchanged.
4. Asserts `check()` returns `Err(Corrupt)`.
5. Asserts `check()` does not modify the image.
6. Control: the unpatched volume passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **54 passed / 0 failed** (was 53; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The guard covers the `MAX_FILE_SIZE` limit enforced by the runtime.  When
  indirect blocks are added (future), the guard must be updated to match the
  new maximum.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse root directory inode with wrong mode

Branch: `auto/wovenhat-development` (base `a6d56119`). Working tree was clean.

### Defect / gap

`format()` hardcodes `root.mode = 0o040_755` and no runtime path ever modifies
a directory inode's `mode` field: `stage_create` sets `mode` only on freshly
allocated file inodes, not on the directory.  Any value other than `0o040_755`
is corruption the runtime cannot have produced.

`check()` already validated `root.kind`, `root.links`, `root.data_checksum`,
`root.size`, and `root.direct[0]` / `root.direct[1..]`, but never compared
`root.mode` against the format constant.  A non-zero invalid mode (e.g.
`0o100_644`) evades every existing guard:

- `is_used()` returns true (mode != 0), so the per-inode loop processes the inode.
- The `non-root KIND_DIR` guard fires only for non-root inodes with `KIND_DIR`;
  the root inode number is equal to `root_inode`, so it passes.
- All `KIND_FILE`-specific checks are skipped for the directory inode.
- The root_block is still allocated, named by a live inode → bitmap orphan scan passes.
- None of the root section checks (`kind`, `links`, etc.) compare `mode`.
- `check()` therefore returns `Ok(())` — wrong.

A mode of zero would be caught by the bitmap orphan scan (root_block appears
allocated but unnamed), but any non-zero invalid mode passes silently.

### Change

**`kernel/src/wfs.rs`** `check()` — at the start of the root section, before
the existing `root.kind != KIND_DIR` guard, added:

```rust
if root.mode != 0o040_755 {
    return Err(Error::Corrupt);
}
```

A ten-line comment explains the `format()` invariant, why zero is already caught
by the orphan scan, and why non-zero invalid modes evade every earlier check.

**`tests/wovenfs_crash.rs`** — added test `check_refuses_root_directory_inode_with_wrong_mode`:
1. Formats a fresh volume (`formatted()`).
2. Reads `root_inode` from the superblock and asserts it is 1.
3. For each of `1u32`, `0o100_644`, `0o777`, `u32::MAX`:
   a. Forks the image.
   b. Calls `reseal_inode_mode(&corrupt, root_inode, mode)` to patch the mode
      field and re-seal the inode checksum.
   c. Asserts `check()` returns `Err(Corrupt)`.
   d. Asserts `check()` does not modify the image.
4. Control: asserts the unpatched volume passes `check()`.

The test reuses the existing `reseal_inode_mode` helper (patches bytes 0–3 of
the on-disk inode and re-seals the structural checksum).

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **53 passed / 0 failed** (was 52; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The mode check covers only the fixed `0o040_755` constant that `format()` writes.
  When directory modes become configurable (future mkdir support), this guard must
  be relaxed or replaced with a valid-set check.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse non-root KIND_DIR inode

Branch: `auto/wovenhat-development` (base `6d471968`). Working tree was clean.

### Defect / gap

`check()` did not validate that the only live `KIND_DIR` inode is the root.
`mkdir` is not implemented — this is a flat, single-directory filesystem — so
`stage_create` never allocates a `KIND_DIR` inode; all created inodes carry
`KIND_FILE`.  A phantom `KIND_DIR` inode injected into any non-root slot evades
every guard in the function:

- The `KIND_FILE`-specific checks (links, size, data_checksum, block/size
  consistency) are skipped.
- The per-inode block-range and bitmap checks have nothing to verify when no
  data blocks are set.
- `refuse_shared_blocks` has nothing to check either.
- The orphan-block scan is unaffected (no blocks).
- The orphan-inode scan (`kind != KIND_FILE → continue`) skips the phantom.
- `check()` therefore returns `Ok(())`, calling the image clean — wrong.

### Change

**`kernel/src/wfs.rs`** `check()` — at the start of the per-inode loop, after
the `!node.is_used()` continue and before the first `KIND_FILE` check, added:

```rust
if node.kind == KIND_DIR && number != sb.root_inode {
    return Err(Error::Corrupt);
}
```

An eleven-line comment explains the invariant and why the orphan-inode scan
cannot surface it without this guard.

**`tests/wovenfs_crash.rs`** — added two helpers and one test:

- `reseal_inode_mode(image, inode, mode)`: sets bytes 0–3 (the `mode` field)
  in the on-disk inode and re-seals the checksum.  Setting a non-zero mode
  on an otherwise-zeroed slot makes it "used" (`is_used()` returns `mode != 0`).
- `reseal_inode_kind(image, inode, kind)`: sets bytes 104–105 (the `kind` field)
  and re-seals.  Called after `reseal_inode_mode` so the re-seal inherits the
  correct mode.
- `check_refuses_non_root_dir_kind_inode`:
  1. Formats a fresh volume (`formatted()`).
  2. Forks the image.
  3. Calls `reseal_inode_mode(&corrupt, 2, 0o040_755)` — inode 2 is the first
     free slot after root (inode 1).
  4. Calls `reseal_inode_kind(&corrupt, 2, KIND_DIR)` — now inode 2 is a live
     `KIND_DIR` inode with no data blocks.
  5. Asserts `check()` returns `Err(Corrupt)`.
  6. Asserts `check()` does not modify the image.
  7. Control: asserts the unpatched volume passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **52 passed / 0 failed** (was 51; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The check covers only the root inode as the sole valid `KIND_DIR` entry.
  When `mkdir` is eventually implemented, this guard must be relaxed or replaced
  with a proper directory-tree traversal.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): standalone test for non-zero extra direct block in root inode

Branch: `auto/wovenhat-development` (base `0a9c9b6b`). Working tree was clean.

### Defect / gap

The prior session added a guard in `check()` (`kernel/src/wfs.rs` lines 1462–1466)
that refuses a root directory inode with any non-zero `direct[slot]` for
`slot in 1..DIRECT_BLOCKS`.  That guard was documented as having no standalone
test because exercising it requires a block that is both named by `direct[1]` and
marked allocated in the bitmap — if the bitmap bit is clear, the per-inode
range/bitmap scan fires first; if the bitmap bit is set but the block is not named
by any inode, the orphan-block scan fires first.  The previous session explicitly
listed "A test would require manually setting a bitmap bit for an otherwise-free
block" as an open item.

### Change

**`tests/wovenfs_crash.rs`:**

- Added `set_bitmap_block(image, block)` helper: symmetric to the existing
  `clear_bitmap_block`, it sets one bit in the on-disk bitmap without touching
  any inode or checksum.  Used to make an otherwise-free block appear
  bitmap-allocated so the orphan-block scan passes, leaving the root-specific
  `direct[1..]` guard as the first error path.

- Added test `check_refuses_root_directory_inode_with_nonzero_extra_direct_block`:
  1. Formats a fresh volume (`formatted()`).
  2. Reads `root_block = read_inode_direct0(&base, 1)` — the root directory's
     only data block, allocated at `data_start`.
  3. Computes `extra_block = root_block + 1`, the next free data block.
  4. Asserts `extra_block < total_blocks` (paranoia; 192-sector/32-inode volume
     has many free blocks).
  5. Calls `set_bitmap_block(&corrupt, extra_block)` so the orphan-block scan
     reports the block as "named by root via `direct[1]`" and passes.
  6. Calls `reseal_inode_direct(&corrupt, 1, 1, extra_block)` to set
     `root.direct[1]` and re-seal the inode checksum.
  7. Asserts `check()` returns `Err(Corrupt)`.
  8. Asserts `check()` does not modify the image.
  9. Control: asserts the unpatched volume passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **51 passed / 0 failed** (was 50; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS (exit 0) |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- `direct[2..DIRECT_BLOCKS]` slots have no individual tests; the new test covers
  only slot 1.  The guard uses a loop, so the same code handles all slots.  Adding
  one test per slot would add redundancy without covering new code paths.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Only the root directory inode's `direct` block count is verified; the exact
  content of the directory data block (beyond what `dir_entries` validates) is not
  independently hashed.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse root directory inode with zero first direct block

Branch: `auto/wovenhat-development` (base `04f7e9d1`). Working tree was clean.

### Defect

`format()` always sets `root.direct[0]` to the directory data block and zeroes
`direct[1..DIRECT_BLOCKS]` via `Inode::empty()`.  No runtime path (`stage_create`,
`stage_unlink`) ever modifies a directory inode's `direct` array.  Two invariants
therefore hold for every valid root directory inode:

1. `direct[0] != 0` — there is always exactly one directory data block.
2. `direct[1..DIRECT_BLOCKS]` are all 0 — no extra blocks beyond the first.

`check()` verified the root's `size`, `links` and `data_checksum` fields (added in
the previous series) but never checked its `direct` pointers.  If `direct[0] == 0`,
`dir_entries` silently skips every slot and returns an empty `DirectoryView`; the
orphan check at the end of `check()` would then flag every live file inode as having
no directory entry, but only if the root_block is still in the bitmap.  An image
where `direct[0]` was zeroed AND the bitmap bit for root_block was also cleared would
pass the bitmap orphan scan and then call `dir_entries` on an inode with no blocks,
returning an empty view and hiding all orphaned file inodes.  A non-zero `direct[1]`
is an extra block the runtime never allocates for a directory; `dir_entries` would
read it as a second directory data page.

Neither condition is surfaced by any earlier check in `check()`.

### Change

- `kernel/src/wfs.rs` `check()`: after the existing `root.size != SECTOR_SIZE` guard,
  added a `root.direct[0] == 0` guard and a `for slot in 1..DIRECT_BLOCKS` guard,
  each returning `Err(Error::Corrupt)`.  An eleven-line comment explains the
  `format()` invariant and why `dir_entries` cannot surface either inconsistency.

- `tests/wovenfs_crash.rs`: added two helper functions:
  - `read_inode_direct0` — reads the `direct[0]` field from an on-disk inode without
    mounting a Volume.
  - `clear_bitmap_block` — clears a bitmap bit directly in the image, allowing tests
    to suppress the bitmap orphan scan so a downstream guard becomes the first error.
  
  Added test `check_refuses_root_directory_inode_with_zero_first_direct_block`:
  formats a volume, reads `root_block` from inode 1's `direct[0]`, forks the image,
  clears the bitmap bit for `root_block` (so the orphan-block scan does not fire
  first), patches `direct[0]` to 0 via `reseal_inode_direct`, and asserts `check()`
  returns `Err(Corrupt)`.  Also asserts `check()` does not modify the image, and
  the unpatched volume passes.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **50 passed / 0 failed** (was 49; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- The `direct[1..] != 0` guard is present in `check()` as defense-in-depth but has
  no standalone test: in all normal scenarios the per-inode block range/bitmap check
  in the first loop of `check()` catches it before the root section runs.  A test
  would require manually setting a bitmap bit for an otherwise-free block.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Only the root directory is checked; the root's `direct` block count is now
  verified, but the exact content of the directory data block (beyond what
  `dir_entries` already validates via the entry-level checks) is not independently
  hashed.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse root directory inode with wrong size

Branch: `auto/wovenhat-development` (base `64395463`). Working tree was clean.

### Defect

`format()` always sets the root directory inode's `size` field to `SECTOR_SIZE`
(512 bytes — one directory data block).  No runtime path ever modifies a
directory inode's `size`: `stage_create` updates only the directory inode's
`modified` field, and neither `stage_dir_entry` nor `stage_unlink` touches
`size`.  Therefore any root directory inode with `size != SECTOR_SIZE` represents
corruption that the runtime cannot have produced.

`dir_entries` reads directory blocks by their `direct` pointers and ignores the
`size` field entirely.  Without an explicit guard in `check()` this inconsistency
passes silently — the same oracle-vs-runtime gap family as the recent
`data_checksum`, `links`, and size-0 file series.

### Change

- `kernel/src/wfs.rs` `check()`: after the existing `root.data_checksum != 0`
  guard, added a `root.size != SECTOR_SIZE as u64` guard returning
  `Err(Error::Corrupt)`.  An eight-line comment explains the `format()` invariant
  and why `dir_entries` cannot surface this inconsistency.

- `tests/wovenfs_crash.rs`: added `reseal_inode_size` helper (patches offset 8 in
  the raw inode, re-seals the structural checksum), and added test
  `check_refuses_root_directory_inode_with_wrong_size`.  The test patches the
  root inode's size to each of 0, 1, `SECTOR_SIZE - 1`, and `SECTOR_SIZE + 1` and
  asserts `check()` returns `Err(Corrupt)` in every case without modifying the
  image.  The unpatched volume passes as the positive control.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **49 passed / 0 failed** (was 48; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Only the root directory's `size` invariant is checked; the root's `direct`
  block assignments and their exact block addresses are not independently verified
  beyond the existing bitmap and aliasing checks.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse root directory inode with non-zero data_checksum

Branch: `auto/wovenhat-development` (base `4bac909a`). Working tree was clean.

### Defect

`check()` enforced that a `KIND_FILE` inode with `size == 0` must have
`data_checksum == 0` (added in the previous session), but the parallel invariant
for the root directory inode was never checked.

`format()` calls `Inode::empty()` first, which zeroes all fields including
`data_checksum`.  The `data_checksum` field is then only updated by `stage_write`,
and only for `KIND_FILE` inodes.  No runtime path (`stage_create`, `stage_write`,
`stage_unlink`) ever assigns a non-zero `data_checksum` to a `KIND_DIR` inode.
Therefore any root directory inode with `data_checksum != 0` represents corruption
that the runtime cannot have produced.

`dir_entries` reads directory blocks entirely by block address and ignores
`data_checksum`, so without an explicit guard in `check()` this inconsistency
passes silently.  The inode structural checksum (validated by `Inode::decode`)
would catch raw bit-flip corruption, but a logically resealed inode (one where
the structural checksum was updated after corrupting `data_checksum`) would not
be caught — the same oracle-vs-runtime gap family as the recent `size-0 file`
and `links` series.

### Change

- `kernel/src/wfs.rs` `check()`: after the existing `root.links != 2` guard,
  added a `root.data_checksum != 0` guard returning `Err(Error::Corrupt)`.  A
  five-line comment explains the `Inode::empty()` invariant and why `dir_entries`
  cannot surface this inconsistency.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_root_directory_inode_with_nonzero_data_checksum`.
  Formats a volume, reads `root_inode` from the superblock, forks the image,
  patches the root inode's `data_checksum` to `fnv1a(b"corrupt")` via
  `reseal_inode_data_checksum` (which reseals the structural checksum so
  `mount()` accepts it), and asserts `check()` returns `Err(Corrupt)`.
  Also asserts `check()` does not modify the image and the unpatched volume passes.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **48 passed / 0 failed** (was 47; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse size-0 file inode with non-zero data_checksum

Branch: `auto/wovenhat-development` (base `4e27ff79`). Working tree was clean.

### Defect

`check()` validated file content checksums only when `node.size > 0`, matching
`read_file`'s own skip for empty files (added in a prior session).  Neither path
verified the converse: a `size == 0` file inode **must** have `data_checksum == 0`.

`stage_create` always initialises new files with `Inode::empty()`, which sets
`data_checksum = 0`.  `stage_write` sets `data_checksum` to `fnv1a(data)` only
when content is written.  No runtime path can produce a file inode with
`size == 0` and `data_checksum != 0`.

Without this check, a corrupted image (e.g. direct disk manipulation or a
hypothetical incomplete journal replay that cleared `size` but not
`data_checksum`) would pass `check()` silently.  `read_file` would return
`Ok(0)` without verifying the field, and neither the structural nor the
block-range checks touch `data_checksum` for zero-size files.  This is the same
oracle-vs-runtime gap family as the recent `links` and orphan-inode series.

### Change

- `kernel/src/wfs.rs` `check()`: after the existing `links != 1` guard for
  `KIND_FILE` inodes, added a `size == 0 && data_checksum != 0` guard returning
  `Err(Error::Corrupt)`.  A nine-line comment explains the `stage_create`
  invariant and why the earlier checks do not surface this inconsistency.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_zero_size_file_with_nonzero_data_checksum`.
  Creates a volume, calls `create()` to produce an empty file, forks the image,
  patches `data_checksum` to `fnv1a(b"corrupt")` via `reseal_inode_data_checksum`
  (which also reseals the inode's structural checksum so `mount()` accepts it),
  and asserts `check()` returns `Err(Corrupt)`.  Also asserts `check()` does not
  modify the image and the unpatched image passes.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **47 passed / 0 failed** (was 46; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and bitmap→inode scan are O(n²); acceptable for a consistency
  check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse root directory inode with wrong link count

Branch: `auto/wovenhat-development` (base `a7ad1c2a`). Working tree was clean.

### Defect

`check()` enforces `links == 1` for every live `KIND_FILE` inode but never
validated the root directory inode's `links` field. `format()` always writes
`root.links = 2` (one link from the filesystem root, one from "." inside it),
and no runtime operation (`create`, `write`, `unlink`) ever modifies a
directory inode's link count. Any value other than 2 therefore represents
corruption that `format()` cannot have produced.

Without this check, `check()` called a volume clean even when the root
directory inode carried `links = 0`, `links = 1`, or any other wrong value —
an oracle-vs-runtime gap in the same family as the `links == 1` gap already
fixed for file inodes.

The pattern is exactly parallel: `unlink` already requires `links == 1` before
touching a file inode; `check()` already enforces `links == 1` for all live
file inodes. The root dir's `links == 2` invariant was the one matching gap.

### Change

- `kernel/src/wfs.rs` `check()`: after the existing `root.kind != KIND_DIR`
  guard, added a `root.links != 2` guard returning `Err(Error::Corrupt)`.
  A six-line comment explains the format invariant and why any other value
  is corruption.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_root_directory_inode_with_wrong_link_count`.
  Formats a volume, reads `root_inode` from the superblock, and for each of
  `links` in `[0, 1, 3, u32::MAX]` forks the image, patches the root inode's
  `links` field via `reseal_inode_links`, and asserts `check()` returns
  `Err(Corrupt)`. Also asserts `check()` does not modify the image, and that
  the unpatched image passes.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **46 passed / 0 failed** (was 45; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan and the bitmap→inode scan are O(n²); acceptable for a
  consistency check but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse live file inode not named by any directory entry

Branch: `auto/wovenhat-development` (base `0fcb65a6`). Working tree was clean.

### Defect

`check()` verified the entry→inode direction exhaustively: every live directory
entry must name a live `KIND_FILE` inode.  It did not verify the reverse
direction: every live `KIND_FILE` inode must be named by exactly one directory
entry.

An orphaned inode — `mode != 0`, `kind == KIND_FILE`, but no directory entry
has `inode == number` — represents a permanent space leak.  `unlink()` works
by name: without a directory entry there is no name, so neither the inode slot
nor its data blocks can ever be freed through the normal path.  The file is
also inaccessible: `lookup()` only returns entries whose `entry.inode != 0`.

The bitmap consistency checks cannot surface this because the orphan's blocks
are correctly marked allocated and named by the (still-intact) inode.  The
inode→bitmap check (every inode-named block is in the bitmap) and the
bitmap→inode check (every allocated block is named by some live inode) both
pass.  Only the inode↔entry cross-reference exposes the orphan.

This corruption scenario arises when: the directory entry's inode field is
zeroed without also freeing the inode (e.g., direct disk manipulation or a
partial in-flight patch), or after a hypothetical journal replay that cleared
the directory write but not the inode write.

### Change

- `kernel/src/wfs.rs` `check()`: after the two-entry duplicate scans, added a
  loop over all inodes `1..=inode_count`.  For each live `KIND_FILE` inode,
  it scans `view.entries` to find at least one entry with `entry.inode ==
  number`.  If none is found, returns `Err(Error::Corrupt)`.  A twelve-line
  comment explains the space-leak scenario and why the bitmap checks alone
  cannot detect it.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_live_file_inode_not_named_by_any_directory_entry`.
  Creates a volume with one file ("orphan.txt"), locates its directory entry,
  zeroes the `inode` field (bytes 0..8) in the raw directory block via
  `patch_dir_entry` (turning the entry into a free slot without touching the
  inode table), and asserts `check()` returns `Err(Corrupt)`.  Also asserts:
  the image is unmodified by check(), and the unpatched image passes check().

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **45 passed / 0 failed** (was 44; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphan scan is O(inode_count²) in the worst case; acceptable for a
  consistency-check function but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.

## 2026-10-28 — WovenFS: read_file() spuriously rejects empty files

Branch: `auto/wovenhat-development` (base `4929ee81`). Working tree was clean.

### Defect

`stage_create` initialises a new empty file inode with `Inode::empty()`, which
leaves `data_checksum = 0`.  `read_file` computed `fnv1a(&out[..size])` for the
checksum comparison unconditionally, including when `size == 0`.  The FNV-1a
hash of an empty byte slice is the seed value `0xcbf29ce484222325`, not 0, so
every freshly-created empty file (before any `write_file` call) would fail the
comparison `fnv1a(&out[..0]) != 0` and return `Err(Corrupt)`.

`check()` already documents and correctly skips the checksum for `size == 0`
files, noting that `stage_create` uses a zeroed checksum field by design.
`read_file` diverged from this invariant.

This is a runtime defect, not a test-only gap: any caller that created an empty
file with `create()` and then read it back before writing would receive a
spurious Corrupt error.  `mount_ata_into_vfs` silently skips files that fail
`read_file`, so empty WovenFS files would be silently dropped during import.

### Change

- `kernel/src/wfs.rs` `read_file()`: added `size > 0 &&` guard before the
  FNV-1a checksum comparison, matching `check()`'s documented behaviour.  A
  three-line comment explains the invariant.

- `tests/wovenfs_crash.rs`: added `read_file_succeeds_for_an_empty_file`.
  Creates an empty file with `create()`, reads it back with `read_file()`, and
  asserts `Ok(0)`.  Also asserts `check()` passes on the same volume.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **44 passed / 0 failed** (was 43; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse two directory entries naming the same inode

Branch: `auto/wovenhat-development` (base `81b70ce4`). Working tree was clean.

### Defect

`check()` verified that no two live directory entries share the same name (the
O(n²) pairwise scan added in the duplicate-name session) but did not verify
that no two entries share the same inode number. Under normal operation
`stage_create` always allocates a fresh inode, so two entries with different
names pointing to the same inode number represent an impossible state:
effectively a hard link whose count would be 2, inconsistent with the
`links == 1` invariant that `check()` enforces per-inode. After unlinking
one of those two entries the inode is freed, leaving the other entry pointing
to a now-free inode — exactly the stale-entry corruption that later checks
already detect — but `check()` would have silently called the pre-unlink image
clean, creating an oracle-vs-runtime inconsistency.

### Change

- `kernel/src/wfs.rs` `check()`: added a second O(n²) pairwise scan over live
  directory entries immediately after the duplicate-name scan. For each pair
  (i, j) with i > j and both entries live (inode != 0), if
  `view.entries[i].inode == view.entries[j].inode`, `check()` returns
  `Err(Error::Corrupt)`. At most 64 entries / 2016 pairs — same bound as the
  existing name-duplicate scan.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_two_directory_entries_naming_the_same_inode`.
  Uses `two_file_root()`, finds b.txt's entry index, patches its `inode` field
  (offset 0) to match a.txt's inode via `patch_dir_entry`, and asserts
  `check()` returns `Err(Corrupt)`, the image is unchanged, a.txt's inode
  remains readable, and the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **43 passed / 0 failed** (was 42; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The WovenFS flat-root adapter covers only the root directory; nested
  directories and rename remain unimplemented.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-28 — WovenFS check(): refuse file with mismatched data_checksum

Branch: `auto/wovenhat-development` (base `1df80ada`). Working tree was clean.

### Defect

`check()` verified every structural invariant — inode block allocation, directory
entry validity, cross-inode aliases, bitmap-vs-inode consistency — but never
read a file's data blocks to verify them against the `data_checksum` stored in
the inode.  `read_file` computes `fnv1a` over the complete file content and
returns `Err(Corrupt)` on mismatch; a file whose data blocks were silently
overwritten on disk (with the inode checksum left intact) would therefore fail at
read time but be called clean by `check()` — the same oracle-vs-runtime
inconsistency pattern as previous sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the size-vs-blocks consistency check,
  added a content-checksum verification for live `KIND_FILE` inodes with
  `size > 0`.  Reads each data block with `self.read_block(block)`, chains
  FNV-1a through `fnv1a_seeded` block by block (no allocation needed), and
  returns `Err(Error::Corrupt)` if the final hash does not equal
  `node.data_checksum`.  The tail of the last block is trimmed to `size %
  SECTOR_SIZE` bytes when the file does not fill its final sector, matching
  the logic in `read_file`.

- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_file_with_a_mismatched_data_checksum`.  Uses
  `two_file_root()` (a.txt="alpha", b.txt="bravo"), forks the image, flips
  the first byte of a.txt's data block via `CrashDisk::flip_byte`, and
  asserts `check()` returns `Err(Corrupt)`, the image is unchanged, b.txt
  remains readable, and the unpatched image passes `check()`.

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **42 passed / 0 failed** (was 41; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The data-checksum scan reads every data block; O(data_blocks) I/O for a
  consistency-check function, acceptable but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

## 2026-10-28 — WovenFS check(): refuse bitmap-allocated block named by no inode

Branch: `auto/wovenhat-development` (base `22681df3`). Uncommitted changes found
in `kernel/src/wfs.rs` and `tests/wovenfs_crash.rs`; resumed and completed that
checkpoint.

### Defect

`check()` verified the inode→bitmap direction (every block named by a live inode
must have its bitmap bit set) but not the reverse. A data block whose bitmap bit
was set but no live inode named it represented a leaked sector: the allocator
could never hand it out again, making the space permanently unavailable. The
previous session noted this as open: "The bitmap-leak direction (block allocated
in bitmap but not named by any live inode) is not yet verified by `check()`."

### Change

- `kernel/src/wfs.rs` `check()`: after the per-inode cross-alias scan, added a
  loop over every data block (`sb.data_start..sb.total_blocks`). For each block
  the bitmap marks allocated, it scans all live inodes. If none names that block,
  `check()` returns `Err(Error::Corrupt)`. Uses `.contains(&block)` per Clippy's
  `manual_contains` lint.

- `tests/wovenfs_crash.rs`: Added
  `check_refuses_a_bitmap_allocated_block_named_by_no_inode`. Formats a fresh
  volume, calls `mark_allocated` on `data_start + 1` (a free block), verifies
  `check()` returns `Err(Corrupt)`, verifies the image is unchanged, and verifies
  a clean image still passes. Removed three trailing blank lines between earlier
  test functions (cosmetic, pre-existing).

### Results (run on this host, 2026-10-28)

| Command | Result |
|---|---|
| `cargo test --test wovenfs_crash` | PASS, **41 passed / 0 failed** (was 40; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- The orphaned-block scan is O(data_blocks × inode_count); acceptable for a
  consistency-check function but not for a hot path.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

## 2026-10-27 — WovenFS check(): refuse files whose block allocation is inconsistent with size

Branch: `auto/wovenhat-development` (base `a25016b3`). Working tree was clean.

### Defect

`check()` validated all per-entry and per-inode constraints but did not verify
the relationship between a file inode's `size` field and its `direct[]` block
pointers. `stage_write` enforces the invariant that `direct[i] != 0` for every
slot `i < size.div_ceil(SECTOR_SIZE)` and `direct[i] == 0` for every slot at
or beyond that index. Two deviations from this invariant are possible through
disk corruption:

1. **Size > 0, required block missing**: `direct[i] == 0` for some `i` where
   `i * SECTOR_SIZE < size`. `read_file` returns `Err(Corrupt)` when it hits
   the empty slot, but `check()` silently skipped zero blocks (via `continue`)
   and called the image clean — an oracle-vs-runtime inconsistency.

2. **Extra block beyond size**: `direct[i] != 0` for some `i >= size.div_ceil(SECTOR_SIZE)`.
   `read_file` never accesses that slot and `unlink` would free it correctly,
   but `check()` did not flag the orphaned block.

### Change

- `kernel/src/wfs.rs` `check()`: after the per-inode `links` check, added a
  size-vs-blocks consistency check for `KIND_FILE` inodes. Computes
  `needed = (node.size as usize).div_ceil(SECTOR_SIZE)` and for each slot
  `0..DIRECT_BLOCKS` returns `Err(Corrupt)` if `(node.direct[slot] != 0) !=
  (slot < needed)`.

- `tests/wovenfs_crash.rs`:
  - Added `reseal_inode_direct(image, inode, slot, block)` helper that patches
    one `direct[]` entry in the raw inode sector and reseals the inode checksum,
    analogous to the existing `reseal_inode_links` helper.
  - Added `check_refuses_a_file_inode_whose_blocks_are_inconsistent_with_its_size`
    covering both directions:
    - Case 1: zeroes `direct[0]` of `a.txt` (size=5) → `Err(Corrupt)`.
    - Case 2: patches the inode to `size=0` while keeping `direct[0]` non-zero
      → `Err(Corrupt)`.
    - Both cases: image is unmodified, `b.txt` remains readable.
    - Control: unpatched image passes `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **40 passed / 0 failed** (was 39; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.
- The bitmap-leak direction (block allocated in bitmap but not named by any live
  inode) is not yet verified by `check()`.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse duplicate directory entry names

Branch: `auto/wovenhat-development` (base `63641dce`). Working tree was clean.

### Defect

`check()` validated all per-entry field constraints (inode range, kind, name_len
bounds, null bytes, `/`, `\\`, `"."`, `".."`) but never compared entries pairwise
for duplicate names. `stage_create` rejects any attempt to create a second live
entry with the same name, so two live entries sharing identical `name_len` and
`name[..name_len]` bytes cannot arise through normal operation — they represent
corruption. Under such corruption, `lookup` would always shadow the second entry
(returning the first match), making it permanently unreachable by any name-based
path, but `check()` would call the image clean. Same oracle-vs-runtime
inconsistency pattern as the preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the per-entry validation loop, added an
  O(n²) pairwise scan over all live entries (at most 64 entries; 2016 pairs).
  For each pair (i, j) with i > j, if both entries are live and
  `name[..name_len]` is identical, `check()` returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_duplicate_directory_entry_names`. Uses `two_file_root()`,
  patches `b.txt`'s leading name byte from `b'b'` to `b'a'` so both entries
  carry the name `"a.txt"`, then asserts `check()` returns `Err(Corrupt)`, the
  image is unmodified, `a.txt` remains readable through its inode, and the
  unpatched image passes `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **39 passed / 0 failed** (was 38; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries whose name is "." or ".."

Branch: `auto/wovenhat-development` (base `8cac36e2`). Working tree was clean.

### Defect

`check()` validated null bytes, `/`, and `\\` in name bytes (previous sessions)
but did not reject the reserved directory-navigation names `"."` and `".."`.
`checked_native_name` explicitly rejects both: they can never be created through
the normal path. A corrupted entry whose name is `"."` (name\_len=1) or `".."`
(name\_len=2) would become permanently unreachable by `lookup` or `unlink` — any
attempt to look up either name as a file fails because `checked_native_name` would
refuse the lookup name first — but `check()` would call that image clean.
Same oracle-vs-runtime inconsistency pattern as the preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the backslash check, added a test for
  `entry.name[..name_len] == b"."[..] || entry.name[..name_len] == b".."[..]`.
  A live entry whose name is `"."` or `".."` now returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_whose_name_is_dot_or_dotdot`. Uses
  `two_file_root()`, overwrites `a.txt`'s `name_len` (entry offset 8) and name
  bytes (entry offset 16) to produce `"."` and `".."` in turn, then asserts
  `check()` returns `Err(Corrupt)`, the image is unmodified, and `b.txt` remains
  readable. Control: the unpatched image passes `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **38 passed / 0 failed** (was 37; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries whose name contains a backslash

Branch: `auto/wovenhat-development` (base `f8ed3b64`). Working tree was clean.

### Defect

`check()` validated null bytes and `/` in name bytes (previous sessions) but
did not inspect the name bytes for `'\\'`. `checked_native_name` explicitly
rejects names containing backslash, and every write goes through
`stage_create` → `checked_native_name`. A corrupted entry whose name bytes
include `'\\'` can therefore never be reached by `lookup` or `unlink` — it
becomes permanently orphaned — but `check()` would call that image clean.
Same oracle-vs-runtime inconsistency pattern as the preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the slash check, added
  `entry.name[..name_len].contains(&b'\\')` as an additional rejection. A live
  entry whose name bytes include any `'\\'` now returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_whose_name_contains_a_backslash`. Uses
  `two_file_root()`, patches `a.txt`'s name bytes (offset 16, 18, and 20
  within the entry — first, middle, and last positions within `name_len=5`) to
  `b'\\'` in turn, then asserts `check()` returns `Err(Corrupt)`, the image is
  unmodified, and `b.txt` remains readable. Control: the unpatched image passes
  `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **37 passed / 0 failed** (was 36; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries whose name contains a slash

Branch: `auto/wovenhat-development` (base `35068e87`). Working tree was clean.

### Defect

`check()` validated `name_len` and null bytes in name bytes (previous sessions),
but did not inspect the name bytes for `'/'`. `stage_create` explicitly rejects
names with `name.contains('/')`. Every VFS path also goes through
`checked_native_name`, which rejects `/`. A corrupted entry whose name bytes
include `'/'` can therefore never be reached by `lookup` or `unlink` — it
becomes permanently orphaned — but `check()` would call that image clean. Same
oracle-vs-runtime inconsistency pattern as the preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the null-byte check, added
  `entry.name[..name_len].contains(&b'/')` as an additional rejection. A live
  entry whose name bytes include any `/` now returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_whose_name_contains_a_slash`. Uses
  `two_file_root()`, patches `a.txt`'s name bytes (offset 16, 18, and 20
  within the entry — first, middle, and last positions within `name_len=5`) to
  `b'/'` in turn, then asserts `check()` returns `Err(Corrupt)`, the image is
  unmodified, and `b.txt` remains readable. Control: the unpatched image passes
  `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **36 passed / 0 failed** (was 35; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries with null bytes in name

Branch: `auto/wovenhat-development` (base `ac733d65`). Working tree was clean.

### Defect

`check()` validated `name_len` (range 1..=MAX_NAME, previous session) but did
not inspect the actual name bytes stored in `entry.name[0..name_len]`.
`stage_create` always writes filenames validated by `checked_native_name`, which
rejects names containing a null byte (0x00). A corrupted entry with a null byte
in its name bytes would never match any valid `lookup()` call (Rust `&str`
cannot carry null bytes), making it permanently unlookupable — but `check()`
would call that image clean. The same oracle-vs-runtime inconsistency pattern
as the preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: after the `name_len` range check, added
  `entry.name[..name_len].contains(&0u8)` as an additional rejection.
  A live entry whose name bytes include any null byte now returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_whose_name_contains_a_null_byte`.
  Uses `two_file_root()`, patches `a.txt`'s name bytes (offset 16, 18, and 20
  within the entry — first, middle, and last positions within `name_len=5`) to
  0x00 in turn, then asserts `check()` returns `Err(Corrupt)`, the image is
  unmodified, and `b.txt` remains readable. Control: the unpatched image passes
  `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **35 passed / 0 failed** (was 34; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries with out-of-range name_len

Branch: `auto/wovenhat-development` (base `c85a8df7`). Working tree was clean.

### Defect

`check()` verified that every live directory entry named a `KIND_FILE` inode and
carried `KIND_FILE` in its own `kind` field (added in the previous session), but
did not verify that the entry's `name_len` field was in the valid range
`1..=MAX_NAME`. `stage_create` always sets `name_len` to the actual name length,
which `checked_vfs_name` bounds to `1..=MAX_NAME`. A value of 0 (empty name in a
live entry) or above MAX_NAME (overflow) is corruption. `matches()` already
silently rejects such entries (comparing `usize::from(self.name_len) == name.len()`
which can never be true for `name_len > MAX_NAME` against any valid argument), so
`lookup` would never surface the entry — but `check()` would call such an image
clean. This is the same oracle-vs-runtime inconsistency pattern fixed in the
preceding sessions.

### Change

- `kernel/src/wfs.rs` `check()`: added `entry.name_len == 0 ||
  usize::from(entry.name_len) > MAX_NAME` as an additional rejection in the
  directory-entry loop. A live entry with a zero or oversized `name_len` now
  returns `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_with_out_of_range_name_len`. Uses
  `two_file_root()`, patches `a.txt`'s `name_len` (byte offset 8 in the entry)
  to each of 0, MAX_NAME+1, and 255, then asserts `check()` returns
  `Err(Corrupt)`, the image is unmodified, and `b.txt` remains readable. Control:
  the unpatched image passes `check()`.

### Results (run on this host, 2026-10-27)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **34 passed / 0 failed** (was 33; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter covers only the root directory; nested directories
  and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries with wrong kind field

Branch: `auto/wovenhat-development` (base `f5b07a1f`). Working tree was clean.

### Defect

`check()` verified that every live directory entry named a `KIND_FILE` inode
(added in the previous session), but did not verify that the directory entry's
own `kind` field also equalled `KIND_FILE`. `stage_create` always writes
`kind = KIND_FILE` in both the inode and the directory entry, so a mismatch is
corruption. A crafted or corrupted image could carry an entry whose inode is a
valid `KIND_FILE` inode but whose own `entry.kind` is `KIND_DIR`; `check()`
would call that image clean while the VFS adapter (`wfs_open`) would reject it
because it tests `entry.kind == KIND_FILE`. The inconsistency was between the
oracle (`check()`) and the run-time behaviour of `wfs_open`.

### Change

- `kernel/src/wfs.rs` `check()`: extended the directory-entry loop condition
  from `!node.is_used() || node.kind != KIND_FILE` to also include
  `|| entry.kind != KIND_FILE`. Updated the accompanying comment.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_with_wrong_kind`. Uses `two_file_root()`,
  patches `a.txt`'s directory-entry `kind` field (byte offset 10) to
  `KIND_DIR`, then asserts `check()` returns `Err(Corrupt)`, the image is
  unmodified, and `b.txt` remains readable. Control: the unpatched image passes
  `check()`.

### Results (run on this host, 2026-10-27)

Using the default `target` directory (project-local `.cargo-build` is blocked
by Windows Application Control on this host — pre-existing environment issue):

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **33 passed / 0 failed** (was 32; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.
- The WovenFS flat-root adapter (`read_dir`, `lookup`, `check`) covers only
  the root directory; nested directories and rename remain unimplemented.

Each entry is one bounded checkpoint. Entries record exactly what was run.
Nothing here claims production readiness; completion requires independent
human review.

## 2026-10-27 — WovenFS check(): refuse directory entries naming directory inodes

Branch: `auto/wovenhat-development` (base `90a5110b`). Working tree was clean.

### Defect

`check()` verified that every non-empty directory entry named an in-range,
`is_used()` inode, but did not verify that the inode's `kind` was `KIND_FILE`.
In a valid filesystem every directory entry names a file inode, because
`stage_create` — the only path that writes directory entries — always writes
`KIND_FILE` inodes. The only `KIND_DIR` inode is the root (inode 1). A crafted
or corrupted image could have an entry naming the root, and `check()` would
call it clean. The write paths are unaffected: `unlink` already refuses an
entry whose inode equals `dir_inode`, and `read_file`/`write_file` refuse a
directory-kind inode with `NotADirectory`. The inconsistency was between the
oracle (`check()`) and the invariant maintained by all write paths.

### Change

- `kernel/src/wfs.rs` `check()`: collapsed the two separate checks for the
  directory-entry loop (`inode <= inode_count`, then `is_used()`) into one
  `inode()` read, and added `node.kind != KIND_FILE` as an additional
  rejection. A directory entry that names a directory inode now returns
  `Err(Corrupt)`.
- `tests/wovenfs_crash.rs`: added
  `check_refuses_a_directory_entry_naming_a_directory_inode`. Uses
  `two_file_root()`, patches `a.txt`'s directory entry to name inode 1 (the
  root, `KIND_DIR`), then asserts that `check()` returns `Err(Corrupt)`, that
  the image is unmodified, and that `b.txt` remains readable. Control: on the
  unpatched image `check()` returns `Ok(())`.

### Results (run on this host, 2026-10-27)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`, `CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **32 passed / 0 failed** (was 31; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- `check()` still does not verify that directory entries' `kind` field matches
  the referenced inode's kind (i.e. a `KIND_FILE` entry naming a `KIND_DIR`
  inode would now be caught by the inode kind check, but an entry with
  `kind = KIND_DIR` that names a file inode is not yet flagged by `check()`).
  In practice entry kind has no operational effect, since all paths use the
  inode's kind.
- No repair (fsck) path — `check()` detects but cannot fix corruption.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-27 — WovenFS read_dir(): filter stale entries naming free inodes

Branch: `auto/wovenhat-development` (base `0c529636`). Working tree was clean.

### Defect

`read_dir()` returned a raw `DirectoryView` without sanitizing entries that
reference a free inode. `live()` on `DirectoryView` filtered only `inode == 0`
(empty slots), so a stale entry — one whose inode number is non-zero but the
inode has `mode == 0` (freed) — appeared in `live()` and `count()` results.
This was inconsistent with `lookup()`, which applies the same validity check
and returns `Err(Corrupt)` for such entries. The open item was recorded in the
2026-10-27 `lookup()` checkpoint:

> `lookup` only checks the direct-match entry; callers that iterate `read_dir`
> directly still receive stale entries (inode == 0 entries are filtered by
> `live()` already, but non-zero free inodes are not).

### Change

- `kernel/src/wfs.rs` `read_dir()`: after calling `dir_entries()`, iterates
  the returned `DirectoryView` and zeroes every entry whose `inode != 0` but
  whose resolved inode is out-of-range or has `mode == 0` (`!is_used()`). Uses
  `self.inode(entry.inode).map(|n| n.is_used()).unwrap_or(false)`, which
  mirrors the guard in `lookup()`. The zeroed entries have `inode == 0` and so
  are excluded by `live()` and `count()` automatically.
- `tests/wovenfs_crash.rs`: added
  `read_dir_live_filters_a_stale_entry_naming_a_free_inode`. Patches `a.txt`'s
  directory entry to name a free inode, then asserts that `read_dir().live()`
  does not include the stale entry, `count()` returns 1 (only `b.txt`), and
  `b.txt` is still visible. Control: on the unpatched image, both entries
  appear and `count()` returns 2.

### Results (run on this host, 2026-10-27)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`, `CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **31 passed / 0 failed** (was 30; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- `read_dir` itself does not fail on a corrupt image; it silently filters stale
  entries, consistent with `lookup()`'s fail-closed approach. Callers needing
  to detect corruption should use `check()`.
- No repair (fsck) path — `check()` detects but cannot fix stale entries.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-27 — WovenFS lookup(): refuse stale entry naming a free inode

Branch: `auto/wovenhat-development` (base `dbe30e1e`). Working tree was clean.

### Defect

`lookup()` returned any directory entry matching the requested name, including
stale entries whose inode was subsequently freed. `check()` flags such entries
as corrupt, and `create` refuses to reallocate an inode that a stale entry
names, but `lookup` had no equivalent guard. A caller receiving a stale entry
from `lookup` would then call `read_file` or `write_file` on the free inode
number, which returned `NotFound` — an incorrect error for what is actually a
corrupt on-disk state. The open item in the 2026-10-16 entry ("lookup still
returns an entry naming a free inode") is addressed here.

### Change

- `kernel/src/wfs.rs` `lookup()`: after locating a name-matching entry, reads
  the referenced inode and returns `Err(Error::Corrupt)` if the entry's inode
  number is out of range or the inode's `mode` is 0 (`!is_used()`). This
  mirrors the check `check()` performs at lines 1319–1324 and makes `lookup`
  fail-closed on corrupt images, consistent with all other mutation paths.
- `tests/wovenfs_crash.rs`: added
  `lookup_refuses_a_stale_entry_naming_a_free_inode`. Patches `a.txt`'s
  directory entry to name a free inode, then asserts `lookup` returns
  `Err(Corrupt)`, `b.txt` stays accessible, and `read_file(b)` still returns
  its contents. Control: the unpatched image resolves `a.txt` normally.
- `tests/wovenfs_crash.rs` `read_file_refuses_a_directory_inode`: updated the
  third sub-case, which previously expected `lookup` to succeed on a stale
  entry and then tested `read_file`'s response. Now that `lookup` catches it,
  the sub-case asserts `Err(Corrupt)` directly from `lookup`.

### Results (run on this host, 2026-10-27)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`, `CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **30 passed / 0 failed** (was 29; new test added) |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |
| `cargo test -p wovenhat-os --tests --no-fail-fast` | 138 tests in 22 executables pass; 3 targets (`wovenhat-os` binary, `async_runtime`, `pci_bridge_transaction`) blocked by Windows Application Control (`os error 4551`) — pre-existing environment issue, not a regression |

No QEMU runs this session (one chain at a time rule; prior passing results remain valid).

### Limits / open

- `lookup` only checks the direct-match entry; callers that iterate `read_dir`
  directly still receive stale entries (inode == 0 entries are filtered by
  `live()` already, but non-zero free inodes are not).
- No repair (fsck) path — `check()` detects but cannot fix stale entries.
- Stage 15.3–15.9 QEMU gates and physical USB qualification were not rerun.

## 2026-10-27 — WovenFS check(): fix alias detection beyond 64 blocks

Branch: `auto/wovenhat-development` (base `8139336a`). Uncommitted changes
in `kernel/src/wfs.rs` and `tests/wovenfs_crash.rs` were present from a prior
session; per the resume rule I inspected, validated, and committed them.

### Defect

`check()` tracked seen data blocks in a fixed `[u64; 64]` array that stopped
recording after 64 distinct blocks. Any cross-inode alias involving a block
first seen after the 64th entry was never detected: the verifier would call a
corrupt image clean. The write path's `refuse_shared_blocks` has no such bound,
so the inconsistency was between `check()` and every mutation path.

### Change

- `kernel/src/wfs.rs` `check()`: removed the `seen`/`seen_len` fixed array and
  its per-block inline test. The loop body now calls
  `self.refuse_shared_blocks(&txn, number, &node.direct)` instead, which scans
  every other live inode for any shared block. This reuses the exact same rule
  already applied on all write paths and has no bound on the number of blocks.
- `tests/wovenfs_crash.rs`: added
  `check_detects_an_alias_beyond_the_first_sixty_four_blocks`. The test crafts
  an image with 8 inodes each owning 8 blocks (64 total, filling the old set),
  then two further inodes that both name the same 65th block. Expects
  `check()` to return `Err(Corrupt)` and leave the image byte-identical.

### Results (run on this host, 2026-10-27)

Using project-local `CARGO_BUILD_BUILD_DIR=.cargo-build`, `CARGO_TARGET_DIR=target`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (no output) |
| `cargo test --test wovenfs_crash` | PASS, **29 passed / 0 failed** (was 28; new test added) |
| `cargo test --test wovenfs_crash check_detects_an_alias` | PASS, 1 passed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage15-1-test -- -D warnings` | PASS |

Negative control not re-run interactively, but the new test directly proves the
old behavior was broken: without the fix, the crafted image (two inodes sharing
a block past position 64) would pass `check()` with `Ok(())` instead of
returning `Err(Corrupt)`.

QEMU Stage 15.1/15.2 were not re-run this session (one chain at a time rule);
prior passing results remain valid and unchanged. No prior tests regressed.

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
