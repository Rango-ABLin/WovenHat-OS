# Stage 13.2 hotplug insertion audit — 2026-10-13

PCI rescan now has a bounded insertion path in addition to removal detection.
`rescan_added` scans configuration space without publishing, compares
generation-owned topology addresses and full driver-relevant PCI identity, and returns
newly present or same-slot replacement functions with their probed device
descriptors. Reconciliation still occurs transactionally;
the driver-facing `hotplug_rescan_report` returns additions only after the new
topology is published and bridge routing succeeds. The existing summary-only
`hotplug_rescan` API is preserved as a compatibility wrapper.

If a replacement is detected at an existing BDF before the old owner has been
quiesced, the coordinator aborts the whole batch before issuing teardown
configuration writes. This fail-closed boundary avoids directing old-device
BAR/command cleanup at the replacement; platform hotplug notification must
establish quiescence before that case can be completed.

Newly detected functions are deliberately reported but not auto-bound. Driver
matching and initialization must supply an explicit compatible driver before
claiming a function; unknown hardware remains unowned. Removal now also
releases any persistent BAR rebalance reservation for an unbound function
before invalidating its topology generation, while preserving retryable error
state on disable/release failure.

Validation completed:

- host and freestanding warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791890128143342200`,
  `audit-artifacts/stage13.2-2cpu-1791890136688207600`, and
  `audit-artifacts/stage13.2-4cpu-1791890146247574800`.

QEMU does not provide a physical insertion/removal fixture, so live PCIe
hotplug signaling, link state, power control, and hardware removal races
remain unqualified.
