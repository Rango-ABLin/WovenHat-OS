# Calculator pointer control audit — 2026-10-19

## Result

PASS. WovenCalculator presents a bounded `Evaluate` button and evaluates the
current expression when a primary-button release arrives through the production
WovenShell `WPBT` path. The QEMU harness captures before/after frames and
requires both the serial activation marker and a presented pixel delta.

## Geometry and protocol

- Client surface: 240x150 logical pixels.
- Button rectangle: local x=150..209, y=110..127.
- Input: WovenShell-owned hit testing selects the client body, then sends
  `WPBT` with button 0, the edge state, and clamped local coordinates.
- Activation: Calculator evaluates its bounded expression on release inside the
  rectangle and repaints the result.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app calculator --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app calculator --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app calculator --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/calculator-desktop-1cpu-1792443345911314700`
- `audit-artifacts/calculator-desktop-2cpu-1792443419611875000`
- `audit-artifacts/calculator-desktop-4cpu-1792443487338807800`

## Limits

This accepts one functional control in Calculator over the PS/2-to-shell-to-
client path. The remaining packaged clients have pointer feedback but not yet
accepted functional controls. USB HID qualification and hardware GPU output
remain separate gates.
