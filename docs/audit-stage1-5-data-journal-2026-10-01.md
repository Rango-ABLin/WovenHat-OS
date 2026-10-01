# Stage 1–5 durable data journal — 2026-10-01

FAT32 persistence now writes a bounded on-volume `WOVENJR.BIN` transaction
record before replacing a file. Version 2 records contain the target path,
whether the previous file existed, the previous file bytes up to the VFS
capacity, the intended new length, a prepared/committed state, and checksums
for both the rollback payload and intended replacement. Version 1 records are
still accepted during recovery.

The ordering is:

1. write and flush the prepared record;
2. write the replacement data and metadata;
3. write and flush the committed record;
4. remove and flush the journal record.

Mount recovery rolls back a prepared record to the previous bytes (or removes
the newly-created target). A committed version 2 record is discarded only
after the target's length and checksum match the intended replacement;
otherwise recovery restores the previous bytes. Invalid or tampered records
fail closed rather than being replayed. The journal file is excluded from VFS
import.

The Stage 1–5 self-test covers prepared/committed encoding and payload
tampering. The complete QEMU gate passed on 1, 2 and 4 CPUs with exit 33 after
the journal was integrated into live FAT32 persistence.

This closes bounded single-file data rollback and checksum replay. Multi-file
atomic transactions, files larger than the bounded VFS payload, and physical
power-loss qualification remain open.

## Version 2 replay hardening

Version 2 records also store the intended replacement length and checksum.
During mount recovery, a committed record is retired only when the target
exists with the expected length and checksum. A mismatch re-enters the
rollback path. Legacy version 1 records remain readable and retain their prior
committed-record behavior.
