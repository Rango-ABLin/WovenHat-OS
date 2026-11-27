# Physical hardware readiness preflight — 2026-11-25

Source baseline: `cacbf699d4343548dba12da641dc9c5ee3d2f955` on
`phase2/userspace-platform`.

## Commands

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\physical-host-preflight.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\test-physical-tpm-readiness.ps1
python scripts/test-physical-probe.py --cpus 1
python scripts/test-physical-probe.py --cpus 2
python scripts/test-physical-probe.py --cpus 4
```

## Read-only host inventory

Evidence: `audit-artifacts/physical-host-20261125-105724-588/host.json`

- Manufacturer/model: HP Pavilion x360 Convertible 14-dw1xxx.
- OS: Windows 11 Pro, build `10.0.26300`.
- Hyper-V present: yes.
- Installed RAM: 8,211,910,656 bytes; meets the 4 GiB production floor.
- Network adapter: Intel(R) Wi-Fi 6 AX201 160MHz, PCI, enabled.
- Disks: one disk only, `NVMe SAMSUNG MZVLQ256HBJD-00BH1`, 256,060,514,304
  bytes, GPT, `IsBoot=true`, `IsSystem=true`.
- Secure Boot query: denied because the session is not elevated.
- Assignable-device and firmware BCD queries: denied for the same reason.
- `PhysicalTestPerformed=false`.

## TPM/USB readiness

Evidence: `audit-artifacts/physical-tpm-readiness-20261125-105724/`

- TPM visible: no (`Access denied` from `Win32_Tpm`).
- TPM ready: no.
- QEMU/`swtpm` live TPM backend: no.
- Signed provisioning record: no.
- USB candidates: 0.
- Raw disk opened, disk state changed, media written, TPM provisioned: none.

## QEMU inventory smoke

The read-only `physical-probe` image passed on 1, 2, and 4 configured CPUs.
Each run preserved command, serial, screenshot, QEMU log, and result evidence:

- `audit-artifacts/physical-probe-1cpu-1795597097158653500/`
- `audit-artifacts/physical-probe-2cpu-1795597240087547700/`
- `audit-artifacts/physical-probe-4cpu-1795597254202074100/`

All three runs used image SHA-256
`58e6a1c7b17a7e7cec5ae33ffef2d807c28f885ddbd37b16ce08e7d1725a1b22`. Each
`result.json` records `ap_startup_exercised: false` and
`physical_wifi_acceptance: false`. This is a guest inventory smoke gate, not
native hardware acceptance.

## Disposable USB candidate

A later read-only rerun found one safe removable candidate:

Evidence: `audit-artifacts/physical-tpm-readiness-20261125-110321/readiness.json`

- Disk index: 1.
- Model: `VendorCo ProductCode USB Device`.
- Size: 15,726,735,360 bytes.
- Serial: `4045141272666606378`.
- Bus: USB; `IsBoot=false`, `IsSystem=false`, `safe_for_destructive_gate=true`.

The user explicitly authorized erasing this exact device, but the session is
still non-elevated (`Elevated=False`), so no destructive gate was started. The
script's identity gate must be invoked with this exact model/size/serial, never
with a disk number alone and never against Disk 0, which is the boot/system
NVMe.

## Decision

**Native hardware qualification is blocked, not failed.** There is no
visible/ready TPM, no software TPM backend, no signed provisioning record, and
no elevated session for Secure Boot, device assignment, firmware BCD, or
raw-disk access.

Before destructive or live hardware gates can run, the session must be
elevated and the exact authorized USB identity re-verified. No physical boot,
TPM provisioning, Wi-Fi, GPU, audio, suspend/resume, or installer qualification
is claimed here.
