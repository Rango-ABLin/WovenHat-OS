# WovenHat Overview desktop: M2-M5

2026-11-03. Covers M2-M5 of `CLAUDE-START-HERE.md`'s milestone table:
the Overview top bar (clock/calendar popover, system menu), the
Activities overview (search, window cards, app grid), the Dash/window
switcher/snap/notification placement, and motion/high-contrast/i18n/
accessibility. M1 (the behavior-preserving `main.rs` module split) is
covered separately in `docs/agent-coordination.md`.

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

## M3: the Activities overview

### What M3 delivers

The Activities button now opens a real full-screen overview
(`Desktop::overview_area`, `Desktop::paint_overview_content`) instead of
M2's interim bridge to the small launcher panel. `panel_area` (the
Taskbar layout's small panel) and `overview_area` are mutually exclusive
on `self.layout`; both are driven by the same shared
`launcher_open`/`query`/`selected`/`filtered()` state the launcher
already had, so `toggle_launcher`, `launcher_key`, `filtered`, and
`launch` needed **no changes at all** -- confirmed each was already
layout-agnostic (no hardcoded position, no `taskbar_top()` dependency)
before relying on that, the same verification discipline M2 used for
`paint_qs_content`/`paint_center_content`.

Layout, top to bottom inside `overview_area`:

1. **Search field** (`overview_search_area`): shows "Type to search" when
   empty, or the typed query otherwise, via the existing `label()`
   truncation helper.
2. **This workspace's window cards** (`overview_thumb_area`, one per
   `Desktop::entries()` result, up to `MAX_WINDOWS`): a title-card
   placeholder per window -- a rounded, outlined tile with the window's
   title -- not a live thumbnail. **Confirmed before choosing this**: grepped
   `libwoven::lib.rs`'s `syscall::graphics_compositor_blit_*` family; only
   1:1 region/keyed/damage blits exist, no scaled variant, so a live
   thumbnail would need a new compositor capability. Per
   `CLAUDE-START-HERE.md`'s own instruction ("if the graphics ABI has no
   scaled blit, document it and render title-card placeholders rather
   than adding kernel capability without review"), used the placeholder
   and documented it here instead. When no windows are open, shows "No
   open windows on this workspace" rather than an empty gap.
