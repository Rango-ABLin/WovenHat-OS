# Stage 20.14b desktop across two monitors audit — 2026-10-15

Roadmap Stage 18.9, Multi-monitor Desktop, completed on top of the Stage
20.14a virtio-gpu driver.

## What changed

**Compositor (`kernel/src/compositor.rs`).**
- `attach_gpu_outputs` makes a virtio-gpu with two or more scanouts the
  display. Every head runs at head 0's mode (at most 1280x800), placed
  left to right; a single-head GPU is left alone.
- The surface is resized to the combined geometry. `MAX_WIDTH` is now
  2560, enough for two 1280-pixel monitors.
- `present_state` presents to the GPU heads when there are any, otherwise
  to the boot framebuffer:
  - each damaged region, in physical pixels, is split by head;
  - each part is scaled into that head's backing rows (`Head::write_row`);
  - only that rectangle is transferred and flushed
    (`Gpu::present_rect`).
- Activation does not require the terminal framebuffer when a GPU drives
  the display.
- `outputs_for` reports each monitor in surface coordinates.
- A normal boot attaches the outputs after driver bring-up and logs
  `[DISPLAY] virtio-gpu: N monitors`.

**Syscall 207, `GraphicsCompositorOutputs`.** It returns the monitors left
to right. 206 is Codex's `WaitAnyObjects`, so audio recording is now
reserved 208. libwoven exposes it as
`compositor_outputs`.

**WovenShell.**
- The first monitor is the primary: the taskbar, panels, launcher, new
  windows, the lock prompt and tiling use its width.
- The wallpaper, pointer, full repaints and window dragging cover the
  whole desktop (`desktop_width`).
- Windows are drawn on every monitor.
- An edge snap happens only at the desktop's outer edges. The right edge
  snaps into the last monitor.
- Changing the scale (Quick Settings) refreshes the monitors.

## Bugs found by the gate

- **Drops on the second monitor snapped back to the primary.** The edge
  snap compared the drop point with the primary monitor's width, so any
  drop on the second monitor counted as the right edge. The 2-CPU run
  failed with "window snapped".
- **Windows on the second monitor were clipped.** The shell clipped
  windows to the primary monitor's width.
- **The scale toggle reset the primary.** It set the primary width to the
  whole desktop.
- **The gate's own drag check raced the shell (gate defect).** It read the
  live surface, which the shell writes in several pieces per frame. On
  4 CPUs it once saw a frame half drawn and failed. The check now reads
  only between completed frames: `compositor::between_frames` evaluates it
  under the compositor lock, and only when all damage has been presented.
  On failure it reports which half failed (`injected`, `arrived`,
  `stayed`).

## Gate

`[S20.14B] desktop across monitors: PASSED`. Harness:
`scripts/test-stage20-14b-desktop.py`.

The machine is q35, with the standard VGA as boot console and a
two-output virtio-gpu. The kernel attaches the GPU outputs and starts the
shell. It then checks against its own rendering:
- the taskbar is on the primary monitor only;
- window A opens on the primary;
- A, dragged by its title bar one monitor-width right, appears on the
  second monitor;
- A stays there for a second, so the drop is complete and no snap or
  resize follows. Both checks judge only completed frames;
- everything drawn has been presented.

The kernel then prints six sample pixels with their physical head
coordinates. The harness captures both heads from the host (QMP
`screendump`) and requires each sample to match.

## Evidence (`audit-artifacts/`)

Stage 20.14b on 1/2/4 CPUs, all passing: `stage20.14b-1cpu-1792044894887039300`, `stage20.14b-2cpu-1792044145193099700`, `stage20.14b-4cpu-1792044420265005400`

Three extra 4-CPU runs also passed, and the gate passed again on 1 CPU
after the negative controls restored the source.

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each failed the gate as intended:
- **Heads never written:** the captures are black.
- **Head offset ignored:** head 1 shows head 0's content.
- **Drag clamped to the primary monitor:** A never reaches the second
  monitor.
- **Taskbar across both monitors:** the taskbar check fails.
- **Edge snap at the primary's edge** (the original bug):
  - Before the "stays" check, this control was not caught: on 1 CPU the
    gate saw A after the moves but before the release was processed.
  - With the frame-aligned check, it is caught: no completed frame shows
    A on the second monitor.

**Regression**, all passing on 1/2/4 CPUs, against the final tree (this
stage plus the presentation-performance fixes):
- Stage 20.14a;
- Stages 20.1–20.13;
- Stages 17.1, 18.1, 19.1, 19.3 and 19.10;
- hello-gui, and the compositor gates 19.2, 19.4–19.9 and 19.11–19.14;
- the Stage 13.12 production boot;
- the release gate (`scripts/test-release.py`): all 98 checks pass. The first attempt failed one
  check, `security-1`, because the disk filled while compiling; it passed
  after build space was reclaimed. Evidence: `audit-artifacts/release-validation-1792056031/`.

20.7, the spring-fade gate, failed twice on 1 CPU while this stage was
being validated. The cause was over-drawing in the shell and the kernel,
which left the fade at about 5 fps under emulation. It is fixed separately;
see the
[presentation performance audit](audit-presentation-performance-2026-10-15.md).
After the fix, 20.7 passed six consecutive 1-CPU runs.

**Lint.** Warning-denying Clippy passes for:
- production;
- every Stage 20 feature, including 20.14 and 20.14b;
- `stage13-12-test`, `stage13-3-test`, `stage19-10-test` and
  `stage19-14-test`;
- woven-shell, hello-gui and host.

## Limits

- **Monitor layout.**
  - Monitors share one mode and sit side by side.
  - Hot-plugging monitors and per-monitor scale are not handled.
  - A normal boot uses a virtio-gpu only when it has two or more
    scanouts.
- **No native GPUs.** Physical multi-monitor needs native GPU drivers.
- **Terminal limit on a normal boot.** If the firmware's boot framebuffer
  is the same virtio-gpu, the text terminal cannot draw once the desktop
  releases the display.
- **Polled commands.** Presentation commands are synchronous and polled.
