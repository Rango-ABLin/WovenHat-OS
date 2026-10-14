# Stage 1–5 bounded metadata transactions — 2026-10-14

The WDJ2 batch journal now carries version 2 records while retaining parsing
for the existing version 1 record. Each v2 entry stores the previous and new
inode/sidecar ownership metadata alongside the existing bounded file snapshot.

Recovery no longer treats matching file bytes as sufficient for a committed
batch. It verifies the new inode metadata too. A prepared or incomplete
committed batch restores both the previous file bytes and previous metadata,
then removes only the transaction's pending sidecar and intent. This avoids
deleting a pre-existing ownership record during rollback.

WDJ1 remains readable, and the WDJ2 magic and existing v1 record layout remain
unchanged for old media; new records use the versioned v2 entry extension.
The transaction is still bounded at sixteen files and `NODE_CAPACITY` bytes
per file. It is not a full-volume or arbitrary-size journal.

Validation:

- Stage 1–5 freestanding Clippy with `-D warnings` passed;
- host integration tests passed;
- storage/SMP QEMU acceptance passed on 1, 2, and 4 CPUs with exit 33;
- the live FAT32 recovery fixture exercised prepared rollback, incomplete
  committed rollback, and complete committed metadata verification.

Remaining Stage 1–5 requirements are non-QEMU hardware/DMA qualification,
arbitrary-size/full-volume rollback, and unclean-shutdown/power-loss testing.
