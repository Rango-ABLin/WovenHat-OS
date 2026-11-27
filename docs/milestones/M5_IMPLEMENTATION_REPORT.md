# M5 Implementation Report

Date: 2026-11-20
Branch: `m5/hardware-qualification`
HEAD: `5ca5b9b3` (feat(pci-msi): add owned interrupt mask/unmask and xHCI lifecycle hardening)
Baseline audited from: `6b54dce0` / `10740a4f`

## Implemented in this M5 slice

1. Shared DMA page with checked CPU/device ownership, bounds and device-range
   validation, and quarantine-on-drop (`kernel/src/dma.rs`,
   `kernel/src/dma/ownership.rs`, gate `m5-dma-test`). Re-exported by
   `kernel/src/wifi_hw.rs`.
2. File-mmap regression fix on `master` (`kernel/src/userspace.rs`,
   commit `6d6a0b2b`).
3. NVMe migrated to the shared DMA page (`kernel/src/nvme.rs`, commit
   `6b8d793f`).
4. AHCI migrated to the shared DMA page plus transfer-page ownership
   transitions (`kernel/src/ahci.rs`, commits `da5bed49`, `c849b409`).
5. PCI/MSI-X interrupt hardening pass
   (`kernel/src/hal/pci/msi.rs`, `kernel/src/hal/pci/vector.rs`,
   `kernel/src/hal/pci.rs`, `kernel/src/driver.rs`, `kernel/src/xhci.rs`,
   commit `5ca5b9b3`):
   - LAPIC EOI acknowledgement stays owned by the local IDT handler via
     `crate::smp::eoi()`; the new `hal/pci::ack_msi` hook documents this and no
     longer performs a PCI config/MMIO EOI from worker context.
   - Added owned MSI mask/unmask with function-ownership, capability and
     per-vector checks.
   - xHCI interrupt lifecycle now stores the acquired lease, unmasks before
     enabling the controller interrupter, and rolls back vector/waiter/lease
     state on failure; shutdown masks delivery, clears waiter and lease.
   - Removed unused NVMe DMA page-size imports so warning-denying Clippy passes.

## Validation performed (this branch)

| Gate | Command | Result |
|---|---|---|
| Host DMA ownership tests | `cargo test --test dma_ownership` | PASS, 8/8 |
| M5 DMA QEMU gate, 1/2/4 CPUs | `python scripts/test-m5-dma-qemu.py --cpus {1,2,4} --memory 4G` | PASS, exit 33 |
| Stage 13.12 driver bring-up, devices, 1 CPU | `python scripts/test-stage13-12-drivers.py --mode devices --cpus 1` | PASS, exit 33 |
| Stage 13.13 USB input, 1 CPU | `python scripts/test-stage13-13-usb-input.py --cpus 1` | PASS, exit 33 |
| Stage 13.14 storage, NVMe, 1 CPU | `python scripts/test-stage13-14-storage.py --mode nvme --cpus 1` | PASS, exit 33 |
| Freestanding kernel checks | `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage13-12-test` (and `stage13-13-test`) | PASS |
| Warning-denying Clippy (freestanding, gate) | `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-12-test -- -D warnings` | PASS |
| Warning-denying Clippy (all targets) | `cargo clippy --all-targets -- -D warnings` | PASS |
| Host test suite | `cargo test` | PASS |

## Not implemented in this slice

- Full PCI/MSI-X runtime delivery, interrupt storm protection, malformed
  capability-list handling, nested bridge topology QEMU coverage, e1000, IOMMU,
  xHCI hub support, unified block API completion, Wi-Fi/Bluetooth physical
  support, security/performance/release deliverables.
- Physical hardware qualification (no real hardware in this session).

## Evidence artifacts

- `audit-artifacts/m5-dma-{1,2,4}cpu-*`, `audit-artifacts/stage13.14-nvme-1cpu-*`,
  `audit-artifacts/stage13.12-devices-1cpu-*`, `audit-artifacts/stage13.13-1cpu-*`.
- Full list: `docs/milestones/M5_TEST_RESULTS.md`.

## Limits

- The 1/2/4-CPU requirement applies to supported-device QEMU gates that have a
  QEMU backend; not every M5 device has one yet.
- Physical tests remain `BLOCKED` until real hardware evidence exists.
