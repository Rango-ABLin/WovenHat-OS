# Stage 13.7 — WovenInput Candidate

Baseline: `864f94c` (accepted Stage 13.6 USB HID).

This candidate introduces the device-independent WovenInput event layer and routes the existing PS/2 keyboard path through it while preserving the foreground-terminal single-consumer rule.

Implemented:
- bounded unified input event queue
- device-independent keyboard, pointer, touch, pen and game-controller event types
- PS/2 scancode decoder publishes logical key events into WovenInput
- existing kernel shell/userspace stdin APIs remain compatible
- deterministic queue/self-test coverage
- Stage 13.7 1/2/4 CPU acceptance harness

Scope note: this is the Stage 13.7 framework foundation. USB HID report producers remain in the Stage 13.6 xHCI layer until their runtime service path is moved into the common driver/event pipeline; mouse/touchpad/touchscreen/pen/game-controller event types are defined now so later drivers target WovenInput rather than UI consumers directly.


## Final acceptance record

Stage 13.7 was accepted on 2026-09-30 from commit `7be1916e7feb661b7e29a247ad6e682e4406f68e` by GitHub Actions run `36708518537`.

The full release-validation suite and retained Stage 13.3 through Stage 13.6 hardware gates passed. Dedicated WovenInput results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.7-1cpu-*`, `stage13.7-2cpu-*`, and `stage13.7-4cpu-*`.

## Milestone status

**Stage 13.7 WovenInput framework foundation: COMPLETE at its declared integration boundary.**

The accepted milestone establishes the bounded device-independent input queue and PS/2 keyboard producer while preserving terminal input compatibility. USB HID-to-WovenInput runtime routing and additional device producers remain subsequent integration work and are not claimed by this acceptance.
