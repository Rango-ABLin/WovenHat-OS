# Experimental WovenHat bootable USB installer

This installer replaces the partition table of a selected disk and installs the
current WovenHat development boot image. It boots to the diagnostic shell, not
to the WovenShell desktop. It is an experimental installer prototype, not full
roadmap Stage 28 acceptance or physical-hardware qualification.

## Use the generated image

1. Back up the target computer's files. Installation replaces all partitions on
   the disk you select. Disconnect disks you do not intend to install onto.
2. Write `wovenhat-installer-usb.img` to a spare USB stick with an image writer
   in raw/DD mode. This erases the USB stick. Copying the `.img` as a file onto
   the stick does not make it bootable.
3. Boot the target computer from the USB in **x86-64 UEFI mode**, with Secure
   Boot disabled. Legacy BIOS and ARM machines are not supported.
4. Identify the target using its capacity and firmware device path. Enter its
   displayed disk number. Read the erase prompt, then type `ERASE N`, replacing
   `N` with that disk number. Escape or any different confirmation cancels
   before writing. Do not unplug or replace disks during selection/install.
5. Wait for `Installation complete`. The installer flushes and reads back every
   written region before reporting success. If an error occurs after writing
   starts, the target may be incomplete; the previous OS is not restored.
6. Power off, remove the USB, and use the firmware boot menu to select the
   installed disk's UEFI OS. The bootloader is at `\EFI\BOOT\BOOTX64.EFI`.
   Firmware with no internal-disk fallback discovery may require selecting that
   file manually. The installer does not create a firmware NVRAM boot entry.

No physical disk is accessed by the build or QEMU acceptance scripts.

## Scope and limits

- Requires a target disk exposed by firmware Block I/O, using **512-byte logical
  sectors**, writable media, enough capacity for the payload, and buffer
  alignment no greater than 4096 bytes. 4Kn disks are excluded.
- The installer runs before the kernel, using firmware storage and keyboard
  support. Successful firmware access does **not** establish that WovenHat's
  runtime drivers support that computer's storage, USB, network or graphics.
- The current normal kernel boot may run entirely from RAM VFS. This installer
  does not add a persistent system root, desktop autostart, user account setup,
  networking setup, package installation, recovery or upgrade support.
- The installed ESP retains the development image's size. Remaining capacity
  is unallocated; the backup GPT is relocated to the actual disk end. This is
  replacement installation, not dual boot or an in-place update.
- Old data outside the written regions remains on the device. This is not a
  secure erase. Interrupted installation has no rollback guarantee.
- Source media is excluded by its firmware device-path ancestry. Candidate
  path, media ID, capacity, sector size, alignment, presence and writability are
  checked again after confirmation. Up to 16 eligible targets are displayed.
- The payload is embedded in the EFI executable. There is no separately loaded
  disk payload that can be swapped after selection. The executable is unsigned;
  this is not a Secure Boot or authenticated production distribution.
- Test evidence covers QEMU virtual hardware. Physical qualification remains
  open. Preserve all failure logs and verify on disposable hardware first.

## Build

From the repository root, with the repository's Rust toolchain and Python:

```powershell
python scripts/build-installer.py --output audit-artifacts/installer-new
```

Output directories must be new; previous evidence is never overwritten. The
script builds the default kernel with no self-test features, embeds its UEFI
disk image in the installer, and packages a GPT/FAT16 USB image. `manifest.json`
contains artifact sizes and SHA-256 digests. Cargo uses the launchers' existing
project-local `target` and `.cargo-build` directories.

On Windows, Microsoft `link.exe` can be selected instead of the bundled linker:

```powershell
python scripts/build-installer.py --output audit-artifacts/installer-new --uefi-linker "C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/link.exe"
```

The Microsoft linker needs `/NXCOMPAT:NO` for the EFI application subsystem; the
builder supplies it. This is a linker compatibility option, not a kernel change.

## Verification

Only run one QEMU acceptance chain at a time:

```powershell
python scripts/test-installer-qemu.py --bundle audit-artifacts/installer-new --output audit-artifacts/installer-test-new
```

The default matrix is 1/2/4 CPUs. For each configuration, the harness tests wrong
confirmation without writes, source exclusion, installation onto the selected
of two disks, preservation of the other disk and source image, GPT CRCs and
backup relocation, exact installed ESP content, then a new VM boot with only
the installed disk attached. It retains serial logs and QEMU commands. Each VM
gets its own writable OVMF variables copy, preventing firmware from saving
`NvVars` onto the source media. Only the spawned QEMU process is terminated.

Host policy tests cover malformed device paths, source-disk aliases, CRC vectors,
corrupt GPT headers/tables, undersized targets, ESP preservation and relocated
backup geometry. Build/lint the independent installer crate for both host and
`x86_64-unknown-uefi`; keep its host test results separate from kernel gates.
