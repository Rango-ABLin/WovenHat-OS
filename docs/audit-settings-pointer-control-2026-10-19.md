# Settings pointer control audit — 2026-10-19

## Result

PASS. WovenSettings presents a bounded `Refresh` control. A primary-button
release delivered by WovenShell through `WPBT` re-reads the live settings
values, increments a visible refresh counter, and repaints the shared surface.
The QEMU harness requires the serial activation marker and a presented pixel
delta.

## Geometry and semantics

- Client surface: 360x240 logical pixels.
- Button rectangle: local x=250..339, y=190..213.
- Activation: release inside the rectangle increments a saturating `u32`
  counter and redraws identity, UTC clock, and display information.
- Scope: read-only and non-persistent; no capability or storage mutation is
  introduced.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app settings --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app settings --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app settings --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/settings-desktop-1cpu-1792443636136523800`
- `audit-artifacts/settings-desktop-2cpu-1792443704287364900`
- `audit-artifacts/settings-desktop-4cpu-1792443768442976800`

## Limits

This accepts one non-mutating functional control in Settings over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
