# Bootloader build isolation

Source: crates.io `bootloader` 0.11.17, upstream commit
`ec8a8b4b59bd94f3c0280adc1bcdae530251b003`.
Published crate SHA-256:
`d861319d747a01da5d4d680500b7f808607f902a286b476bd51f991612e39d8e`.
The upstream MIT and Apache-2.0 licenses are retained.

The initial code change is in the UEFI installer command in `build.rs`: give the
nested Cargo invocation its own target and intermediate build directories under
`OUT_DIR`. Otherwise the inherited project-local `CARGO_TARGET_DIR` and
`CARGO_BUILD_BUILD_DIR` can make the installer wait indefinitely for an artifact
lock held by the parent build. Both child directories remain project-local.
The bootloader version and image-generation code are unchanged.
The BIOS build is not enabled by WovenHat and is unchanged.

Package build inputs and upstream tests are retained; upstream repository-only
CI/editor configuration and documentation folders are omitted. Cargo excludes
this dependency from the workspace. The root crates.io patch selects it for
the host image builder; this is not a replacement for the kernel's bootloader
API dependency.

## BLD-B1 runtime configuration boundary

The version-matched `bootloader-x86_64-uefi` 0.11.17 runtime is now retained
under `uefi/`, rather than editing Cargo's global registry cache. Its original
archive SHA-256 is
`a6f5c10daaa697564ff78776f23ff0ae6e401c7ead7af41d980a913869a8b67f`.
The original main/memory-descriptor/manifest/lock files were compared with
that archive before changes. The parent MIT/Apache-2.0 licenses apply.

`uefi/src/config.rs` bounds boot.json to 4096 bytes, accepts existing known
settings with an omitted version or explicit version 1, and rejects malformed,
duplicate, unknown, unsupported-version and trailing-data policy. The UEFI
entry initializes a diagnostic console with defaults on parse failure, emits
a static error and halts before ramdisk loading/ExitBootServices. It never
continues boot with a rejected policy. Unknown requested features fail closed;
root selection, boot menus/timeouts, recovery and kernel command-line forwarding
are not implemented by this increment. Existing valid logging/framebuffer
preferences and the BootInfo/mapper contract are preserved.

Disk and TFTP file-size metadata is checked before allocating/reading a
configuration payload, including empty files, so the parser limit is also a
preallocation bound. Kernel/ramdisk file handling retains its existing policy.
Two upstream lint cleanups remove an unnecessary pointer `mut` and a redundant
formatting borrow. Runtime dependency versions/checksums remain identical to
the original UEFI lockfile; only the direct Serde dependency edge is added.

The local installer uses one isolated target directory and `--force` within
OUT_DIR so edited runtime code cannot silently reuse the registry binary.
It watches a local common directory only if one exists; common code remains
the locked registry dependency. No new hand-written unsafe code is introduced.
