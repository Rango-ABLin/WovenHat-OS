# 2D rasterization and vector geometry — Stage 04 evidence

Status: **ACCEPTED for software renderer scope**
Date: 2026-10-25

The current CPU renderer is implemented in `libwoven::graphics` and the
kernel graphics ABI. It includes bounded paths, deterministic quadratic
flattening, rounded rectangles, antialiased and compound-path fills, clipped
gradients, fixed-point transforms, alpha/source-over blending, image
resampling, and safe clipping. The kernel `CpuRenderer` retains generation-
checked surfaces, bounded geometry, compositing, fences, and stale-surface
rejection.

Evidence:

- Host renderer tests are included in `libwoven/src/lib.rs`; the full
  `cargo test -p libwoven --quiet` suite passes with 147 tests.
- The `gfx02-test` real-QEMU CPU renderer gate passed at 1/2/4 CPUs:
  `target/gfx02-cpu-renderer-1cpu-debug`,
  `target/gfx02-cpu-renderer-2cpu-debug`, and
  `target/gfx02-cpu-renderer-4cpu-debug`.
- Tests cover clipping, gradients, transformed rectangles, path flattening,
  alpha blending, antialias coverage, compound contours, rounded corners,
  bounds rejection, and stale surface generations.

Stage 04 is accepted for the bounded software/UEFI/QEMU renderer. GPU
acceleration, professional shaping, and physical display qualification remain
later gates.

