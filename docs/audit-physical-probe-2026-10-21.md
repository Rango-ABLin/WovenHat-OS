# Physical-host and inventory-probe audit — 2026-10-21

## Read-only host inventory

`scripts/physical-host-preflight.ps1` completed without elevation and wrote
`audit-artifacts/physical-host-20261021-132351-901/host.json`. The host is an
HP Pavilion x360 Convertible 14-dw1xxx running Windows 11 Pro with Hyper-V
present. The attached removable candidate is Disk 1, USB, GPT, 15,728,640,000
bytes, non-boot, and non-system. The preflight reports that Secure Boot,
assignable devices, and firmware BCD could not be queried without elevation.

`PhysicalTestPerformed` is `false`. No physical disk was opened, mounted,
formatted, written, or imaged.

## Inventory image smoke gate

The existing read-only `physical-probe` image was built and booted under QEMU
with 1, 2, and 4 configured CPUs:

- `python scripts/test-physical-probe.py --cpus 1` — PASS
- `python scripts/test-physical-probe.py --cpus 2` — PASS
- `python scripts/test-physical-probe.py --cpus 4` — PASS

Each run verified the read-only PCI inventory markers, screen readiness, QMP
shutdown, and absence of the normal shell/runtime. The retained evidence is
under the corresponding `audit-artifacts/physical-probe-*` directories.

This is an inventory-image/QEMU smoke gate only. It does not prove native USB
boot, native driver operation, Wi-Fi qualification, Secure Boot behavior, or
physical release readiness.
