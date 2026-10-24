# Input queue reliability audit — 2026-10-22

## Scope

This iteration hardens the shared `WovenInput` queue used by the PS/2
keyboard and mouse IRQ paths and by WovenShell. It does not claim physical
USB HID or virtio-input qualification.

## Change

Pointer motion is a lossy stream, so adjacent queued motion events are
coalesced with saturating deltas. If the bounded queue is full, a critical
event (keyboard key, button, scroll, touch, pen, or controller event) evicts
the oldest queued pointer-motion event before insertion. If no motion is
available, the event is rejected and the existing drop counter remains
monotonic. The queue remains fixed-capacity and allocation-free on IRQ paths.

## Verification

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-7-test -- -D warnings` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo test --all --lib` — passed (119 library tests).
- `python scripts/test-hello-gui-compositor-qemu.py --stage 19.12 --cpus 1` — passed; the real i8042 mouse and keyboard loopback reached the focus queue and the compositor clients, exit 33.
- The Stage 13.7 QEMU invocation was started, but its generated evidence
  directory was empty and no acceptance marker was available; this iteration
  therefore does not claim a new Stage 13.7 acceptance result.

## Limits

This fixes bounded-queue starvation under pointer load. It does not add a
virtio-input driver, USB physical-device qualification, PS/2 hot-plug or
controller recovery, or randomized IRQ/device fault injection. Those remain
separate roadmap work.
