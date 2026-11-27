# M5 PCI/MSI-X Baseline Audit

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `0dd3bccd`.
Scope: deep audit of `kernel/src/hal/pci.rs` and `kernel/src/hal/pci/*.rs`, and
the MSI/MSI-X interrupt paths in `kernel/src/driver.rs`, `kernel/src/drivers.rs`
and `kernel/src/interrupts.rs`. Status is from source reading plus the existing
host and QEMU tests I found.

## Inventory

| Subsystem | Files | Status | Notes |
|---|---|---|---|
| PCI config access | `hal/pci.rs`, `hal/acpi.rs` | IMPLEMENTED_UNVERIFIED | Legacy config read/write and MCFG/ECAM. No host test for MCFG table parsing was found. |
| Bus, device and function enumeration | `hal/pci/topology.rs` | IMPLEMENTED_UNVERIFIED | Host test `tests/pci_topology.rs` exists. |
| BAR decode and validation | `hal/pci/bar.rs` | IMPLEMENTED_UNVERIFIED | Host test `tests/pci_bar.rs`. No live-device test in QEMU found. |
| Bridge traversal | `hal/pci/bridge.rs`, `bridge_transaction.rs` | IMPLEMENTED_UNVERIFIED | Host tests `tests/pci_bridge.rs`, `tests/pci_bridge_transaction.rs`. |
| Transactional resource allocation | `hal/pci/resource.rs`, `assignment.rs` | IMPLEMENTED_UNVERIFIED | Host tests `tests/pci_resource.rs`, `tests/pci_assignment.rs`. |
| Capability list traversal | `hal/pci.rs`, `bar.rs` | IMPLEMENTED_UNVERIFIED | MSI, MSI-X, and text styles. No malformed-list host test was found. |
| MSI | `hal/pci/msi.rs` | IMPLEMENTED_UNVERIFIED | Host tests exist. Runtime delivery not verified in this audit. |
| MSI-X | `hal/pci/msix.rs` | IMPLEMENTED_UNVERIFIED | Table and PBA layout validation separate from MMIO. No runtime table write test found. |
| Interrupt vector routing | `hal/pci/routing.rs`, `vector.rs` | IMPLEMENTED_UNVERIFIED | Host tests `tests/pci_routing.rs`, `tests/pci_vector.rs`. |
| Interrupt masking and ack | `hal/pci/msi.rs`, `msix.rs`, `interrupts.rs` | IMPLEMENTED_PARTIAL | LAPIC EOI ack owned by the IDT stub; per-vector MSI mask/unmask added this pass; MSI-X enable already programs masked then unmasked entries. |
| Shared interrupts | `driver.rs`, `drivers.rs`, `interrupts.rs` | PARTIAL | Shared handling exists. Storm protection was not found. |
| Driver bind/unbind | `driver.rs`, `drivers.rs` | PARTIAL | `Unbinding` state exists. Teardown order not audited deeply. |
| IOMMU/VT-d | none | MISSING | No DMAR, IOMMU or VT-d code in `kernel/src`. |
| Device enable/BAR enable, bus master | `hal/pci.rs` | IMPLEMENTED_UNVERIFIED | Used by drivers. |
| Device reset and PCI restore | `hal/pci.rs` | IMPLEMENTED_UNVERIFIED | Restore command used by AHCI and NVMe. |
| Overlap and firmware-conflict detection | `hal/pci/resource.rs`, `assignment.rs` | PARTIAL | Reserves allocations. No `IsECAM`-specific range reconciliation found. |

## Deficiencies found

1. `hal/pci.rs` config reads and writes go through a shared lock and have no
   explicit lock ordering with `driver.rs` MSI state. Audit needed.
2. No IOMMU or VT-d. This is a M5 finding, see
   `M5_PCI_SECURITY_REVIEW.md`.
3. MSI-X table writes are intentionally split from config access for
   BAR safety, but there is no runtime delivery test in QEMU in this repo.
4. Interrupt storm protection and shared-line locking have not been found.
5. No live QEMU bridge-topology or malformed-capability-list test was found.

## Post-audit hardening addendum (2026-11-20)

Findings from this pass, closed in the same branch:

1. End-of-interrupt acknowledgement: LAPIC EOI is written from the local IDT
   handler via `crate::smp::eoi()` (`interrupts::dispatch_pci_device_vector`,
   LAPIC timer/IPI handlers). PCI config port and MMIO EOI writes are not
   part of the delivery path; `hal/pci/msi::ack_msi` now documents this and no
   longer performs a spurious EOI write from a worker context.
2. MSI masking: added `hal/pci/msi::mask_owned_msi` and
   `hal/pci/msi::unmask_owned_msi` with ownership checks, capability decode,
   capability-level enable mask (no per-vector masking) and per-vector mask
   support. The helper takes the IRQ lock through the caller-owned IRQ mutex.
3. MSI-X masking: `hal/pci/msix::enable_owned` already programs the table
   entry masked, unmask then clears Function Mask, and
   `disable_and_mask_function` masks on quiesce; no change required.
4. xHCI interrupt lifecycle: `xhci::start_input` now stores the acquired
   `PciInterruptLease`, unmask before enabling the interrupter and rolls
   back vector/waiter/lease state on every failure after activation;
   `xhci::shutdown` masks delivery, clears the waiter, and clears the local
   lease copy before halting the controller.
5. Lint defect from the NVMe/AHCI DMA migration: unused
   `NVME_ADMIN_PAGE_SIZE`/`NVME_NAMESPACE_PAGE_SIZE` imports in
   `kernel/src/nvme.rs` were removed so `cargo clippy --all-targets -- -D
   warnings` passes again.

## Existing tests

- `tests/pci_topology.rs`
- `tests/pci_bar.rs`
- `tests/pci_bridge.rs`
- `tests/pci_bridge_transaction.rs`
- `tests/pci_msix_layout.rs`
- `tests/pci_resource.rs`
- `tests/pci_assignment.rs`
- `tests/pci_routing.rs`
- `tests/pci_vector.rs`

## Missing tests

- Bridge traversal on a nested QEMU bridge topology.
- Malformed capability list rejection.
- Overlap and firmware-conflict detection in allocation.
- Runtime MSI delivery and masking under QEMU.
- Runtime MSI-X table write and delivery under QEMU.
- Interrupt storm protection.

## Environment

- This audit uses source reading plus the tests listed above.
- No physical hardware was used.
