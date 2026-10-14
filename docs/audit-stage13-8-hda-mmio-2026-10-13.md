# Stage 13.8 HDA MMIO audit — 2026-10-13

The first Stage 13.8 run failed at HDA controller reset with
`Controller(ResetAssertTimeout)`. The cause was confirmed in the HDA MMIO
helpers: they mapped the containing page but discarded the register's page
offset, so all non-zero-offset registers accessed the BAR base.

The 8-, 16-, and 32-bit helpers now split every physical register address into
page and offset, map the page, and perform the volatile access at `mapped +
offset`. After the repair, the feature build, freestanding warning-denying
Clippy, and the complete QEMU matrix passed:

- 1 CPU: `audit-artifacts/stage13.8-1cpu-1791895760922139400`
- 2 CPUs: `audit-artifacts/stage13.8-2cpu-1791895771085454400`
- 4 CPUs: `audit-artifacts/stage13.8-4cpu-1791895781435410400`
- marker: `=== STAGE 13.8 ACCEPTANCE: PASS ===`

The gate covers the QEMU Intel HDA codec, topology, playback/capture stream
and mixer/API boundary. Physical codec interoperability, interrupt/DMA fault
recovery, and hardware qualification remain outside this deterministic gate.
