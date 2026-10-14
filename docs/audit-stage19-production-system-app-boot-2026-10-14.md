# Stage 19 production system-app boot audit — 2026-10-14

## Production image boundary

The `system-apps` image feature embeds the three signed WAPP packages from the
release install plan. Before changing the VFS it verifies every package against
the kernel trust root, parses the ELF payload, validates the normal loader, and
checks all declared paths for collisions. It then installs package files first
and publishes `/share/applications/*.app` descriptors only after all packages
are present. Any failed package or catalog insertion removes only entries and
directories created by that transaction.

Production images require `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` at compile
time. There is no test-key fallback under `system-apps`. The QEMU harness uses
the documented deterministic test key explicitly; release provisioning must
replace it with the deployment key and pass the same public key to the WAPP
signer.

## Validation

Warning-denying freestanding Clippy passes for `system-apps`. The normal-boot
shell/QMP acceptance passed on 4 CPUs, with serial markers showing three
signed applications installed before shell startup and the existing SMP/PS2
smoke markers. The full release chain will be rerun after this trust-policy
change; it subsequently passed, including the 1/2/4-CPU catalog matrix and
the `system-apps-boot` check. The final release image was built with the
feature and an explicit trusted public key.

## Limits

The catalog is currently RAM-backed during boot; persistent FAT32/WovenFS
installation, rollback across power loss, key rotation/revocation, and native
hardware qualification remain separate production requirements. No removable
media was modified.
