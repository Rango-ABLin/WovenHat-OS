# WovenHat Overview desktop: M2 top bar

2026-11-03. Covers M2 of `CLAUDE-START-HERE.md`'s milestone table: the
Overview top bar, its clock/calendar popover reuse, and its system menu.
M1 (the behavior-preserving `main.rs` module split) is covered separately
in `docs/agent-coordination.md`.

## What M2 delivers

A `ShellLayout` enum (`Taskbar` | `Overview`) on `Desktop`, selected by a
new `--layout=taskbar|overview` flag. **Overview is now the default**,
replacing the bottom taskbar as the primary WovenHat desktop; `--layout
taskbar` is the explicit rollback path kept until M7 removes the legacy
chrome, per the user-authorized design in `CLAUDE-START-HERE.md`.

The Overview top bar (`Desktop::paint_top_bar_content`, in `main.rs`):

- **Left**: an "Activities" button, visually matching the legacy
  WovenStart button's rounded-gradient chrome but sized to its own label.
  For M2 it opens the same launcher panel the legacy `WovenStart`/F1
  shortcut does (`toggle_launcher`) -- a real, working stand-in, not a
  fake button, until M3 builds the actual Activities overview (search,
  workspace strip, window thumbnails, app grid).
- **Center**: the clock, centered (GNOME-style), opening/closing the same
  Notification Center state (`center_open`, `toggle_center`) as a
  calendar/notification popover that now drops *down* from the bar
  instead of opening *up* from the taskbar.
- **Right**: the workspace switcher (unchanged, its geometry was already
  derived from the system menu button's position) and a system menu
  button that reuses Quick Settings' exact state and actions (`qs_open`,
  `toggle_qs`, `qs_rows`, the same power/restart/scale/DND/tiling/
  lock/change-PIN/language/3D/reduced-motion rows), opening downward.

## Design approach: branch geometry, not behavior

Rather than writing a parallel, duplicated taskbar implementation, the
geometry functions several call sites already shared (`taskbar_top`,
`clock_area`, `badge_area`, `start_area`, `panel_area`, `qs_area`,
`center_area`, `work_area`) were each given a small `match self.layout`
branch. Everything that already went through those functions --
`handle_input`'s click hit-testing, `repaint_once`'s compositing,
`render_qs_3d`/`render_center_3d`'s area parameter, `close_panels`,
`toggle_qs`/`toggle_center` -- continues to work for both layouts
unchanged, because it never hardcoded a position itself. The two popover
content painters, `paint_qs_content` and `paint_center_content`, are
pure functions of an `Area` parameter already (confirmed by reading
them before reusing them) and needed no changes at all.

`render_desktop` is the one explicit dispatch point: it calls the legacy
`paint_taskbar_content`/`render_taskbar_3d` for `ShellLayout::Taskbar`, or
the new `paint_top_bar_content` for `ShellLayout::Overview`. The legacy
taskbar code path is **not called at all** when `--layout=overview` is
active (the default), and is untouched when `--layout=taskbar` is passed.

## Bugs found and fixed during verification

Four functions independently duplicated "available height above the
taskbar" arithmetic instead of calling the already-correct `work_area()`,
and broke once `taskbar_top()` started returning `0` for Overview:

- `open()` (handles `WOPN`): `!(1..=self.taskbar_top() - TITLE_HEIGHT)
  .contains(&height)` became `!(1..=-14).contains(&height)` -- an
  inverted, always-empty range, so **every** window-open request was
  rejected. Caught by the QEMU qualification run: "window open rejected"
  appeared where the baseline logs "window opened: Files".
- `resized()` (handles `WRSZ`): the identical pattern, would have
  rejected every resize reply.
- `toggle_scale()`: clamped every open window's `y` to a fixed
  `TITLE_HEIGHT`, which would have rendered windows under/behind the top
  bar after a display-scale change.
- `drag_to_pointer()`: didn't even call `taskbar_top()` -- it hardcoded
  `self.height - TASKBAR_HEIGHT` directly, so dragging a window in
  Overview mode would have still reserved a phantom bottom-taskbar-sized
  band instead of letting the window reach the real bottom of the screen.

All four now share one `(area_top, area_height)` computation (the usable
band's top edge and height, excluding the bar wherever it sits), matching
`work_area()`'s own logic. Caught only because the QEMU qualification
journey actually opens a window and reads back the serial log -- a build+
clippy pass alone would not have found this.

## Verification

- `cargo build`/`cargo clippy --target x86_64-unknown-none -- -D
  warnings` clean inside `apps/woven-shell` throughout.
- `scripts/test-desktop-qualification.py` at **1, 2, and 4 CPUs**: all
  three reach the same point as the known-good baseline (sign-in, launch,
  window-open, pointer clicks, all four workspace switches all succeed;
  serial log confirms `WovenFiles: window opened` and `keyboard
  refreshed` at every CPU count), then hit the pre-existing
  `keyboard_refresh` framebuffer-diff-timeout flake already documented in
  `docs/agent-coordination.md` as unrelated to any of this work
  (reproduces on the unmodified pre-change baseline too). Evidence:
  `audit-artifacts/desktop-qualification-{1793696849620925300 (1 CPU),
  1793697025294069600 (2 CPU), 1793697131547554400 (4 CPU)}/`.
- Screenshots (`desktop-signed-in.ppm`, `files-window-open.ppm` under the
  1 CPU evidence directory) visually confirm the designed layout: top bar
  with rounded "Activities" pill, centered clock, workspace switcher +
  system-menu button cluster on the right, and the Files window opening
  correctly positioned below the bar with no visual corruption.
- This is a structural-level verification (layout, window placement,
  popovers, all three CPU counts), not a `docs/stage-status.md` stage
  acceptance: the qualification script's own exit code is still a
  failure because of the keyboard_refresh flake, even though the failure
  point is identical to the accepted baseline's.

## Known gaps, deliberately deferred (not faked)

- **No 3D perspective presentation for the top bar yet.** The legacy
  taskbar has `render_taskbar_3d` (a tilted-card projection); the
  Overview top bar always renders flat for now, even when `three_d` is
  on. Windows/wallpaper/title bars still render in 3D normally; only the
  new chrome itself is flat. A follow-up pass can add the analogous
  projection once the 2D structure above is accepted.
- **No per-window entry list / dash in the top bar.** Clicking a taskbar
  entry to focus a window is explicitly skipped when `layout ==
  Overview` (see the comment at the `handle_input` guard) since the
  legacy `entry_area` geometry isn't meaningful there yet. M4 ("Dash,
  Alt+Tab, snap shortcuts") is where this gets a real Overview-native
  replacement.
- **No account name in the top bar.** The legacy taskbar shows the
  signed-in account's name next to the workspace switcher; the top bar
  does not yet. Minor, deferred rather than rushed.
- **System menu button glyph is a plain `=` character**, not a real
  icon, pending a later `libwoven::icons` pass.
- **No `top_bar.rs` module yet.** `paint_top_bar_content` and the small
  `ShellLayout`/`ACTIVITIES_LABEL` additions stayed in `main.rs` for this
  pass to minimize cross-module import churn while the geometry/behavior
  correctness was still being verified; splitting it out is reasonable
  follow-on work once more Overview-specific code accumulates (M3's
  Activities overview, M4's dash), per CLAUDE-START-HERE.md's "split
  main.rs into modules as you go."
