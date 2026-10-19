# WovenSecurity pointer control audit — 2026-10-19

## Result

PASS. WovenSecurity presents a bounded `Refresh` control. A primary-button
release delivered by WovenShell through `WPBT` re-reads the bounded audit
summary, increments a visible counter, and repaints the shared surface. The
QEMU harness requires the serial activation marker and a presented pixel
delta.

## Geometry and semantics

- Client surface: 360x200 logical pixels.
- Button rectangle: local x=250..329, y=145..164.
- Activation: release inside the rectangle performs a read-only refresh and
  increments a saturating `u32` counter.
- Scope: audit state is not modified.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app security --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app security --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app security --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/security-desktop-1cpu-1792444853700823700`
- `audit-artifacts/security-desktop-2cpu-1792444927353135800`
- `audit-artifacts/security-desktop-4cpu-1792444995675039500`

## Limits

This accepts one read-only functional control in WovenSecurity over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
