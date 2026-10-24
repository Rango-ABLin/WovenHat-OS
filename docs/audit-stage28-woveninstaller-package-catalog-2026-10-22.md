# Stage 28 WovenInstaller package/catalog wiring audit — 2026-10-22

## Scope

The existing `scripts/package-system-apps.py` path now includes the already
implemented `apps/woven-installer` image. It builds the freestanding ELF,
signs it with the existing WAPP v1 producer, validates the envelope, copies
the package to `packages/woven-installer.wapp`, and emits a stable catalog
descriptor at `catalog/woven-installer.app`.

The manifest record uses `/bin/wovenhat-installer` for both `install_path` and
`Exec`. It declares `capabilities: []`, `privileged: false`, and
`interface: display-only`. A fail-closed metadata validator rejects the
installer if any install, disk, partition, format, or bootloader capability
is introduced. The prior ten app definitions remain in their original order
and are not changed.

## Validation

The focused package-manifest tests cover entry preservation, stable paths,
the non-privileged display-only contract, and forbidden-capability rejection.
The installer app is also checked with a freestanding release build and
warning-denying Clippy. Python compilation and the packaging script's
manifest smoke check pass.

## Limits

This is packaging/catalog wiring only. It does not alter kernel recovery,
grant installation authority, partition or format disks, install a
bootloader, or claim the complete `system-apps` QEMU gate. Physical media and
the attached drives are not accessed.