3. **App grid** (`overview_cell_area`, row-major, column count computed
   from the overview's width): each cell reuses `icons::draw_app_icon`
   directly, including its existing `selected` parameter for the outline
   -- this **is** the "visible focus ring" the design brief asks for,
   with zero new drawing code. Grid labels strip a leading "Woven" prefix
   (the icon already carries that branding) so longer names like
   "WovenCalculator" fit the cell without truncating to "WovenCalcula".
   `MAX_APPS` (64) always fits the rows a typical desktop height offers
   at the chosen cell size without real paging; a future larger catalog
   would need actual paging logic, not assumed here.

Click handling (`handle_input`, gated `self.layout == ShellLayout::
Overview`): a window card focuses that window and closes the overview
(`focus`+`toggle_launcher`, areas unioned so both repaints land); a grid
cell launches that app (`launch`, which already closes the overview
itself); anywhere else closes it. Keyboard (`launcher_key`, already
shared, unchanged): `Tab` cycles the selected result, `Enter` launches
it, `Backspace`/printable characters edit the query.

### A design-brief requirement the keyboard ABI can't deliver

Before implementing "Esc closes" and "Tab/arrow keyboard navigation"
(both named explicitly in `CLAUDE-START-HERE.md` section 3.3), checked
`libwoven::Key` (`libwoven/src/lib.rs`): it has exactly `Char`, `Enter`,
`Backspace`, `Tab`, and `F1`-`F4` -- **no `Escape` and no arrow-key
variants at all**. Adding either would mean extending the kernel's own
keyboard-scancode-to-`Key` translation, a kernel-ABI change that
AGENTS.md says to stop and ask about rather than add unreviewed. Did not
add it. Instead:

- **Close**: click-outside (implemented) or the Activities button/F1
  toggle (already closes when open, pre-existing behavior) -- genuinely
  equivalent functionality to Esc, just not that literal key. This
  mirrors the brief's own stated fallback for the unavailable Super key
  ("the fallback is F1 and a hot corner").
- **Keyboard navigation**: `Tab`-cycling through filtered results is
  fully implemented (pre-existing `launcher_key` logic, unchanged); arrow
  keys are not, and can't be, until a separate, reviewed change extends
  the `Key` ABI.

### Bug found via the screenshots themselves, not build/clippy

The qualification script's F1+type+Enter sequence already existed before
M3 (it was testing the state machine, not the visuals). First pass of
the new `activities-overview.ppm` capture used a blind `capture_frame` +
fixed 0.5s sleep; two consecutive runs produced two *different* wrong
screenshots (one showing a stale lock-screen frame, one showing no
visible change at all) while the shell's own serial-log markers
(sign-in/launch/window-open) succeeded both times -- diagnosed as the
capture racing the shell's own up-to-250ms idle frame clock (documented
in `main.rs`'s module doc) under QEMU/TCG timing variance, not a shell
defect. Fixed by switching to `capture_until_changed` against the pre-F1
baseline frame, the same retry-until-materially-different pattern the
script already uses for the post-launch screenshot, instead of guessing
at a longer fixed sleep.

Once the capture itself was trustworthy, it caught one real (if purely
cosmetic) issue: `MAX_APP_NAME`-length catalog names truncated in the
grid ("WovenCalculator" -> "WovenCalcula"). Fixed with the "Woven"-prefix
strip described above.

### Verification

- `cargo build`/`cargo clippy --target x86_64-unknown-none -- -D
  warnings` clean inside `apps/woven-shell` throughout.
- `scripts/test-desktop-qualification.py --cpus 1 --frame-timeout 15
  --input-timeout 40` (generous timeouts -- see below): reaches the same
  point as the accepted baseline (sign-in, overview capture, launch,
  window-open, pointer, all four workspace switches), serial log confirms
  `WovenFiles: window opened`/`window opened: Files`, then hits the
  pre-existing `keyboard_refresh` flake. `activities-overview.ppm`
  (`audit-artifacts/desktop-qualification-1793699470831967700/`) visually
  confirms: search placeholder, "No open windows on this workspace", and
  all 11 catalog apps in a clean 7-column grid with icons and corrected
  labels.
- **2 and 4 CPU: not completed.** Blocked by this machine's own memory
  exhaustion -- `Get-CimInstance Win32_OperatingSystem` showed under 1 GB
  free out of 7.6 GB total physical RAM, with QEMU unable to allocate a
  5 GB guest as a direct result ("cannot set up guest memory 'pc.ram'"),
  independent of disk space (33 GB free at the time) or stale QEMU
  processes. Not retried further pending more host memory being available
  (closing other running applications, or rerunning when the machine is
  less loaded); this is an honest gap, not silently skipped.
- Separately, this session saw two one-off QEMU/timing hiccups that did
  not reproduce on retry (a `WinError 10054`/guest-memory QEMU startup
  failure, and one boot that ran at roughly 2x normal tick-count latency
  and exceeded the qualification script's default `--frame-timeout 5`
  before reaching the Files-launch step) -- both environment variance
  under back-to-back QEMU runs, not code defects. The longer
  `--frame-timeout`/`--input-timeout` values used for verification above
  are a per-session workaround and were not written into the script's
  permanent defaults, which would mask real regressions for other runs.

## M4: Dash, window switcher, snap shortcuts, notification placement

### What was already there, confirmed by reading the code, not assumed

Before writing anything new, checked whether M4's window-management
requirements were already satisfied:

- **"Alt+Tab switcher with focus restoration."** `libwoven::Key` has no
  modifier-key concept at all (confirmed again, same ABI already
  documented for M3's Esc/arrow-key gap) -- `Alt+Tab` cannot be
  distinguished from plain `Tab` at the ABI level. Read `handle_input`'s
  existing bare-`Tab` handler (`InputEvent::Key(Key::Tab)`, outside the
  launcher) and `raise()`: `raise` rotates the focused slot to the end of
  `self.stack` (bottom-to-top, last = focused), so repeatedly focusing
  `self.stack[..stacked].first()` -- what bare `Tab` already does --
  cycles every window on the workspace in strict MRU order, exactly an
  Alt+Tab switcher's behavior. `focus()` already includes the taskbar/top
  bar in its returned dirty area and leaves window content untouched, so
  focus restoration (switching away and back repaints correctly) was
  already correct. **No code changed for this.**
- **Snap halves.** `handle_input`'s drag-release-at-the-screen-edge path
  already calls `layout::half(self.work_area(), x > 0)`, and
  `work_area()` was already made layout-aware in M2. Drag-to-edge snap
  already works correctly for both layouts. **No code changed for this
  either.** The keyboard variant ("Super/Alt+arrows") is blocked by the
  same missing-arrow-key `Key` ABI gap as M3's keyboard navigation --
  documented here rather than silently dropped, consistent with the
  established pattern.
- **Tiling (F2) and workspace keys (F3/F4).** Untouched; confirmed no
  M2/M3/M4 change in this session reaches `toggle_tiling`,
  `switch_workspace`, or `move_focused_to_next_workspace`.

### What's new: the Dash

A Dash row (`overview_dash_area`, `dash_apps`) between the search field
and the window-cards row: favorites (the same curated three-app set as
the existing desktop shortcuts, `DESKTOP_ICON_NAMES`, resolved through
the existing `desktop_icon_app` helper -- no new hardcoded app list) plus
any other app with a running instance (`Desktop::children`), deduplicated,
bounded to `DASH_MAX` (8). Icon-only, no label, distinct from the
labeled app grid below it. Clicking a Dash icon focuses a running
instance if one exists (found by matching a `children` entry's pid
against open windows' `owner`) or launches it otherwise, closing the
overview either way.

**Favorites persistence**: per `CLAUDE-START-HERE.md`'s explicit
constraint ("Persistent theme/accent/wallpaper writes need an authorized
per-user mechanism; there is no broad `/etc` write capability. Do not add
one to satisfy this feature"), favorites are the same fixed curated set
as the desktop shortcuts, not user-customizable. Deferred, not faked.

### Notification placement

`toast_area` is now layout-aware: Taskbar keeps its unchanged top-right
position; Overview centers the toast and drops it below the top bar
(`taskbar_top() + TASKBAR_HEIGHT + TOAST_MARGIN`) instead of the old
fixed `TOAST_MARGIN` from the screen's own top edge, which would have
rendered the toast partially behind the 24px top bar -- a real,
previously-unnoticed visual bug in the M2/M3 code this M4 pass fixed
while implementing the explicitly-requested "toast under the top bar
center" placement. DND and the lock-screen rule (notification remembered,
never shown) were already correctly implemented in the pre-existing
`NOTE` message handler (`if self.dnd || self.locked { ... no toast ... }`
while `remember()` always records it) -- confirmed by reading, no change
needed.

### Verification

`cargo build`/`cargo clippy --target x86_64-unknown-none -- -D warnings`
clean; `cargo test --test shell_input` passes. **No QEMU verification
this pass**: `Get-CimInstance Win32_OperatingSystem` showed under 1 GB
free out of this machine's 7.6 GB total physical RAM throughout, so QEMU
could not allocate a guest at all (not even far enough to reach the
earlier "cannot set up guest memory" point -- guest creation itself was
infeasible). Verified instead by: careful reading of every touched
function and its call sites before editing (documented above), a full
diff review after the edits, and reasoning through the overview's
vertical layout arithmetic (search + margin + Dash row + margin + thumb
row + margin + at least one grid row comfortably fits the smallest
screen height this codebase boots at). This is explicitly a lower bar
than M2/M3's QEMU-confirmed verification, and is reported as such -- not
a stage acceptance, and QEMU re-verification (1/2/4 CPU, matching M2/M3's
journeys) is still owed once this machine has enough free memory for a
guest boot.

## M5: motion, high contrast, i18n, accessibility

### Motion

Added `Desktop::overview_fade: Spring`, wired into the existing
`animating`/`animate`/`tick` frame clock exactly the way window fade-in
already was (same `Spring` type, same `is_settled`/`step` pattern) --
the overview now fades in (0 to 255 opacity) when opened under the
Overview layout, respecting `reduced_motion` (jumps straight to full
opacity instead of animating, the same convention window open/close
already used). Close is a **snap, not a fade-out**: `toggle_launcher`
already forces an immediate full-screen repaint on close (pre-existing,
for the launcher's 3D-projection-corner cleanup -- see
`panel_damage_area`'s doc comment), so a gradual fade-out would never
actually be visible once `launcher_open` is false and `overview_area`
reads empty; animating toward 0 regardless would just keep the 20ms
frame clock running for ~300ms after the panel is already gone, for no
visible benefit. Documented as a deliberate asymmetry, not an oversight.

Implementing the fade required one structural change: `paint_overview_
content`'s background was `fill_rounded_blended` (rounded corners, like
the smaller floating panels); switched to a plain `canvas.blend_rect`,
since the overview spans most of the screen below the top bar rather
than reading as a small floating card, and a non-rounded fill has no
transparent corner pixels -- which matters because `draw_blended`
(`Painter`, already existing, used unchanged) blends the *entire*
rendered canvas at a uniform alpha with no keyed-transparency handling;
a rounded corner's untouched pixels would have blended in whatever stale
tile-buffer content was left from a previous frame.

### High contrast

Every other new surface (top bar, overview panel background, search
field fill, window cards, grid text) was already routed through an
existing `theme_*` helper (`theme_taskbar`, `theme_panel`, `theme_text`,
`theme_accent`, `theme_entry`) and so was already high-contrast-correct
without any M5 change -- confirmed by reading `paint_top_bar_content`/
`paint_overview_content` fully, not assumed. The one gap: the search
field's "Type to search" placeholder and the "No open windows" message
used a literal muted gray (`rgb(150, 160, 175)`), not a `theme_*` call.
Added `theme_muted()` (`tokens.rs`): the same muted gray normally, but
pure white in high-contrast mode -- deliberately *not* a dimmer shade of
`theme_text`, since high-contrast mode's whole point is maximum
legibility over a subtle "this is secondary text" visual cue, the same
tradeoff `theme_accent`'s own high-contrast branch (plain yellow) already
makes elsewhere.

Dash and app-grid icon colors (`icons::icon_colors`, per-app brand
colors) are **not** high-contrast-aware -- confirmed this is pre-existing
behavior shared by every caller of `draw_app_icon` (the desktop
shortcuts, the legacy launcher panel, now the overview too), not
something this work introduced or should silently "fix" by changing a
widely-depended-on existing function's behavior outside this pass's
scope.

### i18n

Renamed the lock screen's `fill_lock_catalog`/`lock_catalog`/
`LOCK_CATALOGS`/`LockCatalogs` to `fill_shell_catalog`/`shell_catalog`/
`SHELL_CATALOGS`/`ShellCatalogs` (a mechanical rename only -- every
existing English/Chichewa key and value is byte-for-byte unchanged, the
doc comment's note that "English values are byte-for-byte the literals
every already-accepted lock-screen gate expects" still holds) since the
one `Catalog`-per-language mechanism now serves the top bar and overview
too, not just the lock screen. Added three new keys (`activities`,
`type_to_search`, `no_open_windows`) in both languages, well within
`libwoven::i18n::MAX_TRANSLATIONS` (32; 12 used). The Activities button's
width (`start_area`) and its label (`paint_top_bar_content`) both now
resolve the same catalog lookup, so the button is sized correctly for
whichever language's string is actually drawn, not hardcoded to the
English string's width.

### Accessibility: keyboard reachability audit

Every M2-M4 control was checked against what `handle_input` actually
wires up, not assumed reachable:

- **Keyboard-complete**: Activities (F1, already existed), search typing
  and Tab-cycling through the app grid (`launcher_key`, already existed,
  unchanged), Enter to launch the selected result, closing via F1 again
  or clicking outside.
- **Mouse-only, no keyboard path at all**: window cards, Dash icons (both
  new in M3/M4 -- `Tab` only cycles the one list `launcher_key` already
  drives, the filtered app grid, and the ABI has no second list-cycling
  key to spare for the others), and, pre-existing and **not introduced
  by this work**, Quick Settings/the system menu button and the
  Notification Center/calendar popover, in *either* layout -- confirmed
  by grep that `toggle_qs`/`toggle_center` are called only from pointer-
  button handling, never from any `Key` match arm. This is the most
  significant accessibility gap found this session. It cannot be closed
  without a reviewed `Key`-ABI extension (a modifier bit, or dedicated
  keys for these specific panels) -- out of scope to add unreviewed per
  AGENTS.md, same category as the already-documented missing Esc and
  arrow-key gaps. Recorded here as a concrete, scoped follow-up: whoever
  picks up the `Key` ABI extension should close all of Esc, arrow-key
  grid/thumbnail/Dash navigation, and QS/Notification-Center keyboard
  access in one reviewed pass, since they share the same root cause.

### Verification

`cargo build`/`cargo clippy --target x86_64-unknown-none -- -D warnings`
clean; `cargo test --test shell_input` passes. **No QEMU verification
this pass**, same constraint as M4: this machine's free RAM stayed under
1 GB of 7.6 GB total throughout. The fade animation in particular is
inherently a perceptual/visual correctness question that static review
can reason about (the `Spring`/`draw_blended` mechanics are sound,
reused unchanged from the already-QEMU-verified window-fade path) but
cannot fully confirm without actually seeing it render across frames;
stated here plainly rather than claimed as verified. QEMU
re-verification for M2-M5 together (1/2/4 CPU) is owed once this machine
has enough free memory for a guest boot.
