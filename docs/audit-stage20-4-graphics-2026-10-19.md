# Stage 20.4 graphics-track acceptance audit — 2026-10-19

## Result

`python scripts/test-woven-shell-qemu.py --stage 20.4 --cpus N` passed for
`N = 1`, `2`, and `4`.

The final serial marker for each run was:

`[S20.4] windows that move and close: PASSED`

## Accepted behavior

- Surface pixel budgets reject oversized, owner-over-budget, and
  system-over-budget allocations, and release capacity after destruction.
- WovenShell takes the display and opens Alpha, then opens the larger Wide
  window with its close control.
- Real i8042 input drags Alpha by its title bar. The gate checks that the
  window moved, the old region was repainted, and the new region is correct.
- Clicking Wide's close control asks the owner to exit and removes the window.
- Alpha retains focus, receives its exit key, and the display returns to the
  terminal after shell teardown.

## Evidence and limits

The harness preserves per-run serial and build evidence in
`tar\target\woven-shell-windows-{1,2,4}cpu`.

This is software/2D WovenShell window-manager acceptance. It does not claim
pointer forwarding into application clients, USB HID qualification, hardware
GPU presentation, or direct QMP automation of a production WovenFiles drag.
