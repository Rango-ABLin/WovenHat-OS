# Stage 22.4 production WovenPackage startup audit — 2026-10-20

## Scope

Verify that the signed Ring-3 WovenPackage service is part of the production
`system-apps` image build and is started and publication-checked during boot.

## Implementation

- Added `scripts/build-package-service.py` to build and sign the service image.
- Added `WOVENHAT_PACKAGE_IMAGE` to the `system-apps` kernel image path.
- Added bounded startup publication validation owned by the spawned service PID.
- Updated the system-apps shell gate to preserve CPU-specific evidence and
  require the service publication marker.
- Corrected package-sandbox helper feature gates so production Clippy does not
  compile unreferenced Stage 21-only helpers.

## Validation

- Production system-apps shell/PS2/IOAPIC QMP gate passed on 1 CPU.
- Production system-apps shell/PS2/IOAPIC QMP gate passed on 2 CPUs.
- Production system-apps shell/PS2/IOAPIC QMP gate passed on 4 CPUs.
- Each serial log contains `[SERVICES] woven-package published as pid 1`.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- Stage22 freestanding Clippy, host all-targets Clippy, Python syntax, and diff
  checks: passed.

## Limits

The broker is now production-started, but its mutation operations deliberately
return storage status. A signed package payload/transaction ABI and its
crash-recovery acceptance remain required before package-management release
closure. Physical USB qualification remains separate and unperformed.
