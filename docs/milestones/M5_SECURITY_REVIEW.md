# M5 Security Review

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `5ca5b9b3`

## Scope

DMA ownership, interrupt delivery and capability handling for M5 hardware
drivers.

## Findings

1. Shared DMA page (`kernel/src/dma.rs`, `kernel/src/dma/ownership.rs`):
   - Checked CPU/device ownership before DMA; bounds and device-range
     validation; quarantine-on-drop for device-owned pages. Host tests
     `tests/dma_ownership.rs` PASS.
2. MSI/MSI-X interrupt masking: added owned mask/unmask with ownership and
   capability checks; capability-level enable mask and per-vector mask
   support. LAPIC EOI owned by the local IDT handler; no worker-context EOI
   write. xHCI lifecycle stores/rolls back the interrupt lease.
3. IOMMU/VT-d: MISSING in `kernel/src`. No DMA isolation layer. Physical
   security review item; tracked as M5 finding.
4. Capability handling: malformed capability lists and overlap/firmware
   conflict detection not covered by QEMU runtime tests yet (audit findings).
5. Interrupt storm protection: not found/implemented yet; high risk while
   shared interrupt lines are used.

## Open risks

- No IOMMU/VT-d: DMA from a malicious or buggy device cannot be isolated.
- Interrupt storm/shared-line handling not hardened yet.

## Mitigation status

- Current mitigated paths: shared DMA ownership/bounds, owned interrupt mask
  and unmask, vector allocator generation safety, ordered teardown.
- Remaining: IOMMU, storm protection, capability-list runtime validation.

No security regressions were introduced by this M5 slice; the new interrupt
mask/unmask code follows the existing IRQ-lock and ownership checkpoint
contracts.
