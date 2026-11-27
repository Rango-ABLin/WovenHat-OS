# Stage 29 Z3 — live-test USB preparation (2026-11-27)

**Result: DONE (media preparation only).** No native boot, reboot, firmware
change, boot-entry change, or trust/TPM operation was performed. The USB is
ready for an owner-controlled boot attempt; it is not evidence that WovenHat
boots on bare metal.

## Authorization and target

The operator chose “Prepare bootable USB only” after explicitly authorizing the
same exact spare device used for the Z3 Phase 1 storage test:

- Model: `VendorCo ProductCode USB Device`
- Size gate: `15726735360` bytes
- Serial: `4045141272666606378`
- Disk: `PhysicalDrive3`, `BusType=USB`, `IsBoot=false`, `IsSystem=false`

The writer's identity gate found exactly one matching disk. The boot/system
`SK hynix BC901` NVMe was never selected. The previous WovenFS volume on the
authorized spare was intentionally overwritten by this operation.

## Image build

Built from current `master` commit `c1d224a5` with the `hardware-demo` feature
and the same deterministic acceptance app-signing key used by the release and
shell gates (`d75a98…751a`). `scripts/package-system-apps.py` produced 11 signed
apps; `build-package-service.py`, `build-search-service.py` and the release
WovenShell were rebuilt from this checkout before the kernel image.

- Image:
  `audit-artifacts/native-hardware-image-20261127/wovenhat-hardware-demo.img`
- Size: `35717120` bytes
- SHA-256: `57ec93021ed6f465d64af23fc6ed6cc4871f3e467b1afab0dfa32fea3f8a274a`
- Provenance: `audit-artifacts/native-hardware-image-20261127/image.json`
- Full build output: `audit-artifacts/native-hardware-image-20261127/build.log`

The first direct `cargo run --features hardware-demo -- --print-image` failed
because `system-apps` image environment variables were unset; the build was
re-run with the acceptance packaging environment rather than by disabling the
missing inputs.

## Write and verification

`scripts/write-live-usb.ps1` ran in an Administrator child:

1. Verified the image SHA-256 against the supplied digest.
2. Verified the exact USB identity and non-boot/non-system status.
3. Wrote all `35717120` bytes and read that region back before partitioning;
   the writer reported `read-back SHA-256 matches`.
4. Created the `WOVENLOG` FAT32 partition at offset `36700160` and set the
   label in both FAT32 boot-sector locations.

Independent read-only checks after the write:

- Disk 3 remains non-boot/non-system with two GPT partitions: EFI System at
  offset `17408`, size `35651584`, and the basic-data `WOVENLOG` partition at
  offset `36700160`.
- The ESP hash read from the USB equals the ESP hash read from the source image:
  `ad75accc51edf6904e214f65a3ca55c6ae3685d3809c465c48279ce2619ff875`.
- The log partition boot sector reports label `WOVENLOG   ` and filesystem
  `FAT32   `.

An earlier whole-image-region hash check is preserved at
`audit-artifacts/z3-bootable-usb-20261127/readback-verify.json` and reports
`match: false`. That is expected: the writer intentionally added the second
GPT partition after its own readback, so GPT metadata inside the first
`35717120` bytes changed. It is not a payload mismatch. The correct per-ESP
comparison is recorded in `partition-readback-verify.json`.

## Evidence

- Build: `audit-artifacts/native-hardware-image-20261127/`
- Write: `audit-artifacts/z3-bootable-usb-20261127/write-live-usb.log`,
  `write.out.log`
- Read-only post-write checks:
  `audit-artifacts/z3-bootable-usb-20261127/postwrite-inspect.json`,
  `partition-readback-verify.json`

## Limits and next bit

This records prepared media, not a passed boot category. The hardware-support
classification remains Tier 2 with 0 native-passed categories. No boot was
attempted, so no native boot, graphics, input, Wi-Fi, power-loss or TPM claim
is made. The next bit still requires an owner-controlled boot from this USB,
physical console/serial capture (this host has no serial port), and separate
authorization/power control for any destructive power-loss cycle.
