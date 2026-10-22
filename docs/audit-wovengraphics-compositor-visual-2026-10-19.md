# WovenGraphics compositor visual audit — 2026-10-19

## Result

Stage 19.2 passed on QEMU 1/2/4 CPUs. The production `hello-gui` application
rendered and presented through the compositor, and every run reported the
WovenGraphics visual frame, 2D frame, and software 3D frame as passed. The
same acceptance verified a retained window after application exit, zero exit
status, and owner-slot release.

## Evidence

- 1 CPU: `target/hello-gui-compositor-1cpu`
- 2 CPUs: `target/hello-gui-compositor-2cpu`
- 4 CPUs: `target/hello-gui-compositor-4cpu`

Each serial log contains:

- `[S16G] WovenGraphics visual frame: PASSED`
- `[S16G2] WovenGraphics 2D frame: PASSED`
- `[S16G3D] WovenGraphics software 3D frame: PASSED`
- `[S19.2] hello-gui through the compositor: PASSED`

## Boundary

This audit accepts software WovenGraphics rendering through the production
compositor and the standard virtio-gpu 2D presentation path. It does not claim
accelerated GPU-backed scanout; the VirGL lifecycle and visible-scanout
boundary are recorded separately.
