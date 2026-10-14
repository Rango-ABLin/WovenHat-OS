# Stage 13.2 persistent BAR resource audit — 2026-10-13

The explicit firmware BAR rebalance path now owns its reservations in a
bounded persistent PCI resource state. A successful `rebalance_bars_from_firmware`
transaction retains the allocator reservations and records the function and
all six BAR assignment slots. Duplicate function admission and allocation
table exhaustion are rejected before hardware programming. The corresponding
`release_bars_from_firmware` path disables PCI command-space decoding, clears
and verifies every assigned BAR register, refreshes the published inventory,
then releases the exact reservations and removes the record only after every
step succeeds; a failed disable, clear, refresh, or release therefore leaves
the record authoritative for retry.

The existing transactional planner still reserves all BARs before config-space
programming and rolls back provisional reservations on probe, encoding,
verification, or restoration failure. The persistent state closes the prior
stack-lifetime defect where a successful rebalance returned assignments but
dropped the allocator that owned them.

Validation completed:

- freestanding kernel check with `stage13-2-test`;
- warning-denying freestanding kernel Clippy with `stage13-2-test`.
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, with exit 33 and
  evidence in `audit-artifacts/stage13.2-1cpu-1791886909736158900`,
  `audit-artifacts/stage13.2-2cpu-1791886920671967900`, and
  `audit-artifacts/stage13.2-4cpu-1791886932096798900`.

The BAR-clear, inventory-refresh, and reroute follow-up was revalidated by the
latest 1/2/4-CPU preservation gate:
`audit-artifacts/stage13.2-1cpu-1791890690873217500`,
`audit-artifacts/stage13.2-2cpu-1791890699703532100`, and
`audit-artifacts/stage13.2-4cpu-1791890709466970800`.

The full Stage 13.2 gate remains open: this increment is an explicit resource
admission primitive, not proof of automatic normal-bind BAR rebalance. Bridge
window routing, generic physical MSI/MSI-X activation, physical PCIe
hotplug, and valid physical BAR qualification remain outstanding.
