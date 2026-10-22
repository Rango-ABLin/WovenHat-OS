# Stage 28 installer UI/state model audit - 2026-10-22

## Scope

This slice adds `scripts/installer-ui-model.py` and
`tests/test_installer_ui_model.py`. It is a pure state model for a future
installer UI, not a graphical application and not an installer executor.

## Implemented behavior

- Screens/events cover welcome, target selection, partition-plan review,
  user/timezone/keyboard/network settings, progress, cancellation, success,
  and failure.
- Only the ordered flow is legal: welcome, target, plan review, settings,
  progress.
- Target selection requires caller-provided `safe: true` and
  `ambiguous: false`; the model does not independently inspect a disk.
- Summaries and messages are bounded: 128 characters for target/plan/settings,
  256 for failure messages, and 128 for progress messages.
- Control characters are rejected. Progress is limited to 0..100 and cannot
  move backward.
- Progress and success require a separate `confirm_install` event carrying
  `confirmed: true`. This is an explicit future-operation gate, not execution.
- Cancellation is two-phase (`request_cancel`, then `cancelled`). Failure and
  cancellation are terminal and reject subsequent events.

## Validation

The focused installer host suite passes, including the existing package,
ISO-probe, and plan-validator tests plus the new UI-model tests. Python
compilation checks and `git diff --check` pass.

## Explicit limits

The model performs no disk or filesystem I/O; it invokes no discovery,
partition, format, mount, bootloader, subprocess, or graphical-rendering
operation. It does not create an ISO, write a target, roll back installation,
or claim graphical installer, real installation, physical hardware, or release
certification support. A future executor must independently validate the plan,
recheck target identity, and implement power-loss-safe semantics before using
the confirmation boundary.
