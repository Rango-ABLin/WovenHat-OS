# Release system-application integration audit — 2026-10-14

The authoritative release chain now includes the external Stage 19 application
workspaces and the signed catalog boundary. `scripts/test-release.py` runs
warning-denying Clippy for WovenCalculator, WovenTerminal, WovenFiles, and
WovenMonitor, WovenSettings, WovenSecurity, WovenEditor, WovenImages,
WovenUpdate, and WovenStore; runs all ten through signed Ring-3 harnesses on 1,
2, and 4 CPUs; and runs `scripts/test-system-app-catalog-qemu.py` on 1, 2,
and 4 CPUs. The ten-application checks and the complete release chain passed;
the debug QEMU exit-width correction was exercised by the release matrix.

`scripts/package-release.py` now includes the validated system-app manifest,
ten WAPP packages, ten bounded catalog descriptors, and SHA-256 entries for
every release artifact in both the release directory and ZIP archive. The
final release image is independently hashed in the release manifest.

The release artifact is still a software/QEMU qualification. The feature-gated
release image installs the ten packages into RAM after verification, while
persistent-media updates, power-loss recovery, key rotation/revocation, and
native hardware qualification remain required before physical deployment. No
removable media was modified.
