# Stage 1–5 WDJ2 capacity extension — 2026-10-14

The versioned WDJ2 batch journal now accepts up to sixteen file entries. The
previous four-entry records remain valid because the on-disk magic, version,
entry encoding, checksum, and recovery state are unchanged. The encoder and
parser remain bounded by the fixed sixteen-entry maximum and the existing VFS
file-size limit; they do not claim arbitrary-size or full-volume transactions.

The kernel self-test now encodes and parses a five-entry prepared record,
checks old-data preservation at both ends of the record, and retains checksum
tamper rejection. Warning-denying freestanding Clippy for
`stage1-5-test` passed.

Fresh QEMU acceptance passed with exit 33 and the required Stage 1–5 marker:

- `audit-artifacts/stage1-5-1cpu-1791962142731816000`
- `audit-artifacts/stage1-5-2cpu-1791962209812403600`
- `audit-artifacts/stage1-5-4cpu-1791962253016653000`

The broader disposable ATA/FAT32 storage boot regression also passed on all
CPU counts, after the shared PCI `qemu-test` feature guard was corrected:

- `target/storage-regression-1-debug`
- `target/storage-regression-2-debug`
- `target/storage-regression-4-debug`

Each exited 33 with the complete storage/boot-suite markers. The disposable
images are workspace-local and are not the inserted USB disk.

This increment does not close metadata transactions beyond the bounded
sidecars, arbitrary-size/full-volume rollback, DMA and non-QEMU storage
qualification, or unclean-shutdown/power-loss evidence.
