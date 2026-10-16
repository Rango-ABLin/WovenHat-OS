# Stage 20.14a two monitors through virtio-gpu audit — 2026-10-15

Roadmap Stage 18.9, Multi-monitor Desktop, first part: a display device
with several outputs.

## What changed

**`kernel/src/virtio_pci.rs`: the modern virtio 1.x PCI transport.**
- It walks the device's capability list for the vendor capabilities:
  common configuration, notification (with its offset multiplier), ISR
  status, and device configuration. These are mapped from their BARs.
- It resets the device and negotiates `VIRTIO_F_VERSION_1` with features
  the device offers.
- It sets up 16-descriptor split queues in one DMA page each.
- `exchange` posts a two-descriptor request and response chain, notifies
  the queue, and polls the used ring, with a bound.

virtio-net keeps its legacy transport.

**`kernel/src/virtio_gpu.rs`: a 2D driver.**
- `probe` finds the device (1af4:1050) and reads the scanout count from
  device configuration.
- `display_info` returns each head's reported mode.
- `set_mode` creates a B8G8R8X8 resource, the kernel framebuffer's
  little-endian 0x00RRGGBB layout. It attaches backing memory in 16-page
  runs and binds the resource to the scanout.
- `present` transfers to the host and flushes.

Both modules are compiled only for `stage20-14-test` until the extended
desktop (20.14b) uses them.

## Gate

`[S20.14] two monitors: PASSED`. Harness:
`scripts/test-stage20-14-monitors.py`.

The machine is q35, with the standard VGA as boot console and
`virtio-gpu-pci,max_outputs=2`. The kernel brings the GPU up and gives both
heads a 640x400 resource. QEMU reports head 1 as disabled, because no UI is
attached; binding a resource to it enables it.

The kernel draws:
- on head 0, an exact gradient: R = x·255/639, G = y·255/399, B = 0x40;
- on head 1, a 16-pixel checkerboard.

The harness captures each head from the host (QMP `screendump`, device
`gpu0`, head 0 or 1) and requires every one of the 2 × 256,000 pixels to
match.

## Evidence (`audit-artifacts/`)

Stage 20.14 on 1/2/4 CPUs, all passing: `stage20.14-1cpu-1792018526128537300`, `stage20.14-2cpu-1792017952477129400`, `stage20.14-4cpu-1792018036357418900`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Head 1 never presented:** all 256,000 of its pixels differ.
- **R8G8B8X8 format instead of B8G8R8X8:** every pixel of both heads has
  red and blue swapped.
- **Transfer replaced by a flush:** the device answers the command with an
  error (0x1203).
- **Both resources bound to head 0:** head 0 shows the checkerboard, and
  head 1 keeps the default 640x480.

After the restore, the gate passes again.

**Lint.** Warning-denying Clippy passes for production,
`stage20-14-test`, the Stage 13 features that share the PCI bus-master
helper, and host `--all-targets`.

## Limits

- **Synchronous commands.** Commands complete by polling the used ring,
  not through interrupts.
- **No cursor.** The cursor queue is not set up.
- **Fixed mode in the gate.** The resource size is chosen by the driver;
  mode changes reported by the host are not followed.
- **Not yet presenting the desktop.** The desktop compositor does not drive
  the heads yet; that is Stage 20.14b.
