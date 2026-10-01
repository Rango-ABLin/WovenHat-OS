# Physical inventory qualification tooling audit — 2026-10-01

## Scope

The existing `physical-probe` image now emits a machine-readable read-only
inventory for CPU features, ACPI presence, PCI/ECAM summary, and every
recorded PCI function including class, subclass, programming interface,
revision, command state, vendor, and device ID. It still halts before storage,
network, radio, audio, or other driver activation.

This improves the evidence collected during a real hardware lab run. It does
not qualify a device, prove DMA or interrupt behavior, or replace physical
storage and power-loss testing.

## Safety boundary

The probe performs no disk I/O, does not enable bus mastering or MSI, does not
map device BARs for use, does not allocate device DMA buffers, and does not
issue controller or radio commands. It remains suitable for inventory on a
machine whose normal operating-system disks must not be modified.

## Evidence

- `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features physical-probe`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features physical-probe -- -D warnings`: passed.
- `python scripts/test-physical-probe.py --cpus 1`: passed.
- `python scripts/test-physical-probe.py --cpus 2`: passed.
- `python scripts/test-physical-probe.py --cpus 4`: passed.

The QEMU runs validate image construction and the inventory-only stop
condition. They are not physical hardware qualification.
