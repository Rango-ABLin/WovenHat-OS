# Design: roadmap Stage 18.3 — Window Experience

Status: proposal, 2026-10-13. Not started. Repo stages 20.4–20.8 below are
proposed numbers, following 20.1–20.3 (roadmap 18.1–18.2).

The roadmap asks for WovenHat's own visual identity: dynamic depth, subtle
translucency, adaptive surfaces, smooth physics-based animation, workspace
transitions and intelligent tiling. This document maps each item onto the
primitives that exist after Stage 20.3, names the kernel work each one still
needs, and orders the work so that every step has an acceptance gate of the
same kind as 20.1–20.3: the kernel reads the compositor surface back and
compares it with its own rendering, under real i8042 input.

## Where things stand

The desktop (`apps/woven-shell`) recomposes exposed rectangles bottom to top
from its own software layers and clients' shared surfaces, using region
blits (syscalls 120–121). Three gaps limit everything in 18.3:

1. **Windows are tiny and fixed.** Client surfaces are capped at 320×240,
   four per owner and sixteen in total. A window cannot be moved, resized or
   closed from the shell.
2. **Translucency is whole-surface only.** `GraphicsCompositorBlend`
   (114/115) blends an entire surface. The repaint model needs every layer
   clipped to the exposed rectangle, so translucent layers need region
   blends.
3. **No motion model.** `FrameClock` paces frames, but nothing interpolates
   state between them.

## Proposed stages

### 20.4 — Windows that move, resize and close

The prerequisite for everything else; nothing in 18.3 reads well with
fixed 320×240 windows.

- **Kernel: per-owner pixel budget.** Replace the 320×240 dimension cap with
  a pixel budget. The proposed bounds are about 1 M pixels (4 MB) per owner
  and 4 M pixels in total, with dimensions up to the compositor's bounds.
  Every allocation path must keep its failure modes: owner full, system
  full, out of range. The gate must show that a budget-exceeding create is
  refused, and that release returns budget.
- **Shell:** drag a window by its title bar, with a pointer press, motion
  and release state machine. Add a close control that sends the owner a
  `WCLS` request; the window goes when the surface does, as today.
  Resizing is a protocol request (`WSIZ`, the new size) that the client
  answers with a new surface; the shell never resizes a client's pixels.
- **Gate:** drag A over B and back, checking that exposed regions repaint
  correctly at each step. Close B through its control. Check that a client
  that ignores `WCLS` keeps its window.

### 20.5 — Depth

- **Kernel: region blend.** Add syscalls 122/123,
  `GraphicsCompositorBlendRegion(/Shared)`: the region blit with an opacity
  in place of the copy. That needs four values in three arguments, so the
  opacity rides in the destination word's top byte. The x and y offsets
  would drop from 32 bits to 24, which is ample for coordinates bounded by
  1920×1200; the ABI note must say so.
- **Shell:** a shadow under every window, offset down and right and drawn
  as stacked translucent bands from a shadow tile in the staging surface,
  so the edge is soft. The focused window gets a deeper shadow than the
  others ("elevation"). Unfocused windows can be dimmed slightly with a
  translucent veil over their client area.
- **Gate:** shadow pixels at known offsets equal the blend formula,
  recomputed by the kernel. Changing focus changes shadow depth on both
  windows.

### 20.6 — Translucency and adaptive surfaces

- The taskbar, panel and toast become translucent over *windows*, not only
  over the wallpaper. Each is rendered into the staging surface and
  region-blended last. The taskbar today blends only over the wallpaper,
  and only because windows never overlap it.
- **Adaptive surfaces:** chrome tint derived from the wallpaper. The shell
  computes the wallpaper's mean colour, which is a closed form for the
  procedural wallpaper, and mixes it into the accent and taskbar colours.
  A future image wallpaper feeds the same function.
- **Gate:** a window under the taskbar shows through at the expected blend.
  Changing the wallpaper parameters changes the chrome tint to the
  recomputed value.

### 20.7 — Physics-based animation

- **libwoven: fixed-point spring.** A critically damped (or tunable)
  spring integrator, `Spring { position, velocity, target }` stepped once
  per `FrameClock` frame. It is fixed-point, because there is no FPU state
  management in userspace yet; that is to be confirmed before choosing.
  `skipped` frames are folded in, so motion stays on time when frames drop.
  Host unit tests pin convergence, no overshoot when critically damped,
  and determinism.
- **Shell uses it for:**
  - window open, a fade-in through region blends;
  - minimise to the taskbar entry, a slide-and-fade;
  - release after a drag (settle to a snap target).
- **Gate:** sample the animated property at fixed frames under a frozen
  frame count. The kernel recomputes the integrator. A negative control
  with the spring replaced by a jump fails the intermediate-frame checks.

### 20.8 — Intelligent tiling

- **Layout engine** in the shell, as pure functions with host tests:
  - snap to left or right halves and quarters when a window is dragged to
    an edge or corner;
  - a keyboard tiling mode: a key combination, needing modifier support in
    `woven_input` (today only printable keys, Enter, Backspace, Tab and F1
    arrive);
  - an auto-tiling mode that lays out all windows in a master and stack
    arrangement.
- "Intelligent" is kept concrete: tiling respects each window's minimum
  size from its `WOPN` request, and never shrinks a window below it.
- **Gate:** drag to the right edge, and the window fills the right half.
  Toggle auto-tiling with three windows: the frames land at the layout's
  computed positions, recomputed by the kernel.

### Workspace transitions

These need workspaces, which are roadmap 18.8. The transition itself is a
slide of every window on the outgoing and incoming workspaces, driven by
the 20.7 spring, and fits naturally into the 18.8 stage. They are recorded
here and not scheduled under 18.3.

## Cross-cutting requirements

- **Keyboard modifiers.** Tiling and window shortcuts need Shift, Ctrl and
  Alt (and a "super" key) in `woven_input` and the `InputPoll` encoding.
  That is an ABI addition to schedule before 20.8.
- **Pointer events for applications.** Today the shell consumes all pointer
  input. Forwarding motion and clicks inside a window to its owner,
  translated to window coordinates, is needed before any real application
  is usable. It could join 20.4.
- **Frame cost.** Region blends cost per pixel. A full-screen translucent
  layer at 640×400 is 256 K blends per repaint. Animations must repaint
  only the moving rectangle, and the gates should log repaint area per
  frame so regressions are visible.
- **Lock and teardown review** for every new kernel object, per AGENTS.md:
  the pixel budget accounting and the region blend's snapshot-then-lock
  pattern (the same as the region blit).

## Open questions for the owner

1. **Pixel-budget sizes** (20.4). These depend on the memory target for a
   desktop session; the QEMU gates run with 256 MB.
2. **Modifier keys.** Which combination WovenHat uses for window
   management.
3. **Visual direction.** Shadow depth, translucency levels and spring
   stiffness are design choices, not engineering ones. This proposal will
   use conservative values unless told otherwise.
