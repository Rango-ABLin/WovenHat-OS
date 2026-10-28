# GFX-07 / GFX-08 graphics qualification — 2026-10-23

## Scope and truth boundary

This is a qualification plan and executable QEMU regression entrypoint, **not an assertion that GPU acceleration or production certification has passed**.

The 2026-10-19 `docs/audit-graphics-functional-controls-and-gpu-boundary-2026-10-19.md` records software/2D client-controls acceptance and a passing VirGL *lifecycle* gate, explicitly **not visible GPU-backed presentation**. Keep this distinction in release notes.

## GFX-07 regression entrypoint

Run from a clean repository checkout with appropriate QEMU and OVMF paths:

```sh
python scripts/test-gfx07-qemu-matrix.py --qemu /usr/bin/qemu-system-x86_64 --firmware /path/to/OVMF_CODE.fd --output audit-artifacts/gfx07-unique-run
```

The runner executes **nine existing acceptance invocations**:
- `scripts/test-woven-shell-qemu.py --stage 20.4` for CPUs 1, 2, 4;
- `scripts/test-stage20-14b-desktop.py` for CPUs 1, 2, 4;
- `scripts/test-stage20-15-session.py` for CPUs 1, 2, 4.

It refuses dirty/unknown HEAD, missing emulator/firmware or an existing evidence directory. It records command, exact SHA, log digests, exit statuses and failures in `manifest.json`. Nonzero exit, timeout and identified fatal markers fail the aggregate gate. Keep the underlying harness artifacts as well. A green result is **QEMU graphics regression only**, not graphics performance or physical device certification.

## GFX-07 remaining work

1. Capture actual frame-time distributions (p50/p95/p99), redraw damage, CPU utilization, memory peak, measured display geometry, driver identity, image SHA and firmware/toolchain versions from instrumented normal desktop sessions.
2. Set explicit hardware-dependent performance objectives with measured baseline, not fabricated 60-FPS claims.
3. Test clipping, resize, surface teardown, simultaneous client writes, focus switching, console switch and poweroff under concurrent scheduling.
4. Inject invalid surfaces, low-memory allocation failures, compositor termination, missed fences and device-loss paths; verify no stale mapping, livelock, privilege escalation or unrecoverable session.
5. Do not mark accelerated rendering complete until an **independently captured GPU-backed frame** is shown to match expected pixels and the device output path is demonstrated. A successful GPU context or command submission alone is insufficient.
6. Confirm QEMU GPU 2D presentation and CPU framebuffer fallback as separately identified backends. Track native display driver support as experimental until independently qualified.

## GFX-08 release gate: all required

- [ ] GFX-07 full fresh 1/2/4 CPU matrix passing from one frozen source SHA.
- [ ] Reliable normal UEFI boot to login/desktop and F12 recovery with real keyboard/mouse events.
- [ ] Real physical graphics output on a named laptop and named desktop SKU, with firmware, PCI IDs, GPU driver, resolution, display outputs, GPU/fallback classification and captured evidence.
- [ ] GPU failure/recovery and software fallback (only where framebuffer validity is proven).
- [ ] Multi-monitor, resolution change, session security, clipboard/screenshot permissions, full lock/unlock and teardown tested.
- [ ] Performance data measured and performance budget accepted on designated reference hardware.
- [ ] Negative security tests for surface ownership, buffer bounds, lifetime, mapping and DMA isolation where applicable.
- [ ] Device compatibility limitations explicitly documented; no unsupported Intel/AMD/NVIDIA claims.
- [ ] Critical/high review findings resolved; independent review logged as reviewed or PENDING (never invent Claude approval).
- [ ] Release-quality artifacts, hashes, source/toolchain identity, no contradictory FAILED/panic text; human approval before production claim.

## Status

**GFX-07: NOT YET ACCEPTED** by this document. Runner is added but its execution results must be collected.

**GFX-08: BLOCKED** pending device, GPU presentation, security and performance evidence.

Preserve the existing graphics API, SMP, WovenGuard and normal desktop startup. Make changes in small reviewable commits, and never bypass a failing gate.
