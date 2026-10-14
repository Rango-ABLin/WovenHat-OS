# Stage 13.2 automatic BAR admission audit — 2026-10-13

Normal `bind_pci_with_bars` now invokes the persistent BAR admission path after
claiming the function. If all required BARs are unassigned, the path reserves
and programs the complete BAR set transactionally, refreshes the published
device view, and only then leases the resulting MMIO authority. A mixed
assigned/unassigned layout is rejected fail-closed because moving only part of
a function requires device-specific quiescence.

The normal-bind path reroutes bridge forwarding after a successful rebalance.
On teardown, command decoding is disabled, every assigned BAR register is
cleared and verified, the published inventory is refreshed, bridge forwarding
is recomputed, and only then are allocator reservations and function authority
released. BAR-clear or routing failures retain retryable ownership state.

The binding records whether it owns a persistent rebalance. Ordered unbind
first quiesces interrupts and revokes owner mappings, releases ordinary BAR
leases, disables command-space decoding, releases the persistent BAR
reservations, and only then releases the PCI function claim. Any failed phase
leaves the binding in `Unbinding` with the remaining authority checkpointed for
retry.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791890690873217500`,
  `audit-artifacts/stage13.2-2cpu-1791890699703532100`, and
  `audit-artifacts/stage13.2-4cpu-1791890709466970800`.

The QEMU device set does not provide a valid unassigned physical BAR plus
firmware aperture for positive hardware allocation qualification. Mixed-layout
and physical BAR interoperability remain explicit hardware-dependent limits.
