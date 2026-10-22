# WovenTerminal pointer control audit — 2026-10-19

## Result

PASS. WovenTerminal presents a bounded `Clear` control. A primary-button
release delivered by WovenShell through `WPBT` clears the in-window bounded
command preview, updates its status text, and repaints the shared surface.
The QEMU harness requires the serial activation marker and a presented pixel
delta.

## Geometry and semantics

- Client surface: 360x220 logical pixels.
- Button rectangle: local x=250..329, y=170..189.
- Activation: release inside the rectangle clears the preview buffer and sets
  the bounded status text to `Input cleared by pointer control`.
- Scope: the full CLI and interactive command paths are unchanged.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app terminal --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app terminal --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app terminal --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/terminal-desktop-1cpu-1792444276207044800`
- `audit-artifacts/terminal-desktop-2cpu-1792444352186856500`
- `audit-artifacts/terminal-desktop-4cpu-1792444426996986300`

## Limits

This accepts one preview-only functional control in WovenTerminal over the
PS/2-to-shell-to-client path. Other packaged clients still have pointer
feedback rather than accepted functional controls. USB HID qualification and
hardware GPU output remain separate gates.
