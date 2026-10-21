# Stage 25.6 low-memory wake-page reservation audit (2026-10-21)

## Accepted scope

SMP early preparation now reserves two distinct usable pages below 640 KiB:
the existing AP startup page and a separate firmware-reachable wake page.
Both are selected before the frame allocator excludes low memory, and neither
is returned to general allocation. If firmware exposes only one suitable low
page, the wake page remains unavailable rather than aliasing the AP page.

The suspend-preparation token now requires the distinct wake page and carries
its physical address internally for the future trampoline builder. No code is
written to the page yet, and no FACS waking-vector field is changed.

## Verification

- Default production-kernel Clippy with `-D warnings` passed.
- The prior Stage 25.5 `system-apps` production-kernel Clippy, host
  all-targets Clippy, full host tests, and signed QEMU 1/2/4-CPU matrix remain
  green; the final low-page change is also covered by the next gate rerun.
- `git diff --check` passed with only the repository's CRLF conversion warning.

## Explicit limits

This stage does not build executable wake code, install a FACS vector, save or
restore CPU state, quiesce secondary CPUs for sleep, enter S3, or handle
firmware wake. Physical memory-map and firmware behavior still require real
hardware qualification.
