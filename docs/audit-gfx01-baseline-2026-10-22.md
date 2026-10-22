# GFX-01 hybrid graphics baseline - 2026-10-22

## Source and interpretation

The supplied Hybrid Graphics Architecture specification is treated as the
graphics direction and incremental acceptance plan. Its GFX-01 instruction is
to inventory the existing stack and add only a smallest nonbreaking,
versioned abstraction; it explicitly does not claim implementation or
production readiness. This repository remains authoritative for current
behavior and acceptance results.

## Verified repository baseline

- `kernel/src/display.rs` owns the existing off-screen `Surface`, `Rect`, and
  bounded `Damage` presentation path.
- `kernel/src/compositor.rs` owns the kernel compositor surface, display lease,
  client-surface authorization, damage presentation, and virtio-gpu handoff.
- `kernel/src/virtio_gpu.rs` owns the current virtio-gpu 2D resource,
  scanout, transfer, and flush path, with a separately gated VirGL probe.
- `kernel/src/session.rs` and the existing input path preserve automatic
  WovenShell startup, sign-in, F12 diagnostic ownership, and keyboard focus.

## GFX-01 change

`kernel/src/graphics_abi.rs` adds a versioned backend-neutral contract for
typed pixel formats, generation-bearing surface/fence/output IDs, bounded
damage presentation requests, renderer operations, display modes, and bounded
backend recovery. It does not create a second surface store, compositor,
renderer, or scanout path; later stages must adapt the existing modules.

## Verification

- `cargo test --all --lib` - passed, 119 tests.
- `cargo clippy --all-targets -- -D warnings` - passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage20-14b-test -- -D warnings` - passed with existing shell/GUI image artifacts.
- `python scripts/test-stage20-14b-desktop.py --cpus 1/2/4` - passed after
  the taskbar projection correction; evidence is listed below.

## GFX-01 completion

The shared 3D taskbar projection was corrected so its receded edge remains
attached to the full primary-monitor width while retaining its depth
gradient. The fixed 23/20 compensation is the inverse of the 30-on-200
perspective tilt.

The affected graphics matrix passed:

- `audit-artifacts/stage20.14b-1cpu-1792680626117557900`
- `audit-artifacts/stage20.14b-2cpu-1792680764038570600`
- `audit-artifacts/stage20.14b-4cpu-1792680864231737200`

GFX-01 is accepted. GFX-02 is not started; CPU renderer implementation and
its separate 1/2/4 CPU acceptance are the next gate.
