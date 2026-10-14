# Release validation audit — 2026-10-14

`python scripts/test-release.py` passed on the current tree. The durable
report is `target/release-validation/results.json`, with preserved serial and
QEMU evidence for every gate.

The chain passed warning-denying freestanding kernel Clippy, host Clippy, all
host integration-test compile/run pairs, memory/storage/network QEMU gates on
1/2/4 CPUs, legacy-PIC memory boot, 4-CPU release memory/storage/network,
release compilation, and the release shell smoke test. QEMU runs exited via
the expected success marker; network runs additionally verified host-forwarded
UDP and TCP payloads.

The ten external system applications each passed warning-denying Clippy,
signed 1/2/4-CPU Ring-3 acceptance, ten-package catalog acceptance, and the
normal-boot `system-apps-boot` gate. The debug QEMU exit-width correction was
validated by the complete chain; every QEMU run returned the expected exit 33.

This is a reproducible software/release qualification. It does not replace
native hardware qualification, destructive USB media testing, firmware Secure
Boot/TPM evidence, physical PCIe hotplug, or device-specific radio/input
interoperability.
