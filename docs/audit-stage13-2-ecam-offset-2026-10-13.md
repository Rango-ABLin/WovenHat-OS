# Stage 13.2 ECAM offset audit — 2026-10-13

## Finding

The ECAM path passed the physical address of a configuration dword to the
legacy page mapper but used the returned page base without adding the dword's
offset within that page. Consequently, live QEMU PCI discovery read the
vendor/device dword for every ECAM offset, and ECAM writes could target the
page base instead of the requested register.

## Repair

`read_config_unlocked` and `write_config_unlocked` now add the physical page
offset after mapping and use checked arithmetic. Legacy I/O-port configuration
access is unchanged. This preserves the public PCI ABI while restoring correct
class-code, capability, BAR, command, and MSI configuration access.

## Evidence

- Warning-denying freestanding kernel Clippy passed for `stage13-2-test`.
- All-target warning-denying Clippy passed.
- The complete Stage 13.2 preservation chain passed on 1/2/4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791893707568086100`,
  `audit-artifacts/stage13.2-2cpu-1791893716313293800`, and
  `audit-artifacts/stage13.2-4cpu-1791893724978189100`.
- Serial evidence shows correct live class/capability decoding and generic MSI
  activation followed by ordered teardown on the QEMU SATA controller.

## Limits

This qualifies ECAM access and generic MSI lifecycle behavior in QEMU. It does
not qualify physical PCIe bridge interoperability, MSI-X table delivery,
hotplug, reset/recovery, or arbitrary physical hardware.
