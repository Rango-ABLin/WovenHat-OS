# Stage 19 system-app packaging audit — 2026-10-14

## Scope

The accepted WovenCalculator, WovenTerminal, WovenFiles, WovenMonitor,
WovenSettings, and WovenSecurity binaries are now
packaged together by `scripts/package-system-apps.py`. The script builds each
separate app workspace with the pinned freestanding target, signs every ELF
using `tools/wapp-packager`, validates the WAPP v1 envelope and ELF payload,
and writes a manifest with SHA-256 hashes for packages and catalog entries.

## Safety and limits

The packager writes only under its selected output directory and does not
install to a boot volume or erase removable media. Catalog descriptors contain
fixed executable paths and no privilege escalation. The release `system-apps`
image feature consumes the plan in RAM after validation; Stage 16.1 still
accepts one package per boot through `WOVENHAT_APP_IMAGE`.

## Validation

The packaging command was run after the app-specific host checks, freestanding
Clippy checks, and signed QEMU 1/2/4 acceptance gates recorded in the six
application audits. The resulting manifest and package hashes are generated
under `target/system-apps/0.8.0` and are intentionally ignored build evidence.

## Result

The reproducible signed packaging/catalog boundary passes for the four accepted
apps. Stage 19 remains incomplete until the remaining applications, persistent
media update/rollback policy, and full GUI integration are accepted.
