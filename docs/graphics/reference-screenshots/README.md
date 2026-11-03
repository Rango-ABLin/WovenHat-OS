# Reference screenshots

Approved-correct captures to diff future renders against with
`scripts/compare-against-reference.py`, closing the Phase 2/9 gap in
`docs/graphics/modernization-tracker.md` (both phases' acceptance criteria
ask for automated pixel/tolerance comparison against reference images, not
just manual inspection).

## Convention

- One file per named scene, `<scene>-<capture-method>-<width>x<height>.png`.
- Commit the image itself (small, bounded size — these are 640x480 or
  smaller desktop captures, not photographs).
- When a change intentionally alters a captured scene's appearance
  (a palette change, a new icon, a layout change), **replace the reference
  deliberately in the same commit as the change**, with a commit message
  explaining why the new appearance is correct. Never let a reference go
  stale silently — a stale reference makes the comparison tool worse than
  useless (false confidence).
- Capture via QMP `screendump` with explicit `device`/`head` arguments
  when targeting a specific display (see
  `docs/graphics/modernization-tracker.md`'s Phase 6 entries for why this
  matters — an untargeted screendump silently captures the wrong device).

## Current references

- `sign-in-virtio-gpu-640x480.png` — the WovenHat sign-in screen
  (navy-to-teal wallpaper with the diagonal weave motif, account picker,
  PIN prompt), captured 2026-11-03 through the virtio-gpu 2D presentation
  path (`RUN-DESKTOP.ps1 -Display virtio-gpu`, QMP `screendump` targeting
  `device='gpu0', head=0`). This is the first confirmed-correct capture of
  the virtio-gpu rendering path in this project's history (see the Phase 6
  tracker entries) and doubles as evidence that the wallpaper repalette
  (commit `3af7d167`) renders correctly through that path too.
