# Graphics functional controls and GPU boundary audit — 2026-10-19

## Accepted software/2D graphics track

All ten packaged desktop clients have bounded functional pointer controls
accepted through QEMU's PS/2 input path, WovenShell's `WPMV`/`WPBT` protocol,
and the client shared-surface repaint path. Each client passed on 1/2/4 CPUs;
the per-client audit documents record geometry, semantics, serial markers, and
frame deltas.

The accepted track includes WovenGraphics software rendering, Canvas surface
uploads, WovenShell compositor/window interaction, bounded resize negotiation,
pointer forwarding, and functional client controls.

## GPU boundary

The VirGL lifecycle gate passed with fresh evidence:

- `audit-artifacts/stage20.14-1cpu-1792446089134374600`
- `audit-artifacts/stage20.14-2cpu-1792446146854719200`
- `audit-artifacts/stage20.14-4cpu-1792446172361578000`

Those runs prove capability discovery, context/resource setup, bounded command
submission, and teardown. They do not prove a visible GPU-backed frame. The
tested QEMU 3D/blob resource-to-scanout path remains rejected, so software/2D
scanout is intentionally retained as the production fallback.

Physical GPU qualification and USB HID qualification require separate hardware
test evidence and are not claimed by the QEMU gates.
