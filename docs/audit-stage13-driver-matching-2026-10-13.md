# Stage 13 driver matching audit — 2026-10-13

The bounded WovenDriver table now supports explicit PCI identity matchers
(vendor, device, class, subclass, and programming interface). An empty
matcher is rejected, matching is deterministic by registration order, and
`bind_matching_pci` separates discovery/matching from the existing
owner-token binding transaction. Unknown functions remain unclaimed.

The existing generic `register` and explicit `bind_pci` APIs remain unchanged.
Insertion reports can now be passed to the matcher without inventing a driver
for unsupported hardware.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791887987650651800`,
  `audit-artifacts/stage13.2-2cpu-1791888007030937500`, and
  `audit-artifacts/stage13.2-4cpu-1791888027621533400`.

The Stage 13.2 runtime lifecycle probe also registers a vendor/device matcher,
verifies it selects the synthetic driver, and binds through
`bind_matching_pci` before exercising MMIO and teardown.

This adds matching policy, not device-specific drivers or physical PCIe
interoperability qualification.
