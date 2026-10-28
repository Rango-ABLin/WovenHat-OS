# VirGL 2D-on-GL scanout boundary audit — 2026-10-19

## Result

The VirGL capability/context/resource/command lifecycle remains accepted on
QEMU 1/2/4-CPU runs. A separate capability-gated probe attempted to use the
conventional 2D resource path on the GL-capable device, fill the known
gradient, and present it on head 0. QEMU rejected `SET_SCANOUT` with
`Device(4611)` (`0x1203`). Therefore this probe does not accept a visible
GPU-backed frame; it records the current QEMU/device boundary and preserves
the software/2D path as the authoritative fallback.

## Evidence

- Initial visible-scanout probe failure:
  `audit-artifacts/stage20.14-1cpu-1792446321007536000`
- Fresh capability-gated lifecycle acceptance, 1 CPU:
  `audit-artifacts/stage20.14-1cpu-1792446427459206700`
- Fresh capability-gated lifecycle acceptance, 2 CPUs:
  `audit-artifacts/stage20.14-2cpu-1792446460661458800`
- Fresh capability-gated lifecycle acceptance, 4 CPUs:
  `audit-artifacts/stage20.14-4cpu-1792446484234216200`

The fresh serial logs contain both the lifecycle PASS marker and the explicit
`VIRGL 2D scanout unavailable: Device(4611)` boundary marker. The ordinary
VirGL gate intentionally does not require the unavailable visible-scanout
marker. A future visible GPU acceptance must use a QEMU/device combination
that accepts the scanout operation or a physical GPU backend.

## Scope and limits

This audit covers QEMU VirGL probing only. It does not qualify physical GPU
drivers, USB HID input, or a visible 3D/blob scanout path. Those remain
separate hardware/device-dependent gates.
