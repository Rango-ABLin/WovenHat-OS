# Stage 1–5 FAT32 write-fault audit — 2026-10-14

## Result

The FAT32 mutation self-test now runs a deterministic write-budget fault at
each sector-write boundary of a bounded file creation. For every injected
`DeviceFault`, it verifies that the volume remains structurally checkable, the
new file is absent, and the pre-existing file still returns its original
bytes. It also verifies successful completion after the fault range.

The test is part of `fat32::self_test()`, so the existing Stage 1–5 storage
acceptance gate exercises it in the freestanding kernel as well as the host
regression suite. The production `BlockDevice` ABI is unchanged; the fault
device exists only in the self-test implementation.

## Boundary and limits

The injected device models a sector write that fails and returns to the caller.
It does not model power removal during a device programming cycle, volatile
controller caches, torn sectors, DMA completion ordering, or a reboot into a
new kernel. FAT32 FSInfo is an advisory hint and may be reported as stale by
the checker after an injected failure; FAT and directory structure are the
authoritative recovery assertions in this bounded test.

Physical flash/ATA qualification, torn-write recovery, arbitrary-size or
full-volume rollback, and unclean-shutdown reboot evidence remain open and are
not claimed by this audit.
