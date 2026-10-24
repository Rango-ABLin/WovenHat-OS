# Stage 13.14 storage on NVMe and AHCI audit — 2026-10-14

## What changed

- **Selectable root device.** `block_io` has a root device, `RootDevice::
  {Ata, Nvme, Ahci}`, and `with_root` runs an operation on it. The
  worker's `direct_primary`, the `PrimaryAta` handle (sector count,
  read-only state, presence), and `storage::mount_ata_root` all use
  `with_root`. `storage.rs` no longer uses the ATA driver. The default
  root is ATA, so existing configurations are unchanged.
- **Root selection.** `drivers::storage_root` returns the first ready
  `nvme0` or `ahci0`.
- **Production boot.** A normal boot with no ATA volume mounted selects
  that device after bring-up and mounts it at `/mnt`. It logs
  `[FS] FAT32 mounted at /mnt from nvme0` (or `ahci0`), or that the
  device has no mountable volume.

## Gate

`[S13.14] NVMe/AHCI storage: PASSED`. Harness:
`scripts/test-stage13-14-storage.py`.

The machine is q35, which has no ATA, with a blank FAT32 image from
`create-fat32.py` on NVMe or on the chipset AHCI controller. The kernel
checks:
- no ATA disk is present;
- bring-up yields a storage root;
- the volume mounts at `/mnt` from the selected device;
- `/mnt/WOVEN.TXT` is written and persisted;
- after `remount_mnt`, reading the file directly from the disk returns
  exactly what was written.

After QEMU exits, the harness requires both the file's bytes and its
`WOVEN   TXT` directory entry to be in the image.

In production mode, a kernel with no test features boots from the NVMe
image. It must report the mount and reach the shell.

## Evidence (`audit-artifacts/`)

Stage 13.14 on 1/2/4 CPUs, all passing:
- `nvme`: `stage13.14-nvme-1cpu-1792001143000348500`, `stage13.14-nvme-2cpu-1792000265099941200`, `stage13.14-nvme-4cpu-1792000348894914400`
- `ahci`: `stage13.14-ahci-1cpu-1792000984066447000`, `stage13.14-ahci-2cpu-1792000521713763700`, `stage13.14-ahci-4cpu-1792000608351884900`
- `production`: `stage13.14-production-1cpu-1792001085352292800`, `stage13.14-production-2cpu-1792000729769511300`, `stage13.14-production-4cpu-1792000768143619800`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Root never selected:** the volume does not mount.
- **File not persisted:** the remount readback fails, and the bytes are
  not in the image.
- **AHCI selected but I/O still sent to ATA:** the mount fails.
- **Production boot skips the mount:** "production boot did not mount the
  NVMe volume".

After the restore, `nvme` passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- ATA storage regression (`test-storage-qemu.py`, live FAT32 mutation on
  the ATA path);
- the Stage 15.2 WovenFS device gate;
- Stage 13.12 `devices` and `production`, and Stage 13.13, on 2 CPUs;
- 1-5: `stage1-5-1cpu-1792001766046240900`, `stage1-5-2cpu-1792001853927339600`, `stage1-5-4cpu-1792001925379426100`
- 10.8: `stage10.8-1cpu-1792001998368022200`, `stage10.8-2cpu-1792002087490436000`, `stage10.8-4cpu-1792002159983687200`
- 12.1: `stage12.1-1cpu-1792002226367299500`, `stage12.1-2cpu-1792002302790727900`, `stage12.1-4cpu-1792002362112628600`
- 13.3: `stage13.3-1cpu-1792002421343947600`, `stage13.3-2cpu-1792002506875713100`, `stage13.3-4cpu-1792002563690624400`
- 13.4: `stage13.4-1cpu-1792002620083867300`, `stage13.4-2cpu-1792002684797238800`, `stage13.4-4cpu-1792002732988226200`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-14-test`, `stage13-13-test`, `stage13-12-test`,
  `stage13-3-test` and `stage13-4-test`;
- `stage1-5-test`, `stage10-8-test`, `stage12-1-test`, `stage15-2-test`
  and `qemu-test`;
- host `--all-targets`.

## Limits

- **Only `/mnt`.** One FAT32 volume is mounted, from the first ready
  storage driver. Multiple volumes, and a WovenFS root on NVMe or AHCI,
  are not handled (`wfs.rs` still uses the ATA primary master).
- **ATA takes precedence.** The selection is made once at boot; a machine
  with an ATA volume keeps it as the root.
- **Polled I/O.** NVMe and AHCI I/O is still synchronous and polled, now
  behind the block-I/O worker.
- **QEMU only.** Physical NVMe and SATA devices are not qualified.
