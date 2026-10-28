# WovenMonitor pointer control audit — 2026-10-19

## Result

PASS. WovenMonitor presents a bounded `Refresh` control. A primary-button
release delivered by WovenShell through `WPBT` re-reads the clock, display,
and bounded storage snapshot, increments a visible counter, and repaints the
shared surface. The QEMU harness requires the serial activation marker and a
presented pixel delta.

## Geometry and semantics

- Client surface: 360x220 logical pixels.
- Button rectangle: local x=250..329, y=170..189.
- Activation: release inside the rectangle performs a read-only refresh and
  increments a saturating `u32` counter.
- Scope: no storage or system mutation is introduced.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app monitor --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app monitor --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app monitor --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/monitor-desktop-1cpu-1792444570664141600`
- `audit-artifacts/monitor-desktop-2cpu-1792444639221998800`
- `audit-artifacts/monitor-desktop-4cpu-1792444716385316700`

## Limits

This accepts one read-only functional control in WovenMonitor over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
