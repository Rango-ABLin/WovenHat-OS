# Stage 13.2 PCIe topology/ownership audit — 2026-09-27

## Status

Bounded topology/ownership increment implemented. Full Stage 13.2 acceptance is **not claimed**.

Stage 13.3/NVMe is outside this change and must not be advanced by this audit.

## Implemented

- Allocation-free topology bounded at 64 PCIe functions and 96 logical MMIO leases.
- Generation-tagged function handles prevent stale slot reuse.
- Single-owner function claims reject owner zero and competing owners.
- Generation-tagged BAR leases require the current function owner.
- Function teardown invalidates every logical BAR lease before the function generation advances or its slot is reusable.
- Type-1 bridge primary/secondary/subordinate bus windows are recorded.
- Parent selection chooses the narrowest matching bridge window in the same segment.
- Removing a bridge reparents surviving children from current topology.
- Discovery builds replacement topology off-lock and publishes no partial topology after insertion failure.
- PCI rank-10 inventory/configuration locks are not nested inside the rank-20 topology lock.
- Runtime acceptance now requires the topology lifecycle marker; the former inventory-only marker cannot certify Stage 13.2.
- Focused host tests cover nested bridges, ownership rejection, stale handle/lease invalidation, duplicate rejection and bounded capacity.
- Dedicated preservation gate runs host topology tests, build, warning-denying kernel Clippy, runtime-harness tests and QEMU on 1/2/4 CPUs.

## MMIO lifetime boundary

The current topology lease is an authorization token. Teardown invalidates that logical authority before reuse.

The paging layer currently exposes MMIO mapping but no reviewed physical unmap/revoke primitive. Therefore this increment does **not** claim that page-table mappings are revoked during topology teardown. A physical mapping lifetime primitive is required before that stronger guarantee can be made.

## Full Stage 13.2 blockers

Full acceptance remains blocked until all of these are implemented and tested:

1. BAR size probing plus bounded I/O, 32-bit MMIO and 64-bit/prefetchable MMIO allocation/rebalance with rollback.
2. PCI-to-PCI bridge I/O, memory and prefetchable window routing/programming with subordinate resource validation.
3. Generic MSI and MSI-X vector allocation, programming, masking/quiescence and teardown; the existing AX200-specific MSI path is not sufficient.
4. PCIe hotplug/rescan with generation-safe insertion/removal and teardown ordering.
5. WovenDriver binding/unbinding integrated with PCIe generation ownership so removal cannot leave a bound driver or live lease.
6. Warning-denying Rust validation and the dedicated 1/2/4 CPU QEMU preservation matrix for the final implementation.

## Acceptance rule

Do not change Stage 13.2 to accepted, do not restore the inventory-only marker, and do not begin Stage 13.3 as a consequence of this increment until the blockers above are closed with evidence.
