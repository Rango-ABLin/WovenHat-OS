# GFX-01 hybrid graphics baseline — 2026-10-22

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

- `cargo test --all --lib` — passed, 119 tests.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage20-14b-test -- -D warnings` — passed with existing shell/GUI image artifacts.
- `python scripts/test-stage20-14b-desktop.py --cpus 1` — failed before
  acceptance: the existing uncommitted 3D-default desktop work failed the
  taskbar-across-monitors pixel gate and exited 35. Evidence is under
  `audit-artifacts/stage20.14b-1cpu-1792680218104856300`.

## Gate boundary

GFX-01 is not accepted and GFX-02 is not started. The required graphics gate
must be green after the shared Claude 3D work is reconciled. The contract is
kept as an explicitly incomplete, reviewable change until the affected 1/2/4
CPU acceptance matrix passes.
