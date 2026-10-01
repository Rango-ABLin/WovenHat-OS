# Stage 1–5 long-name collision hardening — 2026-10-01

The FAT32 long-name mutation path now checks the user-visible, NFC-normalized
name before generating or comparing the implementation's 8.3 alias.

- Creating an existing long-name file reuses its existing directory entry and
  performs the normal data replacement with rollback on allocation or write
  failure.
- Creating a directory at an existing long-name file is rejected.
- Renaming onto an existing long name is rejected even when alias generation
  would choose a different short alias.
- The existing long-name record is preserved during overwrite; the old data
  chain is released only after the replacement directory entry is durable.

The FAT32 self-test now covers long-name overwrite and long-name rename
collision rejection. The complete Stage 1–5 QEMU gate passed on 1, 2 and 4
CPUs with exit 33.

| CPU profile | Result |
| --- | --- |
| 1 CPU | PASS |
| 2 CPUs | PASS |
| 4 CPUs | PASS |

This closes a software correctness gap in the bounded FAT32 mutation layer.
It does not claim physical storage qualification, interrupt/DMA-backed block
completion, or full multi-operation data rollback; those remain explicit
follow-up boundaries in the production gap audit.
