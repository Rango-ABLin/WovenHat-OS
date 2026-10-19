# WovenImages pointer control audit — 2026-10-19

## Result

PASS. WovenImages presents a bounded `Refresh` control. A primary-button
release delivered by WovenShell through `WPBT` re-reads the bounded `/mnt`
image summary, increments a visible counter, and repaints the shared surface.
The QEMU harness requires the serial activation marker and a presented pixel
delta.

## Geometry and semantics

- Client surface: 360x220 logical pixels.
- Button rectangle: local x=250..329, y=170..189.
- Activation: release inside the rectangle performs a read-only refresh and
  increments a saturating `u32` counter.
- Scope: image decoding/editing and storage mutation are not enabled.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app images --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app images --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app images --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/images-desktop-1cpu-1792445381591665300`
- `audit-artifacts/images-desktop-2cpu-1792445446823871500`
- `audit-artifacts/images-desktop-4cpu-1792445510977257600`

## Limits

This accepts one read-only functional control in WovenImages over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
