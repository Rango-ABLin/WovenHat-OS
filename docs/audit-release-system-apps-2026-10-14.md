# Release system-application integration audit — 2026-10-14

The authoritative release chain now includes the external Stage 19 application
workspaces and the signed catalog boundary. `scripts/test-release.py` runs
warning-denying Clippy for WovenCalculator, WovenTerminal, and WovenFiles,
then runs `scripts/test-system-app-catalog-qemu.py` sequentially on 1, 2, and
4 CPUs. The full release chain passed every host test, debug/release memory,
storage, network, legacy-PIC, shell, and application check.

`scripts/package-release.py` now includes the validated system-app manifest,
three WAPP packages, three bounded catalog descriptors, and SHA-256 entries for
every release artifact in both the release directory and ZIP archive. The
validated release image remains independently hashed and unchanged.

The release artifact is still a software/QEMU qualification. The image does
not silently install external packages during ordinary boot; an atomic
installer with rollback and provisioned release trust policy remains required
before physical deployment. No removable media was modified.
