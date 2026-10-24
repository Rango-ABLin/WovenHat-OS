# Presentation and repaint performance audit — 2026-10-15

A fix to accepted Stage 19/20 behaviour, found by the Stage 20.7 fade gate.
It is not a roadmap stage.

## The defect

The Stage 20.7 spring-fade gate failed intermittently on 1 CPU. It requires
the window to pass through at least three intermediate opacities. Failing
runs saw `[0, 205, 251, 255]`, and even passing runs jumped, for example
`[0, 20, 217, …]`.

Profiling the shell during the fade showed why. The fade is driven by a
50 fps frame clock and the spring advances by the frames that elapsed, so
the timing was correct, but most frames were never drawn. Measurements are
TSC cycles per frame in the 1-CPU gate (unoptimised test kernel, TCG):

| Work per frame | Before | After |
|---|---|---|
| Fading frame: presented pixels | 167,424 | 52,224 |
| Fading frame: `compositor_present` | ~110–160 M | ~70–90 M |
| Window-open frame: shell repaint + present | ~1.8 G (≈1 M px) | ~60 M |
| Distinct opacities the gate sees | 4–6 | 10 |

Three causes, all over-drawing:

1. **The shell merged unrelated dirty areas into one bounding box.**
   - Opening a window repainted:
     - the new window, at the top left;
     - the window it unfocused;
     - the taskbar along the bottom;
     - the retiled windows.

     All of it went into one box over most of the screen.
   - A notification merged the toast with the Notification Center panel.
   - During a fade, the frame's animation area, the clock or toast, and the
     tray-icon damage were merged the same way.
2. **The kernel damage list collapsed early.**
   - `Damage` holds 16 regions and collapses to their bounding box when
     full.
   - The shell draws in 320×40 tiles, so a single repaint of one window
     filled the list with adjacent pieces of the same rectangle.
3. **Presentation wrote byte by byte.**
   - `present_scaled` handled each physical pixel separately: a division,
     a bounds check, a pixel-format match and three single-byte stores.
   - At scale 2 every logical pixel is four physical pixels.
   - The virtio-gpu path likewise read every physical pixel through
     `Surface::pixel`.

## Fixes

- **WovenShell (`apps/woven-shell/src/main.rs`).**
  - A `Dirty` set keeps up to four separate areas per frame. Areas merge
    only when their union costs no more pixels than the two apart; when the
    set is full, the new area merges into the area that grows least.
  - The main loop, window opening and notifications return `Dirty`, and
    each area is repainted on its own.
- **Damage (`kernel/src/display.rs`).**
  - `Damage::add` drops tracked regions the new one covers.
  - It merges regions whose union is exactly a rectangle, so no extra
    pixels are presented.
  - Collapse to a bounding box is unchanged: it still happens when the list
    is genuinely full of separate regions, and the existing collapse tests
    still pass.
- **Present (`kernel/src/display.rs`).**
  - `present_scaled` converts each logical row once into a staging buffer
    in the framebuffer's format. That buffer is copied into each of the
    `scale` physical rows it covers.
  - It never reads the framebuffer, because write-combined memory is slow
    to read back.
  - Pixel sizes other than 1–4 bytes are not drawn.
- **virtio-gpu present (`kernel/src/compositor.rs`).**
  - A new `Surface::scaled_row` builds each physical row once per surface
    row.
  - That row is written to every physical row it covers.

## Tests

New host tests in `tests/display_compositing.rs`:
- `scaled_presentation_matches_the_per_pixel_definition`:
  - compares `present_scaled` with a per-pixel reference;
  - covers RGB and BGR, 3- and 4-byte pixels, and scales 1–3;
  - uses regions that cross staging chunks and are clipped by the surface;
  - uses a mapping that ends partway through a row.

  A mutation that broke the scaling step was caught.
- `a_scaled_row_repeats_each_pixel_and_pads_with_black`.
- `damage_merges_regions_that_form_exactly_a_rectangle`: a tiled window
  repaint and a distant clock stay as two regions.
- `damage_keeps_regions_whose_union_would_add_pixels`.

## Evidence

All results are against the final tree (Stage 20.14b plus these fixes),
in one chain:
- **Lint.** Warning-denying Clippy passes for:
  - woven-shell and hello-gui;
  - every kernel feature;
  - host `--all-targets`.

  `stage16-1-test`, `stage19-app-catalog-test` and `system-apps` report
  needs-image, as before.
- **Host tests.** The host and libwoven tests pass.
- **Stage 20.7, the fade gate.** It passed six consecutive 1-CPU runs, and
  on 2 and 4 CPUs. Before the fixes it had failed two of five 1-CPU runs.
- **Regression on 1/2/4 CPUs.** All of these pass:
  - Stages 20.1–20.14, including 20.14b;
  - Stages 17.1, 18.1, 19.1, 19.3 and 19.10;
  - hello-gui;
  - compositor gates 19.2, 19.4–19.9 and 19.11–19.14;
  - the Stage 13.12 production boot.
- **The release gate.** `scripts/test-release.py` passes all 98 checks
  (`audit-artifacts/release-validation-1792056031/`). An earlier attempt failed
  `security-1` only because the disk filled during compilation.

Evidence is under `audit-artifacts/` and `target/` (per-harness serial
logs). The second failing 20.7 run before the fix is preserved as
`audit-artifacts/stage20.7-1cpu-flake-1792040542`. The first was
overwritten by a later run of the same harness.

## Limits

- **Optimisation level.** The gates boot an unoptimised (`dev` profile)
  kernel under TCG; release images use `--release`. A present still costs
  about 1,500 cycles per physical pixel there. Most of that is
  unoptimised code and emulated stores, not the algorithm.
- **Shell dirty set.** It keeps at most four areas. More separate changes
  in one frame merge by least growth.
- **Pointer motion.** It is still repainted per event, as before.
