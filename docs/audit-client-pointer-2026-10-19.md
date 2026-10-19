# Client pointer forwarding graphics audit — 2026-10-19

## Result

`python scripts/test-files-desktop-qemu.py --app calculator
--exercise-client-pointer --cpus N` and the equivalent Terminal command passed
for `N = 1`, `2`, and `4`.

Each Calculator run produced:

`WovenCalculator: pointer updated`

Each Terminal run produced:

`WovenTerminal: pointer updated`

The harness also compared before/after QEMU screendumps and required a
presented Calculator-surface delta.

## Accepted protocol

- WovenShell keeps desktop chrome, title bars, close controls, and title-bar
  drags shell-owned.
- `WPMV` is an 8-byte message: tag, client-local x `u16`, client-local y
  `u16`.
- `WPBT` is a 10-byte message: tag, button `u8`, pressed `u8`, client-local x
  `u16`, client-local y `u16`.
- The shell clamps coordinates to the receiving surface and captures the
  primary-button sequence for its client.
- Calculator and Terminal draw bounded markers for the received position and
  button state.

## Boundaries

This is software/2D PS/2 pointer delivery to Calculator and Terminal. The
other packaged clients do not yet consume pointer controls; USB HID
qualification and hardware GPU presentation are not claimed.
