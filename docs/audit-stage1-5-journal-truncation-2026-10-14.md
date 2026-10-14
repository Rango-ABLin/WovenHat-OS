# Stage 1–5 journal truncation audit — 2026-10-14

The storage self-test now enumerates every shortened byte prefix of a valid
prepared WDJ2 record and requires the parser to reject each one. It also keeps
the existing checksum-tamper rejection and maximum-capacity round-trip tests.
This prevents a reboot recovery path from treating an incomplete journal
record as an authoritative transaction.

This is software validation of record framing and checksum boundaries. It does
not establish that a physical flash/ATA device writes a sector atomically, nor
does it replace power-loss, DMA completion, or real-device reboot
qualification.
