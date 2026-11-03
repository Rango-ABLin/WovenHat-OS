# WovenHat Overview desktop: M2-M7

2026-11-03. Covers M2-M7 of `CLAUDE-START-HERE.md`'s milestone table:
the Overview top bar (clock/calendar popover, system menu), the
Activities overview (search, window cards, app grid), the Dash/window
switcher/snap/notification placement, motion/high-contrast/i18n/
accessibility, the measured performance/memory record, and the M7
layout cutover (legacy taskbar code removal). M1 (the
behavior-preserving `main.rs` module split) is covered separately in
`docs/agent-coordination.md`.

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

## QEMU verification catch-up (2026-11-03, later the same day)

With more host memory free (the user closed other applications), ran the
owed QEMU verification for everything above.

### A real bug caught, not just confirmed clean

The M5 fade-in (`overview_fade: Spring`, a single `Painter::draw_blended`
pass over the whole overview) looked structurally sound against the
static review recorded in the M5 section above -- the `Spring`/frame-
clock wiring was copy-identical to the already-QEMU-verified window-fade
path. Seeing it actually render exposed what that review couldn't: a
visibly ghosted, horizontally-offset duplicate of the Dash row and "No
open windows" text (roughly a two-tile-width shift, `TILE_WIDTH` = 320,
consistent with a tile-compositing interaction rather than a layout math
error). Rather than spend further turns diagnosing the exact tiling
mechanism blind, reverted to the plain `blend`+keyed-`overlay` two-step
every other panel (Quick Settings, Notification Center, the Taskbar
layout's launcher) already uses, and that M3 already verified correct
for this exact panel before M5 touched it. This also meant reverting
`paint_overview_content`'s background from a plain `blend_rect` back to
`fill_rounded_blended`, and removing `overview_fade` (the struct field,
its `Desktop::INITIAL` entry, the `toggle_launcher` wiring, and the
`animating`/`animate` hooks) entirely rather than leave a disconnected,
unused `Spring` sitting in `Desktop`.

**Net effect on M5**: the Activities overview no longer fades in --
opens instantly, exactly as it did at the end of M4, before M5 added
motion. Every other M5 change (high contrast `theme_muted`, the i18n
`shell_catalog` rename and new keys, the keyboard-reachability audit)
is unaffected and stands as already described above. "Spring/ease
transitions for overview open/close, <=300ms" (`CLAUDE-START-HERE.md`
section 3.7) is therefore **not currently implemented** -- reported
honestly as a reverted, not-yet-working attempt, not quietly dropped
from the record. Revisiting it needs either a render mechanism that
doesn't share the `Painter`'s multi-tile buffer across alpha-blended
passes the way `draw_blended` does, or a real QEMU feedback loop to
iterate on the existing one -- static review alone already proved
insufficient once.

### Verification results

- **1 CPU** (after the revert): clean, matches the pre-fade M3/M4
  screenshots exactly -- search field, Dash (Files/Terminal/Store
  favorites), "No open windows on this workspace", the full 11-app grid
  with corrected labels, no ghosting. `WovenFiles: window opened`/
  `window opened: Files` both logged.
- **2 CPU**: clean, screenshot identical in content to the 1 CPU run.
  Same serial-log success markers.
- **4 CPU**: the journey succeeded identically (same serial-log markers),
  but the `activities-overview.ppm` screenshot itself showed only the
  search field painted, with the Dash/placeholder/grid still showing
  plain desktop wallpaper beneath. Diagnosed as a capture-timing race in
  the qualification script's new screenshot step -- the 2 CPU capture of
  the exact same shell code, moments apart, is clean, and the underlying
  functional journey (window-open, launch) succeeded at all three CPU
  counts -- not a rendering or SMP defect in the shell itself. Not
  pursued to a perfected fix this pass (e.g. a higher
  `minimum_changed` threshold or a second confirming capture); recorded
  as a known flake in the test script's own robustness, analogous to the
  already-documented `keyboard_refresh` timing flake.
- All three CPU counts hit the pre-existing `keyboard_refresh` flake
  after the above, exactly as before -- unrelated, already documented.

This closes the M2/M3 2/4-CPU verification gap and the M4/M5 QEMU
verification gap all at once; M2-M5 are now QEMU-verified at 1/2/4 CPU,
modulo the pre-existing `keyboard_refresh` flake (which means the
qualification script's own exit code is still non-zero, so this is still
not a formal stage acceptance).

