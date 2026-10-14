# Stage 13.2 generic interrupt selection audit — 2026-10-13

Bound PCI drivers now have a generic interrupt activation boundary. The
policy checks the published capability and retained BAR authority, attempts
MSI-X when it is admissible, and falls back to MSI only after a clean MSI-X
failure. If MSI-X reports `ActivationRetained`, the generic path returns that
live lease obligation without attempting fallback. Existing individual MSI,
MSI-X, attach, and ordered teardown APIs remain available and unchanged.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33, with
  disposable QEMU NVMe and e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.
- The QEMU serial log reports correct PCI class/capability decoding for the
  virtio fixture, `interrupt-selection=active`,
  `interrupt-selection teardown=true`, and successful high-MMIO release.

The probe attempts generic interrupt activation only when discovery reports a
capability and the required BAR is admissible. Generic MSI-X activation and
teardown are now qualified by the QEMU virtio fixture. Physical vector
delivery, PCIe link/power hotplug, reset/recovery, and physical hardware
qualification remain open.
