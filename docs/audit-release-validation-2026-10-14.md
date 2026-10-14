# Release validation audit — 2026-10-14

`python scripts/test-release.py` was rerun on the current tree. The durable
report is `target/release-validation/results.json`; it records the passing
lint/host checks and the first failing gate, `memory-1-debug`.

The chain passed warning-denying freestanding kernel Clippy, host Clippy, all
host integration-test compile/run pairs, memory/storage/network QEMU gates on
1/2/4 CPUs, legacy-PIC memory boot, 4-CPU release memory/storage/network,
release compilation, and the release shell smoke test. QEMU runs exited via
the expected success marker; network runs additionally verified host-forwarded
UDP and TCP payloads.

The ten external system applications each passed warning-denying Clippy,
signed 1/2/4-CPU Ring-3 acceptance, ten-package catalog acceptance, and the
normal-boot `system-apps-boot` gate. The full release chain did not reach those
later checks because `memory-1-debug` emitted all validation markers but timed
out waiting for exit 33. No final release manifest is claimed from this
failed chain.

This is a reproducible software/release qualification. It does not replace
native hardware qualification, destructive USB media testing, firmware Secure
Boot/TPM evidence, physical PCIe hotplug, or device-specific radio/input
interoperability.