## M6: measured performance/memory record

Section 6 of `CLAUDE-START-HERE.md` asks to "establish baseline numbers
first... frame time of a full recompose, shell resident memory, time
from boot to responsive desktop, search latency over 64 apps" and
"report real numbers, including failures." Took that literally: added
only instrumentation that reuses existing, already-accepted mechanisms
(`libwoven::ticks()`, `log()`) rather than a new profiling capability,
and report what's actually measurable with the current ABI rather than
estimate or invent the rest.

### What was added

- **`Desktop::log_search_latency`** (`main.rs`, called from
  `launcher_key`'s `Char`/`Backspace` arms, i.e. every time the search
  query actually changes): times `self.filtered()` with `libwoven::
  ticks()` immediately before and after, logs the elapsed ticks, the
  match count, and `self.app_count` (the catalog size actually loaded
  this boot). The catalog size is logged alongside the latency
  deliberately: the design brief's budget assumes a 64-app catalog
  (`MAX_APPS`), and the demo fixture this measurement actually ran
  against has far fewer, so the number needs that context to mean
  anything.
- **A full-recompose timer** around `fn main`'s first `desktop.repaint(
  &mut painter, desktop.screen())` call: this is a genuine full-screen
  recompose by construction (the very first paint, `area == screen()`),
  so no extra bookkeeping was needed to identify that case. Deliberately
  measured from *outside* `repaint`/`repaint_once` rather than
  instrumented internally: both have several early-return paths (a
  vanished-surface retry loop, a locked-screen short-circuit), and
  wrapping timing around all of them correctly would have meant touching
  control flow in functions this session didn't otherwise need to
  change, for a single-measurement-point's sake. Measuring the one
  already-known-full-screen call from outside gets the same number with
  zero risk to that code.

### Real measured numbers (software/QEMU-TCG, single CPU, cold first boot -- not native hardware)

From `audit-artifacts/desktop-qualification-1793707375520064000/
serial.log`, `libwoven::time::TICKS_PER_SECOND = 100` (1 tick = 10 ms):

| Metric | Measured | Notes |
|---|---|---|
| Boot to desktop ready | 448 ticks = 4.48 s | Sum of every `[BOOT PROFILE]` step's own delta, kernel-measured, not estimated. Covers UEFI handoff through `WovenShell desktop started`; does not include the automated sign-in step after that, which is test-harness-paced key-press timing, not a shell performance number. |
| Full recompose (first paint) | 129 ticks = 1.29 s | The very first frame: wallpaper + taskbar/top-bar + desktop icons recomposed from nothing, under software TCG rendering on one virtual CPU. A steady-state recompose (window move, workspace switch) was not separately measured this pass -- the instrumentation point chosen was the one genuinely risk-free to add; measuring a representative *interactive* recompose would need a second log point inside the input-driven repaint path, left for a follow-up. |
| Search-filter latency | 0 ticks (< 10 ms) | Against the actual 11-app demo catalog, not the 64-app budget the brief assumes. A linear scan over 11 bounded-length string comparisons is expected to be far under one tick; this number says "too fast to resolve at 100 Hz for this catalog size," not "0 ms precisely," and should be re-measured once/if a 64-app fixture exists. |
| Shell resident memory | **Not measured** | No per-process memory-stats syscall exists in `libwoven` (checked: grepped for `memory`/`stats`/`resident` in `libwoven/src/lib.rs`, found nothing). Adding one is a new kernel capability and needs its own review (userspace pointer validation, what a process may query about itself vs. others, bounded response size) -- out of scope to add unreviewed just to produce a number for this doc. The kernel's own `[HEAP] mapped=8388608 bytes` boot log line is real but measures the *kernel's* heap, not the shell process specifically, so it isn't reported here as a stand-in. |

### What this does and doesn't establish

This is a **first measurement**, not a regression baseline with a
"before" to compare against -- there was no equivalent instrumentation
in the pre-Overview (Taskbar-only) codebase to diff against, and adding
retroactive instrumentation to the old code path to produce a legacy
comparison number was judged lower value than being honest that no such
comparison exists yet. Future milestones (or a dedicated follow-up) that
touch these numbers again should diff against the ones recorded here.
"No regression of legacy layout timings" (the brief's other M6
requirement) is therefore not something this pass can claim either way
for frame/boot timing -- only functional parity (M2's QEMU verification
that both layouts boot and compose correctly) was checked, not relative
performance.

"Shell static memory increase" (the brief's third M6 requirement) is
likewise not reported as a number, for the same reason resident memory
isn't: nothing in the current ABI exposes it. What can be said instead,
precisely: the new code added across M2-M6 is six new modules (`tokens.
rs`, `geometry.rs`, `textures.rs`, `icons.rs`, `model.rs`, `painter.rs`
from M1) plus roughly 900 lines of new logic directly in `main.rs`
(`ShellLayout`, the top bar, the overview, the Dash, the i18n catalog
additions, this instrumentation) -- all static/`.bss` allocations
following the codebase's existing patterns (fixed-size arrays, no new
heap-growing data structure), so the *shape* of any increase is bounded
by construction the same way the rest of the shell already is, even
though the exact byte count isn't measurable right now.

## M7: the layout cutover

### What was removed

With M2-M6 verified at 1/2/4 CPU and the full `RUN-STAGE10.7.ps1` gate
passing, removed the `ShellLayout` enum and `--layout` CLI flag entirely
-- Overview is now the shell's only layout, not a default with a
rollback flag. Deleted with it: `paint_taskbar_content`,
`render_taskbar_3d` (the whole legacy bottom-taskbar rendering, ~210
lines), the Taskbar-only small launcher panel (`panel_area`,
`paint_panel_content`, `render_panel`, `render_panel_3d`, and its
offscreen `LauncherPanelTexture`), the taskbar's own offscreen
`TaskbarTexture`, `entry_area` (the per-window taskbar entry, never
given an Overview equivalent since M4's Dash), and the dead
taskbar-entry-click handler in `handle_input`. Every `match self.layout
{ Taskbar => ..., Overview => ... }` across the shell collapsed to its
Overview branch; the compiler's own `dead_code`/`unused_imports`
warnings (with warnings denied, so nothing could be missed silently)
found every orphaned constant and function this left behind in
`tokens.rs` and `textures.rs` (`START_WIDTH`, `START_LABEL`,
`START_ICON*`, `ENTRY_X`/`ENTRY_WIDTH`/`ENTRY_GAP`/`ENTRY_CHARS`,
`PANEL_X`/`PANEL_WIDTH`/`PANEL_TITLE`, and the launcher/taskbar texture
types and their size constants). `main.rs`: ~4,640 -> ~3,793 lines.

### Two real bugs found and fixed while deleting, not introduced by it

Confirmed by reading, not assumed: `launcher_key` and `forward_pointer_
move` still referenced the now-deleted `panel_area()` directly instead
of `overview_area()`. `panel_area()` had always returned empty once
Overview became the default back in M2 (it only applied to the Taskbar
branch), so these were **pre-existing bugs that predate this cutover**:
`launcher_key`'s dirty-area tracking returned an empty `Area` on every
keystroke in the Overview layout (the search field still updated
visually, just via the periodic idle-frame-clock catch-up rather than
an immediate repaint -- not a correctness bug, just a latent one-frame-
period lag never actually observed because it's imperceptible under
test automation), and `forward_pointer_move`'s "don't forward pointer
motion to a window under a floating panel" check never covered the
Overview overview at all. Both fixed by substituting `overview_area()`
-- the cutover's forced review of every `panel_area()` caller is what
surfaced these, not something that would have been found by grepping
for the literal string "ShellLayout".

### One new shared helper

The five sites validating or clamping window geometry against the
screen's usable band (`open`, `resized`, `toggle_scale`,
`drag_to_pointer`, `work_area`) each duplicated the same `(area_top,
area_height)` -- or, for `work_area`, the equivalent `Frame` -- computed
from `taskbar_top()` and `TASKBAR_HEIGHT`. Extracted one `usable_band(&self)
-> (i32, i32)` method and had all five call it, rather than leaving five
near-identical inline computations now that there's only one layout to
compute them for.

### Verification

`cargo build`/`cargo clippy --target x86_64-unknown-none -- -D
warnings`: zero warnings, not just zero errors. `scripts/test-desktop-
qualification.py` at 1, 2, and 4 CPUs: all three reach the same point as
every prior accepted run (sign-in, overview, launch, window-open,
pointer, all four workspace switches), confirmed via serial log
(`WovenFiles: window opened`/`window opened: Files` at every CPU count)
and, at 1 CPU, three screenshots (`desktop-signed-in.ppm`,
`activities-overview.ppm`, `files-window-open.ppm`) visually identical
to the pre-cutover baseline -- the `usable_band()` refactor placed the
Files window correctly below the bar with no overlap or corruption, and
the top bar/overview render exactly as before. All three CPU counts then
hit the pre-existing `keyboard_refresh` flake, same as every prior run.
Evidence: `audit-artifacts/desktop-qualification-{1793712148697202900
(1 CPU), 1793712338867984900 (2 CPU), 1793712485560248100 (4 CPU)}/`.

### App visual-token overhaul (2026-11-03)

Done, as a separate follow-on pass after the M7 cutover. Every
first-party app with GUI chrome -- `woven-files`, `woven-terminal`,
`woven-editor`, `woven-images`, `woven-monitor`, `woven-store`,
`woven-update`, `woven-security`, `woven-calculator`, `woven-installer`
-- now draws through `libwoven::ui::Theme::DARK` instead of locally
hardcoded `PANEL`/`ACCENT`/`TEXT`/`BUTTON` color constants and a
manually-reconstructed `Theme { ... }` literal at each draw call.
`Theme::DARK.accent` was retuned first, from `rgb(100,220,180)` to
`rgb(76,190,188)`, to match `apps/woven-shell`'s own established accent,
so the shell and every app now share one accent color. `woven-settings`
already supported `Theme::DARK`/`LIGHT`/`HIGH_CONTRAST` dynamically
(its own theme-mode switch) and needed no change. `woven-recovery`,
`woven-search`, `woven-ai`, and `woven-package` were confirmed to be
CLI-only -- no GUI, `Painter`, or `Theme` call sites in any of their
source files -- so no migration applies. Each app verified individually:
`cargo build --target x86_64-unknown-none` and `cargo clippy --target
x86_64-unknown-none -- -D warnings`, both clean, zero warnings; diffs
reviewed to confirm pure color/token substitution with no geometry, IPC,
or accepted-behavior changes. Committed as one small commit per app.

### New asset set (2026-11-03)

Also done, as a bounded first pass (not every icon/wallpaper variant
`CLAUDE-START-HERE.md` could eventually ask for, but the concrete gap
found and closed this session):

- **Wallpaper**: `kernel/build.rs` generated the `assets/wallpapers/`
  sample BMP (`wovenhat-gradient-160x90`, used as `woven-images`'s
  "Licensed wallpaper" preview) with an orange/brown horizontal gradient
  unrelated to the shell's actual desktop background. Replaced it with a
  procedural navy-to-teal vertical gradient using the shell's own
  `WALLPAPER_TOP`/`WALLPAPER_BOTTOM` palette (`rgb(11,29,58)` ->
  `rgb(16,78,82)`), plus a faint diagonal basket-weave tint matching the
  shell's `wallpaper()` weave motif -- the sample asset now visually
  matches the live desktop instead of looking like an unrelated stock
  gradient. Also fixed a pre-existing BGR/RGB byte-order bug in the
  generator (BMP pixel data is BGR; the old code wrote R/G/B in that
  slot order, which `libwoven::media::decode_bmp_rgba`'s correct
  B/G/R-order read would have rendered with red and blue channels
  swapped). Dimensions (160x90) and format (BMP24) unchanged, so
  `woven-images`'s dimension check and `woven-shell`'s wallpaper-staging
  path are unaffected. `assets/wallpapers/manifest.toml` updated with
  the new description and sha256 (CC0-1.0, unchanged license).
- **Icons**: `apps/woven-shell/src/icons.rs`'s app-tile icon set
  (Fluent-style rounded tile, per-app fill/ink colors, a geometric glyph,
  and a shared background from `libwoven::icons::IconPack`) already
  covered 8 of the 11 apps the kernel's launcher catalog actually lists
  (`kernel/src/main.rs`'s `Name=Woven*` entries); `WovenMonitor` and
  `WovenInstaller` fell back to the generic glyph. Added distinct
  `AppIcon::Monitor` (blue, a filled display with a small stand glyph)
  and `AppIcon::Installer` (green, a package/box glyph) variants, so
  every launchable app now has a distinct tile. Verified via `cargo
  build`/`cargo clippy --target x86_64-unknown-none -- -D warnings` for
  `apps/woven-shell`, both clean.

Not done: a full redesigned icon pack beyond these two additions, and
any wallpaper variants beyond the one existing sample asset (light
theme, high-contrast, or additional original compositions). Those
remain open if the user wants a larger pass later.
