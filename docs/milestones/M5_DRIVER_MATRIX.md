# M5 Driver Qualification Matrix

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `5ca5b9b3`

## Status key

- `EMULATED_PASS`: QEMU-emulated backend gate passed on all required CPU counts.
- `SOFTWARE_ONLY_PASS`: host/QEMU software-fixture pass without emulated devices.
- `PHYSICAL_NOT_TESTED`: no physical run performed.
- `BLOCKED`: physical run blocked on hardware availability; may not be claimed.

## Implemented and emulated-passed subsystems

| Subsystem | Implementation | QEMU evidence | CPU counts | Notes |
|---|---|---|---|---|
| Shared DMA ownership | `kernel/src/dma.rs`, `kernel/src/dma/ownership.rs` | `scripts/test-m5-dma-qemu.py` | 1, 2, 4 | PASS, exit 33. `[M5 DMA] shared DMA ownership: PASSED`. |
| File mmap self-test | `kernel/src/userspace.rs` | Stage 13.12 drivers gate | 1 (and previously 4) | PASS after ordering fix. |
| NVMe | `kernel/src/nvme.rs` shared DMA page | Stage 13.14 storage, nvme | 1 | PASS, exit 33. |
| AHCI | `kernel/src/ahci.rs` shared DMA page + transfer ownership | Stage 13.14 storage, ahci | 1, 2, 4 | PASS. |
| xHCI + USB HID input | `kernel/src/xhci.rs` interrupt lifecycle | Stage 13.13 usb input | 1 | PASS after mask/lease hardening. |
| PCI/MSI-X interrupt enable/lease | `kernel/src/driver.rs`, `hal/pci/msi.rs`, `hal/pci/msix.rs` | Stage 13.12/13/13 QEMU gates | 1 | PASS via bring-up and input gates. |

## Known remaining (not yet qualified)

| Subsystem | Gap | Next action |
|---|---|---|
| MSI runtime delivery | No dedicated QEMU MSI delivery gate beyond vector allocator self-tests; delivery ack owned by LAPIC EOI path. | Add runtime delivery harness. |
| MSI-X runtime table write/delivery | Layout tests only host-side. | QEMU runtime table write test. |
| e1000 / e1000e | Not implemented. | Blocked on design + hardware. |
| Wi-Fi | No physical chipset, no firmware blobs in repo; software validation only. | Physical BLOCKED; software fixtures pass. |
| Bluetooth | HCI framing self-tests; no end-to-end controller run. | Physical BLOCKED. |
| IOMMU/VT-d | Not implemented. | Critical DMA security gap; physical BLOCKED. |
| Network | Stage 10.7 async TCP accepted; per-protocol evidence partial. | M5 may not require network as a supported device. |
| xHCI hub support | Not implemented. | Open. |

## Physical hardware

No physical device was available in this session; physical rows above are
`PHYSICAL_NOT_TESTED` / `BLOCKED` until real hardware evidence is collected.
The plan for those runs is `docs/milestones/M5_PHYSICAL_TEST_PLAN.md`.
