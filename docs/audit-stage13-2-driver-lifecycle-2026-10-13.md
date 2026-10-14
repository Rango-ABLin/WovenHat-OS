# Stage 13.2 PCI/WovenDriver lifecycle integration audit — 2026-10-13

## Scope

This increment connects the existing generation-safe PCI topology and BAR
leases to the WovenDriver ownership table. The Stage 13.2 acceptance probe
selects one discovered PCI function, claims it for a bounded synthetic driver
owner, retains every currently programmed MMIO BAR that can be validated, and
tears the binding down in dependency order.

The probe is feature-gated and does not activate arbitrary hardware during a
normal production boot. It proves lifecycle integration without treating PCI
presence as driver compatibility.

## Invariants checked

- The function claim is published before the driver binding.
- BAR leases are retained only after generation and owner validation.
- Partial admission rolls back the outer function claim.
- Teardown quiesces subordinate resources before releasing function ownership.
- A failed or completed probe cannot leave the synthetic binding table entry.
- Existing unsupported devices remain unbound.

## Evidence

`run-stage13-2-preservation.ps1` passed:

- focused PCI host tests;
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings`;
- runtime harness regressions;
- QEMU Stage 13.2 boots on 1, 2, and 4 CPUs, exit 33.

The owner-scoped MMIO follow-up was also revalidated by this matrix. The
discovered QEMU firmware BAR at `0x10011af410011af0` is outside the kernel
direct-map range, so `map_mmio_owned` rejects it fail-closed; the driver still
completes ordered unbind, and the gate proves mapping, revoke, and same-page
remap on a reserved test page. This is not physical device-BAR qualification.
Current serial evidence is retained in `audit-artifacts/stage13.2-1cpu-1791884820933030900`,
`audit-artifacts/stage13.2-2cpu-1791884835369436700`, and
`audit-artifacts/stage13.2-4cpu-1791884857633831800`.

The latest probe also completes driver-aware no-removal rescan and bridge-route
reconciliation after teardown. Current evidence is retained in
`audit-artifacts/stage13.2-1cpu-1791885536550950300`,
`audit-artifacts/stage13.2-2cpu-1791885544104217200`, and
`audit-artifacts/stage13.2-4cpu-1791885553144637100`.

The rescan-specific follow-up passed with evidence in
`audit-artifacts/stage13.2-1cpu-1791885536550950300`,
`audit-artifacts/stage13.2-2cpu-1791885544104217200`, and
`audit-artifacts/stage13.2-4cpu-1791885553144637100`.

## Remaining Stage 13.2 limits

This closes the driver-binding integration and owner-scoped MMIO revoke
primitive items. Full Stage 13.2 remains open for live BAR rebalance/admission
policy, bridge-window routing in normal device admission, generic physical
MSI/MSI-X activation and quiescence, physical PCIe hotplug qualification, and
valid-BAR hardware qualification.
