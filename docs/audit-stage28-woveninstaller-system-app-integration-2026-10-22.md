# Stage 28 WovenInstaller production system-app integration audit

## Scope

This slice wires the already signed `apps/woven-installer` image into the
kernel production `system-apps` image and the Stage 19 catalog-test image. It
does not add disk, filesystem, partition, bootloader, subprocess, or physical
media access.

## Contract

- Build input: `WOVENHAT_INSTALLER_IMAGE`.
- Stable binary path: `/bin/wovenhat-installer`.
- Catalog path: `/share/applications/woven-installer.app`.
- Catalog Exec: `/bin/wovenhat-installer`.
- Capability grant: none; the package remains non-privileged and display-only.
- Existing ten catalog entries and paths remain unchanged.

## Validation

- `python -m unittest discover -s tests -p 'test_package_system_apps.py' -v`: passed (3 tests).
- `python -m py_compile` on all modified packaging/QEMU launchers: passed.
- `python scripts/package-system-apps.py --version 0.8.0`: passed; 11 signed packages emitted.
- Production kernel Clippy with all signed image environment variables,
  WovenShell, package service, and trusted key supplied:
  `cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-system-app-catalog-qemu.py --cpus 1`: passed, exit 33,
  eleven-app marker.
- `python scripts/test-system-app-catalog-qemu.py --cpus 2`: passed, exit 33,
  eleven-app marker.
- `python scripts/test-system-app-catalog-qemu.py --cpus 4`: passed, exit 33,
  eleven-app marker.
- `git diff --check`: passed.

Evidence is preserved in `audit-artifacts/system-app-catalog-{1,2,4}cpu`.
No physical disk was opened, partitioned, formatted, or modified.
