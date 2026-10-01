# Stage 1–5 metadata capacity audit — 2026-10-01

## Scope

This increment removes the original four-sector ownership-metadata ceiling
without changing the legacy WMD1 layout. It does not claim an unbounded
metadata store, native WovenFS, or full multi-operation crash atomicity.

## Implementation

`kernel/src/fat32.rs` retains the WMD1 region at the end of the FAT32 reserved
area and adds a WMD3 extension region in eight earlier reserved sectors when
the volume has at least 18 reserved sectors. The reader, writer, finalizer, and
remover search both regions. Old volumes with fewer reserved sectors continue
to use only WMD1, so the extension cannot overlap the existing journal or inode
metadata regions.

Each sector stores 21 records. The compatible capacity therefore grows from
84 records to 252 records. The test disk models the extension sectors, and the
Stage 1–5 self-test persists and reads 100 additional metadata records, which
crosses the old four-sector limit.

## Evidence

- `rustfmt --check kernel/src/fat32.rs`: passed.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage1-5-test`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage1-5-test -- -D warnings`: passed.
- `python -m unittest discover -s tests -p 'test_*.py'`: 11 tests passed.
- Stage 1–5 QEMU acceptance: 1, 2, and 4 CPUs passed with exit 33.

## Boundaries

The table is still bounded by reserved-sector geometry. Multi-operation
transaction replay, hardware/DMA qualification, and physical unclean-shutdown
testing remain open production work.
