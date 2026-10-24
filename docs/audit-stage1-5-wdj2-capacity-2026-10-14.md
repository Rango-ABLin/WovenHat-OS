# Stage 1–5 WDJ2 capacity audit — 2026-10-14

The versioned WDJ2 batch journal now accepts up to 255 file entries. The
previous four-entry and sixteen-entry records remain valid because the on-disk
magic, version, entry encoding, checksum, and recovery state are unchanged.
The encoder and parser remain bounded by the full one-byte entry-count range,
the recovery allocation limit, the per-file node-size limit, and the available
FAT32 volume; they do not claim arbitrary-size or full-volume transactions.

The kernel self-test now constructs and round-trips a maximum-capacity
prepared record, including the metadata-snapshot fields, before any live-device
work. Warning-denying freestanding Clippy for `stage1-5-test`, the full host
test suite, and the release build pass.
The same self-test verifies that a 256-entry request is rejected before record
encoding, preserving the fail-closed boundary beyond the 255-entry limit.

Fresh QEMU acceptance passed with exit 33 and the required Stage 1–5 marker:

- `audit-artifacts/stage1-5-1cpu-1791962142731816000`
- `audit-artifacts/stage1-5-2cpu-1791962209812403600`
- `audit-artifacts/stage1-5-4cpu-1791962253016653000`
- `target/storage-regression-1-debug`
- `target/storage-regression-2-debug`
- `target/storage-regression-4-debug`

Each disposable storage image passed the complete storage/boot suite on 1/2/4
CPUs. The disposable images are workspace-local and are not the inserted USB
disk.

The broader disposable ATA/FAT32 storage boot regression also passed on all
CPU counts, after the shared PCI `qemu-test` feature guard was corrected.

The earlier bounded-capacity audit recorded the corresponding Stage 1–5
artifacts under `audit-artifacts/stage1-5-*`; those historical artifacts are
retained and the newer `target/storage-regression-*` logs are additional
evidence for this 255-entry increment.

This increment removes the former sixteen-entry implementation ceiling. It
does not close metadata transactions beyond the bounded sidecars,
arbitrary-size/full-volume rollback, physical DMA/torn-write recovery, or
unclean-shutdown/power-loss evidence.
