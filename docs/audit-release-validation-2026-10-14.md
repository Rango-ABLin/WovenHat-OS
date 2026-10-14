# Release validation audit — 2026-10-14

`python scripts/test-release.py` completed successfully on the current tree.
The durable report is `target/release-validation/results.json`, with the
validated image manifest in `target/release-validation/validated-image.json`.

The chain passed warning-denying freestanding kernel Clippy, host Clippy, all
host integration-test compile/run pairs, memory/storage/network QEMU gates on
1/2/4 CPUs, legacy-PIC memory boot, 4-CPU release memory/storage/network,
release compilation, and the release shell smoke test. QEMU runs exited via
the expected success marker; network runs additionally verified host-forwarded
UDP and TCP payloads.

The current release chain also passed warning-denying Clippy for all three
external system applications, signed catalog acceptance on 1/2/4 CPUs, and
the normal-boot `system-apps-boot` gate. The packaged release includes the
three WAPP payloads, catalog descriptors, and complete artifact hashes. The
final image was built with `system-apps` and an explicit application trust
public key; its release manifest records `working_tree_dirty=false`.

This is a reproducible software/release qualification. It does not replace
native hardware qualification, destructive USB media testing, firmware Secure
Boot/TPM evidence, physical PCIe hotplug, or device-specific radio/input
interoperability.
