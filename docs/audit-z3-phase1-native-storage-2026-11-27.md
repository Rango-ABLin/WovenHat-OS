# Stage 29 Z3 Phase 1 — physical USB storage (2026-11-27)

**Result: PASSED (bounded physical-device/QEMU-passthrough evidence).** This is
not native-driver or Tier 1 evidence.

## Authorization and identity gate

The operator explicitly authorized erase and Phase 1 storage testing of the
exact spare USB, and confirmed it contains no data to preserve. The device was
re-verified before every destructive run by model, exact byte size and serial:

- Model: `VendorCo ProductCode USB Device`
- Size: `15726735360` bytes
- Serial: `4045141272666606378`
- Disk: `PhysicalDrive3`, `IsBoot=false`, `IsSystem=false`
- Z1 readiness flag: `safe_for_destructive_gate: true`

The boot/system disk (`SK hynix BC901`, Disk 1) was never targeted. No boot
entry, firmware setting, TPM or other device was changed.

## Procedure and results

All commands ran in an Administrator child with UAC approval. Output was
captured under `audit-artifacts/z3-phase1-20261127/`.

1. Read-only `test-wovenfs-physical.ps1 -Inspect`: identity gate passed; LBA0
   was blank before testing.
2. `test-wovenfs-physical.ps1 -Cpus 1`: QEMU exit `33`,
   `[S15.2] WovenFS physical device: PASSED`.
3. `test-wovenfs-physical.ps1 -Cpus 2`: QEMU exit `33`, same marker.
4. `test-wovenfs-physical.ps1 -Cpus 4`: QEMU exit `33`, same marker.
5. Read-only `-Inspect` after the matrix: LBA0 `WOVENFS1`, superblock present,
   checkpoint sequence `15`.

The Stage 15.2 serial logs include sector write acceptance, unformatted-disk
rejection, format, two-file create/write, authorized write-through across
remounts, VFS path isolation, invalid-path rejection, oversized/overlong
mutation preservation, rejected sparse-write behavior, remount content
verification, denied-VFS-write preservation, and unlink persistence.

## Evidence

- Run logs: `audit-artifacts/z3-phase1-20261127/phase1-storage.log`,
  `phase1-storage-2cpu.log`, `phase1-storage-4cpu.log`, `inspect.log`,
  `inspect-after.log`.
- Per-CPU serial/QEMU evidence:
  `audit-artifacts/wovenfs-physical-1cpu-20261127-161351/`,
  `audit-artifacts/wovenfs-physical-2cpu-20261127-161459/`,
  `audit-artifacts/wovenfs-physical-4cpu-20261127-161544/`.
- Bound report and classification:
  `audit-artifacts/hardware-qualification-20261127-z3-phase1/report.json`,
  `classification.json`.

## Classification and limits

The physical USB observation is recorded with `environment: qemu`, because the
kernel drove the real device through QEMU ATA passthrough rather than a native
WovenHat USB driver. The classifier therefore remains **valid Tier 2**: 0 native
passed categories, 17 incomplete native categories, 0 failed observations. The
QEMU-passthrough pass is not promoted to a native `usb` pass.

Phase 1 rollback is **not** claimed: the authorized spare disk now carries a
WovenFS volume by design (`WOVENFS1`, sequence 15). No boot entry was modified.
Phases 2–4 remain open: native boot repeatability needs physical console/serial
evidence, controlled power-loss needs physical power control, and firmware/trust
is skipped because this host reports TPM not visible/not ready and no signed
provisioning record.
