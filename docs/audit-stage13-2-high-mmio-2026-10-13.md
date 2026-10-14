# Stage 13.2 bounded high-BAR MMIO audit — 2026-10-13

## Scope

The previous MMIO mapper formed a virtual address by adding the physical BAR
address to `0xffff_fe00_0000_0000`. That arithmetic rejects high 64-bit BARs
and made owner-scoped MSI-X table mappings dependent on the BAR being in the
low alias range.

The mapper now retains the actual virtual address in every bounded mapping
entry. Low physical pages preserve the historical fixed alias; pages whose
physical address cannot fit that alias use a separate 256-slot high-MMIO
window. Release and owner revocation unmap the retained virtual address, so
teardown does not reconstruct or guess it.

## Evidence

- Warning-denying freestanding kernel Clippy passed for `stage13-2-test`.
- The Stage 13.2 probe activated and tore down a real QEMU MSI-X lease.
- The probe mapped physical `0x2000_0000_0000` to a high-window virtual page
  and released it successfully (`high-mmio ... released=true`).
- The complete 1/2/4-CPU preservation gate passed, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The high-MMIO window is deliberately bounded at 256 pages and is not a
general virtual-memory allocator. BARs larger than the supported mapping
capacity, bridge-window routing, physical hotplug, reset/recovery, and
hardware-specific device protocols remain separate acceptance work.
