# Stage 1–5 DMA allocation cleanup audit — 2026-10-14

## Scope

This increment hardens the existing AHCI and NVMe block-driver DMA ownership
paths without changing the `BlockDevice` ABI or claiming physical-device
qualification. A DMA page now returns its allocator frame if the physical
memory mapping cannot be established. AHCI also releases all pages when
controller initialization fails before the controller becomes globally owned.
The existing NVMe `DmaPage` drop path remains the owner of successful and
failed initialization allocations.

The controller objects are intentionally process-lifetime resources after
successful initialization: the controller lock owns the DMA pages while the
device can issue commands. No page is released while hardware may still
reference it.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-3-test -- -D warnings` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-4-test -- -D warnings` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` — passed, including the
  WovenFS crash, page-cache, PCI, async-runtime, display, and Wi-Fi suites.
- NVMe QEMU controller gate, 1/2/4 CPUs — passed (exit 33):
  `audit-artifacts/stage13.3-1cpu-1791970727679324400`,
  `audit-artifacts/stage13.3-2cpu-1791970800950732500`,
  `audit-artifacts/stage13.3-4cpu-1791970842780539000`.
- AHCI QEMU controller gate, 1/2/4 CPUs — passed (exit 33):
  `audit-artifacts/stage13.4-1cpu-1791970885814072600`,
  `audit-artifacts/stage13.4-2cpu-1791970942613349500`,
  `audit-artifacts/stage13.4-4cpu-1791971012860405400`.

## Limits

This is allocator/error-path evidence only. AHCI/NVMe physical controller
qualification, DMA/IOMMU isolation, interrupt-driven completion, torn-sector
behavior, power-loss recovery, arbitrary-size/full-volume transactions, and
the inserted USB flash device remain unqualified. The flash device was not
written or erased by this audit.
