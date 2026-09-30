# Stage 13.4 AHCI/SATA candidate audit

## Scope

Stage 13.4 adds a native AHCI/SATA block backend on top of the accepted Stage 13.2 PCIe and Stage 13.3 NVMe foundations. It discovers PCI class 01h/subclass 06h/programming-interface 01h controllers, enables PCI memory decoding and bus mastering, selects an implemented active SATA disk port, programs command-list and received-FIS DMA bases, and exposes 512-byte SATA media through the existing `BlockDevice` contract.

## Architecture

The driver uses allocator-owned 4 KiB DMA pages for the command list, received FIS area, one command table/PRDT and a bounded transfer buffer. It stops the AHCI command engine before rebasing, clears port errors/status, restarts the engine, issues IDENTIFY DEVICE, requires 48-bit LBA support, and uses READ DMA EXT / WRITE DMA EXT / FLUSH CACHE EXT for block I/O. Access is serialized by the existing IRQ-safe controller mutex.

Legacy ATA PIO and NVMe are preserved as independent backends. Stage 13.4 does not replace either path.

## Acceptance

`run-stage13-4-acceptance.ps1` requires build, warning-denying freestanding Clippy, and isolated QEMU boots on 1/2/4 CPUs. The runtime harness attaches a dedicated 16 MiB SATA disk to an ICH9 AHCI controller. Each boot must discover AHCI, initialize a SATA disk, perform write/flush/read round-trip through `BlockDevice`, restore sector zero, and exit through the standard debug-exit success path.

## Production gaps

The stage deliberately uses one command slot and bounded polling for synchronous completion. Interrupt-driven completion, NCQ/multi-slot scaling, ATAPI, hotplug, staggered spin-up, enclosure management, controller reset recovery, multiple disks/controllers, advanced power management, TRIM/DSM, and broad real-hardware qualification remain later production work.


## Final acceptance record

Stage 13.4 was accepted on 2026-09-30 from commit `a4c00388187f63cc801716979b21e4841dd0945f` by GitHub Actions run `36704495845`.

The complete release-validation gate passed, including the accepted Stage 13.3 NVMe regression and the dedicated Stage 13.4 AHCI/SATA runtime gate. Dedicated AHCI/SATA results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.4-1cpu-*`, `stage13.4-2cpu-*`, and `stage13.4-4cpu-*`.

## Milestone status

**Stage 13.4 native AHCI/SATA: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers native PCI AHCI discovery, BAR/MMIO access, DMA command structures, SATA IDENTIFY with 48-bit LBA validation, READ DMA EXT / WRITE DMA EXT / FLUSH CACHE EXT through `BlockDevice`, round-trip sector verification with restoration, and successful 1/2/4-CPU execution.

The production extensions listed above and physical-hardware qualification remain later work.
