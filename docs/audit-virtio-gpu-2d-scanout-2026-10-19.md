# Standard virtio-gpu 2D scanout audit — 2026-10-19

## Result

The standard `virtio-gpu-pci` software/2D path is accepted as the visible
graphics presentation path. Each run booted the stage 20.14 kernel, created
bounded per-head resources, transferred and flushed the known frame, and
captured both heads through QMP. The harness verified every captured pixel
against the gradient on head 0 and checkerboard on head 1.

## Evidence

- 1 CPU: `audit-artifacts/stage20.14-1cpu-1792446632406296700`
- 2 CPUs: `audit-artifacts/stage20.14-2cpu-1792446677691577900`
- 4 CPUs: `audit-artifacts/stage20.14-4cpu-1792446718126918000`

All three runs report:

`Stage 20.14: PASS (... both heads verified pixel for pixel)`

## Boundary

This accepts the software/2D virtio-gpu presentation path, not hardware GPU
acceleration. The optional GL/VirGL lifecycle is separately accepted for
transport, while its visible scanout probe is rejected by QEMU with
`Device(4611)` (`0x1203`); see
`docs/audit-virgl-2d-scanout-boundary-2026-10-19.md`. Physical GPU and USB HID
qualification remain separate hardware-dependent work.
