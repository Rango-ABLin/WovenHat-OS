# Stage 13.2 MSI-X owner-scoped table mappings — 2026-10-13

## Scope

MSI-X activation now asserts Function Mask and keeps MSI-X disabled before any
table read or write. Table reads, masked programming, restoration, and entry
unmasking use owner-scoped page mappings instead of the legacy shared MMIO
mapper. Pages are released immediately after each bounded volatile operation.
If a page cannot be released, the operation fails with `MmioMap`; if table
programming has already made the device-visible entry live, the activation
returns a retained `MsixLease` so the driver can quiesce it during ordered
teardown.

This preserves the existing public MSI-X capability and lease structures while
closing the vector-loss and unowned-mapping failure paths.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` — passed.
- `cargo clippy -p wovenhat-os --tests -- -D warnings` — passed.
- `run-stage13-2-preservation.ps1` — passed on 1/2/4 CPUs, exit 33.
- Latest evidence with disposable QEMU NVMe and e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The QEMU virtio fixture exposed a valid MSI-X table BAR; the preservation probe
qualified Function Mask ordering, table writes, owner-scoped mappings, vector
ownership, and ordered teardown. Physical interrupt delivery, PCIe hotplug,
reset/recovery, and arbitrary hardware qualification remain open.
