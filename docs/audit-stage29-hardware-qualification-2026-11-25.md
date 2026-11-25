# Stage 29 hardware qualification matrix — 2026-11-25

Source baseline: `da263d3f` on `phase2/userspace-platform`.

## Evidence and policy

- Elevated read-only inventory:
  `audit-artifacts/hardware-inventory-20261125/inventory.json`.
- Physical WovenFS USB result:
  `docs/audit-physical-wovenfs-2026-11-25.md`.
- Native boot attempts:
  `docs/audit-native-hardware-boot-attempt-1-2026-11-25.md` and
  `docs/audit-native-hardware-boot-attempt-2-2026-11-25.md`.
- Qualification manifest:
  `audit-artifacts/hardware-qualification-20261125/manifest.json`.
- Validator result:
  `audit-artifacts/hardware-qualification-20261125/result.json`.

`scripts/qualify-hardware-report.py` accepted the manifest and classified the
current evidence as **Tier 3** because the second native boot recorded two
native failed observations: WovenShell remained alive without activating the
display, and the `WOVENLOG` live-log path was not initialized. No native
category has a passed observation. The real-USB WovenFS pass is recorded with
`environment: qemu`; the policy intentionally refuses to convert
QEMU-passthrough evidence into native Tier 1 evidence.

## Host

- HP Pavilion x360 Convertible 14-dw1xxx, AMI BIOS F.50.
- Intel Core i5-1135G7, 4 cores / 8 logical processors, 8 GiB RAM.
- Windows 11 Pro 10.0.26300; Hyper-V present.
- Intel Iris Xe Graphics `8086:9A49` at 1920x1080.
- Realtek HD Audio plus Intel Smart Sound Technology.
- Intel Wi-Fi 6 AX201 `8086:A0F0` and Intel Wireless Bluetooth `8087:0026`.
- One 256 GB Samsung NVMe, non-boot non-system VendorCo USB, two Intel xHCI
  controllers, one HP webcam, ELAN touchpad and touchscreen, one battery.
- TPM 2.0 is enabled and activated. Secure Boot is disabled.
- Firmware BCD contains a `USB Drive (UEFI)` firmware application.
- No wired Ethernet, SATA device, AMD/NVIDIA GPU, desktop, or second monitor.

## Category matrix

| Category | On this host | Native WovenHat status |
| --- | --- | --- |
| intel-pc | Yes | Failed natively; console/keyboard boot reached diagnostic shell, but WovenShell stayed alive without desktop activation |
| laptop | Yes | Blocked; battery, lid, suspend/resume and touch behavior untested |
| nvme | Yes, boot/system disk | Blocked; never selected for destructive or native-driver tests |
| sata | No | Skipped |
| usb | Yes, xHCI and removable disk | Real USB WovenFS passed under QEMU passthrough; native `WOVENLOG` live-log path failed to initialize, so no native USB category pass |
| ethernet | No | Skipped |
| wifi | Yes, AX201 | Blocked; PCI identity observed, no native firmware/DMA/radio pass |
| audio | Yes | Blocked; no driver/device pass |
| intel-graphics | Yes | Blocked; no native graphics stack pass |
| amd-pc | No | Skipped |
| amd-graphics | No | Skipped |
| nvidia-graphics | No | Skipped |
| touchpad | Yes | Blocked; no HID driver/input pass |
| webcam | Yes | Blocked; no UVC/media pass |
| bluetooth | Yes | Blocked; no radio pass |
| multi-monitor | No, one display | Skipped |
| desktop | No | Skipped |

## Decision

**Full native hardware qualification has not been achieved.** The fail-closed
report is **Tier 3** because native failures were observed, although those
observations diagnose WovenHat startup/logging paths rather than proving a USB
controller hardware defect. Completing the matrix requires repaired native
WovenShell and live-log evidence plus device-specific evidence on this model,
and separate systems for the desktop, AMD, NVIDIA, SATA, Ethernet and
multi-monitor categories. Inventory, TPM state, QEMU smoke runs, and the
QEMU-passthrough WovenFS gate do not close those gaps.
