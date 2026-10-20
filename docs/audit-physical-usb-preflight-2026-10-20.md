# Physical USB preflight (2026-10-20)

This is a read-only host inventory, not hardware acceptance.

## Host

- HP Pavilion x360 Convertible 14-dw1xxx
- Windows 11 Pro 10.0.26300
- Hypervisor present: yes
- Intel Wi-Fi 6 AX201 160MHz (`8086:A0F0`)
- No serial ports reported
- Secure Boot and boot-configuration state could not be queried without
  elevation

## Attached USB disk

- Disk 1, USB, online and healthy
- VendorCo ProductCode USB Device
- GPT-labelled but currently has zero partitions and no mounted volume
- 15,726,400,000 bytes according to Storage Management
- Not the system or boot disk

The raw-media test scripts intentionally require Administrator access and a
full identity match. `scripts/write-live-usb.ps1` erases the selected disk
before installing a release image; `scripts/test-wovenfs-physical.ps1` also
clears the selected disk before passing it through to QEMU. Neither action was
performed during this preflight.

Evidence: `audit-artifacts/physical-host-20261020-171457-651/host.json`.
