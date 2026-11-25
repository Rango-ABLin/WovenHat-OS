# Native hardware boot preparation — 2026-11-25

Source baseline: `da263d3f` on `phase2/userspace-platform`.

## Authorization and target

The user authorized overwriting the exact USB previously used for the WovenFS
gate:

- Model: `VendorCo ProductCode USB Device`.
- Size: 15,726,735,360 bytes.
- Serial: `4045141272666606378`.
- Bus: USB; `IsBoot=false`, `IsSystem=false`.

The boot/system Samsung NVMe was never selected. Secure Boot is already
disabled on the host; no firmware setting or boot entry was changed.

## Image

A current full-desktop `hardware-demo` UEFI image was built from this source
baseline:

- Image:
  `audit-artifacts/native-hardware-image-20261125/wovenhat-hardware-demo.img`.
- Size: 22,085,632 bytes.
- SHA-256: `8c749cc21d363e14f1d119949cd0255655792d185846d94ac309230a8164a73f`.
- Build evidence:
  `audit-artifacts/native-hardware-image-20261125/image.json` and the adjacent
  build logs.

This is a disposable hardware-demo image with documented test accounts and the
acceptance app-signing key. It is not a production release image.

## Write

An elevated UAC child verified the exact model/size/serial, verified the image
hash, cleared only that removable disk, and invoked
`scripts/write-live-usb.ps1` with the same identity gate. The writer recorded:

- raw image read-back SHA-256 matched the expected digest;
- ESP partition 1 at offset 17,408, 22,020,096 bytes;
- `WOVENLOG` FAT32 partition 2 at offset 23,068,672, 15,704,522,752 bytes.

Evidence:

- `audit-artifacts/native-hardware-usb-write-20261125/transcript.log`
- `audit-artifacts/native-hardware-usb-write-20261125/writer.log`

The wrapper reported a false failure because it checked `$LASTEXITCODE` after a
PowerShell script invocation. The writer itself reached `DONE`, and its raw
read-back matched; no re-write was performed for that wrapper bug.

## Read-only confirmation

A later elevated read-only comparison read the 22,085,632-byte image region
from `PhysicalDrive1` and compared it byte-for-byte with the source image. The
only differing byte ranges were 27 ranges between offsets 528 and 1,246, all
inside the GPT header and primary partition table. This is the expected change
from adding the `WOVENLOG` partition after the writer's image read-back. No
payload bytes differed.

Evidence:

- `audit-artifacts/native-hardware-usb-compare-20261125-launcher.log`
- `audit-artifacts/native-hardware-usb-compare-20261125/diff-ranges.txt`
- `audit-artifacts/native-hardware-usb-compare-20261125/transcript.log`

An earlier verifier attempt reported a hash mismatch because it did not require
full 4 MiB reads from the raw device; that script was corrected, and the byte
comparison above establishes the actual metadata-only difference. The first
mismatch output is retained in
`audit-artifacts/native-hardware-usb-verify-20261125-launcher.log`.

## Native boot status

**PENDING USER PHYSICAL BOOT.** The prepared stick has not yet produced native
screen, serial, or `WOVENLOG` evidence. No native driver, Wi-Fi, graphics,
audio, TPM, power-loss, or release qualification is claimed by this preparation.
