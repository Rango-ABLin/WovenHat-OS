# WovenFiles pointer control audit — 2026-10-19

## Result

PASS. WovenFiles presents a bounded `Refresh` control. A primary-button
release delivered by WovenShell through `WPBT` re-reads the bounded `/mnt`
listing, increments a visible counter, and repaints the shared surface. The
QEMU harness requires the serial activation marker and a presented pixel
delta.

## Geometry and semantics

- Client surface: 320x240 logical pixels.
- Button rectangle: local x=220..299, y=206..225.
- Activation: release inside the rectangle redraws the bounded listing and
  increments a saturating `u32` counter.
- Scope: read-only; no file creation, deletion, or storage write is added.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app files --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app files --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app files --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/files-desktop-1cpu-1792444009011466300`
- `audit-artifacts/files-desktop-2cpu-1792444078493137900`
- `audit-artifacts/files-desktop-4cpu-1792444150344457600`

## Limits

This accepts one read-only functional control in WovenFiles over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
