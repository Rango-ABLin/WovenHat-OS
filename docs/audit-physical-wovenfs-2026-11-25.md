# Physical WovenFS qualification — 2026-11-25

Source baseline: `cacbf699d4343548dba12da641dc9c5ee3d2f955` on
`phase2/userspace-platform`.

## Authorized target

The user explicitly authorized erasing exactly one removable disk, identified by
model, byte size, and serial rather than by disk number:

- Disk index: 1 (`PhysicalDrive1`).
- Model: `VendorCo ProductCode USB Device`.
- Size: 15,726,735,360 bytes.
- Serial: `4045141272666606378`.
- `IsBoot=false`, `IsSystem=false`, USB bus, removable media.

Disk 0 is the boot/system `NVMe SAMSUNG MZVLQ256HBJD-00BH1`; it was never
selected or opened by the destructive gate.

## Commands

An elevated child process was launched via UAC because the tool shell itself is
non-elevated. The child wrapper invoked:

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File
  C:\Users\banda\AppData\Local\Temp\opencode\run-physical-wovenfs-20261125.ps1
```

The wrapper called `scripts/test-wovenfs-physical.ps1` with the exact identity
above at 1, 2, and 4 CPUs. Afterward, a second elevated read-only wrapper called
the same script with `-Inspect`.

## Results

All three CPU profiles passed with QEMU exit code 33 and
`[S15.2] WovenFS physical device: PASSED`. The 1-CPU run also recorded
`[S15.2] device sectors 30720000`. The full success sequence included format,
two-file create/write, authorized write-through across remounts, VFS
write-through and namespace isolation, invalid-path rejection, oversized and
overlong mutation rejection, sparse-write rejection, remount/unmount refresh,
denied-write preservation, unlink, and unlink survival across remount.

Evidence:

- Elevated launch transcript:
  `audit-artifacts/physical-wovenfs-elevated-20261125-110713/transcript.log`
- 1 CPU: `audit-artifacts/wovenfs-physical-1cpu-20261125-110840/`
- 2 CPU: `audit-artifacts/wovenfs-physical-2cpu-20261125-110925/`
- 4 CPU: `audit-artifacts/wovenfs-physical-4cpu-20261125-111010/`
- Read-only post-test inspection:
  `audit-artifacts/physical-wovenfs-inspect-20261125/transcript.log`

The expected “Removable media cannot be set to offline” error appeared on each
run and was handled as designed by relying on volume dismounts; it did not
affect the result. The final inspection read LBA0 as `WOVENFS1` and reported
checkpoint sequence 15. The USB stick now carries a WovenFS volume that Windows
cannot read.

## Boundary of this result

This qualifies the kernel's WovenFS transaction against a **real removable
block device passed through to QEMU as the ATA primary master**. It does not
claim native WovenHat boot from the USB stick, Secure Boot, firmware handoff,
NVMe/SATA controller qualification, Wi-Fi, GPU, audio, battery, suspend/resume,
power-loss recovery, or installer/release certification. TPM hardware remains
unavailable and the boot/system NVMe was not touched.
