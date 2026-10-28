# Stage status

## Stage 32.11 GDB session state machine QEMU gate - 2026-10-28 - ACCEPTED

Added `kernel/src/gdb_session.rs`, a bounded lifecycle owner around the Stage
32.10 command boundary. It tracks active/stopped state and stop signals,
returns `Sxx` stop replies, refuses commands while running, and translates
continue, step, and kill into explicit actions without executing them in the
parser layer. Session state is local to the caller and does not dereference
guest pointers or introduce task ownership.

Passes on 1/2/4 CPUs with exit 33. Evidence:
`target/stage32-11-gdb-session-{1,2,4}cpu/`. Target and host warning-denying
Clippy, host tests, and Python syntax validation pass. Live trap ownership,
task binding, register mutation, transport polling, and actual continue/step
execution remain open.

## Stage 32.10 GDB register context and command dispatch QEMU gate - 2026-10-28 — ACCEPTED

Added `kernel/src/gdb_debugger.rs`, a pure bounded command boundary over the
Stage 32.8 packet layer. It encodes a fixed 24-register x86-64 context in
target-endian hexadecimal and dispatches `?`, `g`, bounded `m`, `qSupported`,
`c`, `s`, and `k` commands. Memory reads are limited to a caller-owned window
and 256 bytes; invalid or out-of-range requests return `E01` without guest
pointer access.

Passes on 1/2/4 CPUs with exit 33. Evidence:
`target/stage32-10-gdb-dispatch-{1,2,4}cpu/`. Target and host warning-denying
Clippy, host tests, and Python syntax validation pass. This is a pure command
and context boundary; live trap entry, stopped-task ownership, register
mutation, memory mapping, continue/step execution, and debugger lifecycle
remain open.

## Stage 32.9 second-UART debug serial QEMU gate - 2026-10-27 — ACCEPTED

Added the bounded COM2 transport for the WovenDev debugger path in
`kernel/src/debug_serial.rs`. It configures a 16550-compatible second UART at
115200 8N1, provides bounded transmit polling and nonblocking receive, and
performs a loopback self-test before emitting a marker through the second QEMU
serial device. The `debug-serial` production feature is zero-cost when off;
`stage32-9-test` enables the QEMU gate.

Passes on 1/2/4 CPUs with exit 33. Evidence:
`target/stage32-9-debug-serial-{1,2,4}cpu/`, including `debug-serial.log`.
Target and host warning-denying Clippy plus host tests pass. This is the
second-UART transport boundary only; live GDB command handling, trap entry,
register/memory access, and debugger session lifecycle remain open.

## Stage 32.8 GDB RSP packet framing QEMU gate - 2026-10-27 — ACCEPTED

Wire-level GDB Remote Serial Protocol packet framing: pure functions over byte
slices, no I/O, no allocation, no global state. `gdb-stub = []` production
feature; `stage32-8-test = ["qemu-test", "gdb-stub"]` QEMU gate. New module
`kernel/src/gdb_stub.rs`:
- `checksum(data) -> u8` — 8-bit modular sum (RSP spec § E.1).
- `encode(data, out) -> Option<usize>` — writes `$<data>#<cs_hi><cs_lo>`.
- `decode(input) -> Option<Packet>` — parses and checksum-verifies a packet;
  extra trailing bytes are ignored.
- `Packet::data() -> &[u8]` / `as_str() -> Option<&str>` — accessor pair.
- `self_test()` covers: known checksum values (including wrap-around), encode
  output for `?`/`OK`/empty, encode/decode round-trip, trailing-byte
  tolerance, and five rejection cases (wrong start byte, checksum mismatch,
  too short, invalid hex, no `#`).
Passes on 1/2/4 CPUs, exit 33.
Evidence: `target/stage32-8-gdb-stub-{1,2,4}cpu/serial.log`.
Next: Stage 32.9 — GDB debug serial port (second COM port I/O).

## Stage 32.7 log-level filter QEMU gate - 2026-10-27 — ACCEPTED

Minimum-severity gate on `diagnostics::emit()` using a lock-free `AtomicU8`.
`log-filter = []` production feature; `stage32-7-test = ["qemu-test",
"log-filter", "log-ring"]` QEMU gate. New module `kernel/src/log_filter.rs`:
- `set_min_level(level)` / `min_level() -> Level` — atomic store/load
  (`Ordering::Relaxed`); safe from IRQ context.
- Default: `Level::Debug` — passes Error/Warn/Info/Debug, drops only Trace.
- `emit()` check: `if level > min_level() { return; }` — fires before any
  formatting, serial write, or ring push.
- `Level` gained `#[repr(u8)]` in diagnostics.rs so its discriminant can be
  stored directly as a `u8` (non-breaking; ordering unchanged).
- Zero-cost stubs (`#[inline(always)]` no-ops) when `log-filter` is disabled.
- `self_test()` verifies Level ordering assumptions and (when `log-filter`
  is active) the atomic set/get round-trip.
QEMU demonstration: sets min=Info, emits Debug (dropped) then Info (passes),
checks ring count=1 to confirm the filter is live.
Serial output confirms:
```
INFO filter_test: should_appear
[S32.7] log filter: PASSED
```
(`DEBUG filter_test: should_be_dropped` is absent — correctly suppressed.)
Passes on 1/2/4 CPUs, exit 33.
Evidence: `target/stage32-7-log-filter-{1,2,4}cpu/serial.log`.

## Stage 32.6 kernel debug dump QEMU gate - 2026-10-27 — ACCEPTED

Structured kernel debug dump combining a panic crash context with a ring
buffer snapshot. `debug-dump = ["log-ring"]` production feature;
`stage32-6-test = ["qemu-test", "debug-dump"]` QEMU gate. New module
`kernel/src/debug_dump.rs`:
- `emit_dump(rip, rsp, cr2)` — drains the ring first (releases lock), then
  writes to serial via `format_record` + `serial::write_bytes` directly (bypasses
  `diagnostics::emit` to avoid recursive ring-push):
  1. `ERROR debugger: panic_context | rip=… | rsp=… | cr2=…`
  2. `DEBUG debugger: ring_snapshot | count=N`
  3. `DEBUG debugger: ring_entry | idx=I | record=…` × N (up to `DRAIN_MAX`=8)
- Zero-cost `#[inline(always)]` stub when `debug-dump` is disabled.
- `self_test()` verifies `format_record` output for the crash context and
  ring-snapshot headers without serial I/O or lock acquisition.
Passes on 1/2/4 CPUs, exit 33. Serial confirms end-to-end:
```
ERROR debugger: panic_context | rip=0xffffdeadc0de | rsp=0xffff800000100000 | cr2=0xbad00000
DEBUG debugger: ring_snapshot | count=2
DEBUG debugger: ring_entry | idx=0 | record=INFO boot: debug-dump test seed 1
DEBUG debugger: ring_entry | idx=1 | record=WARN sched: debug-dump test seed 2
```
Evidence: `target/stage32-6-debug-dump-{1,2,4}cpu/serial.log`.

## Stage 32.5 log ring buffer QEMU gate - 2026-10-27 — ACCEPTED

Allocation-free bounded circular ring buffer that `diagnostics::emit()` feeds
automatically. `log-ring = []` production feature; `stage32-5-test = ["qemu-test",
"log-ring"]` QEMU gate. New module `kernel/src/log_ring.rs`:
- `push(bytes)` — copies up to [`ENTRY_MAX`] (256) bytes into the next slot of
  the fixed-capacity ring; IrqMutex-protected; IRQ-safe.
- `len() -> usize` — current number of valid entries (0…[`RING_CAPACITY`] = 64).
- `drain(out, lens) -> usize` — copies entries oldest-first into caller-provided
  arrays without allocation; stops when either output slice is exhausted.
- When the ring is full, the oldest entry is overwritten silently.
- All three symbols are `#[inline(always)]` no-ops when the `log-ring` feature
  is disabled (zero overhead on default builds).
- `diagnostics::emit()` now calls `log_ring::push()` after serial write when
  `log-ring` is enabled; no call-site instrumentation required.
- `self_test()` verifies push/drain order, exact-capacity fill, overflow
  (oldest dropped), and byte-level truncation.
Passes on 1/2/4 CPUs, exit 33. Boot serial shows `ring_count=0` (the minimal
QEMU test path emits nothing via `emit()`; the integration fires on production
boots that use the diagnostics API).
Evidence: `target/stage32-5-log-ring-{1,2,4}cpu/serial.log`.

## Stage 32.4 performance profiler QEMU gate - 2026-10-27 — ACCEPTED

Allocation-free kernel performance profiler built on the tracing and diagnostics
primitives. `profiler = []` production feature; `stage32-4-test = ["qemu-test",
"tracing", "profiler"]` QEMU gate. Module `kernel/src/profiler.rs`:
- `record(subsystem, name, tsc_delta)` — accumulates per-`(subsystem, name)`
  count/sum/min/max TSC-cycle statistics into a fixed-capacity static table; key
  lookup by `&'static str` pointer equality avoids byte-by-byte comparison.
- `report()` — emits all accumulated slots as structured diagnostics records:
  `INFO profiler: slot | sub=… | name=… | count=… | avg_tsc=… | min_tsc=… | max_tsc=…`.
- `reset()` — clears all accumulated samples.
- When both `tracing` and `profiler` features are enabled, `tracing::span_end`
  automatically calls `record`; no call-site instrumentation is needed.
- When `profiler` is disabled (default), all three are `#[inline(always)]`
  no-ops that the compiler eliminates entirely.
- `self_test()` verifies accumulation arithmetic, slot allocation, min/max/avg
  computation, and `reset()` behavior without touching the static IrqMutex.
Passes on 1/2/4 CPUs, exit 33.
Evidence: `target/stage32-4-profiler-{1,2,4}cpu/serial.log`.

## Desktop software graphics acceptance closed - 2026-10-27

ACCEPTED AND CLOSED for the reported software/QEMU desktop repair at source
`cea33d1a2f6ef1ddb5cdb71a8f99d393952c0df7`. This supersedes the open desktop pixel failures in the historical records;
those historical failed artifacts remain preserved. All 87 required matrix
cases pass on 1/2/4 CPUs, including TCG/WHPX keyboard/mouse/performance,
software 3D, GPU recovery, two monitors and session behavior. The strict
clean-source GFX-07 checker passes 9/9 with its fatal detector unchanged.
Build, warning-denying lints, Rust tests and all 205 Python tests pass.

See [the acceptance record](graphics/desktop-acceptance-2026-10-27.md) for exact
source/evidence provenance, stronger completed-frame pixel assertions,
close-time repaint recovery, and validation limits. This closes software
desktop acceptance, not native GPU/HID, accelerated 3D, GFX-08, the whole
graphics mandate or the full historical Windows acceptance chain.


## Stage 20 reported-gate revalidation - 2026-10-27 — ACCEPTED

Revalidated the seven gates previously reported as failing after the current
tree advanced to `744b5839`. No production source change was required: the
current flat compatibility renderer and its existing independent kernel
oracles passed the complete 1/2/4-CPU matrix with QEMU exit 33:

- Stage 20.2 desktop applications
- Stage 20.3 application launcher
- Stage 20.4 windows that move and close
- Stage 20.6 translucent overlays
- Stage 20.8 tiling
- Stage 20.9 notification center
- Stage 20.13 workspaces

Evidence is preserved under `target/woven-shell-{apps,launcher,windows,
translucency,tiling,notifications,workspaces}-{1,2,4}cpu/`. The one
intermittent 2-CPU Stage 20.6 failure was rerun cleanly and passed; no
assertions or timeouts were weakened. The image-backed Stage 20 lint sweep,
production lint, and host `--all-targets` Clippy also pass. This closes the
reported seven-gate regression set; it does not claim full roadmap Stage 20,
native hardware qualification, or full Stage 32 acceptance.

## Stage 32.3 tracing span API - 2026-10-27 — ACCEPTED

Zero-cost kernel tracing span API built on the diagnostics primitive.
`tracing = []` production feature; `stage32-3-test = ["qemu-test", "tracing"]`
QEMU gate. New module `kernel/src/tracing.rs`:
- `span_begin(subsystem, name) -> (id, tsc_start)` — emits `span_begin` record.
- `span_end(subsystem, name, id, tsc_start)` — emits `span_end | tsc=<cycles>`.
- `SpanGuard::enter(subsystem, name)` — RAII guard; `Drop` emits `span_end`.
- When `tracing` feature is disabled (default), all three are `#[inline(always)]`
  no-ops; the compiler eliminates them entirely (`SpanGuard` is a ZST).
- `self_test()` verifies monotone span-ID counter and non-decreasing TSC.
Passes on 1/2/4 CPUs, exit 33. Serial shows span records with non-zero TSC
deltas (e.g. `TRACE tracing: span_end | name=self_test | id=3 | tsc=4885175`).
Evidence: `target/stage32-3-tracing-{1,2,4}cpu/serial.log`.

## Stage 32.1 structured-diagnostics QEMU gate - 2026-10-27 — ACCEPTED

Boot-time QEMU acceptance for the structured-diagnostics primitive.
Feature `stage32-1-test` (chains `qemu-test`) calls `diagnostics::self_test()`
from the boot sequence and exits 33 on pass. Passes on 1/2/4 CPUs:
- 1 CPU: exit 33, `[S32.1] structured diagnostics: PASSED`
- 2 CPU: exit 33, `[S32.1] structured diagnostics: PASSED`
- 4 CPU: exit 33, `[S32.1] structured diagnostics: PASSED`
Evidence: `target/stage32-1-diagnostics-{1,2,4}cpu/serial.log`.
Note: QEMU 11.1 deprecated `-accel tcg,tb-size=128`; harness uses `-accel tcg`.

## Stage 32.2 structured panic crash-dump QEMU gate - 2026-10-27 — ACCEPTED

Boot-time QEMU acceptance for the structured panic crash-dump path.
Feature `stage32-2-test` wires `diagnostics::emit_panic_dump()` into
`panic::kernel_panic()` and calls `diagnostics::self_test_panic_dump()` at
boot to verify the exact record format. Does NOT trigger a real panic (which
would put "KERNEL PANIC" in the serial log and fail `validation_errors`).
Passes on 1/2/4 CPUs:
- 1 CPU: exit 33, `[S32.2] structured panic dump: PASSED`
- 2 CPU: exit 33, `[S32.2] structured panic dump: PASSED`
- 4 CPU: exit 33, `[S32.2] structured panic dump: PASSED`
Evidence: `target/stage32-2-panic-dump-{1,2,4}cpu/serial.log`.
After acceptance: `emit_panic_dump()` promoted to unconditional production code
in `kernel_panic()` (feature guard removed); `#![allow(dead_code)]` removed.

## Graphics shared accessibility-profile parser - 2026-10-25

`libwoven::accessibility::Settings::apply_json` now parses the bounded
accessibility profile format shared by the installer and WovenShell. It
accepts only quoted recognized keys, checked booleans, and clamped small
integer values; malformed or unknown fields fail closed. The full 193-test
libwoven suite and warning-denying host/freestanding Clippy pass. Direct
settings write-back remains intentionally unavailable because packaged apps
have no `/etc` write authority.

## Graphics persisted accessibility profile handoff - 2026-10-25

The installer now emits bounded `high_contrast` and `reduced_motion` keys in
`/etc/wovenhat/settings.json`. Production system-apps boot imports that file
from the mounted installation, passes it to WovenShell, and the shell applies
only recognized `true` values with a fixed 512-byte read bound. The no-file
path remains unchanged. Warning-denying target Clippy passes for WovenShell
and the kernel `system-apps` feature; the production desktop boot gate passes
on 1/2/4 CPUs. Settings UI write-back and compositor-wide persistence remain
open.

## Graphics shell high-contrast launch mode - 2026-10-25

WovenShell now accepts `--high-contrast` and routes wallpaper, taskbar,
launcher, panels, title bars, badges, workspace/Quick Settings entries, and
text through a high-contrast palette. The default production palette remains
unchanged. Target release build and warning-denying target Clippy pass; the
default desktop gate passes at 1/2/4 CPUs with evidence under
`target/woven-shell-{1,2,4}cpu`. This is an application launch mode, not
persisted compositor-wide accessibility settings or full Stage 11 acceptance.

## Graphics bounded HTTP chunked-body increment - 2026-10-25

`libwoven::web` now records chunked transfer encoding and decodes bounded
HTTP/1.1 chunked bodies into caller-owned storage. Hex sizes, extensions,
CRLF framing, trailers, overflow, truncation, and output capacity are checked
fail-closed. The full 191-test libwoven suite and warning-denying freestanding
target Clippy pass. Socket transport, TLS, policy, and network imagery remain
open.

## Graphics bounded grid-repeat increment - 2026-10-25

The browser grid parser now recognizes bounded `repeat(N, …)` column syntax
with a fixed maximum of 16 columns, while retaining explicit track-list
support and fail-closed invalid counts. The full 190-test libwoven suite and
warning-denying freestanding target Clippy pass.

## Graphics bounded media-query increment - 2026-10-25

`StyleSheet::parse_for_viewport` now evaluates bounded `@media
(min-width: Npx)` and `@media (max-width: Npx)` blocks, skips inactive rules,
and rejects malformed or over-nested input. The full 190-test libwoven suite
and warning-denying freestanding target Clippy pass. Broader media features and
responsive semantics remain open.

## Graphics bounded responsive-length increment - 2026-10-25

Browser inline and stylesheet `width`/`height` now accept checked percentage
lengths in addition to pixels. Width resolves against the containing box and
height against the bounded viewport; invalid or overflowing values fail closed.
The full 189-test libwoven suite and warning-denying freestanding target
Clippy pass. Media queries and full responsive semantics remain open.

## Graphics bounded stylesheet cascade increment - 2026-10-25

`libwoven::browser` now captures HTML class/id attributes and parses a
fixed-capacity stylesheet with tag, class, and id selectors. Matching rules
apply by selector specificity, then source order, and inline declarations apply
last; bounded inherited text color follows the parent chain, and the existing
flex/grid layout and paint paths can consume the stylesheet through dedicated
APIs. The 188-test libwoven suite, host all-targets Clippy, and freestanding
target Clippy pass with warnings denied. Media-query evaluation and full
responsive semantics remain open.

## Graphics roadmap ledger correction - 2026-10-25

The live audit now classifies the existing bounded browser substrate and scalar
JavaScript seam as PARTIAL rather than MISSING: browser DOM/layout/paint/software
rendering and inline script execution are implemented and tested, while full
browser, ECMAScript, DOM, module, async, and sandbox capabilities remain open.

## Graphics bounded PCM WAVE parser increment - 2026-10-25

`libwoven::media::parse_wav_pcm` now validates RIFF/WAVE chunk bounds, PCM
format, sample alignment, byte rate, and caller-visible sample ranges without
allocating or copying audio data. Eleven media tests pass for the affected
module, and freestanding target Clippy passes with warnings denied. This is a
container/metadata boundary only; full streaming playback and codec support
remain open.

## Graphics browser bounded flex/grid layout increment - 2026-10-25

The borrowed browser layout substrate now supports bounded `display:flex` rows
and columns, `display:grid` columns, and fixed inline `gap` placement. Child capacity,
column count, coordinate arithmetic, viewport bounds, and overflow remain
fail-closed. The 184-test libwoven suite, host all-targets Clippy, and
freestanding target Clippy pass with warnings denied. This is not a full CSS
cascade or responsive browser claim.

## Graphics theme-mode boundary increment - 2026-10-25

WovenSettings now selects the shared `libwoven::ui::Theme` palette through
bounded `--light` and `--high-contrast` GUI modes, while retaining dark as the
default. The target release build and warning-denying target Clippy pass. This
is an application-level theme increment only: the shell still uses its
accepted production palette, and no compositor-wide persisted accessibility
settings service is claimed.

The existing signed WovenSettings QEMU gate also passes at 1/2/4 CPUs; evidence
is retained under `audit-artifacts/woven-settings-{1,2,4}cpu`.

## Graphics Stage 24.59 low-memory Change-PIN acceptance - 2026-10-25

The 256 MiB system-apps fixture initially failed in the bootloader because
128 service-sized 1 MiB task stacks plus the bounded VFS/catalog footprint
left no mapping headroom. Stage 24.59 now uses an explicit 256 KiB task-stack
profile (four times the ordinary stack) while production system-apps sizing
remains unchanged. The complete Change-PIN flow passes on 1/2/4 CPUs with
QEMU exit 33 and target Clippy green; evidence is under
`target/woven-shell-change-pin-{1,2,4}cpu`.

## Graphics Stage 20.15 session acceptance - 2026-10-25

Repaired the session oracle to account for the launcher’s app-icon text
offset and the projected Quick Settings top edge. The session now passes
catalog loading/listing, display and input ownership, F12 console handoff and
return, keyboard delivery, Quick Settings access, and desktop power-off on
1/2/4 CPUs. Target Clippy is green; evidence is preserved under
`audit-artifacts/stage20.15-{1,2,4}cpu-*`.

## Stage 13.20 power-off matrix - 2026-10-25

The ACPI PM1/_S5 power-off gate passes on 1/2/4 CPUs. Evidence is preserved
under `audit-artifacts/stage13.20-poweroff-{1,2,4}cpu-*`.

## Graphics Stage 20 feature-lint sweep - 2026-10-25

With the built hello-gui and woven-shell images supplied through the required
environment variables, `python scripts/lint-all-features.py --only stage20`
passes every Stage 20 feature, production kernel Clippy, and host
`--all-targets` Clippy with warnings denied.

## Graphics Stage 20.6 translucency acceptance - 2026-10-25

Repaired the stage oracle for the current wallpaper and accent system and
isolated this flat-overlay contract with the shell's supported `--no-3d` mode,
leaving the later 3D toast behavior to its dedicated gate. Toast and launcher
panel translucency, pointer transparency, layering over a window/shadow,
close, teardown, and display restoration pass on 1/2/4 CPUs with QEMU exit
33. Target Clippy for `stage20-6-test` is green; evidence is under
`target/woven-shell-translucency-{1,2,4}cpu`.

## Graphics Stage 20.5 window-depth acceptance - 2026-10-25

The complete depth/shadow gate passes on 1/2/4 CPUs with QEMU exit 33. It
verifies focused-window shadow ordering, shadow contrast, taskbar clipping,
raise/lower behavior, application teardown, and display restoration. Evidence
is retained under `target/woven-shell-depth-{1,2,4}cpu`; warning-denying
freestanding kernel Clippy for `stage20-5-test` passes.

## Graphics Stage 20.9 notification-center acceptance - 2026-10-25

Repaired the notification-center oracle to match the production wallpaper,
taskbar, accent, and clock geometry. The complete toast, unread-badge,
newest-first ordering, clear, close, key handoff, and teardown gate passes on
1/2/4 CPUs with QEMU exit 33. Evidence is retained under
`target/woven-shell-notifications-{1,2,4}cpu`; warning-denying freestanding
kernel Clippy for `stage20-9-test` passes.

## Graphics Stage 20.12 sign-in acceptance - 2026-10-25

Repaired the sign-in oracle to match the production wallpaper and projected
background. The complete account-file, account-selection, wrong-PIN,
successful-sign-in, taskbar identity, identity-bound lock/unlock, and teardown
gate passes on 1/2/4 CPUs with QEMU exit 33.

## Graphics Stage 20.11 lock-screen acceptance - 2026-10-25

Repaired the Stage 20.11 lock-screen oracle to match the production flat
wallpaper palette, weave, and projected 3D background. The complete lock,
input isolation, PIN masking, wrong-PIN, unlock delivery, lockout, recovery,
and teardown gate passes on 1/2/4 CPUs with QEMU exit 33.

## Graphics Stage 11 desktop catalog blocker repair - 2026-10-25

The production catalog blocker was traced to `woven-images.wapp` (166,248
bytes) exceeding the 128 KiB VFS node/exec-image bound. The bounded capacity is
now 256 KiB. The focused signed system-app catalog gate passes on 1 CPU with
all eleven applications installed; the desktop screenshot/input matrix now
passes on 1/2/4 CPUs, while theme-variation screenshots and low-power runtime
remain open.

## Graphics Stage 11 design tokens increment - 2026-10-25

Added allocation-free `libwoven::ui::DesignTokens` for spacing, radii,
elevation, and motion durations, plus light, dark, and high-contrast `Theme`
variants selected from the shared accessibility settings. Host qualification
passes with 182 libwoven tests and warning-denying libwoven Clippy. This is a
code/library increment only; full Stage 11 desktop screenshot, animation
starvation, and low-power runtime acceptance remains open.

## Graphics Stage 11 motion/pacing acceptance increment - 2026-10-25

Requalified the existing graphics runtime slices against the current shell.
Stage 19.14 display scaling/frame pacing and Stage 20.7 spring animation now
pass on 1/2/4 CPUs with exit 33. The animation oracle was repaired to use the
actual pre-window framebuffer pixel, keeping its monotonic alpha and teardown
checks independent of wallpaper/projection geometry. Warning-denying freestanding kernel Clippy for
`stage20-7-test` passes. Evidence is in
`docs/graphics/stage-11-motion-evidence.md`; full Stage 11 screenshot/theme,
starvation, and low-power acceptance remains open.

The Stage 20.7 matrix now also injects neutral pointer packets throughout the
fade and passes its sustained-input starvation check on 1/2/4 CPUs. Full Stage
11 screenshot/theme and low-power acceptance remains open.

The earlier desktop screenshot qualification stopped before the session
because the production system-app catalog could not fit `woven-images.wapp`.
After raising the bounded VFS node capacity to 256 KiB, the full desktop
qualification passed on 1/2/4 CPUs with six captured PPM frames per run and
signed-in WovenFiles/WovenTerminal interaction evidence under
`audit-artifacts/desktop-qualification-1792952128648637100`,
`audit-artifacts/desktop-qualification-1792952312640236600`, and
`audit-artifacts/desktop-qualification-1792952447677142500`. Theme-variation
screenshots and low-power runtime remain open.

## Graphics Stage 11 shared-theme visual regression - 2026-10-25

Added allocation-free golden pixel signatures for a representative focused
Button and checked Checkbox composition under dark, light, and high-contrast
WovenUI themes. All three themes remain visually distinct and stable. The
libwoven suite passes 183/183 tests and warning-denying all-targets Clippy
passes. This is host visual-regression evidence; the full desktop screenshot,
starvation, and low-power gates remain open.

## Graphics Stage 11 Quick Settings accessibility acceptance - 2026-10-25

Repaired the Stage 20.10 oracle to match the current shell's branded wallpaper
weave and flat compatibility presentation. The panel model now composes its
translucent background over the actual beneath-surface, including the tiled
client. The complete Quick Settings gate passed on 1/2/4 CPUs with exit 33,
covering Reduced Motion visibility, Do Not Disturb, tiling, scale 1/2,
and now an actual Reduced Motion toggle with visible `On` state, plus
keyboard/pointer routing and teardown. Warning-denying freestanding kernel
Clippy for `stage20-10-test` passes. Full Stage 11 screenshot/theme,
starvation, and low-power acceptance remains open.

## Graphics Stage 20.8/20.13 flat-renderer oracle repairs - 2026-10-25

Updated the tiling/resize and workspace acceptance oracles to the current
flat WovenShell renderer: branded wallpaper gradient/weave and taskbar token.
Stage 20.8 and Stage 20.13 each passed on 1/2/4 CPUs with QEMU exit 33,
covering edge snap, resize, master/stack tiling, workspace switching, focus,
hidden-window teardown, and display recovery. These close additional Stage 10
dependency slices; the full roadmap Stage 10 acceptance remains broader than
these component gates.

## Graphics Stage 20.4 interaction oracle repair - 2026-10-25

Repaired the current-head window interaction oracle to match WovenShell's
accepted flat software default: the previous fixture still expected an old
palette, weave pitch, and projected 3D wallpaper. The move, repaint, close,
focus, and teardown gate now passes on 1/2/4 CPUs with QEMU exit 33. This
repairs the Stage 10 dependency slice; full roadmap Stage 10 still requires
its complete focus, snapping, workspace, clipboard, keyboard-navigation, and
accessibility acceptance matrix.

## Graphics roadmap Stage 09 compositor and composition scheduler - 2026-10-25

Stage 09 is accepted for the compositor scheduler scope. The roadmap-native
`stage09-compositor-test` gate passed on 1/2/4 CPUs with QEMU exit 33. It
combines the damage-only composition contract with overlapping-window
stacking, raise/focus, keyboard routing, compositor lease ownership, and
owner teardown. The gate also removes the damage fixture before exercising
the window-manager fixture, so both paths are validated in one boot without
stale filesystem state. Existing Stage 20.8 remains the complementary
resize/tiling/killed-window recovery gate. Evidence is recorded in
`docs/graphics/stage-09-compositor-evidence.md`.

## Graphics roadmap Stage 08 secure userspace surfaces and IPC - 2026-10-25

Stage 08 is accepted for the secure userspace surface/IPC and QEMU scope. The
roadmap-native `stage08-graphics-test` gate passed on 1/2/4 CPUs with exit 33:
two independent Ring-3 processes submitted surfaces, unauthorized private
surface reads were refused, shared pixels composed correctly, and client
surfaces/grants were released on exit. Evidence is recorded in
`docs/graphics/stage-08-surfaces-evidence.md`.

## Codec qualification update - 2026-10-25

- External randomized codec campaign: **PASS** — 50,000 inputs and 250,000
  decoder cases through `tools/codec-fuzz`.
- Multi-frame GIF playback emission: implemented, unit-tested, and guest
  qualified through the WovenImages QEMU gate.
- Caller-workspace-bounded WebP final decoding: **implemented and tested** via
  `decode_webp_rgba_stream_bounded`, with dimension/file/memory limits.
- Strict whole-process allocation certification: **PASS for the pinned
  exercised paths** through `tools/webp-allocation-cert`; peak live allocation
  was 702,669 bytes under a hard 1 MiB budget, with no leak. Universal proof
  for every future zenwebp code path is not claimed because the backend
  documents that some internal allocations may ignore its configured limit.
- WovenImages guest qualification: **PASS** on 1/2/4 CPUs, exit 33, with
  streamed BMP, interlaced GIF, and two-frame GIF playback markers.

## Bounded multi-frame GIF playback emission - 2026-10-25

`decode_gif_animation_stream` now emits every source frame through a
synchronous `GifFrameSink`, using caller-owned canvas/previous buffers for
composition. Frame delays, Background disposal, Previous disposal, local
palettes, transparency, interlace mapping, and fail-closed malformed input
handling are integrated. The 180-test libwoven suite and host/freestanding
warning-denying Clippy pass.

## Bounded GIF playback state - 2026-10-25

`GifPlayback` now provides caller-owned canvas/previous buffers, frame timing
metadata, and checked Keep/Background/Previous disposal behavior. The 179-test
libwoven suite and host/freestanding warning-denying Clippy pass. The GIF
source parser still emits only one frame; multi-frame parsing and playback
integration remain open.

## Codec truncation/property qualification - 2026-10-25

The codec suite now exhaustively checks every truncated BMP/GIF fixture prefix
and malformed/truncated WebP stream prefix, requiring fail-closed errors. The
178-test libwoven suite and host/freestanding warning-denying Clippy pass.
External fuzzing and long-running randomized campaigns remain separate
qualification work.

## Userspace stack qualification for streamed codecs - 2026-10-25

The WovenImages Ring-3 self-test exposed a stack fault while entering the
streamed GIF path under the previous 16-KiB stack. LZW tables are heap-backed
with fixed sizes, and `UserStack::SIZE` is now 64 KiB plus its guard page to
provide bounded codec/browser/compositor headroom. Host/freestanding lint and
the corrected 1/2/4-CPU guest matrix pass; serial evidence is retained under
`audit-artifacts/woven-images-{1,2,4}cpu`.

## Memory-bounded interlaced GIF rows - 2026-10-25

The source-driven GIF decoder now supports the four-pass interlace mapping
while retaining fixed palette/LZW workspace and direct caller-owned output.
The 176-test libwoven suite and host/freestanding warning-denying Clippy pass.
Animated frame timing, disposal, and playback remain open.

## Memory-bounded GIF stream decoder - 2026-10-25

`decode_gif_rgba_stream` now parses GIF headers, palettes, sub-blocks, and LZW
codes directly from `ImageStream`; it no longer assembles the encoded payload.
The decoder retains only fixed palette/LZW tables and a 1024-byte source
request, then writes directly into caller-owned pixels. The 175-test libwoven
suite and host/freestanding warning-denying Clippy pass. Animation, disposal,
and timing remain open.

## GIF incremental-delivery boundary - 2026-10-25

`libwoven::codecs::decode_gif_rgba_stream` now consumes GIF sources through
validated 1024-byte requests and hands the checked payload to the bounded
single-frame GIF decoder. Source truncation, allocation failure, and metadata
mismatch fail closed. The 175-test libwoven suite and host/freestanding
warning-denying Clippy pass. Memory-bounded GIF decoding and animation remain
open.

## Bounded BMP row streaming - 2026-10-25

`libwoven::codecs::decode_bmp_rgba_stream` now reads BMP headers and rows via
the shared `ImageStream` boundary with validated 1024-byte requests. It
supports uncompressed 24/32-bit BMPs, preserves caller-owned RGBA output, and
rejects indexed, compressed, oversized-row, and malformed inputs. The
174-test libwoven suite and host/freestanding warning-denying Clippy pass.

## Bounded JavaScript boolean/comparison operators - 2026-10-25

The scalar runtime now supports boolean literals, `!`, `==`, `!=`, integer
ordering, and boolean `&&`/`||` with checked precedence and type validation.
The 173-test libwoven suite and host/freestanding warning-denying Clippy pass.
Objects, strings, functions, DOM, async, modules, JIT, and sandbox
certification remain open.

## Bounded GIF pixel decoder - 2026-10-25

`libwoven::media::decode_gif_rgba` now rasterizes a validated first GIF frame
with global/local palettes, transparency control, and bounded LZW tables into
caller-owned RGBA storage. Interlaced and animated/multi-frame inputs fail
closed. The 172-test libwoven suite and host/freestanding warning-denying
Clippy pass. GIF streaming delivery, animation playback, and GPU decode remain
separate gates.

## Bounded browser script seam - 2026-10-25

`libwoven::browser::execute_script` now connects inline `<script>` node text
to the bounded scalar JavaScript runtime. It rejects non-script nodes and
missing source, preserves fixed runtime capacity, and exposes no DOM or
system capabilities. The 171-test libwoven suite and host/freestanding
warning-denying Clippy pass. Full ECMAScript, DOM, module, async, and browser
sandbox gates remain open.

## Browser software-canvas renderer - 2026-10-25

`libwoven::browser::render` now consumes the bounded paint list through the
existing software `Canvas`, rendering fill and text commands into caller-owned
pixels. Coordinate conversions are checked and fail closed. The 170-test
libwoven suite and host/freestanding warning-denying Clippy pass. GPU
composition, full CSS, networking, JavaScript, and text shaping remain open.

## Browser paint-command boundary - 2026-10-25

`libwoven::browser::paint` now turns the bounded DOM/layout result into a
capacity-checked renderer-neutral command list for background fills and text.
Inline six-digit CSS hex `color` and `background-color` are supported, with
tests for command ordering and capacity failure. The 169-test libwoven suite
and host/freestanding warning-denying Clippy pass. Full CSS cascade, layout
features, network, JavaScript, shaping, and GPU composition remain separate
gates.

## Incremental WebP stream delivery - 2026-10-25

`libwoven::codecs::decode_webp_rgba_stream` now consumes WebP input through
the shared `ImageStream` boundary in validated 1024-byte requests, checks the
decoded dimensions, and writes RGBA pixels into caller-owned storage. The
pinned `zenwebp 0.3.2` backend buffers the encoded RIFF payload internally
before rasterization, so this closes incremental input delivery but does not
claim memory-bounded WebP decoding. Animation remains outside this gate.

## WovenFiles shared-control migration - 2026-10-25

WovenFiles now uses shared bounded Label and Button controls for its title and
Refresh action. The existing VFS listing, resize behavior, and shell window
protocol remain unchanged; the shared release-inside pointer contract is now
used for activation. Freestanding warning-denying Clippy passes. Broader
application migration remains in progress.

## Bounded streaming PNG/JPEG codec reader - 2026-10-25

`libwoven::codecs` now exposes a seekable `ImageStream` boundary for PNG and
JPEG decoding. Reads are split into validated 1024-byte source requests and
pixels remain caller-owned. The 166-test libwoven suite, host/freestanding
Clippy, and WovenImages target lint pass. WebP has a separate incremental
delivery path, but its backend buffers the encoded payload before decode.

## Bounded freestanding JavaScript runtime foundation - 2026-10-25

`libwoven::javascript::Runtime` provides a no-std fixed-capacity scalar
evaluator for integer arithmetic, precedence, comments, `let` bindings,
updates, and statement sequencing. Script length, variable count, nesting,
and arithmetic are checked and fail closed. The 165-test libwoven suite and
host/freestanding warning-denying Clippy pass. Full ECMAScript semantics,
DOM bindings, promises, modules, JIT execution, and sandbox certification
remain open.

## Scalable I/O and launcher capacity increment - 2026-10-25

The safe userspace API no longer treats the kernel's per-syscall transfer
validation as an application-level payload ceiling. `libwoven::read_all`,
`write_all`, and `write_file_all` stream arbitrarily sized buffers in validated
chunks; standard output and media VFS loading use those helpers. The WovenShell
launcher no longer truncates the production catalog at ten entries: its
metadata staging capacity is 64 applications and its child-process staging is
32. Pointer, integer-overflow, memory, IPC, path, and resource limits remain
intentional safety gates and are not removed. Host tests and warning-denying
Clippy pass; production system-app QEMU passes at 1/2/4 CPUs with 1280x800
desktop evidence.

## Graphics roadmap Stage 07 image/asset increment - 2026-10-25

Stage 07 is accepted for the freestanding/QEMU scope. The wallpaper
deliverables are complete: a CC0-1.0, SHA-verified deterministic BMP is
generated by `kernel/build.rs`, embedded in the kernel build, installed
read-only at `/share/wallpapers/wovenhat-gradient.bmp`, and read back through
the boot VFS image. WovenImages now loads and decodes the VFS asset and draws
its thumbnail. Freestanding WebP uses pinned alloc-only `zenwebp 0.3.2`, with
VP8, VP8L, and VP8X/ALPH coverage. The 160-test suite, warning-denying
checks, and dedicated signed Ring-3 QEMU self-test passed at 1/2/4 CPUs. See
`docs/graphics/stage-07-images-evidence.md`.

## Graphics roadmap Stage 06 iconography - 2026-10-25

Stage 06 is accepted for the versioned code-native icon pack and WovenShell
integration. Eight semantic icons, MIME metadata, Normal/Active/Disabled
states, Light/Dark themes, and integer-DPI rendering are covered by 152
libwoven tests. All Ring-3 release images build, and the launcher/QEMU
acceptance passed at 1/2/4 CPUs. SVG import, external thumbnail services, and
physical/GPU qualification remain outside this bounded gate. See
`docs/graphics/stage-06-iconography-evidence.md`.

## Graphics roadmap Stage 05 typography increment - 2026-10-25

Stage 05 is accepted for the bundled static-font kernel/QEMU scope. The
bounded `libwoven::graphics::FontFallback` provider and glyph cache,
`TextInputState` UTF-8 editing contract, licensed Noto Sans fixture, kernel
sfnt/cmap/hmtx/glyf loader, and raster self-test are implemented. Host tests,
warning-denying Clippy, and the kernel QEMU gate pass at 1/2/4 CPUs. See
`docs/graphics/stage-05-typography-evidence.md` for limits and evidence.
The userspace text layer now also provides full UAX #9 reordering and
transactional bounded IME preedit/commit/cancel behavior; the full libwoven
suite passes 150 tests.
The existing guest font-coverage gate passed at 1/2/4 CPUs, and the new
bundled-font loader/raster gate passed at 1/2/4 CPUs. Full UAX #9 reordering
and IME composition are now implemented; script-specific glyph shaping remains
an OpenType capability boundary.

## Graphics roadmap Stage 04 raster/vector acceptance - 2026-10-25

Stage 04 is accepted for software renderer scope. Bounded raster/vector,
clipping, gradients, transforms, alpha, antialiasing, and stale-surface tests
pass; the `gfx02-test` QEMU matrix passed at 1/2/4 CPUs. See
`docs/graphics/stage-04-raster-vector-evidence.md`.

## Graphics roadmap Stage 03 accepted - 2026-10-25

Stage 03 is accepted for software/UEFI/QEMU scope. Damage/ownership and
scaling/frame-pacing gates passed at 1/2/4 CPUs; evidence is recorded in
`docs/graphics/stage-03-presentation-damage-evidence.md`.

## Graphics roadmap Stage 02 accepted - 2026-10-25

The supported-QEMU Stage 02 gate is accepted in
`docs/graphics/stage-02-display-hal-evidence.md`. Single-output and two-head
scanout passed at 1/2/4 CPUs, and fault-injected VirtIO presentation recovery
passed at 1/2/4 CPUs. Advertised mode diagnostics and bounded 1920x1080-per-
output support are implemented. Physical display/GPU certification remains
unclaimed.

## Graphics display recovery increment - 2026-10-25

Commit `4062f3eb` adds bounded VirtIO reprobe/resource rebuild after a
presentation failure and falls back to the UEFI framebuffer if reprobe fails.
Kernel warning-denying Clippy and 147 libwoven tests pass; QEMU single-output
scanout passes. Stage 02 remains partial until fault-injected guest recovery
and supported-mode coverage are executed.

## Graphics roadmap Stage 03 buffered presentation - 2026-10-25

`docs/graphics/stage-03-presentation-damage-evidence.md` records the green
damage/ownership sub-gates. Real QEMU hello-gui damage acceptance passed at
1/2/4 CPUs. Dedicated frame pacing, repeated buffer reuse, and flicker/stress
evidence remain open, so Stage 03 is partial.

## Graphics roadmap Stage 01 architecture contracts - 2026-10-25

Accepted the current-boundary architecture contract in
`docs/graphics/stage-01-architecture-contracts.md`. It maps WovenDisplay,
WovenRender, WovenText, WovenAssets, WovenCompositor, WovenWindow, WovenUI,
and WovenDesktop to concrete source paths and records capability, pointer,
lock, bounded-resource, CPU-fallback, migration, and rollback invariants.
This stage does not claim later web, codec, JavaScript, GPU, or hardware gates.

## Graphics roadmap Stage 02 display HAL - 2026-10-25

Partial evidence is recorded in `docs/graphics/stage-02-display-hal-evidence.md`.
The real VirtIO-GPU single-output and two-head scanout harnesses passed at
1/2/4 CPUs with exact pixel captures. EDID/full supported-mode diagnostics and
injected display error recovery remain open, so Stage 02 is not accepted.

## Graphics roadmap Stage 00 baseline acceptance - 2026-10-25

The complete graphics Stage 00-24 roadmap was audited against the current
repository and preserved runtime evidence. The detailed ledger is
`docs/graphics/roadmap-2026-10-25-stage-00-audit.md`. The initial sign-in
timeout was fixed in the headless QMP input and PPM harness paths; the corrected
real-QEMU desktop qualification passed serially at 1/2/4 CPUs with 5 GiB
configured RAM and the 4 GiB usable-memory floor. Stage 00 is accepted. No
later graphics stage is accepted by this entry.

## Browser layout substrate increment - 2026-10-25

`libwoven::browser` now provides a fixed-capacity borrowed DOM layout pass
with inline `width`, `height`, `margin`, and `display:none` handling. It emits
checked block rectangles for a caller viewport and rejects invalid or
overflowing layouts. The 162-test libwoven suite and host/freestanding
warning-denying Clippy pass. Full CSS cascade, networking, JavaScript,
sandboxing, and pixel rendering remain open.

## Bounded WebView HTTP(S) network boundary - 2026-10-25

Added `libwoven::web` with allocation-free HTTP(S) endpoint parsing,
credential/control-character rejection, bounded explicit/default ports, and
HTTP/1.1 response-head parsing with content-length extraction. The boundary
is designed for the existing async TCP layer; TLS, socket policy, body
streaming, and browser execution remain separate. The full libwoven host
suite passed 143 tests, Settings warning-denying Clippy passed, and
formatting/diff checks passed.

## WovenUI bounded WebView host boundary - 2026-10-25

Added a fail-closed WebView host boundary with bounded HTTP(S) URI
validation, control-character rejection, explicit loading completion state,
focus rendering, and unsupported-content presentation. The full libwoven host
suite passed 141 tests, Settings warning-denying Clippy passed, and
formatting/diff checks passed. A browser engine, network stack, JavaScript
runtime, and security certification are not claimed.

## WovenInstaller shared-control migration - 2026-10-25

WovenInstaller now uses shared bounded Label and Button rendering for its
title and Close action while preserving the installer state and WCLS close
protocol. Target Clippy, target build, formatting, and diff checks passed. No
QEMU client acceptance is claimed for this increment.

## WovenCalculator shared-control migration - 2026-10-25

WovenCalculator now uses shared bounded Label and Button rendering, and its
Evaluate action uses the Button release-inside contract while preserving the
existing evaluator. Target Clippy, target build, formatting, and diff checks
passed. No QEMU client acceptance is claimed for this increment.

## WovenUI bounded ImageView and VideoView components - 2026-10-25

Added caller-owned, allocation-free ImageView and VideoView controls.
ImageView validates source bounds and performs bounded nearest-neighbor
scaling; VideoView provides deterministic playback state and current-frame
rendering. The full libwoven host suite passed 139 tests, Settings
warning-denying Clippy passed, and formatting/diff checks passed. Codec and
browser-engine functionality are not claimed by these primitives.

## WovenUI bounded Navigation component - 2026-10-25

Added a fixed-capacity vertical Navigation control to `libwoven::ui` with
bounded insertion, keyboard/pointer selection, focus rendering, and selected
item painting. The full libwoven host suite passed 137 tests, Settings
warning-denying Clippy passed, and formatting/diff checks passed. Media/view
abstractions and broader application migration remain open in this stage.

## WovenUI bounded Tabs and Dialog components - 2026-10-25

Added fixed-capacity Tabs and a bounded modal Dialog to `libwoven::ui`.
Tabs support keyboard/pointer selection and focus rendering; Dialogs own
explicit bounded accept/cancel actions and close only while open. The full
libwoven host suite passed 136 tests, Settings warning-denying Clippy passed,
and formatting/diff checks passed. Navigation and broader application
migration remain open in this stage.

## WovenUI bounded Menu and Tree components - 2026-10-25

Added fixed-capacity Menu and hierarchical Tree controls to `libwoven::ui`.
Menu activation and pointer selection fail closed when disabled or outside the
frame; Tree parent validation rejects invalid references and rendering is
depth-limited. The full libwoven host suite passed 134 tests, Settings
warning-denying Clippy passed, and formatting/diff checks passed. Dialog,
Tabs, Navigation, and broader application migration remain open in this
stage.

## WovenUI bounded Table component - 2026-10-25

Added a fixed-capacity Table to `libwoven::ui` with borrowed cells, bounded
row insertion, keyboard row navigation, pointer row selection, focus
rendering, and selected-row rendering. The full libwoven host suite passed
132 tests, Settings warning-denying Clippy passed, and formatting/diff checks
passed. Tree, Menu, Dialog, Tabs, Navigation, and broader application
migration remain open in this stage.

## WovenUI bounded ListBox component - 2026-10-25

Added a fixed-capacity selectable ListBox to `libwoven::ui` with borrowed
labels, bounded insertion, keyboard navigation, pointer row selection, focus
rendering, and selection rendering. The full libwoven host suite passed 130
tests, Settings warning-denying Clippy passed, and formatting/diff checks
passed. Table, Tree, Menu, Dialog, Tabs, Navigation, and broader application
migration remain open in this stage.

## WovenTerminal shared-control migration - 2026-10-25

WovenTerminal now uses shared bounded Label and Button rendering, and its
Clear action uses the Button release-inside contract. Target Clippy, target
build, formatting, and diff checks passed. No QEMU client acceptance is
claimed for this increment.

## WovenStore shared-control migration - 2026-10-25

WovenStore now uses shared bounded Label and Button rendering, and its Refresh
action uses the Button release-inside contract. Target Clippy, target build,
formatting, and diff checks passed. No QEMU client acceptance is claimed for
this increment.

## WovenImages shared-control migration - 2026-10-25

WovenImages now uses shared bounded Label and Button rendering, and its Refresh
action uses the Button release-inside contract. Target Clippy, target build,
formatting, and diff checks passed. No QEMU client acceptance is claimed for
this increment.

## WovenEditor shared-control migration - 2026-10-25

WovenEditor now uses shared bounded Label and Button rendering, and its Refresh
action uses the Button release-inside contract. Target Clippy, target build,
formatting, and diff checks passed. No QEMU client acceptance is claimed for
this increment.

## WovenMonitor shared-control migration - 2026-10-25

WovenMonitor now uses shared bounded Label and Button rendering, and its
Refresh action uses the Button release-inside contract. Target Clippy, target
build, formatting, and diff checks passed. No QEMU client acceptance is
claimed for this increment.

## WovenUpdate shared-control migration - 2026-10-25

WovenUpdate now uses shared bounded Label and Button rendering, and its
Refresh action uses the Button's release-inside contract rather than a
hand-painted hit box. Target Clippy, target build, and formatting passed. The
app-specific QEMU harness failed before sign-in because its default guest had
insufficient usable memory; the proper 5 GiB desktop gate timed out during
sign-in under this TCG run. The migration is accepted as source/build work,
not as a passed QEMU acceptance gate.

## WovenUI keyboard/pointer action boundary - 2026-10-25

Buttons now expose enabled-aware keyboard/accessibility activation, and
TextFields use bounded pointer coordinates to acquire or clear focus. The
libwoven host suite passed 128 tests with interaction coverage, Settings
warning-denying Clippy passed, and formatting/diff checks passed. Complete
focus traversal and full application migration remain open.

## WovenSettings shared-label migration - 2026-10-25

WovenSettings now uses the shared bounded WovenUI Label primitive for its title
and Identity heading. The libwoven host suite passed 128 tests, Settings
warning-denying Clippy passed, and formatting/diff checks passed. A Checkbox
is not used for a cosmetic option; it remains reserved for a real persisted
setting.

## WovenUI labels and checkboxes - 2026-10-25

Added bounded allocation-free Label and Checkbox primitives to `libwoven::ui`.
Checkbox activation is enabled-state aware and release based; both controls
have rendering coverage. The libwoven host suite passed 128 tests and the
Settings warning-denying Clippy gate passed. The complete WovenUI catalog and
full application migration remain in progress.

## WovenUI bounded native widget foundation - 2026-10-25

Added `libwoven::ui` with bounded allocation-free Button and UTF-8-safe fixed
capacity TextField primitives, shared theme tokens, and vertical stack layout.
WovenSettings now renders its Refresh control through the shared Button.
The libwoven host suite passed 126 tests, Settings warning-denying Clippy
passed, and changed files are rustfmt-clean. The complete WovenUI catalog and
full application migration remain in progress.

## Stage 20.17/20.18 3D acceptance repair - 2026-10-25

Updated the stale 3D acceptance fixtures to the current branded teal
wallpaper/accent and made both tests begin with `--no-3d` before exercising the
real Quick Settings 3D toggle. Stage 20.17 Quick Settings and Stage 20.18
window title bars passed warning-denying kernel Clippy and QEMU acceptance on
1, 2, and 4 CPUs. This closes the software 3D presentation gates only; GPU
acceleration and native hardware qualification remain separate.

## Stage 20.1 graphics oracle and fallback contract - 2026-10-25

Closed the branded wallpaper/taskbar mismatch in the foundational desktop
gate. Stage 20.1 now explicitly qualifies the deterministic flat software
renderer with `--no-3d`; production WovenShell remains 3D by default and its
dedicated 3D stages are unchanged. The oracle was corrected to the current
branded teal accent, start-button fill, and one translucent taskbar blend.
Warning-denying kernel Clippy passed, and QEMU acceptance passed on 1, 2, and
4 CPUs, including clock advancement, framebuffer scaling, shell teardown, and
display return. This closes Stage 20.1's software/QEMU gate only.

## Graphics mandate baseline audit - 2026-10-25

Added `docs/graphics/current-state-audit.md` from the pasted graphics
modernization mandate. The current system has an automatic system-apps QEMU
desktop, buffered software display, compositor, VirtIO-GPU 2D support, and
WovenShell visual polish. The audit keeps full graphics phases, the Stage 20.1
projection-oracle mismatch, hardware acceleration, native qualification, and
independent review open or unverified where evidence is absent.

## Launcher and desktop visual polish - 2026-10-25

Added readable code-native app icons to the launcher and desktop shortcuts for
Files, Terminal, and Store, including the `Woven*` production catalog names.
The launcher remains a crisp foreground surface while the desktop retains its
3D wallpaper/taskbar presentation. Full-screen recomposition on launcher close
prevents stale projected edges. Stage 20.3 launcher QEMU acceptance passed on
1 CPU and WovenShell warning-denying Clippy passed. Stage 20.1's branded
wallpaper oracle still needs a small projection-rounding repair and remains
open; no production graphics gate is claimed fully green from this change.

## CI blocker repair - 2026-10-25

Closed the locally reproduced red CI defects: Stage 13.12 now has a complete
OVMF VARS argument/copy/attachment path; Stage 20.5's independent oracle is
aligned with the current branded shell palette and explicitly exercises the
flat depth compositor; and live FAT32 batch persistence passes the focused
storage gate. The 1/2/4-CPU focused QEMU results are green for Stage 20.5,
storage, and driver restart. This closes the software/QEMU blockers only; it
does not change the separate physical/native qualification or production trust
boundaries.

## Release Completion Mandate M00 baseline - 2026-10-24

Completed the mandate's inventory/closure baseline at commit `3eee80b8`.
Added the versioned ledger, subsystem dependency graph, and machine-readable
resume checkpoint under `docs/release-qualification/`. The baseline records
the current 158/158 bounded release chain, installed-image 1/2/4-CPU evidence,
the unaccepted strict desktop gate, the fail-closed virtual trust preflight,
and the separate physical/native qualification boundary. M00 is complete as a
documentation and reproducibility gate; it is not a production-readiness
claim. M01 is the next permitted milestone.

## Release Completion Mandate M01 - 2026-10-24

Closed the current-head Stage 20.5 focused-shadow regression by making the
accepted 2D compositor path the default (`apps/woven-shell/src/main.rs`);
3D remains opt-in pending its own pixel acceptance matrix. Stage 20.5 and
Stage 20.15 passed on 1/2/4 CPUs. FAT32 persistence passed on NVMe and AHCI on
1/2/4 CPUs. Stage 31 write-fault containment passed on 1/2/4 CPUs by observing
the expected fail-closed batch-persist marker. M01 is accepted for software
and QEMU scope; physical/native, Secure Boot/TPM and release-key gates remain
open.

## Release Completion Mandate M02 - 2026-10-24

Closed the boot-to-desktop and session-reliability slice on the current tree.
The strict QEMU desktop qualification passed on 1/2/4 CPUs with 5 GiB
configured guests, leaving at least 4 GiB usable after firmware reservations,
covering sign-in, first desktop frame, Files launch/composition, mouse input,
Files close, Terminal launch/composition, keyboard input, Terminal close, and
post-close framebuffer restoration. The harness now uses the shell's actual
cascade geometry and recomposes the full screen when a client surface is
released, preventing stale pixels after window teardown. Its default input
budget is 20 seconds to match the accepted TCG/QEMU measurements while
remaining bounded. The 4 GiB OS floor is a usable-memory requirement; QEMU's
4 GiB configuration is insufficient because firmware/device reservations
reduce the kernel-visible usable map.

The normal shell repeated-boot suite passed two sequential boots at 1/2/4
CPUs, and the guest reset gate passed at 1/2/4 CPUs. Evidence is retained in
the timestamped `audit-artifacts/desktop-qualification-*`,
`audit-artifacts/stage31-repeated/shell-{1,2,4}cpu-2boots-rerun-*/`, and
`audit-artifacts/stage13.20-reboot-{1,2,4}cpu-*` directories. M02 is accepted
for software/QEMU cold-boot, interaction, and guest-reset scope; physical
power loss, native-device recovery, TPM/Secure Boot, and production signing
remain open.

## Release Completion Mandate M03 - 2026-10-24

The current-head kernel safety/concurrency regression gate passed: `cargo test
--all-targets` completed successfully, the full Python host suite completed
204 tests successfully, warning-denying kernel-target Clippy passed, and
warning-denying host all-targets Clippy passed. The existing owner-generation,
IPC, IRQ-mailbox, allocator, PCI resource, user-address, filesystem crash,
and input/graphics contract tests remain green. M03 is accepted for the
repository's software safety/concurrency scope; arbitrary in-guest faults,
physical power-loss/torn-sector behavior, native devices, and independent
security review remain outside this evidence.

## Release Completion Mandate M05 - 2026-10-24

The emulated hardware-facing software slice passed on 1/2/4 CPUs: controller-
owned NVMe/AHCI/xHCI/HDA restart passed; Stage 20.14b graphics scanout passed
on both heads; and Stage 13.13 USB input passed. The clean repeated shell
roots also confirm `meets_minimum=true` with 5 GiB configured QEMU guests and
at least 4 GiB usable. M05 is accepted for deterministic QEMU/device-model
scope. Native GPU, USB HID, storage, audio, network, Bluetooth, and physical
hardware certification remain open.

## Release Completion Mandate M06 - 2026-10-24

The bounded network/radio software slice passed on 1/2/4 CPUs. The live
network gate passed DHCP, DNS, ICMP, host-verified UDP/TCP, and a seeded QEMU
virtio-net link loss/recovery cycle at every profile. Stage 13.10 passed the
Wi-Fi protocol, security, firmware/backend, Bluetooth LE, and transport
contract markers at 1/2/4. M06 is accepted for software and emulated-device
scope; native Wi-Fi/Bluetooth hardware, RF behavior, physical interoperability,
and certification remain open.

## Release Completion Mandate M07 - 2026-10-24

The graphics/compositor software slice passed on 1/2/4 CPUs. Stage 20.14b
dual-head scanout, Stage 20.14S single-output pixel verification, and the
GFX-02 CPU renderer feature gate all passed. M07 is accepted for the bounded
software/QEMU graphics path; native Intel/AMD/NVIDIA GPU drivers, physical
scanout, accelerated performance, and device-loss recovery remain open.

## Release Completion Mandate M08 - 2026-10-24

The desktop UX slice is covered by the current strict 1/2/4-CPU desktop
matrix: sign-in, first frame, keyboard and mouse routing, window composition,
teardown, framebuffer restoration, and guest reset all passed. M08 is accepted
for software/QEMU UX scope; native accessibility devices and physical display
qualification remain open.

## Release Completion Mandate M09 - 2026-10-24

The current-head authoritative release chain passed 158/158 checks. This
includes the signed 11-application catalog and boot, WovenStore app-store
flows, package transactions and rollback, update, security, monitor, settings,
editor, images, terminal/files, AI broker/providers/actions/search/security,
service supervisor recovery, and application lint gates on the required
1/2/4-CPU matrix. M09 is accepted for software/QEMU scope; production trust
provisioning and native certification remain open.

## Release Completion Mandate M10 - 2026-10-24 - BLOCKED

The virtual trust preflight remains fail-closed. QEMU and OVMF CODE are
available, but this host has no x86_64 OVMF VARS template, `swtpm`, OpenSSL, or
caller-supplied development key/certificate. The release-key boundary test
passes by rejecting missing material and accepting only matching disposable
test material. No key was invented, no firmware variable store was modified,
and no Secure Boot/TPM qualification is claimed.

## Release Completion Mandate M11 - 2026-10-24

Recovery software is covered by the accepted Stage 27.5 recovery, update
rollback, WovenFS crash-atomicity, FAT32 remount/FSINFO repair, sudden-shutdown
and service-supervisor evidence recorded in M04/M09. Native recovery and
physical power-loss evidence remain open.

## Release Completion Mandate M12 - 2026-10-24

Software performance/repeatability evidence is present: Stage 30 benchmark
history, the current 1/2/4 release matrix, full host tests, and isolated
repeated shell boots with the usable 4-GiB floor. This is bounded QEMU/host
evidence, not a native performance or long-duration production soak.

## Release Completion Mandate M13 - 2026-10-24 - BLOCKED

Physical qualification and native hardware certification remain `PHYSICAL_NOT_TESTED`.
The read-only host preflight and deterministic hardware stimulator are
available, but no physical media, firmware, TPM, radio, GPU, HID, or reboot
operation was performed on the laptop.

## Release Completion Mandate M14 - 2026-10-24 - BLOCKED

Software release packaging and provenance pass through the current 158/158
chain, but production signing-key custody, Secure Boot/TPM trust, clean-room
provenance, SBOM attestation, and independent security review are not
provisioned in this environment.

## Release Completion Mandate M15 - 2026-10-24 - NO-GO

The software/QEMU gates are green through the current release chain, but the
mandatory trust, physical/native, and independent-review gates are open.
Final release status is **NO-GO**; production readiness is not claimed.

## Release Completion Mandate M04 - 2026-10-24

The persistent-storage and installer slice passed in disposable scope. Stage 28
GPT/FAT32/EFI/provisioning and rollback acceptance passed; installed-image
boot with exact digest verification passed on 1/2/4 CPUs; the focused
installer/storage suite passed 45 tests; Stage 27.5 recovery passed on 1/2/4;
and the M01 NVMe/AHCI FAT32 persistence and Stage 31 write-fault evidence
remain green. M04 is accepted for file-image and QEMU scope only. Physical
disk durability, power-loss/torn-sector media evidence, Secure Boot/TPM,
production keys, and native hardware remain open.

## Hardware qualification CI workflow - 2026-10-24

Added `.github/workflows/hardware-qualification.yml`. Push and pull-request
jobs run the qualification unit suites and deterministic plan-only checks on
Ubuntu and Windows; Ubuntu additionally runs the complete 108-job virtual
matrix and verifies zero failures plus `PHYSICAL_NOT_TESTED`. The workflow
uploads generated evidence and performs no raw-device, media-writing or
native-certification operation.

## Read-only physical qualification preflight - 2026-10-24

Added `scripts/physical-qualification-preflight.py` with a three-test
stdlib contract. It records host identity/capacity using read-only standard
library APIs, enumerates all 17 required qualification categories, rejects
raw-device-looking output paths, and keeps every category at
`PHYSICAL_NOT_TESTED` until a separate signed native evidence workflow exists.
The live preflight smoke produced the expected schema and 17-category report;
it did not open media, change firmware, reboot, or access credentials.

## Hardware-qualification matrix orchestrator - 2026-10-24

Added `scripts/run-hardware-qualification-matrix.py` with four unit tests.
It expands the declarative hardware matrix over deterministic profile, CPU and
RAM combinations, supports plan-only and evidence-producing runs, validates
each child result, and fails closed on native, tier-1, raw-device and physical
qualification arguments. Independent verification passed the unit suite and
a plan-only graphics run. The orchestrator writes only under its requested
output directory and keeps physical qualification at `PHYSICAL_NOT_TESTED`.

## Hardware-qualification stimulator foundation - 2026-10-24

Added `scripts/hardware-qualification-stimulator.py` and its four-test
stdlib test suite. The runner consumes `tests/hardware-matrix.json`, produces
deterministic seeded manifests/results/evidence, accepts the 1/2/4/8/16-CPU
and 256 MiB/1 GiB/4 GiB software profiles, and rejects physical-device/raw
disk/native/tier-1 arguments. A 16-CPU/4 GiB storage smoke generated
`EMULATED_PASS` evidence; the result explicitly records
`PHYSICAL_NOT_TESTED` and no physical access. This is a qualification
stimulator and evidence contract, not native hardware certification.

## Stage 28 software tree merge and qualification contract - 2026-10-24

The Stage 28 installer/provisioning tree and the subsequent application,
graphics, packaging, recovery and Stage 31 evidence from
`codex/maturity-audit-20261022` are now merged into local `master` at
`179f0737`. The merge includes the kernel installer boundary, disposable
GPT/FAT32/EFI installation executors, installer service/UI, system-app
packaging, and the existing 1/2/4-CPU software acceptance harnesses.

The new `tests/hardware-matrix.json` records deterministic emulated and
software-only qualification profiles, evidence fields, RAM/CPU dimensions,
fault classes and the rule that emulation cannot claim physical certification.
Focused hardware-qualification, installer-plan/UI, package-installer and
Stage 28 disposable acceptance tests passed via Python `unittest`; the
disposable Stage 28 test ran two image-backed cases in 158.451 seconds.

This is a software-tree integration and focused-test result, not a claim that
all software or hardware work is complete. Mouse/tablet, display-device,
centralized full-stack fault, abrupt QEMU power-loss, automatic driver-crash
recovery, 8/16-CPU expansion, physical hardware, Secure Boot/release-key
qualification and independent external review remain open where not covered
by existing evidence.

## P0-03/P0-07 current-head release validation after Stage 28 repair - 2026-10-24

The authoritative release chain passed all 158 checks with zero failures from
commit `6cc1806a`; current HEAD `792e6994` adds only stricter explicit
QEMU/OVMF preflight validation. It includes warning-denying kernel/host Clippy,
166 host tests, debug/release 1/2/4-CPU memory/storage/network gates,
read/write storage-fault containment, shell, service supervision, orderly
driver restart, application/system-app/package/AI gates, desktop boots, and
final system-app Clippy. The report is retained at
`target/release-validation/results.json`; `validated-image.json` records the
current source hashes and release image digest.

This closes the stale-release-report gap only. The result remains
QEMU/OVMF/release-validation evidence: physical/native hardware, physical
power-loss/torn-sector recovery, TPM/Secure Boot, production release-key
custody, arbitrary fault injection, automatic driver-crash recovery, and
independent Claude review remain open.

## Stage 28 bounded-resource acceptance repair - 2026-10-24

The disposable Stage 28 harness now releases its completed success fixture
before creating rollback fixtures. This preserves the existing success,
rollback, digest, settings, and deterministic-output assertions while avoiding
an unnecessary second 64 MiB image during the rollback phase. The focused
Stage 28 tests pass, and the complete host suite passes 166/166 on the current
working tree. This is a harness resource-discipline fix; it does not expand
physical installation or bootability claims.

## P0-03/P0-07 current-head release validation with storage faults - 2026-10-24

The authoritative release chain passed all 158 checks on commit `76a3b6f6`.
This includes the previous warning-denying kernel/host Clippy, host tests,
debug/release 1/2/4-CPU boot/storage/network matrix, shell and service
supervision, driver restart, application/system-app/package/AI gates, and
desktop boots, plus new disposable QEMU storage-read-fault and
storage-write-fault containment gates at 1/2/4 CPUs. The report is retained at
`target/release-validation/results.json`; zero checks failed.

The release result remains an emulated/release-validation result. Physical and
native hardware, physical power-loss/torn-sector recovery, TPM/Secure Boot,
release-key custody, arbitrary fault injection, and independent Claude review
remain separate release prerequisites.

## P0-03/P0-07 release validation baseline - 2026-10-23

The authoritative release chain passed all 152 checks on source commit
`3b3980e1`. Warning-denying kernel/host Clippy, every host test, debug and
release memory/storage/network QEMU gates at 1/2/4 CPUs, legacy PIC, shell,
service supervision, driver restart, application linting, system-app catalog
and desktop boots, package transactions, AI/security/search/action suites, and
final system-app Clippy all passed. The report is retained at
`target/release-validation/results.json`.

This is a source-commit emulated/release-validation result. It does not close
the separate physical/native hardware, physical power-loss, TPM/Secure Boot,
release-key custody, arbitrary fault-injection, or independent Claude-review
gates.

## Stage 31 seeded 11-scenario campaign with read/write device faults - 2026-10-24

The seeded Stage 31 orchestrator passed all eleven scenarios at 1 CPU with
seed `0x51A7E`: storage write-fault containment, service supervision, network
loss, storage read-fault recovery, packet corruption, process crash, low RAM,
service restart, driver restart, sudden shutdown, and memory boot. The
authoritative result is
`audit-artifacts/stage31-randomized-51a7e-1cpu-11scenarios-rerun-20261023T221235041205Z/result.json`
with eleven zero child return codes. A separate mistyped-firmware attempt was
recorded as an isolated failed plan and is not counted as acceptance evidence.

The write-fault child injects one `write_aio` `ENOSPC` at the FAT32 journal
sector and requires the kernel's explicit `FAILED at batch persist` boundary,
with no panic or false success. This is fail-closed containment evidence, not
physical durability or power-loss recovery.

## Stage 31 QEMU block write-fault containment matrix - 2026-10-24

The disposable QEMU write-fault gate passed at 1/2/4/8/16 CPUs. An
unrestricted first-write injection initially stalled during firmware probing;
the gate was corrected to target the FAT32 journal sector, and the bounded
kernel failure marker was then observed at every CPU count. Accepted evidence
is retained under timestamped
`audit-artifacts/stage31-storage-write-fault-{1,2,4,8,16}cpu-*` directories.

This qualifies device-error containment only. It does not claim write
ordering, flush durability, torn-sector recovery, physical power loss, native
media, or production readiness.

## Stage 31 seeded 10-scenario campaign with storage fault - 2026-10-23

The seeded Stage 31 orchestrator passed all ten scenarios at 1 CPU with seed
`0x51A7E`: process crash, sudden shutdown, multi-flap network loss, QEMU
storage read fault, packet corruption, low RAM, service supervisor recovery,
driver restart, service restart, and memory boot. The result is retained at
`audit-artifacts/stage31-randomized-51a7e-1cpu-10scenarios/result.json` with
all ten child return codes zero. The randomized runner now includes the
storage-read-fault child as a first-class bounded scenario.

This proves deterministic orchestration of bounded gates, not arbitrary
in-guest fault injection or native/physical qualification.

## Stage 31 QEMU block read-fault full CPU matrix - 2026-10-23

The disposable QEMU `blkdebug` read-fault gate passed at 8 and 16 CPUs in
addition to the earlier 1/2/4-CPU passes. Each run injected one ATA
`read_aio` `EIO`, recovered by retry, completed the FAT32 mutation, and
emitted the full boot marker. Evidence is retained under
`audit-artifacts/stage31-storage-read-fault-{1,2,4,8,16}cpu/`.

The Stage 31 torture planner now exposes this gate as a first-class
`storage-read-fault` suite and records its 1/2/4/8/16-CPU coverage. This is
still bounded emulated read-error recovery; write ordering, flush durability,
torn sectors, physical power loss, native media, and arbitrary fault
injection remain open.

## Stage 31 disposable QEMU block read-fault recovery - 2026-10-23

The new `scripts/test-stage31-storage-read-fault.py` gate attaches QEMU's
`blkdebug` layer to a workspace-created FAT32 image and injects one disposable
`read_aio` `EIO`. The full boot validation passed at 1/2/4 CPUs, with the
storage mutation path recovering by retry in each run. Evidence is retained
under `audit-artifacts/stage31-storage-read-fault-{1,2,4}cpu/`.

This is bounded emulated read-error coverage only. It does not qualify write
ordering, flush durability, torn sectors, physical power loss, native media,
or production readiness.

## P0-02 current-head WovenFS crash/fault model - 2026-10-23

`cargo test --test wovenfs_crash --quiet` passed all 16 tests on the current
tree. The suite includes full-volume `NoSpace` atomicity, format/create/write
crash cuts, randomized write-failure cut points, remount consistency, and
corruption detection. This is host-side WovenFS model evidence; it does not
replace QEMU block-device errors, physical torn-sector/power-loss testing, or
native media qualification.

## Stage 29 qualification-report fail-closed hardening - 2026-10-23

Hardened the read-only hardware qualification report validator: `tier1` now
requires a passed native observation, and evidence paths reject parent
directory traversal as well as absolute paths. The focused hardware-report
suite passes 7 tests and the validator is syntax-clean. This improves the
qualification record boundary only; no native hardware was probed or
certified.

## Stage 31 seeded 9-scenario campaign and evidence isolation - 2026-10-23

The seeded Stage 31 campaign passed all nine scenarios at 1 CPU with seed
`0x51A7E`: packet corruption, sudden shutdown, multi-flap network loss,
process crash, low-RAM recovery, service restart, driver restart, service
supervisor recovery, and memory boot. The authoritative result is
`audit-artifacts/stage31-randomized-51a7e-1cpu-9scenarios/result.json`, with
all nine return codes zero; the network child log records all three seeded
flaps. A one-scenario follow-up also passed and recorded `status: "passed"`
in its plan.

The runner now refuses to reuse a non-empty evidence directory: reruns receive
a timestamped directory, and plan status transitions through `running`,
`passed`, or `failed`. Historical folders from the earlier reused directory
were preserved and are not treated as new scenario results. This closes an
evidence-integrity defect without claiming arbitrary in-guest fault injection
or native hardware qualification.

## Stage 31 seeded multi-flap network fault injector - 2026-10-23

Extended the QEMU network harness with a bounded seeded plan of 1-8 virtio-net
link flaps, each held down for 100-500 ms, and retained the plan as
`target/network-regression-{1,2,4}-debug/link-fault-plan.json`. The three-flap
plan with seed `0x51A7E` passed at 1/2/4 CPUs; DHCP/DNS/ICMP/UDP, every link
recovery, TCP echo, and final guest markers passed. The randomized Stage 31
orchestrator now requests this multi-flap mode. This improves deterministic
emulated network-fault coverage, but remains partial and does not claim
arbitrary in-guest fault injection or native hardware qualification.

## Stage 31 fault-coverage metadata alignment - 2026-10-23

Corrected the torture-plan coverage map to reflect the accepted 1/2/4/8/16
CPU service-supervisor/update-service and controller-restart evidence. The
planner contract tests and Python syntax check pass. This is an auditability
correction; the underlying gates remain bounded emulated recovery tests and
do not become arbitrary in-guest fault injection or native qualification.

## Stage 31 current-head network link-loss recovery - 2026-10-23

The live QEMU network gate passed with an emulated virtio-net link flap at
1/2/4 CPUs. DHCP, DNS, ICMP, UDP, link down/up recovery, TCP echo, and final
guest markers all passed; serial evidence is retained under
`target/network-regression-{1,2,4}-debug/`. This qualifies the bounded
emulated link-loss path only, not arbitrary network fault injection or native
network hardware.

## Stage 31 current-head packet-corruption boundary - 2026-10-23

The deterministic malformed ICMPv6 checksum/NDP ingress boundary passed on the
current tree at 1/2/4 CPUs. Evidence is retained under
`audit-artifacts/stage31-packet-corruption-{1,2,4}cpu/`. The result confirms
bounded malformed-packet rejection and continued guest completion. It remains
partial network-fault coverage, not randomized corruption of every receive
protocol, native networking qualification, or production readiness.

## Stage 31 current-head process-crash containment - 2026-10-23

The dedicated Ring-3 unmapped-write probe passed on the current tree at 1/2/4
CPUs with QEMU exit 33. Evidence is retained in
`target/stage31-process-crash-{1,2,4}cpu/`. The gate confirms that the
faulting userspace process is reaped while the kernel remains alive and emits
the success marker. It is a bounded process-lifecycle proof, not arbitrary
in-guest fault injection, driver-crash recovery, native qualification, or
production readiness.

## P0-03/P0-07 current-head release validation - 2026-10-23

The authoritative release validator completed successfully on commit
`e4a8ba79`: warning-denying kernel/host Clippy, host tests, debug and release
QEMU memory/storage/network at 1/2/4 CPUs, shell and service-supervisor
qualification, driver restart, application linting, all system-app catalog,
package, AI/security, and signed system-app desktop boot gates passed. The
final report is `target/release-validation/results.json`; the desktop boots
also captured 1280x800 first-frame evidence for each CPU profile.

The production packager was then invoked and stopped fail-closed because this
environment has no `WOVENHAT_APP_SIGNING_SEED_HEX` (and trusted public key).
The release-key provisioning boundary test passed, but no production-signed
image, ISO, or release archive was emitted. Supplying the release-owner key
material is still required before release artifact packaging or a production
release claim. This acceptance remains QEMU/OVMF-only and does not close
physical media, native hardware, TPM/Secure Boot, physical power-loss, or
independent Claude review gates.

## Graphics single-output virtio-GPU path - 2026-10-23

The production compositor now accepts a single-output virtio-GPU instead of
requiring two scanouts. Multi-head output placement and the boot-framebuffer
fallback are preserved. The affected kernel Clippy gate passed, the complete
host all-targets suite passed, and the existing Stage 20.14 two-head
pixel-accurate QEMU acceptance passed on 1 CPU after the change. A dedicated
single-output capture harness then passed on 1/2/4 CPUs, verifying four
quadrant colors from retained 1280x800 scanouts. Evidence is retained at
`audit-artifacts/stage20.14s-1cpu-1792779432617072300/`,
`audit-artifacts/stage20.14s-2cpu-1792779460061232200/`, and
`audit-artifacts/stage20.14s-4cpu-1792779510914301900/`.

This closes the disposable-QEMU single-output compositor qualification, not
native GPU certification. Physical hardware, device-loss recovery, arbitrary
GPU fault injection, and production readiness remain separate gates.

## P0-02 sudden-shutdown FAT32 recovery slice - 2026-10-23

The abrupt-shutdown harness interrupted the live FAT32 mutation boundary and
then recovered the same disposable image successfully at 1/2/4 CPUs. Each
recovery reached the live mutation and complete-boot markers. Evidence is
retained at
`audit-artifacts/stage31-sudden-shutdown-1cpu-20261023T182410Z/`,
`audit-artifacts/stage31-sudden-shutdown-2cpu-20261023T182509Z/`, and
`audit-artifacts/stage31-sudden-shutdown-4cpu-20261023T182607Z/`.

This is a disposable-QEMU interrupted-transaction recovery slice. It does
not close physical power-loss testing, arbitrary torn-sector injection,
native-media qualification, or the complete P0-02 durability gate.

## P0-07 release-packager marker contract - 2026-10-23

The release packager had required a nonexistent aggregate
`system-apps-boot` result even though the authoritative validator emits
`system-apps-boot-1/2/4`. It now requires the three exact CPU-qualified
markers. A focused host contract test and Python syntax check pass. The
current-head validator subsequently passed all 152 checks, including the
three exact system-app boot rows; production packaging still requires
release-owner signing material and is documented separately above.

## Stage 31 repeated-boot qualification - 2026-10-23

The count-qualified repeated-boot harness completed 1,000 sequential 1-CPU
memory/boot validations successfully. An independent post-run audit found
exactly 1,000 run directories, each with valid JSON, exit code 0, and no
timed-out run. Evidence is retained at
`audit-artifacts/stage31-repeated/memory-1cpu-1000boots/`.

This closes the repository's 1,000-boot disposable-QEMU repeatability sample,
but does not by itself close any broader thousands-of-boots interpretation,
native hardware qualification, physical power-loss testing, or production
readiness. The earlier 100-boot evidence remains at
`audit-artifacts/stage31-repeated/memory-1cpu-100boots/`.

## Current host regression after low-RAM harness tests - 2026-10-23

The complete Python host suite passed on the current head: 153 tests in
132.839 seconds. This includes the low-RAM runner command-contract tests; the
Linux QEMU low-RAM acceptance remains pending its serialized CI run.

## Stage 31 low-RAM matrix harness - 2026-10-23

Added `scripts/test-stage31-low-ram.py` and the corresponding `low-ram`
planner suite. It runs the complete boot validation against the existing
disposable 256 MiB regression profile for every requested 1/2/4/8/16-CPU
configuration, preserves child command/result/serial evidence, and fails
closed on a non-33 exit or failed child gate. `test-memory-qemu.py` now accepts
an explicit RAM value while retaining 256 MiB as its regression default. The
planner and focused host tests pass. Local QEMU/TCG acceptance passed at
1/2/4/8/16 CPUs; evidence is retained under
`audit-artifacts/stage31-low-ram-{1,2,4,8,16}cpu/`. This is low-RAM
disposable-QEMU evidence only and does not lower the declared 4 GiB production
guest floor or qualify native hardware. The Linux CI rerun remains useful
independent confirmation and is not substituted by this local result.

The seeded Stage 31 randomized runner now includes this low-RAM gate as a
ninth bounded scenario, so its schedule cannot omit the resource-pressure
case. This remains orchestration of bounded child gates, not arbitrary
in-guest fault injection.

The expanded seeded nine-scenario campaign passed locally at 1/2/4/8/16 CPUs
with seed `0x51A7E`, including the low-RAM gate. Evidence is retained under
`audit-artifacts/stage31-randomized-51a7e-{1,2,4,8,16}cpu-9scenarios/`. The runner now
uses a count-qualified directory for non-historical campaign sizes so an
expanded campaign cannot overwrite the original eight-scenario report.

## Current host audit - 2026-10-23

The complete Python host test suite passed: 151 tests in 168.209 seconds.
This validates host-side contracts and planners. Local QEMU/OVMF evidence is
recorded below; GitHub Actions remains the independent Linux qualification.

After the full 1/2/4/8/16-CPU harness expansion, the complete host suite was
rerun on the current head: 151 tests passed in 187.428 seconds.

The full CI run exposed and the local exact gate reproduced a Stage 13.5
feature-composition defect: xHCI restart-only shutdown helpers were compiled
for `stage13-5-test`, whose QEMU configuration intentionally omits the PCI
waiter teardown API. Those helpers are now gated solely by
`stage31-driver-restart-test`; Stage 13.5 build, warning-denying Clippy, and
1/2/4-CPU acceptance all pass locally.

The repository-wide host Clippy gate also passed with warnings denied:
`cargo clippy --all-targets -- -D warnings`.

After the current planner/release-validator updates, the complete host Python
suite was rerun: 151 tests passed in 171.754 seconds.

The kernel workflow now supersedes older queued runs per branch, retaining
completed run artifacts while ensuring the latest audited commit reaches the
runner instead of waiting behind stale push matrices.

The GFX-02 feature gate now requires its own terminal renderer marker only.
That feature exits immediately after its acceptance, so requiring the later
generic all-validations marker was contradictory and caused a false negative
despite a passing renderer trace.

The first local supervisor boot also found and fixed a feature-composition
bug: the dedicated system-app recovery image now explicitly includes the
normal production boot path while retaining only the dedicated QEMU exit
helpers needed by its terminal marker.

The current PCI layer has generation-safe resource teardown and explicit
rebind coverage. NVMe, AHCI, xHCI, and HDA now expose controller-specific
quiesce-before-unbind protocols, and the four-controller restart gate passes
at 1/2/4 CPUs. The Stage 31 driver self-test also randomizes the controller
restart order deterministically in-guest (seed `0x51A7E`) and passes at 1/2/4
CPUs. This is bounded, explicitly invoked restart recovery; arbitrary driver
fault supervision and native hardware certification remain open.

The seeded Stage 31 cross-subsystem runner now includes the existing QEMU
virtio link-loss gate as an eighth scenario. The complete eight-scenario
matrix passed at 1, 2, and 4 CPUs under seed `0x51A7E`; evidence is retained
under `audit-artifacts/stage31-randomized-51a7e-{1,2,4}cpu/`. This closes the
network-loss omission in the randomized schedule but remains bounded emulated
coverage, not arbitrary in-guest fault injection or native hardware testing.

The repeated Stage 31 memory suite now has a full ten-boot sample at every
requested CPU profile: 1, 2, 4, 8, and 16 CPUs. All 50 boots exited
successfully; evidence is retained under
`audit-artifacts/stage31-repeated/memory-{1,2,4,8,16}cpu/`. These are
disposable QEMU/TCG reliability samples and do not replace native hardware,
power-loss, or arbitrary in-guest fault qualification.

The first CI run after the randomized-matrix update exposed an invalid GFX-02
workflow command: the kernel package is binary-only, so `cargo test -p
wovenhat-kernel --lib graphics_abi` could never run. The workflow now retains
the warning-denying kernel Clippy gate and invokes the existing 37-test host
`display_compositing` contract target instead. Both commands pass locally.

The Stage 28 disposable acceptance was also rerun on the current head and
passed. It verified the two-partition layout, FAT32-compatible data region,
EFI artifact placement, settings readback, executor output, and rollback. The
report explicitly records `physical_access: none`, `bootability:
not-qualified`, and `release_certification: not-performed`; this is disposable
file-image evidence only.

The controller-owned driver restart gate has now been extended through the
full 1/2/4/8/16-CPU profile. Fresh 8- and 16-CPU QEMU runs passed with 4 GiB
guests and real emulated NVMe, AHCI, xHCI, and HDA devices; the planner and
harness now expose those profiles. This remains explicit bounded restart
qualification, not automatic crash supervision or native hardware evidence.

The signed service-supervisor gate is now also qualified at 8 and 16 CPUs.
Fresh runs passed with 4 GiB guests and the complete signed 11-application
catalog; the Stage 31 planner now exposes service-supervisor jobs across
1/2/4/8/16 CPUs.

The signed update-service restart slice has now been extended to the same
1/2/4/8/16-CPU matrix and rerun at 8 and 16 CPUs. Both passed with 4 GiB
guests and exit 33. The planner now exposes these profiles; the separate
guests and exit 33. The planner now exposes these profiles.

The WovenRecovery and sudden-shutdown recovery gates have also been extended
to 4 GiB guests and 8/16 CPUs. Fresh runs passed at both high-SMP profiles;
all Stage 31 planner suites now expose the full 1/2/4/8/16 CPU matrix.

The seeded eight-scenario randomized runner now accepts the full same CPU
matrix. Fresh 8- and 16-CPU runs passed for packet corruption, process crash,
service supervision/restart, sudden shutdown, driver restart, network loss,
and memory recovery; the earlier 1/2/4 runs remain green. Evidence is under
`audit-artifacts/stage31-randomized-51a7e-{1,2,4,8,16}cpu/`. This is
reproducible bounded scenario orchestration, not arbitrary fault injection.

## Stage 31 randomized controller order qualification - 2026-10-23

The controller restart self-test now applies a deterministic Fisher-Yates
shuffle to NVMe, AHCI, xHCI, and HDA before exercising each controller. The
serial logs record the seed and selected order, making the scenario reproducible
while covering ordering-sensitive teardown interactions. Runs passed at 1, 2,
and 4 CPUs; evidence is retained under the corresponding
`audit-artifacts/stage13.12-restart-*` directories. This does not claim
arbitrary full-stack fault injection, physical qualification, or native
hardware certification.

## Current-head reconciliation - 2026-10-23

The historical entries below retain their original findings and dates. On the
current audited head, GFX-02's bounded CPU renderer is accepted on 1/2/4 CPUs,
and all four PCI controller restart lifecycles are accepted on 1/2/4 CPUs.
Older statements that describe either item as unstarted refer to the earlier
tree and are not current-head status. The remaining graphics gap is GPU
acceleration/native display qualification; the remaining hardware gap is
physical/native qualification.

## Stage 31 service-supervisor recovery acceptance added - 2026-10-23

Added the dedicated `stage31-service-supervisor-test` QEMU image and the
`--supervisor-test` harness mode. The test boots the signed system-app
catalog, kills the currently published `woven.update` process, and requires
the bounded supervisor to reap it, spawn a verified replacement, and publish
a different owner generation through IPC. Warning-denying kernel Clippy and
Python harness syntax checks pass locally. With local QEMU/OVMF and a 4 GiB
guest, the live acceptance passed at 1, 2, and 4 CPUs; serial evidence is
retained under `target/shell-regression/`.

The shell harness now defaults to the declared 4 GiB minimum guest memory;
callers can override it explicitly with `--memory` for diagnostic work.

## Stage 31 shutdown recovery and mounted-volume guard - 2026-10-23

The seeded randomized matrix first exposed stale FAT32 FSINFO after an
abrupt stop. `fat32::repair_fs_info` now reconciles the advisory free-count
and next-free hints from the FAT after journal replay, updating both valid
FSINFO copies before VFS import. The shutdown harness now kills QEMU at a
kernel-emitted mutation boundary instead of a timing delay, and the live
mutation self-test requires `mnt_mounted()` so blank service disks are not
qualified as FAT32 volumes.

The full seeded six-scenario matrix passed locally at 1, 2, and 4 CPUs after
the fixes. Evidence is retained under
`audit-artifacts/stage31-randomized-51a7e-{1,2,4}cpu/`; focused sudden-shutdown
evidence is retained under the timestamped `stage31-sudden-shutdown-*`
directories.

The seeded Stage 31 orchestrator now includes this supervisor scenario in
addition to the existing signed update-service restart scenario. The runner
still explicitly describes itself as bounded scenario orchestration rather
than arbitrary in-guest fault injection.

The auditable Stage 31 torture planner now exposes a separate
`service-supervisor` suite for the supported 1/2/4-CPU profiles and marks the
8/16-CPU rows pending/blocked according to the existing harness limits.

The release validator now runs the signed supervisor recovery image at 1, 2,
and 4 CPUs after the normal system-app smoke gate.

## Release gate covers production service supervisor - 2026-10-23

Updated `scripts/test-release.py` so the authoritative release chain runs
warning-denying kernel Clippy with `--features system-apps` after constructing
the exact signed application environment and packaged image paths. This
ensures production-only service supervision code is linted rather than being
covered only by the default kernel configuration. The complete release chain
was not rerun locally because this Windows host lacks OVMF and has limited
free space; the script change is syntax-checked and the equivalent
environment-aware Clippy command passed locally.

## Stage 31 bounded production service supervisor - 2026-10-23

Added `kernel::service_supervisor` for the signed package, update, and
installer services. Production startup registers each PID generation; the
event loop detects exited services, reaps them through the owning kernel
parent, waits with bounded exponential backoff, and respawns the verified
image with the original least-privilege capability set. Restart attempts are
capped at three per service and stale-generation results cannot overwrite a
newer PID; exhausted services are disabled and logged fail closed.

The `system-apps` kernel Clippy gate passes with the existing packaged
artifacts and trusted test key. This closes a real service-supervision slice,
but does not yet qualify arbitrary driver-crash recovery or the full
in-guest randomized Stage 31 campaign.

## Stage 28/GFX-02 executable QEMU CI matrix - 2026-10-23

Added `scripts/test-feature-qemu.py`, a shared fail-closed feature-image
harness that validates exit code 33, required serial markers, panic absence,
and contradictory failure markers. CI now executes both the Stage 28 installer
self-test and GFX-02 CPU renderer self-test on 1, 2, and 4 CPUs with explicit
OVMF discovery and retained evidence. Local static validation passed; the
Windows host has no local OVMF image, so this Linux QEMU matrix remains a CI
execution rather than a locally claimed result.

## Maturity audit Cycle 1 - 2026-10-23

Read the complete supplied full-system maturity mandate and captured a fresh
machine-readable audit at `audit-artifacts/maturity/20261023-cycle1/`. The
report records the exact toolchain/QEMU environment, findings, validation
results, release blockers, Codex review, pending Claude review, and SHA-256
digests. The later current-head release validation passed all 152 checks on
source commit `e4a8ba79`; production signing, native hardware, physical
power-loss, arbitrary in-guest fault injection, and independent Claude review
remain open.

## Stage 28 installer-plan capacity alignment - 2026-10-23

Aligned the graphical installer's validated snapshot parser with the existing
installer IPC contract: plans now accept the full bounded 192-byte request
capacity instead of an accidental 128-byte frontend limit. Added boundary
tests for the maximum accepted and first rejected plan sizes. Installer host
tests pass (5 tests), and warning-denying release Clippy passes for
`x86_64-unknown-none`. The parser remains bounded and does not grant the GUI
storage authority.

## CI maturity gates wired - 2026-10-23

Added dedicated Linux workflow jobs for the current release blockers. CI now
runs the Stage 28 disposable GPT/FAT32/EFI acceptance and installer-app
warning-denying checks, the GFX-02 kernel/host contract checks, and the seeded
Stage 31 randomized matrix on 1, 2, and 4 CPUs with explicit OVMF discovery.
The matrix remains evidence for emulated recovery and randomized scenario
ordering; it does not convert the documented physical/native qualifications
or arbitrary in-guest fault injection into passing claims.

## Stage 31 runner firmware propagation and planner-test repair - 2026-10-23

The seeded Stage 31 orchestrator now requires an explicit OVMF CODE path and
forwards the selected QEMU and firmware paths through memory, process-crash,
packet-corruption, sudden-shutdown, and service-restart children. This makes
the matrix reproducible on hosted runners whose firmware is not beside the
QEMU executable. The packet-corruption and update-service wrappers now accept
the same explicit paths. Focused planner tests were refreshed to match the
committed eight-suite, five-CPU-profile plan. A Stage 28 disposable acceptance
run passed after reclaiming abandoned temporary output; the host still has
limited free space, so no large rebuild was started.

## Stage 28 confirmed GUI-to-service install request - 2026-10-23

The graphical installer now submits a bounded `Operation::Install` request
through `sys.storage.installer` when its validated snapshot is explicitly
confirmed and on the progress screen. It renders accepted/completed and
fail-closed target/install statuses; the GUI still has no storage capability,
and the default unconfirmed snapshot never submits a destructive request.
The Ring-3 installer builds and warning-denying Clippy pass for
`x86_64-unknown-none`. The kernel transaction and disposable rollback gates
remain the authoritative GPT/FAT32/EFI/provisioning evidence; this increment
closes the missing GUI-to-service request path but does not qualify physical
installation or native boot.

## Stage 31 seeded cross-subsystem fault orchestration - 2026-10-23

Added `scripts/test-stage31-randomized-faults.py`, a fail-closed runner that
uses a reproducible seed to schedule the existing memory, process-crash,
packet-corruption, sudden-shutdown, and signed service-restart gates in a
different order, one at a time, preserving command/result/output evidence for
each run. The seed `0x51A7E` completed all five scenarios on 1 CPU; evidence
is under `audit-artifacts/stage31-randomized-51a7e-1cpu`.

The torture planner records this as partial coverage. It proves randomized
cross-subsystem scenario ordering and preserves independent fault evidence,
but it does not yet inject arbitrary faults inside every driver/service or
qualify physical power loss. Native hardware and full-stack in-guest fault
injection remain open.

## GFX-02 bounded CPU renderer - 2026-10-23

Implemented the first concrete backend behind the GFX-01 contract as
`graphics_abi::CpuRenderer`. It owns eight bounded generation-checked surface
slots, validates dimensions and rectangles, performs nearest-neighbor
compositing with opacity, and provides monotonic submission fences with
stale-surface and invalid-fence rejection. It uses the existing kernel
`display::Surface` and does not create a second compositor or scanout owner.

Kernel feature Clippy, host all-targets Clippy, and the independent GFX-02
self-test passed on QEMU 1/2/4 CPUs. Evidence is retained under
`target/gfx02-{1cpu,2cpu,4cpu}`. The host kernel-bin test remains unavailable
on this Windows target because its existing x86 inline assembly uses GNU
`.previous` directives rejected by the MSVC assembler; the QEMU target gate is
the authoritative acceptance for this `no_std` kernel feature.

This accepts only the CPU renderer contract. GPU acceleration, compositor
migration, native display-driver qualification, and physical hardware
certification remain open.

## Stage 20.10 Quick Settings 3D qualification - 2026-10-23

The Quick Settings acceptance model now follows the production 3D card and
projected desktop background for a stable interior, while interaction checks
cover opening, DND, notification badge/no-toast behavior, tiling, display
scale, close, and display hand-back. The taskbar button coordinate is aligned
with the shell's clock/tray geometry. Kernel warning-denying Clippy and the
QEMU 1/2/4-CPU matrix passed; evidence is under
`target/woven-shell-quick-settings-{1,2,4}cpu`.

This closes the Stage 20.10 qualification slice. Physical/native graphics
qualification and full hardware-driver certification remain outside QEMU
scope and are not claimed here.

## Stage 31 sudden-shutdown recovery slice - 2026-10-23

Added `scripts/test-sudden-shutdown-qemu.py`. For each selected CPU count it
creates a fresh disposable FAT32 image, boots the normal `qemu-test` image,
terminates QEMU after the async-I/O foundation marker but before the live
storage mutation marker, and then reboots the same image. The recovery boot
must return exit 33 and produce both the complete storage-mutation and
`[BOOT] ALL VALIDATIONS PASSED` markers. The 1/2/4-CPU matrix passed with
evidence under `audit-artifacts/stage31-sudden-shutdown-{1,2,4}cpu-*`.

This proves a bounded abrupt-QEMU-termination/reboot path over the disposable
ATA/FAT32 device. It does not prove physical power-loss, torn-sector behavior,
native controller recovery, or randomized full-stack fault injection; those
remain explicit release gates. The interrupted logs and recovery logs are
retained in each timestamped artifact directory.

The Stage 31 fail-closed torture planner now includes this as an executable
`sudden-shutdown` suite for 1/2/4 CPUs rather than reporting the scenario as
open.

## Release validation after SMP fixture repair - 2026-10-23

The first post-Claude full release run reached `shell-smoke` but correctly
failed on a reproducible Stage 7 automatic-rebalance assertion: the test had
migrated every probe away from CPU 0 and then incorrectly required a CPU-0
probe observation (`mask=14`, expected `15`). This was a qualification-fixture
defect, not a weakened scheduler assertion. Commit `cebcea53` adds one
CPU-0-pinned anchor probe while retaining the migratable workload and all
migration/counter assertions.

After the repair, the focused 4-CPU production shell gate passed, kernel and
host warning-denying Clippy passed, and the complete authoritative release
validator passed every recorded gate. The final report is
`target/release-validation/results.json`; shell evidence is under
`target/shell-regression/system-apps-4cpu-1792743581732896300/serial.log`.
Claude's current `kernel/src/main.rs` changes remain uncommitted and
untouched by Codex.

## 3D-default self-test retrofit: Stage 20.3 - 2026-10-23

Stage 20.3's kernel self-test passes on 1/2/4 CPUs against the 3D-default
desktop, migrated onto the shared `projected_card_3d`/`expected_card_3d`
helper (new, for small bounded 3D chrome cards sampled at local texture
coordinates: the launcher panel, and potentially QS/notification
center/toast later). Found a second real rendering defect along the way:
`render_panel_3d`'s offscreen texture isn't cleared before each repaint,
so stale content (an old selection highlight, a leftover glyph) can
persist and show through. Full details, including how this differs from
Stage 20.2's toast finding, are in `docs/agent-coordination.md`.

## 3D-default self-test retrofit: Stages 20.1-20.2 - 2026-10-23

Stage 20.1 and 20.2's kernel self-tests pass on 1/2/4 CPUs against the
3D-default desktop. Root cause of Stage 20.1's previously-stuck taskbar
check: the self-test's duplicated taskbar-projection math was missing the
`23/20` top-edge perspective-shrinkage compensation `render_taskbar_3d`
applies (the same factor Codex's GFX-01 slice already fixed for Stage
20.14b's own copy) -- fixed identically across all 15 duplicated copies
(Stages 20.1-20.13, 20.14b, 24.59), then that duplication was replaced with
a single shared `cfg(any(...))`-gated implementation
(`projected_taskbar_3d`/`expected_taskbar_3d`/`flat_taskbar_3d`) migrated
into Stage 20.1 and 20.2 so far. Full details and the open toast-rendering
bug found along the way are in `docs/agent-coordination.md`.

Remaining: Stages 20.3-20.13, 20.14b (refactor only, already individually
correct), and 24.59 still need migrating onto the shared helper plus
whatever stage-specific pre-3D flat checks each turns out to have.

## Post-Claude shared-tree validation - 2026-10-23

Claude's shared graphics/kernel update was committed externally as
`1f44f822` (`updates`). The remaining uncommitted `kernel/src/main.rs` diff is
preserved and was not staged by Codex. Against that current tree, warning-
denying kernel Clippy and host all-targets Clippy both pass. The fresh Stage
31 8/16-CPU memory, network-loss, and storage runs below also execute against
this post-commit tree; a complete release-validator rerun remains advisable
after the kernel diff is finalized.

The subsequent `cargo test --workspace` also passed, including the 119-test
kernel/libwoven unit suite, all host integration suites, and the 16-test
WovenFS crash/recovery suite. This host result is supplementary to, and does
not replace, the required QEMU and native-hardware gates.

## Full release validation rerun - 2026-10-23

The clean authoritative release validator completed successfully. Its report
is `target/release-validation/results.json`. All recorded checks passed:
warning-denying kernel and host Clippy, the host compile/test suite, debug
QEMU acceptance for memory/storage/network on 1/2/4 CPUs plus legacy PIC,
release memory/storage/network gates, release image construction, shell
smoke, application linting, production application boot on 1/2/4 CPUs,
system-app catalog boot, package transactions, AI provider/security/action
gates, and system-app boot on 1/2/4 CPUs. The previously observed 4-CPU
storage timeout did not reproduce in this clean run, but no source-level root
cause fix has been established, so storage flakiness remains a release risk
until repeated qualification demonstrates stability.

The validator regenerated current serial evidence under `target/` and passed
without changing the shared Claude files. An earlier recovery step used
`cargo clean` after the first release build ran out of space; that removed
ignored historical `target/` logs, so those older artifacts are not claimed
as preserved. No physical disk, TPM, firmware variables, removable media, or
native hardware qualification was attempted. Stage 29 physical qualification,
Stage 31 randomized full-stack fault/recovery coverage, and the independent
Claude review remain open; production readiness is not claimed.

A fresh read-only physical preflight at
`audit-artifacts/physical-host-20261023-083045-801/host.json` and
`audit-artifacts/physical-tpm-readiness-20261023-083050/` confirms an 8.2 GB
HP Pavilion x360 host meets the 4 GiB floor, but the session is not elevated,
only the boot/system NVMe is visible, no safe USB target is available, and no
TPM or QEMU TPM backend is visible. It performed no physical test and changed
no disk, firmware, TPM, or media state.

## Stage 31 repeated high-SMP memory boots - 2026-10-23

The current tree passed two consecutive Stage 31 memory boots at both 8 CPUs
and 16 CPUs. Each run returned success and the runner preserved per-run
command, result, serial, and QEMU evidence under
`audit-artifacts/stage31-repeated/memory-8cpu/` and
`audit-artifacts/stage31-repeated/memory-16cpu/`. This extends fresh repeated
boot evidence beyond the 1/2/4-CPU release matrix; it does not close sudden
shutdown, randomized full-stack fault injection, or native hardware gates.

## Stage 31 high-SMP network-loss recovery - 2026-10-23

The current QEMU network gate passed with a virtio link flap at both 8 and 16
CPUs. DHCP, DNS, ICMP, host-verified UDP/TCP, link down/up recovery, final
SMP/TLB markers, and exit 33 all passed. Serial evidence is under
`target/network-regression-8-debug/serial.log` and
`target/network-regression-16-debug/serial.log`. This qualifies the emulated
virtio link-loss boundary only; packet corruption across the full receive
stack and native NIC failure remain open.

## Stage 31 high-SMP storage qualification - 2026-10-23

The complete disposable FAT32/storage QEMU suite passed at 8 and 16 CPUs,
both with exit 33. Serial evidence is under
`target/storage-regression-8-debug/serial.log` and
`target/storage-regression-16-debug/serial.log`. This is fresh high-SMP
evidence against the earlier timing-sensitive storage concern; repeated
qualification is still required before treating that risk as closed.

## Full-system maturity Cycle 0 - 2026-10-22

The full-system mandate was read and a protected audit branch was created at
`codex/maturity-audit-20261022` from `35bd0d45`. The shared Claude changes in
`apps/woven-shell/src/main.rs` and `kernel/src/main.rs` remain dirty and are
preserved. Fresh Stage 11.2-ASLR randomized guard-fault acceptance passes on
1/2/4 CPUs. Fresh WovenFS scratch-device acceptance passes on 1/2/4 CPUs, and
the host crash-injection suite passes all 16 tests. Physical power-loss,
native hardware, TPM, and secure-boot qualification remain open. The latest
remote GitHub Actions kernel run is red in core-foundation and drivers-radio;
the local branch is not pushed. Machine-readable evidence is under
`audit-artifacts/maturity/20261022-cycle0/`. No production-readiness claim is
made.

The authoritative local release validator also reproduced a 4-CPU storage
boot timeout after the S10.2 async-completion marker. The same timeout was
reproduced from a clean detached worktree at the audited baseline, so it is
an inherited P0-04 storage/SMP blocker rather than an effect of Claude's
uncommitted graphics work. The timeout and serial logs are preserved under
`target/release-validation/storage-4-debug.log` and
`target/storage-regression-4-debug/serial.log`; the release gate was not
weakened or extended.

Temporary serial tracing localized the stall to the live FAT32 directory-growth
workload's ATA/cache path, and some failing runs stop at the handoff immediately
after S10.2 before reaching the storage boundary. Across repeated 4-CPU runs,
the stop moved among the post-S10.2 scheduler handoff, old-file reads, and
metadata/data journal operations, with no rejected worker event signal.
Pinning the worker and forcing post-probe direct I/O reproduced the timeout, so
worker placement/queueing alone is not the cause; the scheduler/task-lifecycle
handoff and shared ATA/cache interaction remain unresolved. All tracing and
experiments were removed after diagnosis, and the shared Claude worktree
remains untouched.

An explicit `yield_now()` immediately after pager publication was also tested
and reverted; the 4-CPU timeout remained after S10.2. The release gate still
requires a real scheduler/task-lifecycle fix rather than an ordering hint.

## GFX-01 hybrid graphics baseline - 2026-10-22

Applied the supplied Hybrid Graphics Architecture direction at GFX-01 only.
Added `kernel/src/graphics_abi.rs` as a versioned, backend-neutral contract
over the existing display surface, compositor lease, damage path, and
virtio-gpu backend; no parallel graphics stack was created. The shared 3D
desktop taskbar projection was corrected to preserve the full primary-monitor
footprint while retaining its depth gradient. Host tests, warning-denying
Clippy, and Stage 20.14b graphics acceptance now pass on 1/2/4 CPUs. GFX-01
is accepted; GFX-02 remains unstarted until its own implementation and gates
are completed. Full details are in `docs/audit-gfx01-baseline-2026-10-22.md`.

## Stage 31 PCI driver restart slice - 2026-10-22

The Stage 13.2 QEMU lifecycle gate now binds a discovered PCI function,
activates its interrupt/BAR ownership, tears it down, binds it again under a
fresh owner transaction, tears it down again, verifies zero remaining BAR
leases, and completes generation-safe rescan. The 1-CPU gate passed with exit
33. This is explicit driver rebind/teardown qualification, not automatic
crash supervision or native hardware certification. Details are in
`docs/audit-stages-consolidated.md`.

The Stage 31 torture planner now records driver-crash coverage as
`partial-stage13-2-explicit-rebind`: explicit rebind and teardown are covered,
but automatic recovery after an injected driver crash is still open.

## Stage 31 signed service restart slice - 2026-10-22

The Stage 27.5 signed `woven-update` acceptance now kills and reaps the first
daemon, verifies endpoint cleanup, reloads the signed image, respawns it,
verifies fresh endpoint ownership and capability discovery, services a query,
and tears it down again. The 1-CPU QEMU gate passed with exit 33 and emitted
`[S27.5] IPC daemon restart/republication: PASSED`. This is service-restart
qualification only; generic supervision, driver restart, sudden shutdown,
randomized full-stack faults, and native hardware certification remain open.
Details are in `docs/audit-stages-consolidated.md`.

## Input queue reliability slice - 2026-10-22

The shared WovenInput queue now coalesces adjacent pointer motion and, when
full, evicts the oldest motion before rejecting a keyboard/button/other
critical event. This keeps high-rate pointer input from starving keys or
buttons while preserving a fixed, allocation-free IRQ path and the existing
drop counter. Host library tests and warning-denying kernel/host Clippy pass;
the existing Stage 19.12 QEMU gate also passed on 1 CPU with real i8042
keyboard/mouse loopback reaching compositor clients. The attempted Stage
13.7 invocation produced no usable evidence, so no new Stage 13.7 acceptance
is claimed. Physical USB HID, virtio-input, hot-plug, and device-recovery work
remain open. Details are in
`docs/audit-input-queue-reliability-2026-10-22.md`.

## Stage 31 packet-corruption boundary - 2026-10-22

The dedicated packet-corruption wrapper now runs the real network QEMU gate
with ICMPv6 checksum and NDP ingress hardening enabled. Deterministic malformed
checksum rejection passed on 1/2/4 CPUs, with host UDP/TCP and final SMP
markers also passing. This is recorded as partial coverage only; randomized
corruption across the complete receive stack remains open. Evidence is under
`audit-artifacts/stage31-packet-corruption-{1,2,4}cpu`.

## Host regression repair - IPC capacity probe - 2026-10-22

The full host suite caught a stale `scripts/test-ipc-handle-generation.py`
fixture that still expected the former 16-handle process capacity after the
accepted SMP expansion to 64 handles. The source-extraction probe now derives
the current documented 64-slot contract; `cargo test -p wovenhat-os --test
ipc_handles` passes.

## Stage 31 process-crash containment - 2026-10-22

Added a feature-gated Ring-3 fault probe and QEMU harness. The probe writes to
an unmapped user address; the kernel records the user fault, terminates only
that process, and returns `-11` to the waiter. The gate passed on 1/2/4 CPUs
with exit 33. Evidence is retained under
`target/stage31-process-crash-{1,2,4}cpu`; the detailed audit is
`docs/audit-stages-consolidated.md`. Driver crashes, service
restart, packet corruption, sudden shutdown, and native hardware faults remain
open.

## Stage 24.59 low-memory regression closure - 2026-10-22

The 256 MiB Change-PIN regression fixture initially exhausted the proportional
bootstrap heap before the compositor's large surface allocation. The heap now
reserves its bounded 8 MiB bootstrap ceiling when available frames can afford
it, while preserving proportional sizing below that threshold and bounded
runtime growth. The complete Stage 24.59 QEMU gate passed on 1 CPU, including
sign-in, Change-PIN rotation, durable-record checks, relock, old-PIN rejection,
and new-PIN unlock. Evidence and limits are recorded in
`docs/audit-stages-consolidated.md`.

## Stage 31 repeated recovery-service lifecycle slice - 2026-10-22

The sequential `scripts/test-stage31-repeated-boots.py` runner now supports
the existing Stage 27.5 durable-recovery gate as a `recovery` suite. Two
back-to-back 1-CPU runs passed with exit 0; per-run command, result, output,
and copied recovery evidence are retained under
`audit-artifacts/stage31-repeated/recovery-1cpu/`. The suite is bounded to
the existing 1/2/4-CPU recovery profiles.

This is repeatable service startup/teardown evidence, not a generic crash
supervisor or automatic restart implementation. Driver restart, physical
power-loss, packet corruption, and the full Stage 31 fault/repetition matrix
remain open.

## Stage 31 repeated signed update-service lifecycle slice - 2026-10-22

The repeated runner now also invokes the signed Stage 27.5
`woven-update --service` gate. Two sequential 1-CPU runs passed, followed by
one-run profiles at 2 and 4 CPUs; per-run package, serial, QEMU, and result
evidence is retained under `audit-artifacts/stage31-repeated/`.

This proves repeatable service publication, request handling, malformed-request
rejection, and orderly teardown across the supported service SMP profiles. It
does not prove automatic restart after a crash, driver restart, sudden power
loss, or native hardware behavior.

## Physical readiness preflight - 2026-10-22

The read-only physical TPM/USB preflight completed without opening or changing
any device. No physical TPM or ready TPM state was visible, no QEMU TPM backend
was installed, no signed provisioning record was present, and no USB target was
available. Native TPM, USB installation, and hardware qualification therefore
remain unqualified; the detailed safety report is
`docs/audit-physical-readiness-2026-10-22.md`.

## Stage 31 filesystem disk-full slice - 2026-10-22

The WovenFS crash suite now includes a 48-sector low-resource volume test. It
fills the data region, requires a subsequent two-sector write to return
`NoSpace` without a partial commit, remounts the image, verifies the prior
payload, and checks filesystem consistency. The complete host suite passes 16
tests. This is filesystem-level disk-full evidence; whole-kernel low-RAM,
device-fault, and physical-media exhaustion coverage remain open.

## Stage 31 low-resource ATA/WovenFS device slice - 2026-10-22

The existing Stage 15.2 QEMU ATA/WovenFS gate passed against disposable 4 MiB
scratch images at both 1 and 4 CPUs, including format, durable writes,
remount, VFS authority/isolation, unlink, and exit 33. Evidence is retained
under the two `target/wovenfs-device-scratch-*` directories recorded in the
dedicated audit. This is emulated scratch-device evidence; physical media
exhaustion, block-device faults, and whole-kernel low-RAM qualification remain
open.

## WovenShell desktop: 3D mode extended to every floating panel and the lock screen - 2026-10-22

Continuing the wallpaper/taskbar 3D work: the launcher panel, Notification
Center, toast, and lock screen now perspective-project in 3D mode, using the
same techniques already proven for the Quick Settings panel/title bars
(bounded panels: render once into an offscreen texture, then
`textured_triangle_3d`) and the wallpaper (full-screen: sample procedurally
via `procedural_triangle_3d`, no texture needed). `render_desktop_background`
was generalized into `render_floor_3d(canvas, color)`, shared by the desktop
wallpaper and the lock screen's dimmed wallpaper -- the two full-screen
backgrounds, differing only in their per-pixel formula.

Two surfaces are deliberately left flat, for the same reason noted when
window bodies were scoped out: window bodies and tray icons are composited
via `compositor_blit_region_shared`, an opaque kernel syscall blitting a
client's shared-memory surface directly into the framebuffer. The shell
never sees those pixels, so texturing them in 3D would need a new kernel
syscall doing the projection/sampling in kernel space -- out of scope for
this userspace-only pass, by explicit user decision. Window title bars
already tilt (pre-existing, Stage 20.18); their shadows stay flat too, since
the existing `blend`/`draw_blended` presentation primitives apply one
uniform opacity to a whole rectangle and have no non-rectangular/partial-
coverage blending path -- tilting a translucent, non-rectangular shadow
would need new presentation-layer plumbing, not just a new triangle
rasterizer.

Verified: `lint-all-features.py --only stage20` reports every feature
clean under warning-denying Clippy; QEMU self-tests pass for Stage 20.1,
20.3, 20.9, 20.11, 20.17, and 20.18. The launcher panel, Notification
Center, and toast have no dedicated 3D self-test yet (only Stage 20.17/18
toggle 3D in an automated harness); they were checked interactively instead.
3D mode itself remains off by default pending the separate, larger effort
to flip that default (needs re-deriving the ~16 duplicated pixel-exact
desktop self-tests for a 3D-default initial state, tracked separately).

The earlier note about a Stage 24.59 allocation failure under the 256 MiB
fixture is stale. Re-running the unchanged current tree passed the complete
Change-PIN flow with exit 33 on 1, 2, and 4 CPUs; fresh serial evidence is in
`target/woven-shell-change-pin-{1,2,4}cpu`. No heap or test-budget change was
needed.

## 16-CPU SMP and bounded-capacity expansion - 2026-10-22

Raised the static SMP control-plane limit from 4 to 16 CPUs and expanded the
bounded scheduler/process/descriptor/IPC tables to 128 tasks, 128 processes,
64 descriptors, 256 IPC objects, and 64 handles per process. Migration and
preemption diagnostic budgets now scale with their explicit SMP workload; no
assertions or rounds were removed. Production and host warning-denying Clippy
pass. The supported memory, storage, network, security, and shell suites now
pass on 1/2/4/8/16 CPUs; the 16-CPU storage and shell runs need an extended
host TCG wall-clock budget but complete successfully.

## Stage 31 reliability evidence slices - 2026-10-22

The sequential `scripts/test-stage31-repeated-boots.py` harness passed a
ten-boot 1-CPU memory sample, three-boot samples on both 2 and 4 CPUs, and
two-boot samples on both 8 and 16 CPUs.
Each iteration has independent command, result, serial, and QEMU evidence
under `audit-artifacts/stage31-repeated/`; the harness stops on the first
timeout or non-zero result and never runs the fixed-port suites in parallel.

The WovenFS host crash-recovery gate also samples 64 deterministic randomized
write cut points with remount/replay verification, while retaining the
exhaustive all-cut tests. The existing Stage 27.5 durable-recovery QEMU gate
passed on 1/2/4 CPUs with exit 33; evidence remains under
`audit-artifacts/stage27.5-recovery-{1,2,4}cpu`.

The network harness now has an opt-in `--link-loss` fault mode using QMP to
take the QEMU `net0` link down and back up after the UDP path passes. The
guest recovered the host TCP round trip and all final SMP markers on 1/2/4/8/16
CPUs. This is a virtio/QEMU link-flap boundary, not physical NIC loss or a
complete randomized packet-corruption campaign.

These are evidence and infrastructure slices, not completion of Stage 31:
the large-count matrix, randomized device faults, low-resource/loss cases,
driver/service restart, sudden-shutdown recovery, and native qualification
remain open.

## 4 GiB production memory floor and scalable memory telemetry - 2026-10-22

Defined the supported production minimum as 4 GiB through
`kernel::config::MIN_SUPPORTED_RAM_BYTES`. Boot telemetry now reports usable
bytes, the minimum, and whether the discovered allocator capacity meets it;
the read-only Windows host preflight records the same comparison. The
interactive QEMU launcher defaults to 4 GiB. The physical frame allocator
continues to derive capacity from the bootloader memory map, and the kernel
heap grows from remaining frames rather than a fixed machine-size assumption.

The existing 256 MiB QEMU gates remain low-memory regression fixtures. This
change does not claim that the complete OS is production-ready: fixed kernel
object capacities, native hardware qualification, fault-injection coverage,
and later roadmap stages remain separate scalability/release work.

## Stage 31 supported-suite matrix execution - 2026-10-22

Completed the bounded planner's five existing suites on 1/2/4/8/16 CPUs:
memory, storage, network, security, and shell. All twenty-five supported-suite
runs passed with exit 33. Fresh evidence is retained in the corresponding
memory/storage/network/shell target directories and
`audit-artifacts/woven-security-{1,2,4,8,16}cpu`. The complete Stage 31
torture acceptance is still open: randomized failure injection, low-resource
and loss scenarios, driver/service restart, sudden-shutdown recovery, and
the full repeated-boot count remains unimplemented.

The 16-CPU shell gate initially exposed fixed preemption-test budgets; those
budgets now scale with the bounded online topology and the rerun passes. A
300-second shell harness window and a 900-second storage diagnostic window are
host-emulation accommodations, not guest pass overrides. See
`docs/audit-stages-consolidated.md` for the exact commands and
preserved evidence.

## Stage 31 memory and storage suite execution - 2026-10-22

Executed the torture-plan memory and storage suites on 1/2/4 CPUs. All six
runs passed with exit 33; the later supported 8/16 profiles also passed.
Evidence is retained in
`target/memory-regression-{1,2,4}-debug/` and
`target/storage-regression-{1,2,4}-debug/`. Network, security, shell,
failure-injection, low-resource, restart, and full repeated-boot rows remain
open.

## Stage 31 torture-plan boundary - 2026-10-22

Added `scripts/stage31-torture-plan.py` and focused tests. The planner makes
the five initial suites (memory, storage, network, security, and shell)
explicit across the roadmap's 1/2/4/8/16 CPU matrix. It marks 8/16-CPU jobs
blocked because the current acceptance harnesses support only 1/2/4, and it
never launches QEMU, injects faults, or treats an omitted job as passed.

The 16-CPU control plane compiles and the full supported suite passes on 8 and
16 CPUs after increasing bounded scheduler/process capacity and scaling the
SMP diagnostic budgets with their explicit workload. This remains the
orchestration foundation only: randomized failure-injection contracts,
low-resource scenarios, service/driver restart coverage, and repeated-boot
acceptance evidence are still required for full Stage 31 acceptance.

## Stage 30 performance matrix rerun - 2026-10-22

Re-ran the existing syscall, IPC, filesystem, compositor, and scheduler
micro-benchmark gates on 1/2/4 CPUs. All fifteen runs passed with exit 33 and
their per-run serial evidence remains under the corresponding
`target/hello-gui-*-{1,2,4}cpu` directories. The first compositor build was
interrupted by a full host volume (`StorageFull`); only regenerable local
build caches were reclaimed, after which the remaining six runs passed. The
reported cycle counts are QEMU TCG baselines, not native performance claims.

## Stage 29 hardware qualification laboratory boundary - 2026-10-22

Added `scripts/qualify-hardware-report.py` and its focused test module. The
tool is a read-only report boundary: it validates repository-relative evidence
records, rejects duplicate observation identities and absolute evidence paths,
and refuses to call QEMU or emulated results Tier 1. Classification is
conservative: native failures are Tier 3, complete native coverage of all
declared categories is Tier 1, and incomplete native coverage without a
failure is Tier 2.

This closes the software/lab-record foundation for Stage 29, but does not
claim native hardware qualification. The current host preflight found no safe
disposable target, so physical device runs, supported-model declarations, and
the final Stage 29 acceptance matrix remain open.

## Stage 28 acceptance audit - 2026-10-22

The first QEMU audit found a triple fault in the existing GPT self-test:
multi-megabyte `RamDisk` values were constructed on the small kernel stack.
The GPT and installer probes now use bounded heap/sparse media instead. Fresh
storage gates pass at 1/2/4 CPUs with exit 33, and the dedicated
`stage28-installer-test` image reports
`[STAGE28 INSTALLER] GPT/FAT32/EFI/provisioning: PASSED` plus full boot
validation at all three CPU counts. No native physical disk was accessed; the
kernel sparse/RamDisk transaction remains the destructive-install execution
evidence.
The read-only host preflight at
`audit-artifacts/physical-host-20261022-131154-963/host.json` found one
boot/system NVMe disk, no disposable target, and a non-elevated Windows
session; it therefore correctly performed no native-disk test.

## Stage 28 kernel-backed physical installation transaction - 2026-10-22

The supervised installer service now reaches syscall 242, whose kernel
transaction validates the confirmed bounded plan, unmounts the active volume
before destructive work, imports the exact running `EFI/BOOT/BOOTX64.EFI`
artifact from the selected GPT device, writes protective MBR plus primary and
backup GPT, formats aligned ESP and data partitions as FAT32, installs the EFI
artifact, and provisions bounded account/settings files. FAT16 and FAT32 ESP
artifact reads are validated and all target I/O passes the controller-backed
StorageIo/WovenGuard boundary. A `stage28-installer-test` feature performs the
same GPT/FAT32/EFI/provisioning transaction on a disposable kernel RamDisk and
checks the installed files.

Warning-denying kernel Clippy passes in production and the installer feature;
the feature-lint matrix, host all-targets Clippy, 119 libwoven tests, and the
disposable Stage 28 acceptance chain pass. The transaction currently targets
the kernel-selected primary device and intentionally fails closed when no
existing readable EFI artifact is available; native hardware qualification and
multi-device selection remain Stage 29 work.

## Stage 28 kernel FAT32 formatter primitive - 2026-10-22

Added `kernel::fat32::format`, which computes bounded FAT32 geometry and
writes a mount-valid boot sector, FSInfo and backup records, both FAT copies,
and an empty root cluster. It refuses read-only devices and rejects volumes
outside the parser's supported cluster range. A sparse formatter probe is now
part of the existing FAT32 boot self-test; kernel/host warning-denying Clippy
and the normal system-apps 1-CPU QEMU gate pass.

The formatter is not yet composed with the GPT writer, EFI payload ingress,
target-selection policy, or installer service transaction/rollback.

## Stage 28 kernel GPT layout writer - 2026-10-22

Added the first reusable kernel-side installation primitive:
`kernel::gpt::write_layout` writes a validated, aligned protective MBR, primary
and backup GPT headers, and primary and backup partition-entry arrays for an
explicit EFI System Partition plus WovenHat data partition. The operation
rejects read-only devices and geometry overflow, flushes after both copies,
and is covered by the existing boot-time GPT self-test. Warning-denying kernel
and host Clippy plus the normal system-apps 1-CPU QEMU gate pass.

This is not yet the installer transaction: target selection, FAT32 formatting,
EFI payload writing, provisioning, and service ABI integration remain open.

## Stage 28 real UEFI artifact ingress - 2026-10-22

Added `scripts/extract-uefi-boot-artifact.py`, a read-only FAT16/FAT32 parser
that locates the first GPT EFI System Partition and extracts the exact
`EFI/BOOT/BOOTX64.EFI` bytes without mounting or opening a device. The built
UEFI image was exercised through this boundary; it produced a 171,520-byte
artifact with SHA-256
`f6260b54b92a27b3b82aba4aaad8dc93aace0f5b1d526dfaad61b6b93ded3607`.
The artifact can now be supplied explicitly to the disposable transaction's
existing `--artifact` input, replacing its deterministic fixture for an
artifact-backed test run. This improves artifact provenance only; it does not
qualify physical installation, bootloader handoff on hardware, or release
certification, and the supervised service still returns `Install` not-ready.

## WovenShell desktop: start-button icon and basket-weave wallpaper - 2026-10-22

Addressed real-hardware feedback that the desktop read as flat and
unbranded: the taskbar's "Woven" launcher button previously had no icon at
all (text only), and the wallpaper was a plain vertical gradient with a
single faint grid line. The button now draws a small basket-weave glyph
(two shades suggesting strands crossing over and under) to the left of its
label, and the wallpaper gradient now carries an actual basket-weave
texture: alternating light/dark `WEAVE_PITCH`-sized blocks with a thin
highlight seam at each boundary, rather than a flat grid.

Both changes are additive, 2D, and keep every existing color/geometry
constant that other code depends on (`START_FILL`, `START_X`,
`START_WIDTH`, `START_HEIGHT`, `WALLPAPER_TOP/BOTTOM`) unchanged; only what
is drawn inside the button and the wallpaper's per-pixel formula changed.
The kernel independently re-derives both for every desktop self-test
(`wallpaper()` is duplicated once per stage, by this codebase's existing
convention, so each self-test catches real regressions rather than trusting
shared code) -- all 16 duplicates across Stage 20.1-20.18 and Stage 24.59
were updated in lockstep, plus the one self-test that checks the start
button pixel-exact (Stage 20.1) now also re-derives the icon.

Verified: `python scripts/lint-all-features.py --only stage20` and
`--only stage24-59` both report every affected feature `ok`/`needs-image`
(no FAIL) under warning-denying Clippy, `host --all-targets` passes, and
QEMU self-tests for Stage 20.1 (1/2/4 CPUs, the one exercising both
changes), Stage 20.13, Stage 20.17 (3D Quick Settings, which also renders
the new wallpaper), and Stage 24.59 (1 CPU) all pass.

This is still a software-rendered 2D desktop; 3D rendering remains limited
to the Quick Settings panel and window title bars, off by default. Making
3D the actual default desktop look is a separate, much larger follow-on
effort: it needs new 3D rendering for the wallpaper, taskbar, every window
beyond its title bar, and icons (none of which has any 3D path today), plus
re-deriving every one of the pixel-exact self-tests above for a 3D-default
initial state. Scoped but not started.

## Stage 28 supervised installer-service boundary - 2026-10-22

The GUI now performs a bounded IPC inventory request against the supervised
service when it opens, displaying target-service availability without gaining
storage authority.

The signed catalog containing this client passes on 1/2/4 CPUs. The exact
rebuilt ISO is `audit-artifacts/stage28-gui-service.iso`, SHA-256
`1280f2ce834ef3f8b7581b4b02ff3219ed156936cbe200b00fa188126cabeeec`; exact
OVMF/QEMU boot gates pass on 1/2/4 CPUs with serial evidence in the matching
`stage28-gui-service-{1cpu,2cpu,4cpu}.log` files.

Added `libwoven::installer_ipc`, a fixed-capacity versioned WIP/WOP request
and response ABI, and a separate `woven-installer --service` daemon. The
kernel starts it as a `SystemService` with IPC, file, and storage authority
and verifies publication under `sys.storage.installer`. The graphical
installer remains a separate non-privileged presentation process. The service
now performs a capability-gated, read-only selected-target sector-count
query; on this QEMU machine with no usable target it returns explicit
unavailable status. Physical transaction execution and service-client wiring
remain the next implementation increment. Normal system-apps boot passed on
1 CPU with the new publication marker, and the signed eleven-app catalog
gate remained green.

The service ABI now validates compact plans without allocation: target sector
identity/capacity, FAT32 selection, minimum partition geometry, username,
timezone, keyboard, network mode, and explicit destructive confirmation are
checked before a request can reach the future write transaction. `Validate`
returns success for a valid plan when inventory is available; `Install` still
returns not-ready because the physical GPT/FAT32/EFI transaction is not yet
implemented in the kernel service.

## Stage 28 graphical installer frontend and current ISO gate - 2026-10-22

`apps/woven-installer` now has a bounded `--gui` mode using the established
WOPN/shared-surface compositor protocol. It renders all validated installer
states, handles bounded resize/pointer/close messages, and has no disk,
filesystem, bootloader, network, or destructive authority. The render buffer
was kept at 320x240 so the eleven-app loader gate retains its memory margin.

The updated signed eleven-app catalog passes QEMU on 1/2/4 CPUs. The exact
current ISO was generated as
`audit-artifacts/stage28-wovenhat-installer-gui.iso` and passes the exact ISO
OVMF/QEMU gate on 1/2/4 CPUs. ISO SHA-256:
`58f1b6c63049b8c0128cd60e2f5812ab0fb7bb4d75849b408bb2cbb7fdc4173e`.
Serial evidence is retained in the matching `stage28-wovenhat-installer-gui-
{1cpu,2cpu,4cpu}.log` files.

The GUI remains a presentation process. A privileged installer service that
performs target-disk discovery, physical partitioning/formatting, bootloader
handoff, account provisioning, and network configuration is not yet present;
physical installation and hardware qualification remain open.

## Stage 28 real chained disk-install transaction - 2026-10-22

Added `scripts/execute-disk-install.py`, which chains the existing GPT layout
builder, FAT32-compatible formatter, and EFI artifact preparer into one
atomic install transaction driven directly from a validated plan, instead of
those three components being invoked independently by a test harness. This
closes two gaps flagged by the disposable-image acceptance harness: no
executor previously performed the real partition/format/bootloader sequence
as a single transaction, and nothing was ever written into the formatted
volume as an actual file.

`format-file-image-fs.py` now accepts an optional bounded `settings` payload
(<= 512 bytes) and writes it as one real FAT32 root-directory file
(`INSTALL.JSN`, cluster 3), backward-compatible with every existing caller
that omits it (byte-for-byte identical output verified by the pre-existing
test suite). A new `read_installed_settings()` reads the file back from the
on-disk directory entry and cluster, independent of the caller already
knowing the expected value.

The new executor only accepts the `fat32` filesystem (the only one with a
real formatter); `ext4`/`wovenfs` are rejected with a clear error.
Whole-transaction rollback was verified by injecting a failure after each of
the three real steps (layout, format, EFI) and confirming the image is
restored to its exact pre-transaction bytes with no leftover temp files.
Focused host tests (`tests/test_disk_install_executor.py`,
`tests/test_file_image_filesystem.py`) and a CLI smoke test pass; the
pre-existing disposable-acceptance and layout/filesystem/EFI test suites
remain green.

This remains disposable-file-image-only: no device path is ever opened, no
filesystem is mounted, and this does not claim bootability, hardware
qualification, or release certification. It does not yet replace
`execute-file-install.py`'s established mock-executor contract (left
unchanged) or wire this real path into the Stage 28 release packager/ISO
pipeline.

The end-to-end Stage 28 disposable acceptance harness now invokes this real
transaction for the final image, verifies the embedded settings by reading
the FAT32 directory entry and cluster back, and injects rollback failures
after layout, format, and EFI steps. The old mock executor remains covered
separately for compatibility, but is no longer the final-image acceptance
path.

Unrelated, pre-existing and confirmed to predate this change: one test in
`tests/test_stage10_firmware_vars_master.py` fails on this Windows host over
a `\\` vs `/` path-separator mismatch in an OVMF pflash drive argument. Not
touched; flagged in `docs/agent-coordination.md`.

## Stage 28 exact ISO artifact gate - 2026-10-22

The exact validated Stage 19 catalog image containing all eleven signed
applications was converted to
`audit-artifacts/stage28-wovenhat-catalog.iso` and booted through OVMF/QEMU
on 1, 2, and 4 CPUs. Each run reached the eleven-application PASS marker with
no serial failure lines. Evidence is retained in
`audit-artifacts/stage28-wovenhat-catalog-iso-{serial,2cpu-serial,4cpu-serial}.log`.
The ISO builder reported SHA-256
`4102e72057702dd37d2ade52d65c36a8b5bc5f01d6776114b4841c512d5af04a`.

This qualifies the file-backed UEFI ISO/QEMU artifact path only. It does not
qualify physical USB/disk installation, native hardware, Secure Boot, or
release publication.

## Stage 28 release artifact now includes the UEFI ISO - 2026-10-22

The file-backed installer packager now has an explicit `--include-iso` path
that uses the reviewed transactional ISO9660/El Torito builder, records the
ISO filename/digest/format in `installer-manifest.json`, and includes it in
`SHA256SUMS`. The release packager now emits the ISO beside the UEFI disk
image and includes it in the release archive and manifest. Focused package
tests, Python compilation, and diff checks pass.

This closes the packaging omission only. The exact ISO still requires the
existing OVMF/QEMU gate; this machine currently has no QEMU executable, and
physical USB/disk installation and hardware qualification remain unclaimed.

## Stage 28 ISO9660/El Torito UEFI artifact and QEMU gate - 2026-10-22

Added `scripts/build-uefi-iso.py`, which extracts the verified EFI System
Partition from a regular GPT WovenHat image and builds a transactional
ISO9660/El Torito image with an EFI-platform boot catalog. FAT16 and FAT32
ESPs are accepted because the current bootloader image is FAT16. Device-like,
protected, symlink, and physical-media paths are refused. Added
`scripts/test-uefi-iso-qemu.py` to boot the exact ISO with caller-supplied OVMF,
preserve serial output, and distinguish firmware/kernel handoff from later
self-test failures.

The generated ISO booted in QEMU using the installed OVMF firmware and reached
`UEFI bootloader started`, `TRACE: exiting boot services`, and kernel output.
The selected existing debug image then reported an app-catalog self-test
failure, so this is not a full release gate. No physical disks were accessed.
The catalog harness now supports `--image-path-output`, making the exact image
used by its validated QEMU run available to downstream ISO generation instead
of relying on target-directory timestamp selection.

The exact-path flow was rerun successfully: Stage 19 catalog QEMU passed with
all eleven signed applications, the emitted image was converted to an ISO, and
that exact ISO reached the same eleven-application PASS marker in OVMF/QEMU.
When the source ESP exceeds El Torito's 65536-sector field, the builder only
accepts an all-zero discarded tail and rewrites the bounded FAT total-sector
field; non-zero truncation is refused.

## Stage 24.59 disk-pressure gap closed (Claude, 2026-10-22)

`stage24-59-test`'s full `system-apps` package build, previously flagged in
this file and in `docs/agent-coordination.md` as structurally disk-tight,
now passes cleanly on 1, 2, and 4 CPUs with fresh evidence
(`target/woven-shell-change-pin-{1,2,4}cpu`). Two independent things closed
it: Codex's `system-apps` feature addition to `serial::log_position`/
`read_log`'s cfg gate (commit `a3617063`, fixing the same cfg-gate-cascade
bug class seen earlier at Stage 20.12) and Codex's WovenInstaller
integration (`b882f09a`/`73c2d34f`), which brought the full 11-app
packaging run to a state that builds and boots within this machine's disk
budget. No code change was needed on this side; this entry just closes the
previously-recorded gap now that it has reproduced clean three times in a
row.

## Stage 28 disposable-image end-to-end acceptance harness - 2026-10-22

Added `scripts/test-stage28-disposable-acceptance.py` and focused coverage in
`tests/test_stage28_disposable_acceptance.py`. The harness creates a new
temporary regular file, validates a canonical plan, exercises the existing
file-backed executor including success and both injected rollback paths, then
assembles and verifies the final disposable GPT-like image, FAT32-compatible
data region, EFI artifact, and deterministic acceptance-manifest digests.
It never accesses physical/device paths or mounts anything. The executor is
kept on a separate disposable copy because its established whole-file mock
filesystem replacement is not a partition-preserving installation operation.

This is not physical installation, bootability qualification, hardware
qualification, ISO generation, or release certification.

## Stage 28 WovenInstaller production system-app integration - 2026-10-22

The signed `woven-installer.wapp` is embedded in the kernel's production
`system-apps` and Stage 19 catalog-test images through
`WOVENHAT_INSTALLER_IMAGE`. Production catalog publication adds the read-only
`/bin/wovenhat-installer` entry and
`/share/applications/woven-installer.app`; the installer remains a display-only
Ring-3 application with no capabilities or privileged authority. Existing ten
entries and their paths are preserved. All system-app QEMU launchers and the
release validation environment now provide the eleventh image.

Focused packaging tests, Python compilation, catalog-image build, and
warning-denying production `system-apps` kernel Clippy pass. The Stage 19
catalog QEMU gate passes on 1/2/4 CPUs with the eleven-app marker; evidence is
under `audit-artifacts/system-app-catalog-{1,2,4}cpu`. This slice performs no
physical-disk access.

## Stage 28 WovenInstaller package/catalog wiring - 2026-10-22

The existing signed system-app packaging path now builds and signs
`apps/woven-installer`, publishes it as `packages/woven-installer.wapp`, and
adds the stable `/bin/wovenhat-installer` install path and catalog entry to
the manifest. The entry is explicitly non-privileged with an empty
capability set and `display-only` interface metadata. Fail-closed metadata
validation rejects install, disk, partition, format, and bootloader
capabilities; the ten existing app entries are unchanged.

Focused packaging tests, Python compilation, freestanding release build, and
warning-denying freestanding Clippy pass. Kernel integration is recorded in
the production system-app integration entry above.

## Stage 28 bounded WovenInstaller Ring-3 application shell - 2026-10-22

Added the disjoint `apps/woven-installer` no-std Ring-3 shell. It consumes a
compact snapshot of the validated installer UI/state model and renders bounded
welcome, target, partition-review, settings, progress, cancelling, cancelled,
failure, and success screens through the deterministic textual fallback.
Unknown fields, duplicate keys, control characters, invalid progress, missing
prerequisites, and snapshots without `validated=1` fail closed. Progress and
success are presentation states only; the output always identifies the install
action as unavailable, and the app has no disk, partition, filesystem,
bootloader, network, or subprocess path.

The three host parser tests, freestanding `x86_64-unknown-none` release build,
freestanding warning-denying Clippy, formatting check, and diff check pass.
No QEMU gate is claimed: the app is not embedded into the kernel by this
disjoint-only slice, so booting it in QEMU would not test the new behavior.
Physical installation and release certification remain open.

## Stage 28 disposable UEFI boot artifact preparation and verifier - 2026-10-22

Added `scripts/prepare-uefi-boot-artifact.py` with focused coverage in
`tests/test_uefi_boot_artifact.py`. It consumes the prior disposable layout,
locates its single ESP, and writes a bounded, checksummed envelope containing
the exact `EFI/BOOT/BOOTX64.EFI` path and artifact bytes. It accepts either an
explicit regular artifact file or a deterministic fixture. Writes are limited
to the ESP and transactional rollback restores the complete ESP after injected
failures.

The envelope is a test-only file-image record, not a mounted FAT directory.
Physical/device paths, symlinks, mounts, firmware flashing, bootloader
installation, and hardware access fail closed. Artifact size is bounded to
4 MiB. Verification reports `bootability: not-qualified`; no kernel
bootability or QEMU boot acceptance is claimed. The Stage 28 focused chain,
Python compilation, and diff checks pass.

## Stage 28 disposable file-image filesystem formatter/verifier - 2026-10-22

Added `scripts/format-file-image-fs.py` with focused coverage in
`tests/test_file_image_filesystem.py`. The formatter consumes the prior
layout builder's checksummed metadata and writes only its single data region.
It emits deterministic FAT32-compatible metadata: BPB, backup boot sector,
FSInfo and backup FSInfo, two matching FATs, reserved entries, root-cluster
entry, and the fixed `WOVENHAT` label. Verification checks sector geometry,
region bounds, FAT bounds/copies, auxiliary-sector pointers, signatures,
determinism, and identity metadata.

The operation is transactional for the disposable image: before/after-write
and after-verification injected failures restore the original data region
byte-for-byte. Symlinks, device/protected identities, missing authorization,
multiple data regions, real mounts, and physical installation fail closed.
The 34-test Stage 28 focused chain, Python compilation, and diff checks pass.
This is not physical installation, bootloader formatting, hardware
qualification, or release certification.

## Stage 28 disposable file-image layout builder and verifier - 2026-10-22

Added `scripts/build-file-image-layout.py` and focused coverage in
`tests/test_file_image_layout.py`. The builder requires an existing regular
non-symlink file and an explicit test-harness authorization, then writes a
deterministic GPT-like image containing a protective MBR, primary/backup GPT
headers and entries, an aligned ESP region, aligned plan data regions, and
checksummed metadata. Verification checks signatures, metadata integrity,
capacity, alignment, bounds, and overlap.

Physical/device/protected paths, symlinks, missing files, and unauthorized
images fail closed. This is disposable-image preparation only: it does not
partition real media, mount or format filesystems, write bootloaders, access
hardware, or claim production installation support.

## Stage 28 bounded installer UI/state model - 2026-10-22

Added `scripts/installer-ui-model.py`, a pure immutable state-transition model
for the welcome, target-selection, partition-plan review, user/timezone/
keyboard/network settings, progress, cancellation, success, and failure
screens. It accepts only bounded caller-provided summaries: ordinary text is
limited to 128 characters, error messages to 256, progress messages to 128,
and progress is an integer in the inclusive range 0..100.

The model requires a safe, unambiguous target summary, enforces the ordered
screen flow, requires `confirm_install` with `confirmed: true` before any
progress or success event, and rejects backward progress. Cancellation must
complete through a separate cancellation state; cancelled and failure states
are terminal and reject all further events. The model performs no disk or
filesystem I/O and invokes no partition, format, bootloader, subprocess, or
graphical UI operation.

Focused host tests and Python compilation/diff checks pass. This is only a UI
state model: no graphical installer, real installation executor, target-disk
access, partitioning, formatting, bootloader installation, rollback, or ISO
support is claimed.

## Stage 28 disposable file-backed installation executor - 2026-10-22

Added `scripts/execute-file-install.py` as a test-only transaction boundary.
It requires a validated installer plan and an explicitly supplied regular
non-symlink image, binds the plan identity to the image's pre-install digest,
rejects device/protected identities, and atomically replaces the image with a
bounded deterministic mock filesystem payload. Injected failures and payload
verification failures restore the original image.

Focused tests cover success, deterministic digest verification, symlink and
protected-identity refusal, digest mismatch, and rollback before/after atomic
replacement. This is not real installation: no physical device, mount,
format, partition, bootloader, or bootable image operation is implemented.

## Stage 28 immutable installer-plan validator - 2026-10-22

Added `scripts/validate-installer-plan.py` as a dry-run, file-free planning
boundary. It accepts an explicit target path/serial identity, a supplied disk
inventory snapshot, bounded sector-aligned partition sizes, filesystem choice,
and normalized user/timezone/keyboard/network settings. It rejects protected
or ambiguous targets, overflow, insufficient capacity, and invalid fields,
then emits deterministic canonical JSON with explicit no-I/O and
not-implemented operation markers.

The validator and its focused tests never open a device, invoke discovery,
partition, format, install, or write a bootloader. This slice does not claim
installation support or Stage 28 acceptance.

## Stage 28 read-only target-disk inventory and safety classification - 2026-10-22

Added `scripts/inventory-target-disks.py` with fixture-driven parsing for
normalized records, PowerShell `Get-Disk` JSON, and Linux `lsblk -J` JSON.
The report can identify a uniquely marked Windows system/boot disk and marks
removable, USB, external, and backup-labelled disks as protected candidates.
It fails closed on missing or duplicate serials, invalid capacity, missing
path/number, or ambiguous system-disk identity. The parser and CLI only read
caller-supplied files; they do not execute discovery commands or touch devices.

Five focused tests and Python compilation checks pass. This remains an
inventory/classification boundary only: no target selection, partitioning,
formatting, installation, ISO generation, or hardware qualification is
claimed.

## Stage 28 ISO capability probe - 2026-10-22

Added `scripts/probe-installer-iso.py` with focused host coverage in
`tests/test_installer_iso_probe.py`. The probe is file-only and fail-closed: it
checks the GPT UEFI image shape, searches for an ISO builder, verifies a
file-backed `EFI/BOOT/BOOTX64.EFI` tree when supplied, and emits no artifact.
It never mounts a filesystem, accesses a physical disk, or claims ISO support.

The current machine is blocked because no `xorriso`, `mkisofs`, `genisoimage`,
or `oscdimg` executable is available; the existing build pipeline emits a GPT
UEFI disk image rather than an ISO-native El Torito layout; and no extracted
EFI boot tree was available for verification. The probe's qualification rule
requires generating an artifact and booting that exact artifact in QEMU before
ISO support can be claimed. Three probe tests, the unchanged three installer
tests, and Python compilation checks pass. No ISO was generated and no QEMU ISO
gate was run.

## Stage 28 WovenInstaller bounded file-artifact slice - 2026-10-22

Added `scripts/package-installer.py` as a non-destructive boundary around the
existing `build.rs` UEFI disk-image output. It consumes the release-validation
image record or an explicitly supplied file image, verifies the recorded
SHA-256 (when present), checks sector alignment and protective-MBR/GPT
signatures, copies the image into a new output directory, and emits a stable
`installer-manifest.json` plus `SHA256SUMS`. Metadata identifies QEMU and USB
file-image use, while recording that ISO generation and physical-disk writes
are not implemented.

Focused host tests pass (3 tests). This is not Stage 28 acceptance: graphical
installation, disk detection, partitioning, ISO generation, target-disk
installation, rollback, and physical boot qualification remain open. No
physical disk was accessed.

## Stage 27.5 bounded WovenRecovery environment foundation - 2026-10-22

WovenRecovery now has a signed Ring-3 system application, a dedicated
capability, and a boot-mode authorization gate. Its fixed ABI exposes bounded
status, filesystem-check, and serial-log reads. Reset and reinstall are
fail-closed admission checks only: they return an explicit not-implemented
result and do not mutate storage. The QEMU acceptance exercises unauthorized
reset, authorized status/log access, both admission paths, and the check
unavailable path without claiming destructive recovery is complete.

The 1/2/4-CPU QEMU gates pass with exit 33. Recovery app Clippy, kernel
feature Clippy, workspace all-target warning-denying Clippy, and 115 libwoven
host tests pass. Evidence is retained in
`audit-artifacts/stage27.5-recovery-{1,2,4}cpu` and documented in
`docs/audit-stages-consolidated.md`.

This closes only the bounded recovery-environment foundation. Destructive
reset/reinstall execution, production update-slot sizing, bootloader handoff,
power-loss recovery, installer integration, and physical qualification remain
open before release certification.

## Stage 27.5 end-to-end WovenUpdate IPC acceptance - 2026-10-22

The Stage 27.5 QEMU gate now builds the signed `woven-update` Ring-3 image,
starts the real `--service` daemon, discovers `woven.update`, sends a valid
WUDI Query, verifies the activated-slot response, sends a truncated malformed
frame, verifies the deterministic bad-request response, and reaps the daemon.
The 1/2/4-CPU gates pass with exit 33. The daemon receive buffer is aligned to
the kernel's 256-byte IPC transport limit; the larger codec bound remains a
parser bound and oversized transport requests fail closed.

Host app tests, freestanding app Clippy, stage27-5 kernel Clippy, and
workspace all-target warning-denying Clippy pass. Evidence is recorded in
`docs/audit-stages-consolidated.md` and the retained
`audit-artifacts/stage27.5-{1,2,4}cpu` directories.

This closes the daemon IPC exchange gate only. Production-sized block-device
slots, bootloader target/attempt handoff, power-loss injection/recovery,
WovenRecovery, installer integration, and physical qualification remain open.

## Reduced Motion accessibility toggle added to Quick Settings (2026-10-22)

Quick Settings gained a "Reduced Motion" row (row 9 of 10) wiring
`libwoven::accessibility::Settings`'s existing `reduced_motion` preference
into WovenShell for the first time: when on, a newly opened window appears
at full opacity immediately instead of animating in via the usual spring.
Off by default, matching `Settings::new()`.

Verified on 1 CPU: `stage20-10-test`, `stage20-11-test`, `stage20-17-test`,
`stage20-18-test`, and `stage20-12-test` all pass with the panel at its
new 10-row height; libwoven host tests and default-feature kernel clippy
are clean. `stage24-59-test` could not be re-verified this round -- its
full `system-apps` package build (10 applications) repeatedly hit
disk-space limits on this machine regardless of reclaiming beforehand.
The code path this change touches (Quick Settings row rendering and click
dispatch) is identical to what the five passing gates above already
exercise, so this looks like an environment-resource gap rather than a
known defect, but it's recorded here rather than silently treated as
covered, per the project's "report incomplete gates explicitly" policy.

## Stage 20.12 fix - accounts now actually load into the kernel identity AUTHORITY (2026-10-21)

`stage20_12_self_test` wrote a real accounts file to the VFS and spawned
WovenShell with `--accounts`, but nothing in that path ever loaded those
credentials into the kernel's `AUTHORITY`, so every sign-in/lock/unlock
check failed with `Error::NotReady` -- indistinguishable from a wrong PIN
at the UI layer. This was found and deliberately left unfixed earlier
today (logged as a cascade through independently-gated syscall layers too
deep to rush). Revisited with a clearer picture and traced the full chain
this time: the `identity_runtime` module and `load_accounts_file`, and
three syscalls the sign-in/lock/unlock flow exercises (`IdentityOpenVerified`,
`IdentityLock`, `IdentityUnlockVerified`, each gated at three layers:
enum variant, dispatch arm, handler), plus `task::set_current_credentials`.
The self-test now also calls `load_accounts_file` after writing the
accounts file and grants `AccountManage`.

Verified on 1/2/4 CPUs: the three previously vacuous or impossible steps
now genuinely exercise real PBKDF2-backed credential verification (ada's
PIN is rejected via `WrongPassword`, not `NotReady`; lin's PIN signs in;
the lock screen refuses ada's and accepts lin's). Re-verified Stage 24.59,
20.11, 20.17, and 20.18, which share the same identity syscalls or
adjacent code, plus host tests and clippy -- all clean.

## Stage 20.18 - 3D desktop mode extended to window title bars (2026-10-21)

The "3D Desktop" toggle (Stage 20.17) now also perspective-projects every
window's title-bar fill, not just the Quick Settings panel: flat colour,
not textured (a title bar's width is the window's own and can exceed the
bounded texture buffer Quick Settings uses), with the same bottom-anchored,
top-narrowed card tilt. Found and fixed a real bug while building this:
the toggle only marked the Quick Settings panel's own area dirty, so open
windows never got told to repaint in the new mode and stayed flat until
something else triggered a redraw; the toggle now marks the whole screen
dirty too.

The Stage 20.18 QEMU gate proves the same kind of non-trivial transform as
Stage 20.17 (a window's flat title-bar top-left corner stops being the
fill colour once 3D is toggled on, while the bottom-left stays anchored),
on 1/2/4 CPUs. Stages 20.17, 20.10, 20.11, and 24.59 were re-verified
clean since they share the same toggle/Quick-Settings code this touched.

## Stage 30.5 - scheduler yield-latency micro-benchmark (2026-10-21)

Added a `hello-gui --scheduler-benchmark` mode: 1000 round trips of
`yield_now`, timed with `libwoven::cycles()`. With no other runnable
process contending for this CPU, the scheduler always re-selects the same
task immediately, so this measures the voluntary-yield path's own overhead
(syscall entry, run-queue bookkeeping, syscall exit) rather than a context
switch to different process state -- a true cross-process context-switch
benchmark is a separate, larger undertaking (needs a ping-pong pair of
processes). Validated on 1/2/4 CPUs via `stage30-5-test`: ~578,000-582,000
cycles min (~687,000-748,000 average) under QEMU TCG -- noticeably more
than the Stage 30.1 bare syscall baseline (~25,000-30,000 cycles),
consistent with the scheduler doing real run-queue/priority bookkeeping on
every yield rather than just validating and returning.

## Stage 30.4 - compositor frame-present-latency micro-benchmark (2026-10-21)

Added a `hello-gui --compositor-benchmark` mode: 500 round trips of
toggling one pixel of a placed window and flushing it to the framebuffer
(`surface.write` + `compositor_present`), timed with `libwoven::cycles()`.
Each iteration alternates the pixel's colour so the compositor cannot skip
the frame as undamaged, which would understate real cost. Validated on
1/2/4 CPUs via `stage30-4-test`: ~338,000-353,000 cycles min (~416,000-
456,000 average) under QEMU TCG, roughly comparable to the Stage 30.3
filesystem-write cost. This is a single-pixel, single-window worst-case
floor, not a measurement of full-desktop frame time with many windows and
damage regions, and it is a TCG emulation number, not real hardware or
real display refresh timing.

## Stage 30.3 - filesystem-latency micro-benchmark (2026-10-21)

Added a `hello-gui --fs-benchmark` mode and a `libwoven::seek` wrapper
(`syscall::lseek` didn't have one yet). It times 500 seek+write and
seek+read round trips of a 256-byte payload against a kernel-pre-created
scratch file, reporting write and read separately since they're distinct
syscalls with distinct costs. Validated on 1/2/4 CPUs via `stage30-3-test`:
write averages ~515,000-586,000 cycles, read averages ~300,000-366,000
cycles under QEMU TCG (both roughly an order of magnitude more than the
Stage 30.1 syscall baseline and comparable to the Stage 30.2 IPC baseline,
consistent with filesystem writes going through VFS node lookup, access
checks, and a full buffer copy per call). These are TCG emulation numbers
for the in-RAM VFS path, not real disk I/O latency or native hardware
timing; they establish a baseline and a repeatable tool, not an absolute
performance claim.

## Stage 20.17 - 3D desktop mode: Quick Settings panel (2026-10-21)

WovenShell's Quick Settings panel can now render as a real, perspective-
projected 3D card (bottom edge anchored, top edge receded and narrowed)
instead of a flat 2D rectangle, toggled by a new "3D Desktop" row in the
panel itself. This is the first time the `libwoven::graphics`
`Camera`/`textured_triangle_3d` primitives (proven standalone in the Stage
20.16 chrome demo) have been wired into WovenShell's actual, live
compositor rather than a disconnected proof of concept.

Off by default: every already-accepted gate's pixel-exact 2D rendering is
unchanged unless a user explicitly opens Quick Settings and toggles it on.
The flat content is rendered once into an offscreen `.bss` texture, then
sampled by two `textured_triangle_3d` calls per tile the panel spans
(matching the shell's existing tiled repaint architecture), with the
panel's flat footprint repainted with wallpaper first so the narrowed top
edge doesn't leave stale pixels from the previous frame.

The Stage 20.17 QEMU gate proves a real, non-trivial transform is applied
(the flat panel's top-left corner becomes wallpaper once 3D is toggled on,
while the deliberately-anchored bottom-left corner is unchanged), on
1/2/4 CPUs. It does not claim 3D rendering anywhere else in the desktop
(window chrome, workspaces, other panels remain flat 2D), nor a
production-tuned visual design -- the perspective amount is a first,
reasonable choice, not a finished aesthetic.

## Stage 27.5 service-process slice - 2026-10-21

`apps/woven-update` now has a bounded `--service` mode. It publishes the
`woven.update` endpoint, decodes the existing fixed-size WUDI request ABI,
forwards only validated operations to `libwoven::update_transaction`, and
returns fixed-size responses. Malformed requests fail closed; package bytes do
not cross IPC; an empty receive yields to the scheduler rather than spinning;
and a failed response send terminates the daemon so supervision can restart it.
The app's host tests and freestanding warning-denying Clippy pass. The
production `system-apps` boot path now verifies and starts both
`woven-package` and `woven-update` with the bounded IPC, file, and storage
capability set. The shell QEMU gate passed on 1/2/4 CPUs with both service
publication markers; evidence is in
`docs/audit-stages-consolidated.md`.

The dedicated Stage 27.5 gate now proves the client/server exchange. This is
not full update/recovery acceptance: production-sized block-device slots,
bootloader handoff, power-loss recovery, and physical USB qualification
remain open.

## Stage 27.5 - privileged update-service transaction boundary (2026-10-21)

Implemented the capability-gated kernel update transaction boundary and
syscall 239. Signed WUPD packages are verified before staging; the inactive
slot is written before its WUAB metadata copy is published; dual-copy state is
selected by sequence with fail-closed corruption/tie handling; activation,
boot-failure, boot-success, and bounded query operations are exposed through
`libwoven`. The Stage 27.5 executable QEMU gate passes on 1/2/4 CPUs, and the
focused kernel/libwoven Clippy and 113 libwoven host tests pass.

This is not a release claim. Current WovenFS slots are single-file bounded to
4 KiB, so the service rejects larger images instead of partially installing
them. The daemon process and IPC loop are now present, but boot-time
supervision/catalog wiring and end-to-end client/server acceptance remain
required, along with block-device slot storage, bootloader handoff/boot-target
attempt persistence, power-loss recovery, and physical USB qualification.

## Stage 27.4 - bounded update IPC contract (2026-10-21)

Added `libwoven::update_ipc`, a fixed-capacity request/response ABI for the
future privileged update service. It validates operation-specific generation,
slot, and `/updates/` candidate-path rules while keeping image bytes outside
IPC. The focused library tests now pass 113 cases and warning-denying Clippy
passes. Evidence is in
`docs/audit-stages-consolidated.md`.

This is not service acceptance: syscall dispatch, capability issuance, signed
candidate staging, durable image-slot writes, bootloader handoff, and
power-loss recovery remain unresolved.

## Stage 27.3 - signed update-package producer (2026-10-21)

Added `tools/wupd-packager`, which creates the bounded version-1 `WUPD`
envelope consumed by the kernel verifier. It validates generation and image
limits, hashes the exact image, signs the immutable manifest and payload with
Ed25519, and requires matching explicit signing/trusted-key inputs in release
mode. Tool tests, warning-denying Clippy, release-key boundary checks, and the
prior Stage 27.1 activation/rollback QEMU gate passed on 1/2/4 CPUs. Evidence
is in `docs/audit-stages-consolidated.md`.

This does not yet install images, own the durable A/B state, expose an
update-service IPC/syscall protocol, perform bootloader slot handoff, recover
from power loss, or qualify physical USB media.

## Stage 26.5 - bounded internationalization foundation (2026-10-21)

Added `libwoven::i18n`, a fixed-capacity userspace foundation for English and
Chichewa locale identifiers, bounded translation catalog lookup, locale-aware
date ordering, integer formatting including `i64::MIN`, and Unicode scalar
validation. It also provides a transactional bounded Latin combining-mark
normalization subset that fails closed for unsupported marks. WovenShell now
uses bounded English/Chichewa catalogs for its lock-screen strings.
The i18n boundary also resolves bounded strong-direction runs for common
Latin/Hebrew/Arabic text and numbers; it does not claim full UAX#9.
Focused and full workspace host gates are recorded in
`docs/audit-stages-consolidated.md`.

This is not full internationalization: complete Unicode normalization and
property data, full RTL/bidi ordering, script shaping, keyboard layouts, IMEs, and wiring
every application string to a catalog remain open.

The same acceptance cycle corrected a separately observed production-shell
stack boundary: the bounded user stack is now 16 KiB plus its guard page, and
the corrected system-app QEMU gate passes on 1/2/4 CPUs.

## Stage 26.1 - bounded accessibility core (2026-10-21)

Added `libwoven::accessibility`, a fixed-capacity userspace contract for
accessibility tree nodes, roles, labels, bounds, enabled/focusable state,
deterministic keyboard focus navigation, and screen-reader focus
announcements. It also provides bounded caption storage and settings for high
contrast, reduced motion, scale, and magnification.

The module has no unbounded allocation and does not depend on locale or text
shaping policy. Its focused and full-workspace host gates pass all 100
`libwoven` tests and warning-denying Clippy. Desktop-wide screen-reader integration, translated strings,
RTL shaping, and physical accessibility-device qualification remain open.

## Stage 30.2 - IPC-latency micro-benchmark (2026-10-21)

Added a `hello-gui --ipc-benchmark` mode that sends and receives a one-byte
message to and from its own process id 1000 times (same-uid self-IPC is
permitted by `may_ipc_with`), timing each round trip with
`libwoven::cycles()`. Validated under QEMU TCG on 1/2/4 CPUs via
`stage30-2-test`: a self send+receive round trip costs roughly 400,000-740,000
cycles on average (1 CPU: min 408,481/avg 566,714/max 9,008,594; 2 CPU: min
356,574/avg 559,584/max 9,388,518; 4 CPU: min 487,630/avg 738,264/max
17,016,406) -- an order of magnitude more than the Stage 30.1 `current_pid`
baseline, consistent with IPC doing a buffer copy, a queue transaction, and
an audit-ledger write per message versus `current_pid`'s pointer-free,
capability-free path. These are TCG emulation numbers from a single process
talking to itself, not cross-process latency or real hardware timing; they
establish a comparable baseline and a repeatable tool, not an absolute
performance claim.

## Stage 30.1 - syscall-latency micro-benchmark (2026-10-21)

Added `libwoven::cycles()`, a thin `RDTSC` wrapper usable from Ring 3 (this
kernel never sets `CR4.TSD`), and a `hello-gui --syscall-benchmark` mode that
times 1000 round trips of `current_pid()` and reports min/avg/max cycle
counts, failing only if the measured average is zero. Validated under QEMU
TCG on 1/2/4 CPUs via `stage30-1-test`: a `current_pid()` round trip costs
roughly 25,000-30,000 cycles on average (min ~25,168, max varies with host
scheduling noise, observed up to ~302,000 on a single outlier). These are TCG
emulation numbers, not a measurement of real hardware syscall latency; they
establish a baseline and a repeatable tool for comparing future syscall-path
changes, not an absolute performance claim.

## Stage 25.15 - task-owned userspace resume target (2026-10-21)

The suspend action now validates the complete syscall frame against the
currently running task's kernel stack and retains its address in the committed
preparation record. The firmware-wake continuation restores the FACS vector,
wake trampoline, secondary CPUs, and drivers, writes a successful return value,
and jumps through the existing syscall epilogue to `iretq` back to userspace.
Invalid frame pointers are rejected before preparation begins.

The Stage 25.15 QEMU gate checks the empty-record and invalid-pointer
invariants on 1/2/4 CPUs. It does not issue S3; physical firmware wake and
hardware qualification remain required.

## Stage 25.13 - persistent firmware-wake record integration (2026-10-21)

Prepared suspend state is now retained in a kernel-owned pending record across
the PM1 transition boundary. The low-memory wake entry consumes the saved CPU
context through the Stage 25.12 restore primitive and transfers to a
continuation that restores the FACS vector, trampoline, secondary CPUs, and
drivers. The guarded integration gate passed on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and the
retained `audit-artifacts/stage25.13-*` directories.

The QEMU gate validates record ownership and continuation wiring without
issuing S3. Actual firmware wake and physical suspend/resume qualification
remain unproven.

## Stage 25.12 - CPU resume-context restoration primitive (2026-10-21)

The kernel now has a validated non-returning restore path that reloads CR0,
CR3, CR4, EFER, GDT, IDT, RSP, and RFLAGS, then transfers to a
kernel-provided continuation. The self-test restores the current context and
continues to a QEMU success marker on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and
the retained `audit-artifacts/stage25.12-*` directories.

This proves the architectural restore primitive, not a complete firmware S3
wake: the wake record/continuation, driver restoration after firmware wake,
and physical suspend/resume qualification remain open.

## Stage 25.11 - PM1 S3 request boundary (2026-10-21)

The prepared suspend token now has an internal `enter_suspend` path that
encodes the firmware-provided `_S3` types, writes PM1a and optional PM1b with
`SLP_EN`, and treats a returning firmware as an explicit failure. If the
machine remains running, vector, trampoline, secondary-CPU, and driver state
are rolled back before returning. The guarded encoding self-test passed on
1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and the
retained `audit-artifacts/stage25.11-*` directories.

The test does not issue a destructive S3 request. This stage does not claim
CPU register restoration, firmware wake completion, a userspace suspend ABI,
or physical suspend/resume qualification.

## Stage 25.10 - secondary-CPU suspend barrier (2026-10-21)

The BSP can now transactionally evacuate and park every online secondary CPU
through the established scheduler-safe hotplug checkpoints, retaining the
original online mask. A failed partial barrier restores the completed prefix;
rollback re-onlines every parked CPU in reverse order and verifies the exact
mask. The guarded barrier self-test passed on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and
the retained `audit-artifacts/stage25.10-*` directories.

This does not claim CPU register restoration, PM1 S3 entry, firmware wake, or
physical suspend/resume qualification.

## Stage 25.9 - transactional FACS wake-vector installation (2026-10-21)

Suspend preparation now installs the reserved low wake-page address into both
validated FACS waking-vector fields and retains the previous values in the
private preparation token. Abort restores both firmware fields before
releasing the trampoline and resuming drivers. The guarded transaction
self-test passed on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and the
retained `audit-artifacts/stage25.9-*` directories.

This does not claim secondary-CPU coordination, CPU register restoration,
PM1 S3 entry, firmware wake, or physical suspend/resume qualification.

## Stage 25.8 - CPU resume-context capture (2026-10-21)

The suspend preparation token now captures the current CPU control state into
kernel-owned storage: CR0, CR3, CR4, EFER, RFLAGS, RSP, and the GDT/IDT
descriptors. The capture is validated for long-mode paging, canonical pointers,
aligned CR3, descriptor limits, and required RFLAGS state before preparation
can succeed. The guarded self-test passed on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and the
retained `audit-artifacts/stage25.8-*` directories.

This does not claim register restoration, secondary-CPU coordination, FACS
vector installation, S3 entry, firmware wake, or physical suspend/resume
qualification.

## Stage 25.7 - wake-trampoline staging (2026-10-21)

The proven AP transition image can now be staged on the reserved low wake
page with a private identity-mapped root, dedicated wake stack, and entry
point. Page-table ownership is transactional: abort releases the image and
restores drivers. The guarded stage25-7 QEMU test passed on 1/2/4 CPUs.

Evidence is in `docs/audit-stages-consolidated.md` and the
retained `audit-artifacts/stage25.7-*` directories.

This does not claim FACS vector installation, CPU-state save/restore,
secondary-CPU coordination, S3 entry, firmware wake, or physical
suspend/resume qualification.

## Stage 25.6 - low-memory wake-page reservation (2026-10-21)

SMP preparation now reserves a distinct usable page below 640 KiB for the
future S3 wake trampoline, separate from the AP startup page and excluded from
the general frame allocator. Suspend preparation refuses to proceed if that
page is unavailable and carries its physical address internally.

The page is reserved only; no code is written and no FACS waking vector is
modified. Evidence and limits are recorded in
`docs/audit-stages-consolidated.md`.

## Stage 25.5 - ACPI FACS wake-capability boundary (2026-10-21)

ACPI discovery now validates the FADT-referenced FACS signature, minimum
length, firmware memory range, version, and waking-vector fields. Syscall 238
and `libwoven::suspend_resume_capabilities` report bounded readiness flags to
`PowerControl` callers without exposing firmware addresses or changing power
state.

Default and `system-apps` production-kernel Clippy, host all-targets Clippy,
the full `cargo test --all` suite, and signed system-apps QEMU acceptance on
1/2/4 CPUs passed. Evidence is in
`docs/audit-stages-consolidated.md`.

This does not claim wake-trampoline allocation, FACS vector installation,
CPU-state save/restore, S3 entry, firmware wake, or physical qualification.

## Stage 25.4 - bounded suspend preparation (2026-10-21)

Accepted bounded preparation slice: the driver registry can quiesce all bound
drivers transactionally and roll back in reverse order when a hook refuses.
The power layer requires ACPI PM1a, `_S3`, and ACPI mode before returning a
private preparation token; abort restores drivers in reverse order.

Production-kernel and host warning-denying Clippy, the full `cargo test --all`
suite, and signed system-apps QEMU acceptance on 1/2/4 CPUs passed. Evidence
is in `docs/audit-stages-consolidated.md`.

This does not claim S3 entry, wake-vector/CPU-resume handling, wake-source
policy, a suspend syscall, device-specific save/restore, or physical
suspend/resume qualification.

## Stage 25.3 - CPU frequency inventory boundary (2026-10-21)

The CPU HAL now reads CPUID leaf `0x16` when available and exposes base,
maximum, and bus MHz inventory through syscall 237 and
`libwoven::cpu_frequency_info`. The query is `PowerControl`-gated, audited,
validates the 13-byte userspace result, and performs no P-state or MSR write.
It returns an explicit unavailable result when firmware omits the leaf.

Signed production-kernel Clippy, `libwoven` Clippy/tests, and signed
system-apps QEMU acceptance on 1/2/4 CPUs pass. Evidence is in
`docs/audit-stages-consolidated.md`.

This does not claim frequency scaling, governor policy, thermal throttling, or
physical frequency validation.

## Stage 30 boot-stage timing, and a real fix it found immediately (2026-10-21)

First slice of performance-engineering tooling (Stage 30): the existing
`BOOT LOADER [n/7]` progress markers are now timestamped (`kernel/src/main.rs`'s
new `boot_stage` helper and `BOOT_STAGE_CLOCK`), logging the tick delta since
the previous marker (or boot start, for the first) as `[BOOT PROFILE]` to
serial. Coarse (10 ms resolution, 100 Hz ticks) but immediately useful.

The first run found demo-account PBKDF2 hashing (2 accounts x 100,000
iterations, force-soft SHA-256 -- this kernel never uses a hardware SHA
extension) consuming 4241 of a ~4350-tick hardware-demo boot -- effectively
the entire boot, hidden inside what the unprofiled boot just called "system
files ready". Fixed in `install_hardware_demo_accounts`: `ada`/`1234` and
`lin`/`9876` are fixed, public demo credentials (not secrets), so their
salt and hash are now precomputed once rather than recomputed by calling
`pbkdf2_hmac` on every single boot of every test run. Re-measured at 2
ticks for the same stage. The precomputed hash was verified correct the
only way that matters: the hardware-demo QEMU gate's own sign-in step
(typing `1234` and reading `woven-shell: signed in: ada` back) still
passes.

This does not touch the general sign-in/change-PIN verify path, which
still computes PBKDF2 at the same iteration count on every real sign-in
attempt (that credential is live-typed, so there is nothing to
precompute). Whether that cost matters in practice is an open question,
not a conclusion this measurement supports: tick counts under a CPU-bound
software loop running inside QEMU's TCG emulation do not necessarily map
1:1 to real wall-clock time, and the interactive sign-in/Change-PIN QEMU
gates have been passing within their deadlines throughout this session.
Lowering `PBKDF2_ITERATIONS` would trade security margin for speed, so
that stays an explicit decision for the user rather than something to
change unilaterally from a single ambiguous measurement.

Validation: kernel clippy (default features) and host `cargo clippy
--all-targets -- -D warnings` passed; `cargo test -p libwoven` (95 tests)
passed; `scripts/test-shell-qemu.py --hardware-demo` and `--system-apps`
both passed on 1/2/4 CPUs.

## Stage 25.2 - CPU idle capability boundary (2026-10-21)

The CPU HAL now reports architectural idle capabilities through syscall 236
and `libwoven::cpu_idle_capabilities`: baseline `HLT`, optional
`MONITOR/MWAIT`, and invariant-TSC support. The query requires `PowerControl`,
validates the userspace output range, records an audit event, and changes no
CPU state. The scheduler continues to use only its existing interrupt-safe
`STI+HLT` idle path.

Production-kernel Clippy, `libwoven` Clippy/tests, and signed system-apps
QEMU acceptance on 1/2/4 CPUs pass. Evidence is in
`docs/audit-stages-consolidated.md`.

This does not claim MWAIT execution, frequency scaling, deep C-state policy,
per-CPU residency accounting, or suspend/resume.

## Stage 25.1 - ACPI power capability boundary (2026-10-21)

The kernel now discovers firmware `_S3` sleep types alongside the existing
`_S5` power-off types and exposes a read-only capability query through syscall
235 and the `libwoven::power_capabilities` wrapper. The query requires
`PowerControl`, validates the userspace output range, records an audit event,
and reports power-off, suspend, and restart capability without performing a
transition.

Signed production-kernel Clippy, `libwoven` warning-denying Clippy and tests
(95 tests), and signed system-apps QEMU boots on 1/2/4 CPUs pass. Evidence is
in `docs/audit-stages-consolidated.md`. The ACPI
structural self-test also directly covers a synthetic `_S3` AML package and
unsupported-state rejection.

This is a capability boundary only. It does not claim S3 suspend/resume,
driver quiescing, wake-source policy, battery/thermal/lid reporting, or
physical hardware qualification.

## Stage 24.60 — release-key provisioning boundary (2026-10-21)

Production WAPP packaging now requires explicit signing-seed and trusted
public-key inputs; only non-production/test packaging may use the deterministic
development key. The production catalog and package-service launchers propagate
the fail-closed mode. Negative and positive packager tests, packager Clippy,
and explicit production catalog/service packaging pass. Evidence is in
`docs/audit-stages-consolidated.md`.

This does not claim custody of a real release secret or physical release
qualification.

## Production account provisioning implementation (2026-10-21)

Commit `7bbcb0f8` extends bounded persistent account import from the
disposable `hardware-demo` profile to normal `system-apps` boots. The shared
cfg correction for the legacy fallback is now exercised by signed production
image acceptance, which passes on 1/2/4 CPUs. See
`docs/audit-production-account-provisioning-2026-10-21.md`.

This is not a release acceptance claim. Physical USB boot and TPM execution
remain open hardware qualifications.

The affected static gates now pass: signed production kernel Clippy,
all-targets warning-denying Clippy, and `cargo test --all`. Repository-wide
format checking remains open because it reports pre-existing diffs.

## Physical qualification readiness rerun (2026-10-21)

The read-only preflight again found one safe USB candidate but no accessible
TPM, no `swtpm` backend, and no signed physical provisioning record. No raw
disk or TPM state was accessed or changed. Evidence is recorded in
`docs/audit-physical-qualification-rerun-2026-10-21.md`.

## Stage 10.7 release-chain revalidation (2026-10-21)

The full Windows chain passed host regressions, warning-denying host Clippy,
host tests, Stage 7.5/7.6 preservation, networking, and all preserved Stage
8 through Stage 10.6 markers. Its first Stage 10.7 image build hit a volume
`no space on device` failure; after disposable build cleanup, the isolated
Stage 10.7 TCP gate passed on 1/2/4 CPUs with two TX exchanges, RX, and
FIN/EOF. This was a validation-only rerun with no source changes. Evidence is
under `docs/audit-stages-consolidated.md` and the preserved
serial logs in `target/stage10-7-regression-{1,2,4}-debug`.

## Stage 20.16 — standalone textured-3D-chrome proof of concept (2026-10-21)

First concrete slice of the user's "fully 3D, not 2D characteristics"
graphics directive (see `docs/agent-coordination.md`, 2026-10-20). Three
additive pieces, each landed and validated separately, none touching any
already-accepted compositor or desktop path:

1. `libwoven::graphics::Vertex3Tex`/`Renderer::textured_triangle_3d`: the
   existing software-3D rasterizer (`triangle_3d`) only interpolates a flat
   per-vertex colour; this sibling samples a source `Canvas` via
   affine-interpolated u/v instead, which is the primitive a 3D-rendered
   desktop needs to show a window's own rendered pixels on a tilted plane
   rather than a solid-coloured shape. 3 new tests (correct per-corner
   sampling, depth-test occlusion, empty-source safety).
2. `libwoven::graphics::Camera`: a fixed-point (16.16, matching `Transform`'s
   existing convention -- no floating-point state in freestanding
   userspace) perspective projector from a 3D world point to the screen
   pixel and depth value the triangle rasterizer wants. 3 new tests (origin
   centring, perspective shrink + depth ordering, behind-camera rejection).
3. `hello-gui --3d-chrome-demo` (`stage20-16-test`): a standalone
   proof of concept wiring the first two together end to end -- render a
   title-bar-like strip offscreen exactly as WovenShell's own chrome does
   today, project it through a tilted `Camera` as a texture-mapped quad
   instead of a flat blit, present it, and have the kernel self-test
   re-read the compositor to confirm what was presented is really there.
   Caught and fixed two real bugs on the way: the demo's canvas/depth
   buffers were first stack-allocated (~67 KiB against an 8 KiB user
   stack -- an immediate page fault) and, once moved to static `.bss`, the
   depth buffer's non-zero initial fill turned out to force it into real
   image data rather than `.bss`, bloating the installed binary past the
   VFS's 64 KiB node cap. Both are fixed; see the two commits for detail.

Validation: kernel clippy (default features and host `--all-targets`), the
95-test `libwoven` suite, `stage20-16-test` on 1/2/4 CPUs, and the
pre-existing Stage 19.2 hello-gui gate (confirming no regression to the
shared binary) all passed.

**Not yet done:** everything that makes this a *desktop* rather than a demo
-- replacing WovenShell's flat blits with this projection for real window
chrome (user explicitly chose this, "3D chrome first," over texturing real
window bodies, which would also need a new bounded pixel-readback syscall
window content doesn't have yet), an on/off toggle, and depth/perspective
values tuned for an actual desktop layout rather than one fixed demo quad.

## Stage 24.59 — deterministic Change-PIN acceptance (2026-10-21)

Accepted. The kernel and WovenShell flow now has a dedicated deterministic
QEMU gate covering sign-in, Quick Settings, current/new/confirmation PIN
entry, durable account-record rotation, rejection of the old PIN, and unlock
with the new PIN. The kernel generates one bounded salt and commits it to both
the identity authority and account record, with rollback if the durable record
update fails.

Required checks passed: production-feature kernel Clippy with packaged signed
system apps, host all-target Clippy, 95 `libwoven` tests, and the Stage 24.59
QEMU gate on 1/2/4 CPUs. Evidence is under
`audit-artifacts/stage24.59-change-pin-20261021`.

This closes the password-change gap. Account creation was already accepted in
Stage 24.15; physical TPM/USB qualification remains a separate release
prerequisite. No USB media was modified.

## Change-PIN kernel primitive and WovenShell UI (2026-10-20/21)

Closes "password change"/"credential rotation" from the four persistent-
accounts gaps flagged 2026-10-20 (account creation is still open).

Kernel: a new `Capability::AccountManage` (desktop-session-only, same
pattern as `PowerControl`; added to `SecurityDomain::SystemService`'s
WovenGuard ceiling and `session::CAPABILITIES`), a new syscall
(`AccountsChangePin`) whose only effect is `kernel::accounts::change_pin`:
read `/etc/wovenhat/accounts`, verify the caller-supplied current PIN by
PBKDF2 against the named account's stored hash, and if it matches, rewrite
just that one line (fresh salt, new hash) through a new
`vfs::overwrite_trusted` that bypasses the file's own `writable = false` --
every other account's line is left byte for byte unchanged. No syscall
exposes a generic write to this path; the PIN check is the only
authorization. `libwoven::accounts::change_pin` wraps the syscall for apps.

UI: WovenShell's Quick Settings panel gained a seventh row, "Change PIN",
reusing the lock screen's existing masked-digit-entry widget under a new
`LockMode::ChangePin` (current PIN -> new PIN -> confirm; Backspace on an
empty entry cancels without changing anything) rather than building a
second input widget. Both stashed PINs are wiped (volatile writes) the
moment they are no longer needed, success or failure.

Validation: kernel clippy (default features, `--features hardware-demo`
with the packaged apps wired up, and the `apps/woven-shell` crate on its
own) all clean; `cargo test -p libwoven` (57 tests, including the PBKDF2
and `Account::parse`/`verify` tests) and host `cargo clippy --all-targets`
clean. `scripts/test-shell-qemu.py --hardware-demo` (the real sign-in path,
unrelated to the new syscall but the thing most likely to regress from a
`Desktop` struct size/field change) passed on 1/2/4 CPUs, after one flaky
failure on a loaded machine (concurrent Codex builds) that reproduced as a
pass on immediate retry -- a timing margin in the *test*, not the feature.

The dedicated deterministic Change-PIN self-test is accepted as Stage 24.59
above. It opens Quick Settings, enters old/new/confirmation PINs, verifies the
durable record, rejects the old PIN after relock, and unlocks with the new PIN.
The existing Stage 20.15-style kernel self-tests drive WovenShell through
kernel-internal input injection rather than QMP key events specifically
because QMP timing is flaky (as just observed); a correct test here should
follow that pattern, not add another QMP-driven gate. This is the next
concrete follow-up if work returns to this feature.

## Stage 24.58 physical TPM broker boot activation (2026-10-21)

Integrated the signed broker records and Stage 24.57 TPM root-read boundary
into the system-app boot path. When a live TPM is present, boot now
revalidates the signed policy records, reads the attestation public key,
obtains a fresh TPM nonce, promotes the policy/PCR-bound NV credential root,
and installs the kernel-owned broker. Every failure leaves hardware
credentials disabled; no TPM or storage error blocks the desktop. Host tests,
warning-denying all-target and system-app Clippy, and exact-tree memory plus
signed system-app acceptance on 1/2/4 CPUs pass. Evidence is under
`audit-artifacts/stage24.58-tpm-broker-boot-activation-20261021`.

QEMU has no live TPM, and all three boots therefore exercised the required
fail-closed path. Physical TPM activation, live credential login, reboot
recovery, and hardware qualification remain open release gates. The attached
USB was not opened or modified.

## Stage 24.57 physical TPM credential-root read boundary (2026-10-21)

Added the kernel-side read-only handoff for the signed credential-root record:
bounded `NV_ReadPublic`, PCR-policy session application, authenticated
policy-session `NV_Read`, refreshed TPM nonce handling, and mandatory session
flush/secret wiping before promoting the root into a `CredentialRootLease`.
The path is fail-closed on malformed metadata, invalid bounds, stale session
state, or TPM errors and does not expose TPM authorization material to
userspace. Host tests (89), warning-denying all-target and affected kernel
feature Clippy, and exact-tree memory plus signed system-app acceptance on
1/2/4 CPUs pass. Evidence is under
`audit-artifacts/stage24.57-tpm-credential-root-read-20261021`.

No live TPM is available in this environment, so physical NV provisioning,
PCR-policy execution against a real TPM, reboot persistence, and hardware
qualification remain open release gates. The attached USB was not opened or
modified.

## Stage 24.56 signed TPM broker record generator (2026-10-21)

Added `tools/tpm-broker-config`, a warning-clean host tool that emits the
fixed 182-byte signed `TCB1` broker policy record from explicit NV index,
attestation handle, quote scheme, PCR, digest, Name, and NV-policy inputs.
It requires a dedicated TPM policy signing seed and matching trusted public
key; it never falls back to a development key. Synthetic generation passed,
while missing-key and invalid-index negative controls were rejected. The
exact-tree memory and signed system-app gates pass on 1/2/4 CPUs. Evidence is
under `audit-artifacts/stage24.56-tpm-broker-record-generator-20261021`.

This tool generates policy only; it does not provision a TPM, read the USB,
or write an OS image. Physical TPM provisioning, live quote qualification,
reboot recovery, and hardware qualification remain required.

## Stage 24.55 physical TPM readiness preflight (2026-10-21)

Added `scripts/test-physical-tpm-readiness.ps1`, a read-only host preflight
that records TPM visibility/readiness, QEMU/swtpm availability, signed broker
record presence, and removable-disk identity/safety. The run found one safe
USB candidate (Disk 1) but no accessible TPM and no software TPM backend.
Evidence is under
`audit-artifacts/physical-tpm-readiness-20261021-063402`.

This is a readiness report, not physical qualification. No raw disk was
opened, no disk state changed, and no media or TPM state was written. An
administrator TPM query, a provisioned physical TPM, and live hardware gates
remain required.

## Stage 24.54 kernel-derived TPM authorization secret (2026-10-21)

Credential-root leases now retain the validated NV index and policy binding.
Root rotation rejects metadata changes, and the physical broker can derive the
attestation object's authorization value inside kernel memory from the active
root. Provisioned broker installation additionally requires the signed `TCB1`
configuration and exact attestation Name; the derived secret is wiped before
return. Host tests (89), all-targets and affected feature Clippy, executable
QEMU, and signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.54-kernel-derived-tpm-secret-20261021`.

No authorization secret is persisted or accepted from userspace. Physical TPM
provisioning, live login, reboot recovery, and hardware qualification remain
required.

## Stage 24.53 signed TPM broker provisioning record (2026-10-21)

Added a bounded signed `TCB1` release record for the physical TPM broker.
The record authenticates the reserved NV index, attestation handle, quote
scheme, PCR, expected PCR digest, attestation Name, and NV policy before any
field is returned. WovenFS persistence and boot-time revalidation are
available, while authorization secrets remain excluded from durable storage.
Host tests (89), all-targets and affected feature Clippy, executable QEMU, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.53-signed-tpm-provisioning-20261021`.

The QEMU image has no TPM2 device and no provisioning record, so the broker
remains uninstalled. Physical TPM provisioning, secret injection, live login,
reboot recovery, and hardware qualification remain required.

## Stage 24.52 physical TPM credential broker boundary (2026-10-21)

The kernel now owns a bounded physical TPM credential broker. It validates the
TPM descriptor, signed attestation Name policy, attestation public key, active
credential root, PCR selection, and signing authorization before accepting a
broker configuration. Each request derives the account-bound challenge,
starts and applies a fresh policy session, executes one authenticated quote,
verifies the quote, flushes the session on every path, and publishes the
move-only decision for syscall 233 consumption. Syscalls 234/233 and matching
libwoven wrappers provide request/consume sequencing without exposing TPM
material. Host tests (89), all-targets and affected feature Clippy, executable
QEMU, and signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.52-physical-tpm-broker-20261021`.

The QEMU image has no TPM2 device and therefore installs no broker; physical
credential login, release provisioning of the broker inputs, reboot recovery,
and hardware qualification remain required.

## Stage 24.51 identity hardware-decision consumption (2026-10-21)

Identity runtime now has a bounded kernel-only one-entry pending decision
handoff. `IdentityConsumeHardware` (syscall 233) consumes a move-only,
generation-tagged decision only for the owner/session and account name bound by
the TPM challenge; locked sessions, wrong owners/contexts, duplicate
consumption, and absent decisions fail closed. Host tests (89), all-targets and
affected feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU
acceptance pass. Evidence is under
`audit-artifacts/stage24.51-identity-hardware-consumption-20261021`.

A live physical TPM broker has not yet published a decision, and no physical
credential login is claimed; reboot recovery and physical qualification remain
required.

## Stage 24.50 hardware credential decision boundary (2026-10-21)

The TPM layer now binds a hardware credential decision to the active
credential-root lease. It derives the exact 32-byte quote challenge from the
root generation and bounded identity context, then requires the existing
signed TAN1 Name, ReadPublic binding, PCR evidence, and cryptographic quote
verification before returning a generation-tagged decision. Inactive or
revoked roots fail before quote authorization. Host tests (89), all-targets
and affected feature Clippy, executable QEMU, and signed system-apps 1/2/4-
CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.50-hardware-credential-decision-20261021`.

Live physical TPM quote success, identity-session consumption of the decision,
reboot recovery, and physical qualification remain required.

## Stage 24.49 credential-root lifecycle (2026-10-21)

The TPM2 layer now owns a generation-tagged kernel credential-root lease.
Installation requires validated NV metadata and exact policy-bound readback;
zero roots are rejected. Rotation validates the replacement before wiping the
old root and advancing the generation, derivation exposes only bounded
context-derived material, and explicit revocation wipes the root and makes the
lease unusable. Host tests (89), all-targets and affected feature Clippy,
executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass. Evidence is
under `audit-artifacts/stage24.49-credential-root-lifecycle-20261021`.

Live TPM NV define/write/read execution, provisioned-key persistence policy,
credential decisions, recovery, and physical qualification remain required.

## Stage 24.48 TPM custody authorization boundary (2026-10-21)

TPM policy-session custody now requires a kernel-owned authority token in
addition to the ordinary WovenFS writer. The token is issued only after a
validated live CRB descriptor and an accepted boot-time signed TAN1 policy;
the durable context loader requires it as well. Invalid descriptors and
storage-only callers fail before custody I/O. Host tests (89), all-targets and
affected feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU
acceptance pass. Evidence is under
`audit-artifacts/stage24.48-tpm-custody-authorization-20261021`.

TPM physical execution, provisioned-key lifecycle, credential decisions,
recovery, and physical qualification remain required.

## Stage 24.47 TPM policy-session nonce refresh (2026-10-21)

Policy-session quote responses now parse the TPM authorization area and retain
the refreshed TPM nonce. The password-session path still accepts an empty
nonce, while the non-password policy path rejects one. Managed quote leases
copy the refreshed nonce before becoming one-shot stale, so a later lifecycle
operation cannot accidentally derive authorization from an old nonce. Host
tests (89), all-targets and affected feature Clippy, executable QEMU, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.47-tpm-session-nonce-refresh-20261021`.

Live physical TPM session execution, durable custody authorization,
provisioned-key lifecycle, credential decisions, recovery, and physical
qualification remain required.

## Stage 24.46 boot-time TPM context recovery (2026-10-21)

After WovenFS mount, system-apps boot now treats the newest validated `TPS1`
record as a recovery candidate, requires a live TPM `ContextLoad`, and flushes
the restored lease immediately because no credential-authority consumer is
enabled yet. Missing storage, absent TPM hardware, invalid records, failed
loads, and cleanup failures all leave hardware credentials disabled. Host
tests, all-targets and affected feature Clippy, executable QEMU, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.46-boot-tpm-context-recovery-20261021`.

The QEMU environment has no durable TPM context, so physical cross-reboot
restore remains unproven. Secure storage authorization, nonce refresh,
provisioned-key lifecycle, credential decisions, recovery policy, and physical
qualification remain required.

## Stage 24.45 durable TPM context custody boundary (2026-10-21)

Opaque TPM policy-session contexts now have a versioned dual-copy WovenFS
custody record with fixed bounds, SHA-256 integrity, sequence arbitration,
and strict session/context metadata validation. A filesystem record is only a
retryable custody candidate; TPM `ContextLoad` remains the final authority.
Host tests (89), all-targets and affected feature Clippy, executable QEMU,
and signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.45-tpm-context-custody-20261021`.

Boot-time custody integration, secure storage authorization, session nonce
refresh, provisioned-key lifecycle, credential decisions, recovery, and
physical qualification remain required.

## Stage 24.44 TPM policy-session context save/load (2026-10-21)

The TPM2 layer now frames, executes, and strictly decodes `ContextSave` and
`ContextLoad` for policy sessions. A successful save invalidates the live
lease and returns a bounded context snapshot; a load consumes that snapshot
exactly once and restores the session under a fresh generation. Failed loads
leave the snapshot retryable. Host tests (89), all-targets and affected
feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance
pass. Evidence is under
`audit-artifacts/stage24.44-policy-session-context-20261021`.

Cross-reboot persistence custody, TPM session nonce refresh semantics,
provisioned-key lifecycle, credential decisions, recovery, and physical
qualification remain required.

## Stage 24.43 managed TPM policy-session lifecycle (2026-10-21)

Policy sessions now have a generation-tagged kernel-owned lease. Managed
transcript and quote helpers reject inactive or nonce-stale leases; a
successful quote consumes the lease for authority reuse, and a successful
`FlushContext` irreversibly invalidates it. Flush failures leave the lease
active for bounded retry. Host tests (89), all-targets and affected feature
Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under
`audit-artifacts/stage24.43-managed-policy-session-20261021`.

TPM context save/load, session nonce refresh after every authenticated TPM
command, provisioned-key lifecycle, credential decisions, recovery, and
physical qualification remain required.

## Stage 24.42 policy-session quote HMAC derivation (2026-10-21)

The TPM2 layer now derives a SHA-256 quote authorization HMAC from the
TPM2_Quote command cpHash, returned session nonce, caller nonce, session
attributes, and caller-supplied attestation-key auth value. The authenticated
quote wrapper uses that derived HMAC and rejects non-32-byte policy-session
HMACs or password-session downgrade. Host tests (89), all-targets and
affected feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU
acceptance pass. Evidence is under
`audit-artifacts/stage24.42-policy-session-hmac-20261021`.

The implementation is limited to the null-salt/null-bind session shape
created by the current StartAuthSession path. Session-state refresh,
provisioned-key lifecycle, credential decisions, recovery, and physical
qualification remain required.

## Stage 24.41 live policy-session transcript execution (2026-10-21)

The TPM2 layer now executes bounded `StartAuthSession` policy sessions and
`PolicyPCR` assertions through the serialized CRB transport. Session startup
validates the returned handle and bounded TPM nonce; PolicyPCR validates the
PCR index and expected SHA-256 digest before issuing the command and requires
an exact successful response. Host tests (89), all-targets and affected
feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance
pass. Evidence is under
`audit-artifacts/stage24.41-live-policy-session-20261021`.

The returned TPM session nonce is not yet refreshed to the caller after each
PolicyPCR state transition, and session-HMAC derivation, quote authorization,
credential decisions, recovery, and physical qualification remain required.

## Stage 24.40 policy-session TPM2 Quote framing (2026-10-21)

The quote transport now accepts a caller-established non-password policy
session, including its TPM session handle, caller nonce, attributes, and
bounded authorization HMAC. The encoder places that authorization area in
the TPM command before parameters and rejects a `TPM_RS_PW` downgrade. The
existing empty-password wrapper remains available only for objects configured
for that mode. Host tests (89), all-targets and affected feature Clippy,
executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass. Evidence
is under `audit-artifacts/stage24.40-policy-session-quote-20261021`.

Policy-session creation, PolicyPCR execution, HMAC derivation from provisioned
authorization material, credential decision integration, recovery, and
physical qualification remain required.

## Stage 24.39 bounded TPM2 Quote execution (2026-10-21)

The TPM2 CRB layer now frames and executes a bounded `TPM2_Quote` request
with an empty password session, SHA-256 PCR selection, and RSA-SSA, RSA-PSS,
or ECDSA scheme selection. The response parser requires a sessions-tagged
response, exact parameter/auth-area framing, bounded quote/signature sizes,
and a supported TPM signature type. The result remains evidence only and
must pass `authorize_quote` before any credential decision. Host tests (89),
all-targets and affected feature Clippy, executable QEMU, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.39-tpm-quote-execution-20261021`.

Physical TPM execution, non-empty object authorization sessions, credential
decision integration, recovery, and physical qualification remain required.

## Stage 24.38 boot-time TPM Name policy loading (2026-10-21)

Release system-apps boots now attempt to load the signed TAN1 attestation
Name policy immediately after the durable WovenFS mount attempt and before
WovenShell starts. The loader uses the compile-time application release trust
root and revalidates the complete bounded record. Missing storage, malformed
records, and signature failures all leave hardware credentials disabled;
none is converted into authority. Host tests (89), all-targets and affected
feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance
pass. Evidence is under
`audit-artifacts/stage24.38-boot-tpm-policy-loader-20261021`.

Live TPM quote execution, credential decision integration, recovery,
physical TPM qualification, and release certification remain required.

## Stage 24.37 fail-closed TPM quote authorization integration (2026-10-21)

The TPM2 layer now combines signed expected-Name policy verification,
`ReadPublic` Name binding, nonce/PCR attestation validation, signature-shape
validation, and RSA/ECC cryptographic verification in one structured
authorization request. A caller cannot substitute a different key or skip an
earlier binding step. Host tests (89), all-targets and affected feature
Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under
`audit-artifacts/stage24.37-quote-authorization-integration-20261021`.

Boot-time policy loading, live TPM execution, credential decision integration,
recovery, and physical TPM qualification remain required.

## Stage 24.36 durable signed TPM Name policy boundary (2026-10-21)

The signed TAN1 attestation-Name policy now has a journal-backed WovenFS
storage boundary. Writes verify the Ed25519 policy before mutation; reads
require the exact 106-byte record size and reverify the signature before
returning a Name. The API is feature-gated to accepted durable-storage and
production system-app configurations. Host tests (89), all-targets and
affected feature Clippy, executable QEMU, and signed system-apps 1/2/4-CPU
acceptance pass. Evidence is under
`audit-artifacts/stage24.36-durable-attestation-name-policy-20261021`.

Physical provisioning, live TPM retrieval, boot-time policy loading, quote
authorization, recovery, and physical TPM qualification remain required.

## Stage 24.35 signed TPM attestation-Name policy (2026-10-21)

The TPM2 layer now verifies a bounded, versioned Ed25519-signed policy
envelope containing the expected 34-byte SHA-256 TPM object Name. Invalid
headers, lengths, trust signatures, or algorithm identifiers fail closed;
the verified Name can then feed the Stage 24.34 binding boundary. Host tests
(89), all-targets and affected feature Clippy, executable QEMU, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.35-signed-attestation-name-policy-20261021`.

Durable record storage, live TPM retrieval, key identity provisioning,
credential authorization, recovery, and physical TPM qualification remain
required.

## Stage 24.34 TPM attestation-key Name binding (2026-10-21)

The `TPM2_ReadPublic` boundary now recomputes the SHA-256 TPM Name from the
exact public area, verifies the returned Name, retains the bounded qualified
Name, and exposes a fail-closed comparison with the provisioned expected
34-byte object Name. Host tests (89), all-targets and affected feature
Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under `audit-artifacts/stage24.34-tpm-name-binding-20261021`.

Live TPM key retrieval, persistent object identity provisioning, quote
authorization integration, recovery, and physical TPM qualification remain
required.

## Stage 24.33 bounded TPM ReadPublic attestation-key boundary (2026-10-21)

The TPM2 layer now frames `TPM2_ReadPublic` and validates the returned public
area for a fixed-TPM, fixed-parent, sensitive-origin, restricted signing
object named with SHA-256. RSA keys are bounded to 2048-4096 bits and passed
through RSA key validation; ECC keys are restricted to valid P-256 points.
Malformed sizes, unsupported curves/types, decrypt-only objects, and trailing
response bytes fail closed. Host tests (89), all-targets and affected feature
Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under `audit-artifacts/stage24.33-tpm-read-public-boundary-20261021`.

Live TPM execution, object-name binding, attestation-key identity/persistence,
credential authorization, recovery, and physical TPM qualification remain
required.

## Stage 24.32 bounded ECC/P-256 quote verification (2026-10-21)

The TPM2 boundary now verifies SHA-256 ECDSA/P-256 quote signatures against
a bounded uncompressed affine public key and exact 32-byte TPM signature
components. Invalid points, wrong scheme/hash, non-canonical component sizes,
and tampering fail closed. Host tests (89), all-targets and affected feature
Clippy, executable QEMU, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under `audit-artifacts/stage24.32-ecc-p256-quote-verification-20261021`.

Live TPM public-key retrieval/validation, quote-key binding, recovery, and
physical TPM qualification remain required.

## Stage 24.31 bounded TPM RSA quote verification (2026-10-21)

The TPM2 boundary now verifies SHA-256 RSA quote signatures (RSASSA and
RSASSA-PSS) against a caller-supplied, bounded 2048-4096-bit public key,
including TPM's exponent-zero default of 65537. Verification is performed
against the exact `TPMS_ATTEST` bytes and fails closed on malformed keys,
wrong scheme/hash, or signature mismatch. The self-test was moved after heap
initialization because the audited verifier allocates. Host tests (89),
all-targets and affected feature Clippy, executable QEMU, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.31-rsa-quote-verification-20261021`.

ECC quote verification, live TPM public-key retrieval/validation, recovery,
and physical TPM qualification remain required.

## Stage 24.30 bounded TPM quote signature framing (2026-10-21)

The TPM2 layer now parses bounded `TPMT_SIGNATURE` shapes for SHA-256
RSASSA, RSAPSS, and ECDSA quote schemes, including exact RSA or ECDSA
component bounds and consumed-length reporting. The executable QEMU
self-test, host tests (89), all-targets and affected feature Clippy, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.30-quote-signature-shape-20261021`.

This is framing only: it does not verify a TPM public key, RSA/ECC signature,
or quote-to-key binding. Physical TPM attestation, recovery, and release
qualification remain required.

## Stage 24.29 bounded TPM quote attestation parsing (2026-10-21)

The TPM2 layer now parses a bounded `TPMS_ATTEST` quote structure and binds
its generated-value magic, quote type, qualifying nonce, single SHA-256 PCR
selection, and 32-byte PCR digest to caller expectations. Clock and firmware
metadata are retained only in the returned evidence structure. The executable
QEMU self-test, host tests (89), all-targets and affected feature Clippy, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.29-quote-attestation-20261021`.

This parses evidence only; no TPM signing key or quote signature is verified,
and no quote can authorize credentials by itself. Physical TPM attestation,
signature verification, recovery, and release qualification remain required.

## Stage 24.28 credential-root read-back promotion boundary (2026-10-21)

The TPM2 layer now validates a credential-root promotion decision as one
constant-time exact-match operation: the decoded NV public metadata must match
the expected index and PCR policy, and the returned 32-byte value must match
the caller-provisioned root. Metadata or value mismatch fails closed with no
promotion. The executable QEMU self-test, host tests (89), all-targets and
affected feature Clippy, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under `audit-artifacts/stage24.28-credential-root-readback-20261021`.

This does not persist or expose the root, execute a live TPM read, validate a
TPM quote, provide crash recovery, or establish physical hardware trust.
Those remain release prerequisites.

## Stage 24.27 policy-session NV read-back (2026-10-21)

The TPM2 layer now frames a policy-session `NV_Read` for exactly the fixed
32-byte credential-root value and parses the returned parameter and session
authorization areas with exact bounds. The executable QEMU self-test, host
tests (89), all-targets and affected feature Clippy, and signed system-apps
1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.27-nv-read-policy-20261021`.

This does not issue a live read, create or write an NV index, persist a
credential, validate a TPM quote, or provide recovery. Physical TPM
provisioning and release qualification remain required.

## Stage 24.26 bounded TPM NV definition framing (2026-10-21)

The TPM2 layer now frames an owner-authorized `NV_DefineSpace` request for
the fixed credential-root index. It bounds the owner password-session
authorization, encodes the SHA-256 name algorithm, 32-byte policy, exact
attributes, and 32-byte data size, and keeps the caller authorization in the
transient command buffer only. The executable QEMU self-test, host tests
(89), all-targets and affected feature Clippy, and signed system-apps
1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.26-nv-define-space-20261021`.

This does not send the command to a TPM, create an index, persist owner
authorization, write a root, validate a quote, or provide recovery. Physical
TPM provisioning and release qualification remain required.

## Stage 24.25 policy-session NV write framing (2026-10-21)

The TPM2 layer now frames a session-authorized `NV_Write` for the fixed
32-byte credential-root payload and parses its bounded response authorization
area. The command requires a nonzero policy session, exact NV index binding,
bounded caller nonce, continue-session attribute, zero offset, and an empty
HMAC for the policy-session shape. The executable QEMU self-test, host tests
(89), all-targets and affected feature Clippy, and signed system-apps
1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.25-nv-write-policy-20261021`.

This does not execute a live NV write, create an index, establish owner
authorization, persist a secret, validate a quote, or provide recovery.
Physical TPM provisioning and release qualification remain required.

## Stage 24.24 bounded TPM PolicyPCR transcript (2026-10-21)

The TPM2 layer now frames a bounded `PolicyPCR` command for the active policy
session, binding one PCR index to the expected 32-byte SHA-256 digest and
validating the exact success response. The executable QEMU self-test, host
tests (89), all-targets and affected feature Clippy, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.24-policy-pcr-20261021`.

This still does not send a live TPM command on the current QEMU platform,
authorize NV writes, create a credential object, validate a quote, or provide
recovery. Complete policy-session lifecycle, NV provisioning, quote
verification, physical TPM qualification, and release custody remain open.

## Stage 24.23 bounded TPM policy-session framing (2026-10-21)

The TPM2 layer now frames a policy `StartAuthSession` request with bounded
caller nonce, null salt/bind handles, null symmetric protection, and SHA-256
policy hashing. It parses the returned session handle and TPM nonce with exact
response bounds. The executable QEMU self-test, host tests (89), all-targets
and affected feature Clippy, and signed system-apps 1/2/4-CPU acceptance
pass. Evidence is under
`audit-artifacts/stage24.23-policy-session-20261021`.

This does not authorize an NV command, create an object, transmit credential
bytes, or validate a TPM quote. The session lifecycle, policy command
transcript, sealed-root provisioning, recovery, and physical TPM
qualification remain required.

## Stage 24.22 credential-root NV policy validation (2026-10-21)

The TPM2 layer now validates the public metadata of the reserved
credential-root NV object against a caller-provisioned index and signed PCR
policy digest. It requires SHA-256 naming, a 32-byte value, a 32-byte policy,
policy-authorized writes, and rejects ordinary owner/object writes and policy
deletion. Host tests (89), all-targets and affected feature Clippy, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.22-credential-root-policy-20261021`.

This is still a validation boundary: it does not create an NV index, open an
authorization session, write or read credential bytes, or prove a TPM quote.
Sealed-root provisioning, recovery, physical TPM qualification, and release
key custody remain required.

## Stage 24.21 bounded TPM NV public metadata (2026-10-21)

The TPM2 layer now frames `NV_ReadPublic` for a caller-selected index and
parses only a bounded NV public area: index, name algorithm, attributes,
bounded authorization-policy bytes, data size, and exact response framing.
The codec is read-only and carries no secret material; malformed sizes,
status codes, policy lengths, and trailing-name framing fail closed. Host
tests (89), all-targets and affected feature Clippy, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.21-nv-public-20261021`.

This does not issue a live NV transaction, create or authorize an NV index,
write a credential, seal a root, validate a TPM quote, or provide recovery.
Those remain required before hardware-backed identity can be enabled. The
attached USB was inventory-only and was not written, formatted, or mounted.

## Stage 24.20 signed PCR policy boundary (2026-10-21)

The TPM layer now validates a fixed-size signed PCR policy envelope. The
provisioned signing key is supplied by the trusted caller, the envelope binds
one PCR index and one SHA-256 digest, and Ed25519 signature/tamper/mismatch
checks fail closed. The QEMU codec self-test, host tests (89), all-targets and
affected feature Clippy, and signed system-apps 1/2/4-CPU acceptance pass.
Evidence is under `audit-artifacts/stage24.20-signed-pcr-policy-20261021`.

This authenticates policy provenance only; no release policy is provisioned
yet, no TPM quote/attestation key is verified, and no identity credential is
sealed to the policy. Those remain the next hardware-credential prerequisites.

## Stage 24.19 bounded PCR evidence read (2026-10-21)

The TPM2 layer now constructs and parses a bounded SHA-256 PCR_Read exchange,
including the selected PCR index, hash algorithm, digest count/length, TPM
response code, and response-size checks. Boot probes PCR 7 only when a valid
TPM2 descriptor exists and wipes the returned digest after inventory logging.
Host tests (89), all-targets and affected feature Clippy, and signed
system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.19-pcr-evidence-20261021`.

PCR reads are evidence, not authentication: no provisioned expected policy,
signed TPM quote, attestation-key validation, or sealed credential root exists
yet. Those remain required before identity credentials can depend on PCR state.

## Stage 24.18 bounded TPM2 CRB probe (2026-10-21)

The validated TPM2 descriptor now feeds a serialized CRB `GetRandom` probe.
The transport checks the CRB interface, command/response capacities and
addresses, fatal status, bounded pre-interrupt polling, response code/length,
and wipes probe output. Random bytes are never persisted or promoted to a
credential root. Host tests (89), all-targets and affected feature Clippy, and
signed system-apps 1/2/4-CPU acceptance pass. Evidence is under
`audit-artifacts/stage24.18-tpm2-crb-probe-20261021`.

Authenticated PCR/attestation evidence, sealed credential-root provisioning,
recovery, and physical TPM qualification remain required.

## Stage 24.17 TPM2 platform discovery boundary (2026-10-21)

ACPI discovery now validates the TPM2 table length, control-area address, and
advertised start method, retains the bounded platform record, and emits an
explicit boot inventory line. A missing or malformed table is never treated
as a credential root; the
kernel reports hardware credentials as unavailable and continues only where
the existing non-hardware path permits. The ACPI self-test, 89-test host
suite, all-targets Clippy, production system-apps Clippy, and signed
system-apps 1/2/4-CPU gates pass. Evidence is under
`audit-artifacts/stage24.17-tpm2-platform-inventory-20261021`.

This is an integration boundary, not completion of hardware-backed identity.
TPM command transport, authenticated PCR/attestation evidence, sealed
credential-root provisioning, recovery, and physical TPM qualification remain
required before production credentials can depend on hardware.

## Stage 24.16 durable/configurable lockout policy (2026-10-21)

The bounded identity authority now persists an administrator-selected failed-
authentication threshold from 1 through 32. WID1 snapshots remain readable;
new snapshots use the existing reserved header byte for version 2 policy data.
The owner-bound elevated administrator session exposes this operation through
WovenSecurity and audit action 37/syscall 232. Runtime authentication applies
the persisted threshold while retaining the fixed five-second cooldown and
fail-closed behavior. Host tests (89), all affected warning-denying Clippy
gates, focused WovenSecurity QEMU, and signed system-apps 1/2/4-CPU gates
pass. Evidence is under
`audit-artifacts/stage24.16-lockout-policy-20261021`.

Hardware credentials, malformed-request telemetry, release-key provisioning,
and physical qualification remain open. The no-disk hardware-demo fallback is
test-only; production durable publication remains fail-closed.

## Stage 24.15 administration mutation UI (2026-10-21)

WovenSecurity now exposes bounded add-user, add-group, add/remove-membership,
and inactive-user deletion flows after administrator elevation. Directory
records display stable IDs for subsequent operations. The identity runtime
remains the authorization boundary; every mutation requires an owner-bound
elevated administrator session. The no-disk hardware-demo fixture has an
explicit volatile fallback, while production builds require durable WID2 and
trusted account-file publication. The 88-test host suite, all relevant
warning-denying Clippy gates, focused mutation UI QEMU acceptance, and signed
system-apps 1/2/4-CPU gates pass. Evidence is under
`audit-artifacts/stage24.15-admin-mutation-ui-20261021`.

Durable/configurable lockout, hardware credentials, malformed-request
telemetry, release-key provisioning, and physical qualification remain open.

## Stage 24.14 bounded administration query UI (2026-10-21)

WovenSecurity now authenticates an elevated owner-bound administrator session
and displays bounded non-secret user/group directory records through query
syscall 231 and audit action 36. The query ABI has fixed framing and capacity;
salts, hashes, PINs, and session credentials are excluded. The hardware-demo
fixture promotes only its first volatile demo account for this test path; the
production account loader does not promote accounts. The 88-test host suite,
all warning-denying Clippy gates, diff check, focused WovenSecurity QEMU UI
acceptance, and signed system-apps 1/2/4-CPU gates pass. Evidence is under
`audit-artifacts/stage24.14-admin-query-ui-20261021`.

Mutation-management UI, configurable durable lockout, hardware credentials,
malformed-request telemetry, release-key provisioning, and physical
qualification remain open.

## Stage 24.13 group membership administration (2026-10-21)

Elevated administrators can add and remove bounded supplementary group membership through syscalls 229-230. Duplicate membership, unknown identities, full groups, and primary-group removal are rejected; changes publish through WID2 with live rollback on persistence failure. The 87-test host suite, warning-denying host and production Clippy gates, diff check, and signed system-apps 1/2/4-CPU gates pass. Evidence is under `audit-artifacts/stage24.13-group-membership-{1,2,4}cpu-1792534800000000000`.

Administration UI/query APIs, durable/configurable lockout, hardware credentials, malformed-request telemetry, release-key provisioning, and physical qualification remain open.

## Stage 24.12 durable identity administration (2026-10-20)

Administrative mutations now publish through WID2 and synchronize the trusted
account file, with rollback on publication failure. The scheduler migration
self-test waits boundedly for coverage-probe slot reclamation, removing the
prior 4-CPU slot race. The 87-test suite, all warning-denying Clippy gates,
diff check, and signed system-apps 1/2/4-CPU gates pass. Evidence is under:
`audit-artifacts/stage24.12-admin-persistence-1cpu-1792534200000000000`,
`audit-artifacts/stage24.12-admin-persistence-2cpu-1792534200000000000`, and
`audit-artifacts/stage24.12-admin-persistence-4cpu-1792534200000000000`.

Group membership administration, UI integration, durable/configurable
lockout, hardware credentials, malformed telemetry, release-key provisioning,
and physical qualification remain open.

## Stage 24.11 bounded identity administration ABI (2026-10-20)

Elevated administrator sessions can now add users, delete inactive users, and
add groups through bounded syscalls 226–228. Self-deletion and deletion of
accounts with active sessions are refused. The 87-test host suite, host and
kernel Clippy gates, system-apps Clippy, diff check, and signed system-apps
1/2/4-CPU shell gates pass. Evidence is under:
`audit-artifacts/stage24.11-admin-abi-1cpu-1792533600000000000`,
`audit-artifacts/stage24.11-admin-abi-2cpu-1792533600000000000`, and
`audit-artifacts/stage24.11-admin-abi-4cpu-1792533600000000000`.

Persistence to account/WID2 storage, group membership mutation/deletion, and
an administration UI remain open; this is not yet a complete administration
release gate.

## Stage 24.10 kernel identity lockout/rate limiting (2026-10-20)

The owner-bound identity runtime now rate-limits both plaintext and verified
digest authentication. Five failures impose a bounded five-second cooldown;
success resets the account state. WovenShell sends candidate digests to the
kernel for every entered PIN, so the policy covers the trusted login-manager
path. The 86-test suite, host/feature-gate Clippy, system-apps Clippy, diff
check, and signed system-apps 1/2/4-CPU gates pass. Evidence is under:
`audit-artifacts/stage24.10-lockout-1cpu-1792533000000000000`,
`audit-artifacts/stage24.10-lockout-2cpu-1792533000000000000`, and
`audit-artifacts/stage24.10-lockout-4cpu-1792533000000000000`.

Runtime lockout state is not yet durable or administrator-configurable. Account
administration, malformed-request telemetry, hardware credentials, release-key
provisioning, and physical qualification remain open.

## Stage 24.9 live PIN rotation consistency (2026-10-20)

PIN rotation now updates the live kernel authority, the trusted account file,
and WovenShell's bounded account cache consistently. A failed file rewrite
rolls the in-memory credential back. The 86-test host suite, host Clippy,
Stage 24.5 Clippy, system-apps Clippy, diff check, and signed system-apps
1/2/4-CPU shell gates pass. Evidence is under:
`audit-artifacts/stage24.9-pin-rotation-1cpu-1792532400000000000`,
`audit-artifacts/stage24.9-pin-rotation-2cpu-1792532400000000000`, and
`audit-artifacts/stage24.9-pin-rotation-4cpu-1792532400000000000`.

The fixture does not drive the PIN-change UI; rotation-specific proof is the
bounded authority test plus the integrated kernel/shell path. Administration,
lockout/rate limiting, malformed-request telemetry, hardware credentials,
release-key provisioning, and physical qualification remain open.

## Stage 24.8 WID2 runtime recovery (2026-10-20)

System-apps startup now prefers the newest valid authenticated WID2 snapshot
from native WovenFS and falls back to the bounded account file when no durable
copy is present. Snapshot decoding is bounded, sessions are excluded, and the
native read path is fail-closed. Stage 24.3 Clippy, host tests/Clippy,
system-apps Clippy, diff check, and the signed system-apps shell gate on 1/2/4
CPUs pass. Evidence is under:
`audit-artifacts/stage24.8-wid2-runtime-1cpu-1792531800000000000`,
`audit-artifacts/stage24.8-wid2-runtime-2cpu-1792531800000000000`, and
`audit-artifacts/stage24.8-wid2-runtime-4cpu-1792531800000000000`.

The QEMU fixture has no ATA WovenFS disk, so the durable branch is covered by
the Stage 24.3 native self-test and the live gate covers the safe fallback.
Administration/rotation, lockout/rate limiting, malformed-request telemetry,
hardware credentials, real release-key provisioning, and physical
USB/power-loss qualification remain open.

## Stage 24.7 WovenIdentity audit actions (2026-10-20)

Identity open, lock, unlock, elevation, and close outcomes now enter the
kernel security ledger. Stable public audit action IDs 26–30 are decoded by
`libwoven::audit_read`. The host 85-test suite, host Clippy, Stage 24.5
freestanding Clippy, system-apps freestanding Clippy, diff check, and the
signed system-apps shell gate on 1/2/4 CPUs pass. Evidence is under:
`audit-artifacts/stage24.7-identity-audit-1cpu-1792531200000000000`,
`audit-artifacts/stage24.7-identity-audit-2cpu-1792531200000000000`, and
`audit-artifacts/stage24.7-identity-audit-4cpu-1792531200000000000`.

Malformed user-buffer requests return before an identity operation is formed
and are not yet separate audit events. WID2 runtime recovery, administration,
PIN rotation, lockout/rate limiting, hardware-backed credentials, and physical
USB/power-loss qualification remain release prerequisites.

## Stage 24.6 live WovenIdentity runtime and WovenShell login (2026-10-20)

The system-apps boot imports bounded `/etc/wovenhat/accounts` records into a
fresh kernel `Authority` before WovenShell starts. WovenShell verifies the PIN
once and uses capability-gated verified-digest syscalls 224 and 225; the
kernel compares the digest to its own authority before binding credentials to
the shell process. Lock, unlock, owner validation, and process-exit
reclamation are live. The harness synchronizes login input to the shell's
`desktop ready` marker.

The 85-test `libwoven` suite, host Clippy, Stage 24.5 freestanding Clippy,
system-apps freestanding Clippy, and diff check pass. The signed
system-app/hardware-demo shell gate passes on 1/2/4 CPUs. Evidence is under:
`audit-artifacts/stage24.6-live-1cpu-1792530000000000000`,
`audit-artifacts/stage24.6-live-2cpu-1792530000000000000`, and
`audit-artifacts/stage24.6-live-4cpu-1792530000000000000`.

WID2 runtime recovery, account administration, PIN rotation, lockout/rate
limiting, identity audit events, and hardware-backed credentials remain open;
this is not a complete production identity subsystem.

## Stage 24.5 bounded WovenIdentity syscall ABI (2026-10-20)

The WovenHat ABI now reserves syscalls 219–223 for identity session open,
lock, unlock, elevation, and close. Requests use bounded length-prefixed
buffers; the kernel copies them before parsing or authentication, rejects
truncation/empty/oversized fields, and returns the existing error sentinel on
failure. `libwoven::identity` provides safe wrappers that keep request bytes on
the caller's bounded stack and never retain password pointers.

The dedicated gate passed with exit 33 and
`[S24.5] WovenIdentity syscall ABI: PASSED` on 1/2/4 CPUs. The 84-test
`libwoven` suite, Stage 24.5 freestanding Clippy, host all-targets Clippy,
Python validation, and `git diff --check` pass. Evidence is preserved under:
`audit-artifacts/stage24.5-1cpu-1792526996850760700`,
`audit-artifacts/stage24.5-2cpu-1792527013189914600`, and
`audit-artifacts/stage24.5-4cpu-1792527027362429900`.

This stage proves the bounded ABI and parser only. Durable snapshot loading
into the runtime, a live Ring-3 login fixture, process-exit reclamation, audit
events, lockout/rate limiting, and WovenShell migration remain required.

## Stage 24.4 owner-bound kernel identity seam (2026-10-20)

`kernel/src/identity_runtime.rs` introduces the kernel seam between the
accepted identity authority and process credentials. Session tokens are kept
in a fixed binding table with an owner process ID; lock, unlock, elevation,
and close reject tokens owned by another process. Credential application is
performed only after releasing the identity lock and only through a crate-only
task setter, preserving the existing `getuid/getgid` ABI and lock ordering.
If no live process owner exists, opening a session fails closed and cleans up
the authority session and binding.

The dedicated gate passed with exit 33 and
`[S24.4] WovenIdentity task binding: PASSED` on 1/2/4 CPUs. The corrected
evidence is preserved under:
`audit-artifacts/stage24.4-1cpu-1792526710240384400`,
`audit-artifacts/stage24.4-2cpu-1792526744522828500`, and
`audit-artifacts/stage24.4-4cpu-1792526778145524000`.

This is not yet the userspace identity interface: syscall framing, loading the
durable snapshot into the runtime, process-exit binding reclamation, login
manager integration, lockout/rate limiting, and administrative policy remain
required before production identity can be claimed.

## Stage 24.3 kernel-authorized WovenIdentity durable storage (2026-10-20)

The kernel now provides a StorageIo-authorized WovenFS adapter for identity
snapshots. It writes `WID2` records to two fixed native paths with a monotonic
generation and wrapper checksum, selects the newest valid copy on recovery,
and validates the inner authenticated `WID1` snapshot before writing or
returning it. The launcher attaches a disposable IDE image for this destructive
storage gate; the user USB is not touched.

The dedicated gate passed with exit 33 and
`[S24.3] WovenIdentity durable storage: PASSED` on 1/2/4 CPUs. The corrected
evidence is preserved under:
`audit-artifacts/stage24.3-1cpu-1792526461019999200`,
`audit-artifacts/stage24.3-2cpu-1792526482958590800`, and
`audit-artifacts/stage24.3-4cpu-1792526505203409200`.

The earlier failed attempt is retained at
`audit-artifacts/stage24.3-1cpu-1792526354775283900`; it exposed the missing
scratch-disk harness configuration and was fixed without weakening assertions.
This stage does not yet expose user-facing persistence, key management,
credential rotation, lockout/rate limiting, or bind restored identities to
running kernel tasks.

## Stage 24.2 authenticated WovenIdentity snapshot (2026-10-20)

`libwoven::identity` now encodes users, groups, administrator roles, profile
names, salts, and password digests into a fixed-size `WID1` snapshot with an
FNV-1a integrity check. Decoding is fail-closed and transactional: malformed,
duplicate, out-of-range, tampered, or non-zero-padding records are rejected
without changing an existing authority. Active sessions are never serialized,
so reboot cannot restore authentication state.

The dedicated gate passed with exit 33 and
`[S24.2] WovenIdentity snapshot: PASSED` on 1/2/4 CPUs. The 84-test
`libwoven` suite, Stage 24.2 freestanding Clippy, host all-targets Clippy,
Python validation, and `git diff --check` pass. Evidence is preserved under:
`audit-artifacts/stage24.2-1cpu-1792526063075640900`,
`audit-artifacts/stage24.2-2cpu-1792526104893458300`, and
`audit-artifacts/stage24.2-4cpu-1792526146509891500`.

This is the authenticated storage format, not yet durable filesystem
integration: the kernel still needs authorized atomic writes/recovery, record
key management, credential rotation, lockout/rate limiting, and task/session
credential integration.

## Stage 24.1 bounded WovenIdentity core (2026-10-20)

`libwoven::identity` provides a fixed-capacity local identity authority for
users, groups, PBKDF2-HMAC-SHA256 credentials, sessions, lock/unlock state,
user profiles, and explicit password-confirmed privilege elevation. User,
group, session, name, salt, and profile capacities are compile-time bounded;
password comparisons are constant-time over the stored digest, and elevation
resets on lock. The module is policy-only: it does not yet switch kernel task
credentials or persist the identity database.

The dedicated gate passed with exit 33 and
`[S24.1] WovenIdentity core: PASSED` on 1/2/4 CPUs. The 82-test `libwoven`
suite, Stage 24.1 freestanding Clippy, Python validation, and `git diff --check`
pass. A security review also requires elevation to be an explicit administrator
role; ordinary users cannot become UID 0, and full-group insertion is atomic.
The corrected gate evidence is preserved under:
`audit-artifacts/stage24.1-1cpu-1792525654517891200`,
`audit-artifacts/stage24.1-2cpu-1792525721415869300`, and
`audit-artifacts/stage24.1-4cpu-1792525782428769900`.

This is the identity policy foundation, not a production identity subsystem:
kernel task/session integration, durable credential storage and recovery,
group authorization in the capability broker, account administration UI,
lockout/rate limiting, and hardware-backed credentials remain required.

## Stage 23.7 advisory WovenAI security monitor (2026-10-20)

`libwoven::ai_monitor` provides bounded explanations over existing
`security::AuditEvent` records. It classifies denied WovenGuard/sandbox events,
sensitive grants/stores, process faults, and ordinary records into fixed-size
findings capped at 16 entries with 64-byte explanations. It has no
capability-grant, revoke, enforcement, or device-control API; WovenGuard
remains the only enforcement authority.

The dedicated gate passed with exit 33 and
`[S23.7] WovenAI security monitor: PASSED` on 1/2/4 CPUs. The 80-test
`libwoven` suite, Stage 23.7 freestanding Clippy, host all-targets Clippy,
Python validation, and `git diff --check` pass. Evidence is preserved under:
`audit-artifacts/stage23.7-1cpu-1792525067904963600`,
`audit-artifacts/stage23.7-2cpu-1792525085640869300`, and
`audit-artifacts/stage23.7-4cpu-1792525103539579600`.

This is an advisory event-summary contract, not a complete anomaly detector,
monitor UI, alert transport, or automated response system. Any future AI
recommendation must still be evaluated by deterministic WovenGuard policy.

## Stage 23.6 WovenAssistant permissioned action planner (2026-10-20)

`libwoven::ai_assistant` defines bounded plans with seven explicit action
classes: application launch, file read/write, settings, messages, commands,
and network requests. Every action maps to a named permission, side-effecting
actions require confirmation, targets are validated UTF-8 without NUL bytes,
and plans are capped at eight actions.

The dedicated gate passed with exit 33 and
`[S23.6] WovenAssistant actions: PASSED` on 1/2/4 CPUs. The 78-test
`libwoven` suite, Stage 23.6 freestanding Clippy, host all-targets Clippy,
Python validation, and `git diff --check` pass. Evidence is preserved under:
`audit-artifacts/stage23.6-1cpu-1792524840375472300`,
`audit-artifacts/stage23.6-2cpu-1792524861370942600`, and
`audit-artifacts/stage23.6-4cpu-1792524875749738800`.

This is the permissioned action contract, not yet a complete assistant UI or
platform executor. Permission-broker integration, audit presentation, user
confirmation UX, cancellation, and actual application/file execution remain
required before claiming a production assistant.

## Stage 23.5 bounded semantic search core (2026-10-20)

`libwoven::semantic_search` now provides a fixed-capacity, allocation-free
search index covering the roadmap's application, file, setting, message,
document, and command kinds. UTF-8 titles are normalized into bounded ASCII
terms; query hits are scored by term coverage and ranked stably. The index is
capped at 32 entries, 12 terms per entry, 16 bytes per term, and 8 results.

The dedicated gate passed with exit 33 and
`[S23.5] WovenAI semantic search: PASSED` on 1/2/4 CPUs. The 76-test
`libwoven` suite, Stage 23.5 freestanding Clippy, host all-targets Clippy,
Python validation, and `git diff --check` pass. Evidence is preserved under:
`audit-artifacts/stage23.5-1cpu-1792524571689446000`,
`audit-artifacts/stage23.5-2cpu-1792524589227010700`, and
`audit-artifacts/stage23.5-4cpu-1792524606220735100`.

The index now supports owner-scoped privacy filtering, bounded upsert/removal,
and an authenticated fixed-format snapshot for durable reload. It is still not
a complete production filesystem/message ingestion pipeline or vector-
embedding runtime. Source adapters, ranking quality evaluation, and
user-facing search service integration remain required before release
readiness.

## Stage 23.4 WovenAI capability security (2026-10-20)

The Stage 23.4 WovenGuard gate makes the AI security contract explicit and
auditable. A local AI profile is limited to `Ipc + Ai` and receives no file,
network, secret, camera, microphone, USB, or device policy. A cloud provider
profile adds only the explicitly requested `Secrets + NetworkIo` authorities;
the broker remains `Ipc`-only. The SystemService domain ceiling rejects `Ai`,
`Secrets`, camera, and microphone authority, preventing accidental inheritance
by unrelated system services.

The dedicated gate passed with exit 33 and
`[S23.4] WovenAI capability security: PASSED` on 1/2/4 CPUs. Evidence is
preserved under:
`audit-artifacts/stage23.4-1cpu-1792524134603006000`,
`audit-artifacts/stage23.4-2cpu-1792524151189661900`, and
`audit-artifacts/stage23.4-4cpu-1792524165773461400`.

This proves WovenGuard capability and device-policy narrowing; it does not
claim real cloud HTTPS/TLS integration, semantic search, assistant actions,
or physical hardware qualification.

## Stage 23.3 WovenAI provider-plugin boundary (2026-10-20)

`libwoven::ai_provider` defines the bounded provider-plugin contract. A cloud
provider is a separate Ring-3 process published as `woven.ai.cloud`; the
WovenAI broker delegates versioned WAI1 requests to it without copying or
passing credential bytes. The provider owns its generation-safe WovenVault
handle and returns only a bounded descriptor or result. Provider capability
authority is explicit: the acceptance fixture receives `Ipc`, `Secrets`, and
`NetworkIo`, while the broker receives only `Ipc`.

The Stage 23.3 gate passed with exit 33 and
`[S23.3] WovenAI cloud provider: PASSED` on 1/2/4 CPUs. It proves provider
publication, broker delegation, vault-backed credential use, capability denial
for an unauthorized client, response correlation, and teardown. Evidence is
preserved under:
`audit-artifacts/stage23.3-1cpu-1792523642289709300`,
`audit-artifacts/stage23.3-2cpu-1792523675601682100`, and
`audit-artifacts/stage23.3-4cpu-1792523708187643800`.

This acceptance uses a deterministic offline provider fixture. It does not
claim arbitrary cloud API compatibility, HTTPS/TLS transport, endpoint
configuration, credential provisioning UX, rate limiting, billing controls,
or physical network qualification. Those are required before a real cloud
provider can be called production-ready.

## Stage 23.2 bounded WovenAI local provider (2026-10-20)

`libwoven::ai_model` now defines the explicit model ABI and a deterministic,
no-allocation local provider with fixed integer weights. It consumes the
existing bounded WAI1 UTF-8 input, classifies a fixed feature vocabulary, and
returns a packed class/confidence result. The broker's `Describe` response
advertises both the local-provider flag and model ABI; normal broker mode still
returns `NoProvider`.

The dedicated Stage 23.2 gate passed with exit 33 and
`[S23.2] WovenAI local provider: PASSED` on 1/2/4 CPUs. The 71-test
`libwoven` suite, Stage 23.1 and 23.2 freestanding Clippy, host all-targets
Clippy, broker app Clippy, and Python validation pass. Evidence is preserved
under:
`audit-artifacts/stage23.2-1cpu-1792523271142588200`,
`audit-artifacts/stage23.2-2cpu-1792523298416240400`, and
`audit-artifacts/stage23.2-4cpu-1792523326958053700`.

This is a bounded software provider, not a general neural runtime. Model
files, larger tensors, hardware acceleration, cloud credentials, semantic
search, assistant actions, and AI security-monitor policy remain later stages.
No physical USB qualification or USB write was performed.

## Stage 23.1 controlled WovenAI broker boundary (2026-10-20)

`libwoven::ai_ipc` now defines the bounded version-1 `WAI1` request/response
protocol. The Ring-3 `apps/woven-ai` service publishes `woven.ai`, answers a
broker-ABI description request, and explicitly reports `NoProvider` for
completion requests; it does not imply that a model or provider exists. A
dedicated syscall 218 requires both `Ipc` and `Ai` capability before sending a
WAI1 request. The acceptance probe is launched in the normal user domain with
an explicit `Ai` declaration, while the broker itself has only `Ipc`.

The dedicated gate in `scripts/test-stage23-1-ai-broker.py` passed with exit 33
and `[S23.1] WovenAI broker: PASSED` on 1/2/4 CPUs. The 69-test `libwoven`
suite, Stage 23.1 freestanding Clippy, host all-targets Clippy, broker app
Clippy, Python validation, and all Stage 21 feature lints pass. Evidence is
preserved under:
`audit-artifacts/stage23.1-1cpu-1792522717245706600`,
`audit-artifacts/stage23.1-2cpu-1792522745744098400`, and
`audit-artifacts/stage23.1-4cpu-1792522775899902400`.

This is only the controlled broker boundary. Local model execution, provider
plugins, cloud credentials, semantic search, assistant actions, and AI
security-monitor policy remain later roadmap stages. The broker has no
implicit file, camera, microphone, network, secret, application, or device
authority; no physical USB qualification was performed.

## Stage 22.5 userspace WovenPackage transaction authority (2026-10-20)

The signed Ring-3 `apps/woven-package` broker now executes install, remove,
and rollback requests through syscall 217. The syscall validates the operation
and bounded source path, requires `FileRead`, `FileWrite`, and `StorageIo`,
and issues the private native WovenFS writer only after those checks. Package
bytes stay out of IPC frames: install resolves a broker-owned VFS source,
verifies the WPKG envelope, and commits the existing authenticated,
journaled/chunked package publication path. Remove and rollback use the same
durable catalog and recovery records.

The focused gate in `scripts/test-stage22-5-package-transactions.py` formats a
disposable ATA/WovenFS volume, installs a signed fixture, verifies the durable
catalog, removes it, rolls back, and verifies restoration. It passed with exit
33 and `[S22.5] WovenPackage transactions: PASSED` on 1, 2, and 4 CPUs. The
freestanding Stage 22.5 Clippy gate, host all-targets Clippy, 66-test
`libwoven` suite, app build/signing, and Python syntax validation also pass.
Evidence is preserved under:
`audit-artifacts/stage22.5-1cpu-1792521732550569700`,
`audit-artifacts/stage22.5-2cpu-1792521764182836500`, and
`audit-artifacts/stage22.5-4cpu-1792521793861888600`.

The implementation remains deliberately bounded: source paths use the normal
512-byte syscall limit, one source package is limited by the 64 KiB VFS node
capacity, catalogs use the fixed WovenStore record ceiling, and payloads are
published in native WovenFS chunks. Larger ingress, key rotation, update-daemon
ownership, power-loss timing, and physical USB qualification remain open
follow-up work; this acceptance does not claim whole-OS release readiness.

## Stage 22.4 production WovenPackage startup (2026-10-20)

The production `system-apps` build now includes the signed Ring-3
`apps/woven-package` image through `WOVENHAT_PACKAGE_IMAGE`. Boot starts the
service after the signed ten-application catalog is installed and requires a
bounded `woven.package` publication owned by the spawned PID before continuing.
The production shell/PS2/IOAPIC QMP gate passed on 1/2/4 CPUs, with separate
preserved logs under `target/shell-regression-{1,2,4}cpu`.

This closes production startup and image packaging for the broker. Stage 22.5
now supplies the bounded userspace transaction authority described above.
Physical USB qualification remains open.

## Stage 22.3 Ring-3 WovenPackage broker boundary (2026-10-20)

The new `apps/woven-package` signed Ring-3 service publishes the bounded
`woven.package` endpoint, receives a real `WPKI` query request through service
discovery, consults the capability-gated userspace directory authority, and
sends a correlated `WPKI` response to the sender mailbox. Its
buffers are static bounded storage rather than stack allocations, and process
reaping removes the published service. The dedicated Stage 22.3 fixture passed
on 1/2/4 CPUs. App Clippy, host/freestanding warning-denying Clippy, the
66-test `libwoven` suite, and diff checks passed.

This closes the real broker transport/lifecycle boundary, but not the full
package manager: query now proves bounded storage-backed existence checks, while
install,
remove, and rollback return an explicit storage-authority status until a
userspace storage service/API is implemented. The broker is accepted as a
service fixture, not yet started by every production system-app boot. Physical
USB qualification remains open.

## WovenPackage IPC protocol foundation (2026-10-20)

`libwoven::package_ipc` defines a bounded version-1 `WPKI` control protocol
for install, remove, rollback, and query requests plus fixed-size responses.
It validates operation/version/length fields and rejects traversal segments,
invalid path bytes, and oversized frames. Package bytes are intentionally not
copied into IPC messages; the future broker resolves authenticated storage
objects through its own service authority. The 66-test `libwoven` suite,
warning-denying host Clippy, and diff checks pass.

This is a protocol foundation, not a completed service. A Ring-3 broker and
client, service startup/ownership wiring, a QEMU userspace exercise, and
physical USB qualification remain required.

## Stage 22.2 handle-addressed IPC transport (2026-10-20)

Discovered service capabilities are now usable by Ring-3 callers. Syscall 215
copies a bounded message and sends it to a caller-owned endpoint handle; syscall
216 receives from a handle into a validated userspace buffer, prefixing the
sender process ID so a broker can route a response through the existing
capability-gated process mailbox. Both require
`Capability::Ipc`, preserve the nonblocking queue contract, and fail closed on
invalid handles, oversized messages, missing rights, or copy faults.
`libwoven` exposes typed send/receive wrappers. The Stage 22.2 QEMU gate passed
on 1/2/4 CPUs; host tests and warning-denying host/freestanding Clippy passed.

This closes the basic userspace transport needed by service clients. It does
not yet define or implement the WovenPackage request protocol, a long-lived
Ring-3 package broker, blocking service-call convenience APIs, or physical USB
qualification.

## Stage 22.1 userspace service IPC ABI (2026-10-20)

The IPC service registry now has a userspace ABI: syscall 211 creates an
owner-bound endpoint, while syscalls 212–214 publish, unpublish, and discover
bounded service names. Each operation requires `Capability::Ipc`; service names
are copied from userspace before registry access, handles are returned as
process-local values, and the existing registry enforces owner teardown,
rights, namespace isolation, and endpoint lifetime. `libwoven` exposes typed
wrappers for all four operations.

Host `libwoven` tests (63), warning-denying host/freestanding Clippy, and the
dedicated QEMU gate passed on 1/2/4 CPUs. The first attempt is preserved as
`audit-artifacts/stage21.22-1cpu-1792516996924333800`; it failed before the
new marker because the feature did not inherit the required WovenFS gate, so
the feature graph and inherited self-test guards were corrected before the
accepted rerun. Accepted evidence is recorded in
`docs/audit-stages-consolidated.md`.

This increment proves the kernel registry and syscall/library ABI boundary;
it does not yet claim a long-lived Ring-3 WovenPackage broker/client,
service-specific protocols, or physical USB qualification.

## Stage 11 shared-library TLS implementation (2026-10-20)

The loader now plans a bounded combined TLS block across the main image and
shared libraries, copies immutable PT_TLS initializers into a read-only
template, creates a fresh mutable TLS/TCB per process, and resolves the
toolchain's shared-object TLS relocation. Standard Stage 11.5 acceptance
passes on 1/2/4 CPUs; host tests and warning-denying Clippy pass.

The compiler-fixture acceptance now passes on 1/2/4 CPUs, including the exact
signed Rust/LLD shared PIE, shared-library PT_TLS relocation, per-process
template initialization, nonzero-bias load, and owner teardown. The fix uses
the ELF `STT_TLS` symbol type with a non-undefined section index; the kernel
task stack reservation is 256 KiB. See the audit file above for evidence.

## Stage 11 lazy PLT binding (2026-10-20)

Writable PLT `JUMP_SLOT` entries now start with loader-generated RX resolver
stubs instead of eagerly resolved targets. The first Ring-3 call supplies the
GOT slot through the stub; the kernel validates it, breaks COW when required,
and patches the private slot before jumping to the target. RELRO slots remain
eager and over-capacity bindings fail closed.

Validation: 33 ELF tests, warning-denying host/freestanding Clippy, 11 Python
harness tests, and Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs. The signed
fixture verifies an unresolved writable PLT slot before execution and a
successful first-call resolution/teardown afterward. Evidence:
`audit-artifacts/stage11.5-1cpu-1792508068498107100`,
`stage11.5-2cpu-1792508101002664400` and
`stage11.5-4cpu-1792508116869852100`.

The process carries eight bounded lazy bindings; the subsequent shared-library
TLS correction is accepted in the Stage 11 audit above.

## Stage 11 lifecycle callbacks (2026-10-20)

The bounded ELF loader now validates `DT_INIT`, `DT_FINI`, `DT_INIT_ARRAY`
and `DT_FINI_ARRAY` metadata as executable callbacks, including byte-size
alignment and a per-image callback bound. Constructors run in dependency order
before Ring-3 entry; destructors run in reverse dependency order through a
loader-owned RX exit trampoline. The callback return chain is prebuilt in a
reserved user-stack region, and exit teardown snapshots address-space state
before paging work, then rechecks ownership under the process-table lock.

Validation: 33 ELF tests, Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs with
constructor scheduling, Ring-3 constructor/destructor execution, teardown,
and all prior loader-hardening markers. Evidence:
`audit-artifacts/stage11.5-1cpu-1792507060371452000`,
`stage11.5-2cpu-1792507334555453100` and
`stage11.5-4cpu-1792507351184297200`.

Lazy binding and library TLS remain prerequisites before full Stage 11 closure.

## Stage 11 versioned symbol tables (2026-10-20)

The bounded ELF loader now accepts SysV `DT_VERSYM`, `DT_VERDEF` and
`DT_VERNEED` metadata. File-backed version records, auxiliary links, version
indices and names are validated before use; definitions and imports resolve
only when their requested version matches. Relocated images shift dynamic
metadata addresses consistently, and malformed or truncated records fail
closed.

Validation: 31 ELF tests, workspace tests, kernel check, warning-denying
freestanding and host Clippy, and Stage 11.5 QEMU acceptance passed on 1/2/4
CPUs. The signed runtime fixture resolves a versioned cross-image import,
executes it in Ring 3, and verifies teardown. Evidence:
`audit-artifacts/stage11.5-1cpu-1792505125207763300`,
`stage11.5-2cpu-1792505299339871100` and
`stage11.5-4cpu-1792505368683688700`.

Constructors/destructors, lazy binding and library TLS remain open.

## Stage 11 weak symbol binding support (2026-10-20)

The bounded symbol scope now accepts `STB_WEAK` definitions as fallbacks
behind strong definitions. Default-visible weak definitions participate in
normal interposition, while undefined weak imports resolve to zero when no
definition exists. Relocation resolution preserves the existing visibility,
symbol-bounds, W^X and transactional teardown rules; version-table metadata
continues to fail closed.

Validation: 30 ELF tests, kernel check, warning-denying freestanding and host
Clippy, and Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs. The signed
runtime fixture resolves a weak cross-image import, executes it in Ring 3,
and verifies teardown. Evidence:
`audit-artifacts/stage11.5-1cpu-1792504013870714100`,
`stage11.5-2cpu-1792504112883675500` and
`stage11.5-4cpu-1792504158087902300`.

Constructors/destructors, lazy binding and library TLS remain open.

## Stage 11 eager PLT DT_REL relocation support (2026-10-20)

The graph loader now accepts legacy `DT_JMPREL` tables encoded as `DT_REL`.
`DT_PLTRELSZ`, `DT_PLTREL == DT_REL` and the fixed 16-byte entry format are
validated as bounded file-backed metadata; each target must be writable and
non-executable. Addends are read from the mapped target and applied using the
same checked relocation and symbol-scope rules as ordinary DT_REL.

Validation: 28 ELF tests, kernel check, warning-denying freestanding and host
Clippy, and Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs with dedicated
relocation, Ring-3 execution and teardown markers. Evidence:
`audit-artifacts/stage11.5-1cpu-1792503297814943600`,
`stage11.5-2cpu-1792503342880325200` and
`stage11.5-4cpu-1792503373152819300`.

Versioned symbol tables, constructors/destructors, lazy binding and library
TLS remain open.

## Stage 11 bounded DT_REL loader support (2026-10-20)

The ELF loader now validates and applies ordinary `DT_REL` tables with
`DT_RELSZ` and `DT_RELENT == 16`. REL addends are read only from the mapped
private relocation target, interpreted as signed 64-bit values, and applied
under the same relocation-type, symbol, bounds and writable/non-executable
checks as RELA. Malformed tables and read-only targets fail closed.

Validation: 27 ELF tests, kernel check, warning-denying freestanding and host
Clippy, and Stage 11.5 QEMU acceptance on 1/2/4 CPUs passed. The acceptance
harness now includes a signed DT_REL executable that relocates, executes its
cross-image import in Ring 3, and tears down cleanly. Evidence:
`audit-artifacts/stage11.5-1cpu-1792502825772958300`,
`stage11.5-2cpu-1792502960229582900` and
`stage11.5-4cpu-1792502998392183900`.
Symbol versioning/weak binding, constructors/destructors, lazy binding and
library TLS remain open.

## Stage 11 eager PLT RELA relocation support (2026-10-20)

The ELF parser and production graph loader now support bounded `DT_JMPREL`
metadata encoded as `DT_RELA`. The loader validates the file-backed table,
entry size, relocation types, symbol indices and writable non-executable
targets, then applies PLT RELA entries eagerly before W^X and RELRO. Lazy
binding, legacy `DT_REL`, versioned/weak symbols, constructors/destructors and
library TLS remain explicitly outside this increment.

Validation: 25 ELF tests, warning-denying freestanding Stage 11.5 Clippy,
warning-denying host Clippy, and Stage 11.5 QEMU acceptance on 1/2/4 CPUs all
passed with exit 33. Legacy PLT REL is accepted in the follow-up above. This
is a bounded Stage 11 increment; the full inherited release chain and
remaining loader interoperability work are still required.

## Stage 11 ASLR arena/guard overlap corrected (2026-10-20)

The production userspace loader now plans the complete mmap arena, optional
static-TLS reservation, stack guard and user stack as disjoint checked ranges.
Previously, the highest mmap slot could contain the guard for a low forced
stack draw; the Stage `11.2-aslr` fixture reproduced this by writing the guard
and exiting 99 instead of faulting. The planner now raises the requested stack
base above the arena and TLS reservation, preserving the requested draw as a
lower bound and rejecting layouts that exceed the library window.

Validation: kernel check, warning-denying freestanding Stage 11 ASLR Clippy,
warning-denying host Clippy and the dedicated Stage `11.2-aslr` QEMU gate all
passed on 1/2/4 CPUs (exit 33). This is an accepted bounded increment, not
full Stage 11 or production readiness; broader loader interoperability,
release-chain preservation and physical qualification remain open.

## Diagnostic shell no longer overwrites a slow-starting WovenShell session; waits and reports instead (2026-10-20)

The previous fix (below) made `session::start()`'s outcome visible on
screen, but the user re-flashed and re-tested and reported no change at all:
still straight to the plain `WOVENHAT DIAGNOSTIC SHELL (F1 TO OPEN DESKTOP)`
banner, with no `SESSION:` line, and F1 still only reaches the kernel's own
`ACTIVATE`/`SECOND` placeholder window -- never WovenShell's real sign-in
desktop, even though the new `SESSION:` line was confirmed present in the
exact image flashed (byte-searched the `.img` for the literal string).

Root cause: `session::start()` only reports whether the process was
*spawned*; it says nothing about whether that process ever reaches
`compositor::activate_display()`, which is a syscall WovenShell itself
issues once its own init runs far enough. The former code treated `Ok(pid)`
as the end of the story and immediately fell through to building and
showing the diagnostic shell unconditionally (`kernel/src/main.rs`, the
`console.clear()` + banner that follows). Whenever `with_text_framebuffer`
sees `terminal::display_owned == true` it already turns every console draw
into a no-op (`kernel/src/terminal.rs:372-387`), so if WovenShell had won
that race the diagnostic banner would have silently failed to draw and the
user would have seen WovenShell's own frame instead. The user reported
seeing the diagnostic banner text itself, which proves the opposite: on
this real machine, WovenShell had *not* taken the display by the time that
code ran, i.e. it is failing or hanging early, well before sign-in, in a way
that produces no serial output and (confirmed by a still-empty `WOVENLOG`
after this boot) nothing on the stick either.

Fixed in `kernel/src/main.rs` and `kernel/src/task.rs`:

- Added `task::process_alive(id: u64)` (cfg-gated to `feature = "system-apps"`,
  its only caller): whether a process named by its raw id is in the process
  table and has not exited, for a caller on the far side of the
  `ProcessId`-erasing boundary `session::start`'s `Result<u64, Error>`
  return already has.
- After `session::start` returns `Ok(pid)`, the boot thread now waits up to
  ~5s (500 ticks, the same bounded `timer::ticks()` + `task::yield_now()`
  busy-wait pattern the stage self-tests' local `until` helpers already use
  elsewhere in this file -- no new assumption that this thread can sleep,
  only that it can yield) for `session::active()` to confirm the display was
  actually taken, or for `task::process_alive(pid)` to go false (the process
  died trying). The result is three-way, not two: **ready** (desktop up),
  **exited** (crashed/exited before showing anything), or **still running,
  not shown yet** (hung or just slow) -- each with its own `SESSION:` status
  line.
- The diagnostic shell's placeholder `ACTIVATE`/`SECOND` desktop, its banner,
  and its `console.clear()` are now only shown when `!session_ready`. A real
  user who gets as far as a working WovenShell desktop is never shown a
  developer test window; F12 still reaches the kernel's diagnostic console
  either way, since that path (`woven_input::publish` →
  `session::toggle_console()`) runs ahead of and independent of this loop.

This does not yet explain *why* WovenShell fails or hangs before sign-in on
this specific machine -- only the fallback's silence, which it no longer is.
The next real boot will say, on screen, one of: started as pid N; pid N
exited before the desktop appeared; or pid N has not shown the desktop yet
(still running) -- which narrows the real bug to "crashes" vs. "hangs" vs.
"never gets input/display resources it needs," each a different next step.

Validation: `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none`
(default features), `--features hardware-demo` (with the packaged
system-app images wired up the way `scripts/test-shell-qemu.py
--hardware-demo` does), and host `cargo clippy --all-targets -- -D
warnings` all passed. `python scripts/lint-all-features.py` passed for
every feature touching `session`/`task` (several others report
`needs-image`, which is a pre-existing gap in supplying their embedded-app
env vars, not a compile failure); the one genuine `FAIL` it found
(`physical-wifi`, missing `WOVENHAT_AX201_FIRMWARE`) is pre-existing and
unrelated -- that feature touches none of the lines changed here.
`scripts/test-shell-qemu.py --hardware-demo` passed on QEMU 1/2/4 CPUs,
`Hardware demo: sign-in + 10 applications + desktop + PS/2 IOAPIC +
smptest: PASS` on each, with the serial log showing no
`has not taken the display`/`exited before` lines -- confirming
`session_ready` still flips true quickly under QEMU and the gate's behavior
is unchanged there. A fresh image (SHA-256
`6d887f0f0b94ebf213ad3f02d4b3a50378445ba24f28df677711504e91bcdd59`) is
queued to write to the user's confirmed stick (serial `2007072809430484`,
Win32_DiskDrive size 518192640) via `scripts/write-live-usb.ps1`. Not yet
re-tested on the reporting user's physical machine.

## WovenShell session status now shown on the diagnostic-shell screen (2026-10-20)

The user re-tested the MAX_CPUS/IOAPIC fix below on real hardware: the boot
no longer stalls, but the desktop never came up -- the screen showed
`WOVENHAT DIAGNOSTIC SHELL (F1 TO OPEN DESKTOP)`, and F1 opened only the
kernel's own placeholder demo window (`ACTIVATE`/`SECOND` buttons,
`kernel/src/main.rs` around line 1700), never WovenShell's real sign-in
desktop. Pulling the stick's `WOVENLOG` after that boot found no
`LOGS\BOOTnnnn.TXT` at all -- `livelog::start_deferred()` never wrote
anything on this boot either, so there was zero on-disk evidence and, with
no serial cable, zero other evidence of why.

The code already distinguishes `session::start()`'s `Ok`/`Err` outcome, but
only logs it to serial (`kernel/src/main.rs:1664-1676`): on screen, the
`BOOT LOADER [6/7]/[7/7]` progress lines it prints are wiped a moment later
by the unconditional `console.clear()` that starts the diagnostic-shell
fallback (`kernel/src/main.rs:1716`), so a failed session and a session
quietly running behind the compositor looked identical on a real screen.

Fixed in `kernel/src/main.rs`: the session outcome is now also kept in a
`session_status` string (`"SESSION: started as pid N ..."` or `"SESSION: NOT
STARTED (Image|Spawn)"`) and printed again, persistently, right after the
`WOVENHAT DIAGNOSTIC SHELL` banner -- it is no longer wiped, so the next
real-hardware boot will say on screen whether WovenShell actually started,
and why not if it didn't. This does not yet explain *why* `session::start`
might fail or why `livelog` wrote nothing on that boot; it only makes the
first one visible without a serial cable, which is the prerequisite for
diagnosing either from the screen alone.

Validation: `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
--features hardware-demo -- -D warnings` (with the packaged system-app
images wired up the way `scripts/test-shell-qemu.py --hardware-demo` does)
passed; host `cargo clippy --all-targets -- -D warnings` passed;
`scripts/test-shell-qemu.py --hardware-demo` passed on QEMU 1/2/4 CPUs,
`Hardware demo: sign-in + 10 applications + desktop + PS/2 IOAPIC + smptest:
PASS` on each, with no change to the serial markers the gate checks (the new
line is screen-only). A fresh image (SHA-256
`59d7d5f223bc7cbc2171ff5d0cb92fab5768757f65ab9d511840308f239734ff`) is
queued to write to the user's confirmed stick (serial `2007072809430484`,
Win32_DiskDrive size 518192640) via `scripts/write-live-usb.ps1`, which
needs an elevated shell this session does not have. Not yet re-tested on
the reporting user's physical machine.

## Boot-stall real root cause: MAX_CPUS=4 rejected real topologies; IOAPIC-count relaxed; panics now shown on screen (2026-10-20)

The user re-tested the stick with the PIT/AP-timeout fix below flashed and
reported the identical stall at "BOOT: starting CPU and interrupt setup".
Pulling the `WOVENLOG` log from the stick showed no `LOGS\BOOTnnnn.TXT` at
all, confirming the failure happens before `livelog::start_deferred()` is
ever reached -- i.e. still inside or immediately around `smp::start()`,
consistent with but not explained by the earlier fix.

Found two further hardware-topology asserts in `smp::start()`, both fatal,
silent (serial-only) panics hit before any AP, PS/2, or livelog setup:

- `assert!(!topology.truncated && topology.processor_count <= MAX_CPUS, ...)`.
  `MAX_CPUS` is a compile-time constant fixed at 4. Any real machine with
  more than 4 logical processors -- the overwhelming majority of modern
  laptops and desktops, since even a plain hyperthreaded quad-core reports
  8 -- hit this immediately, before the PIT-timing or AP-SIPI code this
  session's earlier fix touched was ever reached. None of the QEMU gates
  catch this because they only ever run at exactly 1, 2, or 4 CPUs.
- `assert_eq!(topology.io_apics, 1, "one IOAPIC required")`. Some real
  boards report more than one IOAPIC in their ACPI MADT (for example one
  behind a PCIe root complex); `hal::acpi::update_madt_summary` already
  only ever records the first IOAPIC's address/GSI base regardless of how
  many exist, so the extra ones were never going to be used anyway.

Fixed in `kernel/src/smp.rs`: a topology reporting more processors than
`MAX_CPUS` now boots with the first `MAX_CPUS` (the BSP plus whichever APs
the bring-up loop finds first, which now stops once every slot is filled
instead of indexing past the end of the per-CPU arrays) and logs the rest,
instead of refusing to boot. The IOAPIC check now accepts `>= 1` instead of
requiring exactly `1`.

Also added, since this class of bug is otherwise undiagnosable on hardware
with no serial cable attached: `kernel::terminal::panic_screen`, called from
`kernel_panic` alongside the existing serial report, draws the panic message
directly on the framebuffer without ever taking `TERMINAL`'s lock (a panic
must not try to take a lock it might already hold). `console::draw_char` and
`console::fill` are now `pub(crate)` so `terminal.rs` can reuse them. A
one-time, lock-free snapshot of the raw framebuffer pointer/geometry is
taken in `terminal::init` (single-threaded, pre-SMP) for this purpose.

Validation: kernel target check and `cargo clippy -p wovenhat-kernel
--target x86_64-unknown-none` passed; host `cargo clippy --all-targets --
-D warnings` passed; `scripts/test-shell-qemu.py --hardware-demo` passed on
QEMU 1/2/4 CPUs with `online=4 expected=4` and no new warning lines (QEMU is
configured at exactly 4 CPUs and one IOAPIC, so neither new code path
triggers there -- no behavior change under QEMU). A fresh hardware-demo
image (SHA-256 `f8bd474e3658d8a81b38d6e02c8ddff2d9270fb541d377638f360574d4e595ae`)
was written to the user-confirmed USB `USB2.0 Flash Disk USB Device`
(serial `2007072809430484`) and read back byte-for-byte matching; the 488
MiB `WOVENLOG` partition was recreated. Not yet re-tested on the reporting
user's physical machine -- that is the next step, and this time, if it
still fails, either the stick's `WOVENLOG` log or the screen itself (now
that panics are shown there) should actually say why.

## Boot-stall follow-up: cached PIT-unavailable result, non-fatal AP timeout (2026-10-20)

The user reported that, even after the "Boot stall hardening" flash below, a
real machine still appeared to hang (very slowly, not instantly) at "BOOT:
starting CPU and interrupt setup". Root cause: the bounded port-0x80 fallback
in `smp::delay_10ms` only bounded a *single* call to ~5 s before falling
back -- it re-ran that whole ~5 s "is the PIT there" probe on *every* call,
and AP bring-up calls it up to roughly a dozen times on `MAX_CPUS` hardware.
On firmware where the PIT channel-2/speaker gate never responds, that is
tens of seconds of a static screen with no progress indicator, which reads
as a hang even though it was technically bounded.

Fix in `kernel/src/smp.rs`: a `PIT_UNAVAILABLE` flag now caches the result
after the first probe, so only that first `delay_10ms` call pays the ~5 s
cost; every later call during AP bring-up goes straight to the fast
port-0x80 fallback. Also hardened: an AP that never responds to SIPI no
longer hits `assert!(..., "AP startup timeout")`, which is a fatal panic
visible only on the (unconnected, on real hardware) serial port -- the
on-screen console never shows panics, so this previously looked exactly
like a permanent silent hang with zero way to diagnose it, and it happens
before `livelog::start_deferred()` has even queued, so it would not reach
the stick's `WOVENLOG` either. Boot now continues with whichever CPUs did
come online (the BSP is always present) and logs the skipped APIC ID to
serial instead.

Validation: kernel target check and `cargo clippy -p wovenhat-kernel
--target x86_64-unknown-none` (default features) passed; host `cargo clippy
--all-targets -- -D warnings` passed; `scripts/test-shell-qemu.py
--hardware-demo` passed on QEMU 1/2/4 CPUs with `online=4 expected=4` and no
AP-timeout or PIT-fallback lines in the serial log (QEMU's PIT works, so
neither new code path triggers there -- this is a no-op on the existing
QEMU gates and only changes behavior on real hardware where the PIT/speaker
gate or an AP is actually unavailable). Not yet re-validated on the
reporting user's physical hardware; that is the next step.

## Boot-stall fix flashed (2026-10-20)

The rebuilt 13,697,024-byte hardware-demo image was written to the confirmed
USB target `USB2.0 Flash Disk USB Device`, serial `2007072809430484`, and read
back with matching SHA-256
`ac9f8058a0fdc8f64ea6f3547f1f4dbce4e963e75b1a457b22d0591f9151bfc3`. The
remaining 488 MiB FAT32 `WOVENLOG` partition was recreated. Evidence is in
`audit-artifacts/hardware-demo-usb-write-20261020-boot-stall-fix.txt`.

## Boot stall hardening (2026-10-20)

Normal boot now shows sub-progress after the graphics marker while it brings
up CPUs, probes PS/2, enables interrupts, and starts service workers. The SMP
PIT channel-2 settle delay has a bounded port-0x80 fallback for firmware where
the speaker gate does not signal after UEFI handoff. This prevents repeated
SIPI delays from appearing as a multi-minute freeze on physical hardware.

Validation: target kernel check passed; host `cargo clippy --all-targets --
-D warnings` passed; and the hardware-demo shell gate passed on QEMU 1/2/4
CPUs. The direct hardware-demo feature Clippy invocation cannot run without
the generated packaged-app environment variables, so the packaged acceptance
launcher supplied those inputs for the authoritative build.

## Hardware-demo USB refreshed (2026-10-20)

The user-confirmed USB `USB2.0 Flash Disk USB Device` (serial
`2007072809430484`) now contains the updated hardware-demo image with visible
boot-loader progress. The guarded writer verified the exact physical device,
read back all 13,697,024 image bytes with matching SHA-256, and recreated the
remaining 488 MiB FAT32 `WOVENLOG` partition. Evidence is preserved in
`audit-artifacts/hardware-demo-usb-write-20261020-loader.txt`.

## Boot loader progress feedback (2026-10-20)

Normal interactive boots now print visible `BOOT LOADER [n/7]` progress on the
framebuffer after heap setup. The markers cover graphics-surface creation,
scheduler/I/O startup, storage probing, system-file installation, device
startup, WovenShell launch, and desktop readiness. This makes the compositor
allocation gap observable instead of leaving the framebuffer unchanged while
the normal boot path continues. The change does not alter the diagnostic or
QEMU self-test sequence.

## Heap-crash root cause fixed: fallible compositor surface allocation
## (2026-10-20)

The `KERNEL ALLOC ERROR: layout size=4096000 align=4` crash found during
live interactive desktop testing (see the 2026-10-19 entry below) is
root-caused and fixed. `compositor::rescale_to` -- run on every display-
scale change -- and four `user_graphics.rs` surface-copy helpers run on
every blit (`snapshot`, `snapshot_region`, `snapshot_shared`,
`take_damage_shared`) used the panicking `Surface::new` instead of the
fallible `Surface::try_new`, which exists in the same module for exactly
this reason (`user_graphics.rs`'s own `create` syscall path already used
it correctly). At display scale 1 on a 1280x800 framebuffer, the scale-
change surface is exactly 4,096,000 bytes, matching the crash signature.
On allocation failure these paths now return a new `Error::OutOfMemory`
variant (both `compositor::Error` and `user_graphics::Error`) instead of
invoking the kernel's global `alloc_error_handler`.

**Evidence.** `scripts/test-hello-gui-compositor-qemu.py --stage 19.14`
(display scaling, repeatedly rescales) and `--stage 19.11` (window
stacking, exercises the snapshot helpers) pass on 1/2/4 CPUs.
`scripts/test-shell-qemu.py --hardware-demo` passes on 1/2/4 CPUs. Kernel
`hardware-demo`-feature Clippy and workspace `--all-targets` Clippy are
clean.

**Not fixed, and not this bug:** `--stage 19.9` (damage compositing) fails
with `[S19.9] damage rules FAIL`, reproducible at HEAD without this fix
applied (isolated via `git stash` on just the `user_graphics.rs` side of
the change). Logged in `docs/agent-coordination.md` as a separate,
pre-existing regression for whoever has been in the recent
damage/compositing commits to pick up.

## Session track - persistent account import implemented (2026-10-20)

Hardware-demo boot now checks the mounted FAT32 volume for
`/mnt/etc/wovenhat/accounts`. A bounded, syntax-checked salted-hash file is
copied read-only into `/etc/wovenhat/accounts` before WovenShell starts, so a
release image can carry its login accounts on persistent storage. Missing
accounts retain the disposable QEMU demo fallback (`ada/1234`, `lin/9876`),
while an invalid or oversized persistent file fails closed. The existing
Stage 20.12 sign-in acceptance passed on QEMU 1/2/4 CPUs after this change.

This slice does not add account creation, password change, credential
rotation, or a secure password KDF; those remain release-hardening work.

An isolated FAT32 fixture was also booted interactively in QEMU with
`/mnt/etc/wovenhat/accounts`. The serial log reported the persistent import,
skipped demo-account installation, and started WovenShell; the shared
`wovenhat-disk.img` was not modified. The fixture generator is
`scripts/create-account-fixture.py` and provisions the test account
`persistent`/`2468`.

## Session track - production app launch revalidated after SMP fix (2026-10-20)

The production packaged WovenCalculator path was rerun after the bounded
lock-screen repaint change. The hardware-demo boot signed in through the
launcher, opened the Calculator window, presented a changed desktop frame,
and exercised its bounded control successfully on QEMU 1/2/4 CPUs. Existing
per-client 1/2/4-CPU acceptance remains authoritative for the other nine
packaged clients; no client code changed in this fix.

## Graphics/session track - SMP sign-in repaint fix accepted (2026-10-20)

The WovenShell lock/sign-in path now limits key-driven repaint damage to the
account/PIN regions instead of redrawing the full 1280x800 wallpaper for every
key. This removes the multicore timing failure in the Stage 20.12 visual
assertion: sign-in, wrong-PIN refusal, successful account selection, lock,
unlock, and terminal return now pass on QEMU 1/2/4 CPUs. Stage 20.15 also
passes on 1/2/4 CPUs for session focus, launcher visibility, F12 console
handoff, return to the desktop, Quick Settings, and guest power-off.

The current interactive `hardware-demo` boot still provisions `ada/1234` and
`lin/9876` into volatile VFS state. Persistent release account installation
remains the next session slice.

## Graphics track - WovenGraphics compositor frame revalidated (2026-10-19)

The authoritative Stage 19.2 compositor acceptance passed on QEMU 1/2/4
CPUs. Each run verified the WovenGraphics visual frame, the 2D frame, and the
software 3D frame after presentation through the compositor, plus retained
window, zero exit status, and owner teardown checks. This closes the software
renderer/compositor visual slice; GPU-accelerated scanout remains the
explicitly documented device-dependent boundary.

## Graphics track - standard virtio-gpu 2D scanout revalidated (2026-10-19)

The software/2D presentation fallback is freshly accepted on QEMU 1/2/4-CPU
runs. Both 640x400 heads were captured through QMP and verified pixel for
pixel against the gradient/checkerboard contract. This is the authoritative
visible graphics path for the current release. It does not imply that the
separate GL/VirGL device can scan out a 2D or 3D resource; that boundary is
documented below with its preserved `Device(4611)` (`0x1203`) evidence.

## Graphics track - VirGL 2D-on-GL scanout boundary (2026-10-19)

The VirGL capability/context/resource/command lifecycle still passes on QEMU
1/2/4-CPU runs. A follow-on probe now attempts the conventional 2D resource
and presents the known gradient on the GL-capable device. QEMU rejects that
`SET_SCANOUT` operation with `Device(4611)` (`0x1203`), so this is preserved as
boundary evidence rather than a visible GPU-backed-frame acceptance. The probe
is capability-gated and does not invalidate the ordinary VirGL lifecycle gate.
Software/2D virtio-gpu presentation remains the accepted fallback; physical
GPU and USB HID qualification remain separate hardware-dependent gates.

## Graphics track - functional controls complete; GPU boundary revalidated (2026-10-19)

All ten packaged desktop clients now have an accepted bounded functional
pointer control over the PS/2-to-WovenShell-to-client path. Calculator,
Settings, Files, Terminal, Monitor, Security, Editor, Images, Update, and
Store each passed their production QEMU control gate on 1/2/4 CPUs, with
serial activation markers and presented-frame deltas preserved in the client
audit directories below.

The authoritative VirGL gate was also rerun on 1/2/4 CPUs. Capability,
context, resource, and command lifecycle passed with evidence under
`audit-artifacts/stage20.14-{1,2,4}cpu-*`. This still does not prove a visible
GPU-backed scanout: QEMU's tested 3D/blob presentation path remains rejected,
so software/2D scanout is the accepted graphics fallback. Physical GPU/HID
qualification remains a separate hardware-dependent gate.

## Graphics track - WovenUpdate and WovenStore pointer controls accepted (2026-10-19)

WovenUpdate now presents a bounded, read-only `Refresh` control at local
coordinates x=250..329, y=145..164; a `WPBT` release there re-reads the
bounded `/updates` preflight summary and increments a visible counter.
WovenStore presents the same read-only control at x=250..329, y=170..189; a
release there re-reads the installed catalog and increments its visible
counter. Both production QEMU gates passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/{update,store}-desktop-{1,2,4}cpu-*`.

Together with the earlier accepted slices, all ten packaged desktop clients
now have a bounded functional pointer control. USB HID qualification and
hardware GPU presentation remain separate hardware-dependent gates.

## Graphics track - WovenImages pointer control acceptance (2026-10-19)

WovenImages now presents a bounded, read-only `Refresh` control at local
client coordinates x=250..329, y=170..189. A primary-button release forwarded
as `WPBT` within that rectangle re-reads the bounded `/mnt` image summary and
increments a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app images --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/images-desktop-{1,2,4}cpu-*`.

This accepts an eighth functional client control without enabling image
decoding or mutation; the remaining clients still have pointer feedback
rather than accepted functional controls. USB HID qualification and hardware
GPU presentation remain open.

## Graphics track - WovenEditor pointer control acceptance (2026-10-19)

WovenEditor now presents a bounded, read-only `Refresh` control at local
client coordinates x=250..329, y=170..189. A primary-button release forwarded
as `WPBT` within that rectangle re-reads the bounded `/mnt` workspace summary
and increments a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app editor --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/editor-desktop-{1,2,4}cpu-*`.

This accepts a seventh functional client control without enabling desktop file
mutation; the existing CLI write path remains separate. Remaining clients
still have pointer feedback rather than accepted functional controls; USB HID
qualification and hardware GPU presentation remain open.

## Graphics track - WovenSecurity pointer control acceptance (2026-10-19)

WovenSecurity now presents a bounded, read-only `Refresh` control at local
client coordinates x=250..329, y=145..164. A primary-button release forwarded
as `WPBT` within that rectangle re-reads the bounded audit summary and
increments a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app security --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/security-desktop-{1,2,4}cpu-*`.

This accepts a sixth functional client control without changing audit state;
remaining clients still have pointer feedback rather than accepted functional
controls. USB HID qualification and hardware GPU presentation remain open.

## Graphics track - WovenMonitor pointer control acceptance (2026-10-19)

WovenMonitor now presents a bounded, read-only `Refresh` control at local
client coordinates x=250..329, y=170..189. A primary-button release forwarded
as `WPBT` within that rectangle re-reads the clock/display/storage snapshot
and increments a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app monitor --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/monitor-desktop-{1,2,4}cpu-*`.

This accepts a fifth functional client control without adding mutation;
remaining clients still have pointer feedback rather than accepted functional
controls. USB HID qualification and hardware GPU presentation remain open.

## Graphics track - WovenTerminal pointer control acceptance (2026-10-19)

WovenTerminal now presents a bounded, preview-only `Clear` control at local
client coordinates x=250..329, y=170..189. A primary-button release forwarded
as `WPBT` within that rectangle clears the in-window command preview and
updates its status text. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app terminal --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`.

This accepts a fourth functional client control without changing the separate
CLI command path or adding privileged behavior. Remaining clients still have
pointer feedback rather than accepted functional controls; USB HID
qualification and hardware GPU presentation remain open.

## Graphics track - WovenFiles pointer control acceptance (2026-10-19)

WovenFiles now presents a bounded, read-only `Refresh` control at local client
coordinates x=220..299, y=206..225. A primary-button release forwarded as
`WPBT` within that rectangle redraws the bounded `/mnt` listing and increments
a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app files --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/files-desktop-{1,2,4}cpu-*`.

This accepts a third functional client control without adding file mutation or
storage writes. Remaining clients still have pointer feedback rather than
accepted functional controls; USB HID qualification and hardware GPU
presentation remain open.

## Graphics track - Settings pointer control acceptance (2026-10-19)

WovenSettings now presents a bounded, non-mutating `Refresh` control at local
client coordinates x=250..339, y=190..213. A primary-button release forwarded
as `WPBT` within that rectangle re-reads the live identity/clock/display data
and increments a visible refresh counter. The production QEMU gate
`scripts/test-files-desktop-qemu.py --app settings --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/settings-desktop-{1,2,4}cpu-*`.

This accepts a second functional client control without adding persistence or
privileged mutation. Other clients still have pointer feedback rather than
accepted functional controls; USB HID qualification and hardware GPU
presentation remain open.

## Graphics track - Calculator pointer control acceptance (2026-10-19)

WovenCalculator now presents a bounded `Evaluate` control at local client
coordinates x=150..209, y=110..127. A primary-button release forwarded as
`WPBT` within that rectangle evaluates the current bounded expression and
repaints the result; the button also has a pressed visual state. The
production QEMU gate
`scripts/test-files-desktop-qemu.py --app calculator --exercise-client-click`
passed on 1/2/4 CPUs. Evidence is under
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`.

This accepts one functional client control over the PS/2-to-shell-to-client
path. Other clients still have pointer feedback rather than functional
controls; USB HID qualification and hardware GPU presentation remain open.

## Graphics track - all packaged client pointer acceptance (2026-10-19)

Settings, Monitor, Security, Editor, Images, Update, and Store now consume
bounded `WPMV` motion and `WPBT` primary-button messages and draw distinct
pointer markers. Their production QEMU client-pointer gates all passed on
1/2/4 CPUs, verifying delivery and a presented-surface pixel delta. Evidence
is under the corresponding
`audit-artifacts/{settings,monitor,security,editor,images,update,store}-desktop-*`
directories.

Together with the earlier Calculator, Terminal, and Files slices, all ten
packaged desktop clients now accept the pointer protocol. This is pointer
delivery/visual feedback only; functional controls, USB HID qualification, and
hardware GPU presentation remain open.

## Graphics track - WovenFiles client pointer acceptance (2026-10-19)

WovenFiles now consumes bounded `WPMV` motion and `WPBT` primary-button
messages and draws a distinct pointer marker in its shared surface. The
production QEMU gate
`scripts/test-files-desktop-qemu.py --app files --exercise-client-pointer`
passed on 1/2/4 CPUs, verifying delivery and a presented-surface pixel delta.
Evidence is under `audit-artifacts/files-desktop-{1,2,4}cpu-*`.

Calculator, Terminal, and Files are the accepted client-pointer consumers.
Files remains a bounded, read-only `/mnt` listing; USB HID qualification and
hardware GPU presentation remain open.

## Graphics track - Terminal client pointer acceptance (2026-10-19)

WovenTerminal now consumes the bounded `WPMV` motion and `WPBT` primary-button
messages and draws a distinct pointer marker in its shared surface. The
production QEMU gate
`scripts/test-files-desktop-qemu.py --app terminal --exercise-client-pointer`
passed on 1/2/4 CPUs, verifying delivery and a presented-surface pixel delta.
Evidence is under `audit-artifacts/terminal-desktop-{1,2,4}cpu-*`.

Calculator and Terminal are the accepted client-pointer consumers. The other
packaged clients still need pointer widgets/controls; USB HID qualification
and hardware GPU presentation remain open.

## Graphics track - client pointer forwarding (2026-10-19)

WovenShell now forwards bounded client-local pointer events after handling its
own desktop chrome. `WPMV` carries a clamped `u16` x/y motion position and
`WPBT` carries the primary button edge plus clamped client-local coordinates;
title-bar drags remain shell-owned. WovenCalculator consumes the messages and
draws a visible pointer marker.

The production QEMU gate
`scripts/test-files-desktop-qemu.py --app calculator --exercise-client-pointer`
passed on 1/2/4 CPUs. It verifies the serial delivery marker and a presented
Calculator surface delta. Evidence is under
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`.

Only the PS/2-to-WovenShell-to-Calculator path is accepted here. Other client
controls, USB HID qualification, and hardware GPU presentation remain open.

## Graphics track - Stage 20.4 window interaction acceptance (2026-10-19)

The existing Stage 20.4 QEMU gate passed on 1/2/4 CPUs. Using real i8042
input, it verifies that a title-bar drag moves Alpha and repaints both the old
and new regions, the Wide close control asks its owner to exit and removes the
window, focus and key routing remain correct, and killing the shell returns the
display to the terminal. Evidence is under
`tar\target\woven-shell-windows-{1,2,4}cpu`.

This accepts the production WovenShell window-manager move/close path. Client
pointer forwarding is covered for Calculator by the newer slice above; USB
HID qualification, hardware GPU presentation, and direct QMP drag automation
for WovenFiles remain separate.

## Graphics track - desktop pointer visual acceptance (2026-10-19)

The production desktop QEMU harness now supports `--exercise-pointer`. It
injects bounded relative motion and a left click, captures before/after frames,
and requires a material presented cursor/frame delta. The gate passed on 1/2/4
CPUs with evidence under `audit-artifacts/files-desktop-{1,2,4}cpu-*`.

This accepts the PS/2-backed software cursor overlay and existing shell
hit-testing. USB HID pointer qualification and hardware GPU presentation
remain explicitly unclaimed.

## Graphics track - WovenFiles bounded resize (2026-10-19)

WovenFiles now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/files-desktop-{1,2,4}cpu-*`.

The `/mnt` listing remains bounded and read-only; file-operation input,
pointer widgets, and hardware GPU presentation are not claimed.

## Graphics track - WovenStore bounded resize (2026-10-19)

WovenStore now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/store-desktop-{1,2,4}cpu-*`.

Store remains a read-only installed-catalog view with no package mutation;
pointer widgets and hardware GPU presentation are not claimed.

## Graphics track - WovenUpdate bounded resize (2026-10-19)

WovenUpdate now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/update-desktop-{1,2,4}cpu-*`.

Update remains a read-only, non-mutating preflight; pointer widgets and
hardware GPU presentation are not claimed.

## Graphics track - WovenImages bounded resize (2026-10-19)

WovenImages now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/images-desktop-{1,2,4}cpu-*`.

Image decoding, selection, and editing remain outside this read-only preview;
pointer widgets and hardware GPU presentation are not claimed.

## Graphics track - WovenEditor bounded resize (2026-10-19)

WovenEditor now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/editor-desktop-{1,2,4}cpu-*`.

The desktop client remains read-only; editor text input, editing controls,
pointer widgets, and hardware GPU presentation are not claimed.

## Graphics track - WovenSecurity bounded resize (2026-10-19)

WovenSecurity now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
the loader-safe 480×320 render target. The QMP `--exercise-resize` gate passed
on 1/2/4 CPUs with evidence under
`audit-artifacts/security-desktop-{1,2,4}cpu-*`.

Security remains read-only with audit display capped at 16 events; security
controls, pointer widgets, and hardware GPU presentation are not claimed.

## Graphics track - WovenMonitor bounded resize (2026-10-19)

WovenMonitor now handles `WSIZ` by creating and sharing a replacement Canvas
surface, publishing `WRSZ` before the bounded upload, and clamping requests to
480×320. The QMP `--exercise-resize` gate passed on 1/2/4 CPUs with evidence
under `audit-artifacts/monitor-desktop-{1,2,4}cpu-*`.

Monitor remains read-only and bounded at 128 storage entries; pointer widgets
and hardware GPU presentation are not claimed.

## Graphics track - WovenSettings bounded resize (2026-10-19)

WovenSettings now handles `WSIZ` by creating and sharing a replacement Canvas
surface, sending `WRSZ` before the bounded pixel upload, and clamping requests
to the loader-safe 480×320 render target. The shared QMP `--exercise-resize`
gate passed on 1/2/4 CPUs with evidence under
`audit-artifacts/settings-desktop-{1,2,4}cpu-*`.

Settings remains read-only: settings controls, pointer widgets, and hardware
GPU presentation are not claimed.

## Graphics track - bounded WSIZ/WRSZ resize negotiation (2026-10-19)

WovenShell now gives the resize handshake a five-second bounded grace period,
and WovenCalculator/WovenTerminal create, share, and publish replacement
surfaces with `WRSZ`. Their static render targets are capped at 480×320 to
stay below the current Ring-3 image/stack layout; larger requests are clamped
and acknowledged rather than causing an unbounded allocation. The QMP
`--exercise-resize` gate passed both clients on 1/2/4 CPUs with before/after
resize captures under `audit-artifacts/{calculator,terminal}-desktop-*`.

Other desktop clients still need resize implementations. Pointer widgets,
desktop command execution, and hardware GPU presentation remain separate work.

## Graphics track - Canvas damage registration and Calculator input (2026-10-19)

The kernel userspace-graphics write, fill, stroke, and text paths now register
their touched rectangles as client damage. This closes the compositor update
gap where an app could redraw its shared surface without WovenShell receiving
a damage wakeup. WovenCalculator now uses bounded ASCII/WKEY expression input
and redraws its result; its interaction gate passed on 1/2/4 CPUs with
before/after evidence under `audit-artifacts/calculator-desktop-{1,2,4}cpu-*`.

The existing Calculator CLI remains supported. Desktop pointer widgets and
hardware GPU presentation remain unclaimed; resize is covered by the newer
bounded WSIZ/WRSZ slice above.

## Graphics track - bounded desktop key protocol and Terminal input (2026-10-19)

WovenShell now forwards Enter and Backspace to the focused window as bounded
`WKEY` messages, while preserving one-byte ASCII forwarding and compositor
ownership. WovenTerminal consumes the protocol to render bounded text edits
and submission state on its shared Canvas surface. The extended
`scripts/test-files-desktop-qemu.py --app terminal` gate captures before and
after input frames and passed on 1/2/4 CPUs with evidence under
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`.

This slice does not claim terminal command execution from the desktop, pointer
widgets, or hardware GPU presentation; bounded resize is covered by the newer
WSIZ/WRSZ slice above; the existing
`--interactive` CLI remains the full command-I/O path.

## Graphics track - WovenCalculator desktop acceptance (2026-10-19)

WovenCalculator's existing bounded Canvas/WOPN client is now covered by the
shared `scripts/test-files-desktop-qemu.py --app calculator` visual gate. The
gate launches the production `--gui %p` catalog entry and passed on 1/2/4 CPUs
with evidence under `audit-artifacts/calculator-desktop-{1,2,4}cpu-*`.

The preview evaluates a bounded expression; CLI expression evaluation remains
authoritative. Desktop pointer widgets and hardware GPU presentation remain
unclaimed; Calculator input and resize are covered by newer slices above.

## Graphics track - WovenTerminal desktop preview (2026-10-19)

WovenTerminal now has a bounded read-only `--gui <shell-pid>` preview window
with a static Canvas buffer, WOPN/WCLS lifecycle, and production catalog
`--gui %p` entry. The existing `--interactive` CLI remains authoritative for
terminal input. The shared `scripts/test-files-desktop-qemu.py --app terminal`
visual gate passed on 1/2/4 CPUs with evidence under
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`.

This slice does not claim desktop key forwarding, command execution,
`WSIZ` resize handling, or hardware GPU presentation.

## Graphics track - WovenUpdate and WovenStore desktop clients (2026-10-19)

WovenUpdate now has a read-only `--gui <shell-pid>` preflight window for the
bounded `/updates` candidate count, and WovenStore has a read-only
`--gui <shell-pid>` installed-catalog window for bounded application
descriptor count. Both use full-window Canvas buffers, WOPN/WCLS, and
production catalog `--gui %p` entries. The shared
`scripts/test-files-desktop-qemu.py` launcher/screendump gate passed
`--app update` and `--app store` on 1/2/4 CPUs with evidence under
`audit-artifacts/{update,store}-desktop-{1,2,4}cpu-*`.

The slice also raises WovenShell's bounded catalog capacity from 8 to 10,
matching the ten signed production apps; this prevents alphabetical loading
from silently dropping Update and Terminal. Update remains preflight-only and
Store remains read-only: no package mutation, controls, pointer widgets, or
hardware GPU presentation is claimed. Update and Store resizing are covered by
their newer slices above.

## Graphics track - WovenImages desktop client (2026-10-19)

WovenImages now has a bounded `--gui <shell-pid>` client path. It renders a
Canvas-backed `/mnt` image-workspace summary, lists supported header families,
shares the surface, opens it with `WOPN`, and waits for `WCLS`. The production
catalog passes the shell PID. The shared
`scripts/test-files-desktop-qemu.py --app images` launcher/screendump gate
passed on 1/2/4 CPUs with evidence under
`audit-artifacts/images-desktop-{1,2,4}cpu-*`.

This is a workspace preview only: PNG/JPEG/GIF/BMP decoding, image selection,
and editing controls remain unclaimed. Its bounded resize path is covered by
the newer WovenImages slice above. Existing bounded `--list`/`--info` CLI
behavior is preserved.

## Graphics track - WovenEditor desktop client (2026-10-19)

WovenEditor now has a bounded `--gui <shell-pid>` client path. It renders a
Canvas-backed `/mnt` workspace summary and the existing CLI view/write entry
points, shares the surface, opens it with `WOPN`, and waits for `WCLS`. The
production catalog passes the shell PID. The shared
`scripts/test-files-desktop-qemu.py --app editor` launcher/screendump gate
passed on 1/2/4 CPUs with evidence under
`audit-artifacts/editor-desktop-{1,2,4}cpu-*`.

This is a read-only desktop preview; editor text input, file editing widgets,
pointer widgets, and hardware GPU presentation remain unclaimed. Its bounded
resize path is covered by the newer WovenEditor slice above. The existing CLI
retains the bounded `--view`/`--write` behavior.

## Graphics track - WovenSecurity desktop client (2026-10-19)

WovenSecurity now has a read-only `--gui <shell-pid>` client path. It renders
bounded identity and audit-count data (capped at 16 events) into a full-window
Canvas buffer, shares the surface, opens it with `WOPN`, and waits for `WCLS`.
The production catalog passes the shell PID. The shared
`scripts/test-files-desktop-qemu.py --app security` launcher/screendump gate
passed on 1/2/4 CPUs with evidence under
`audit-artifacts/security-desktop-{1,2,4}cpu-*`.

This slice is intentionally read-only and bounded; no security controls,
pointer widgets, or hardware GPU presentation are claimed. Its bounded resize
path is covered by the newer WovenSecurity slice above.

## Graphics track - WovenMonitor desktop client (2026-10-19)

WovenMonitor now has a read-only `--gui <shell-pid>` client path. It renders
bounded clock, compositor geometry, and `/mnt` storage-count data into a
full-window Canvas buffer, shares the surface, opens it with `WOPN`, and waits
for `WCLS`. The production catalog passes the shell PID. The shared
`scripts/test-files-desktop-qemu.py --app monitor` launcher/screendump gate
passed on 1/2/4 CPUs with evidence under
`audit-artifacts/monitor-desktop-{1,2,4}cpu-*`.

This slice is intentionally read-only and bounded at 128 storage entries; it
does not claim controls, resize (`WSIZ`), or hardware GPU presentation.

## Graphics track - WovenSettings desktop client (2026-10-19)

WovenSettings now has a read-only `--gui <shell-pid>` client path. It renders
bounded identity, UTC clock, and compositor geometry data into a full-window
Canvas buffer, shares the surface, opens it with `WOPN`, and waits for `WCLS`.
The production catalog passes the shell PID, and the shared
`scripts/test-files-desktop-qemu.py --app settings` gate launches it from the
F1 desktop launcher and compares before/after QEMU screendumps. Visual
acceptance passed on 1/2/4 CPUs with evidence under
`audit-artifacts/settings-desktop-{1,2,4}cpu-*`.

This slice is intentionally read-only: settings controls, pointer widgets,
and hardware GPU presentation are not claimed. Its bounded resize path is
covered by the newer WovenSettings slice above. The existing software/2D
compositor remains the accepted QEMU path.

## Graphics track - WovenFiles live launcher/screenshot acceptance (2026-10-19)

`scripts/test-files-desktop-qemu.py` now boots the packaged hardware-demo
desktop, signs in through QMP PS/2 input, launches WovenFiles through the F1
launcher query, waits for the `WOPN`/`WCLS` client lifecycle marker, and saves
before/after QEMU screendumps. The capture dimensions must remain stable and
the post-launch frame must materially change; serial evidence must also show
`WovenFiles: window opened` and `woven-shell: window opened: Files`. The full
visual acceptance passed on 1/2/4 CPUs with evidence under
`audit-artifacts/files-desktop-{1,2,4}cpu-*`.

This closes the desktop-launch visual gate for the WovenFiles client. The
remaining client limits are unchanged: no interactive file operations, and the
display path is still software/2D on QEMU; bounded resize is covered by the
newer WovenFiles slice above, and no hardware GPU presentation claim is made.

## WovenFiles desktop window, and a write_canvas placement bug fix in both
## (2026-10-19)

Following the WovenCalculator desktop window entry below: woven-files is
now wired to the same `share_with()`/`WOPN` desktop-client protocol,
showing a live `/mnt` directory listing (with an "(empty or unavailable)"
fallback). Live QMP screendump testing of both apps then caught a real
bug in both: `GraphicsSurface::write_canvas` always uploads to the
surface's `(0, 0)`, not the `(x, y)` given to `Canvas::new` (that pair is
only the canvas's own internal coordinate-addressing origin). Both apps
had drawn a border directly on the window surface at an inset `(12, 12)`
and separately written a same-sized canvas assuming it would land at that
same offset; the resulting misalignment erased the border's top/left edges
(write_canvas overwrites whatever is under it), which was first mistaken
for a shell resize-grip affordance -- it was not. Fixed by drawing
everything into one canvas sized to the whole window and uploading it in a
single call. See `docs/agent-coordination.md` for the full API-trap
writeup, since this is relevant to any further app-window work.

**Evidence.** Both windows verified via live QMP-driven sessions (sign in,
launch from the desktop launcher) with screendumps showing correct
borders, panel colour and text placement. `cargo clippy --all-targets -- -D
warnings` and `scripts/test-shell-qemu.py --hardware-demo` pass on 1/2/4
CPUs.

**Limits.** WovenTerminal has bounded text input and resize preview but
remains CLI/interactive for full command execution. Calculator has bounded
input and resize preview. Files retain no interactive file-operation controls;
hardware GPU presentation is not claimed.

## Graphics track - WovenFiles desktop client foundation (2026-10-19)

`woven-files` now has a bounded `--gui <shell-pid>` client path: it renders a
Canvas-backed `/mnt` listing into a shared surface, opens it through the
WovenShell `WOPN` protocol, and waits for the shell's `WCLS` close message.
The catalog entry now passes the shell PID. Standalone warning-denying app
Clippy and production system-app packaging/normal-boot QEMU gates passed on
1/2/4 CPUs. This is a GUI client/package acceptance, not yet a live
interactive screenshot assertion for launching Files from the desktop; that
visual workflow remains a follow-up.

## Graphics track - GSUB multiple-substitution extension acceptance (2026-10-19)

The GSUB type-2 slice now has an end-to-end regression for a type-7 extension
wrapper as well as direct lookup coverage. Oversized replacement sequences
fail closed before any output is emitted. The 57-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. This strengthens the
software shaping boundary; complete OpenType feature ordering and hardware GPU
acceleration remain separate.

## Graphics track - bounded GSUB multiple substitutions (2026-10-19)

GSUB lookup type 2 multiple substitutions now expand one shaped input glyph
into a checked replacement sequence capped at 16 glyphs, while the overall
shaped output remains bounded by the caller buffer. Direct lookups and type-7
extension wrappers use the same checked sequence parser; source metadata is
retained on the first replacement and subsequent generated glyphs are marked
as continuation records. The 57-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Full OpenType feature ordering and hardware
GPU acceleration remain separate boundaries.

## Graphics backend boundary - VIRGL lifecycle rerun (2026-10-19)

The capability-gated `virtio-gpu-gl-pci` probe now has fresh 1/2/4-CPU
evidence for VIRGL capability discovery, context/resource lifecycle, and
bounded command submission. All three runs passed under
`audit-artifacts/stage20.14-{1,2,4}cpu-*`. This closes the transport/lifecycle
qualification slice only: QEMU still rejects the tested 3D resource through
classic `SET_SCANOUT`, and the blob path is not accepted by the VirGL context,
so visible GPU-backed compositor presentation remains hardware/QEMU-bound and
unclaimed. Software/2D scanout remains the accepted fallback.

## Graphics track - bounded GSUB reverse-chaining substitutions (2026-10-19)

GSUB lookup type 8 reverse-chaining substitutions now match checked backtrack
and lookahead coverage records before selecting the bounded replacement glyph;
direct lookups and type-7 extension wrappers use the same fail-closed path.
Malformed coverage, guard sequences, and replacement arrays are rejected or
ignored without unbounded work. The 57-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## WovenCalculator desktop window (2026-10-19)

The Stage 19 catalog apps (woven-calculator, woven-settings, etc.) have been
CLI-only diagnostic binaries since Stage 19: the app catalog's `Exec=` lines
never passed `--gui`, and `woven-calculator`'s existing `run_gui()` dead code
called `surface.present()` directly -- the single-authority compositor
pattern, which an ordinary launched process cannot use (no `DisplayIo`
capability). WovenShell is the sole compositor-authority holder; client apps
must use the `share_with()` + `WOPN` IPC protocol `hello-gui
--desktop-client` already exercises (Stage 20.1-20.15).

`woven-calculator` is now wired to that protocol: `--gui <shell-pid>` (the
catalog's `Exec=/bin/wovenhat-calculator --gui %p`) draws its panel, shares
the surface with the shell, sends `WOPN`, and waits for `WCLS` to close.
Fixing this also exposed a real latent bug in the original dead code: its
108 KB scratch pixel buffer was a stack local on an 8 KiB task stack, causing
a page fault the moment the path was actually exercised. It now lives in
`.bss` via the same `UnsafeCell`-backed static pattern hello-gui's audio
buffer uses.

**Evidence.** A live QMP-driven session (sign in as `ada`, F1, type `calc`,
Enter) shows a real "Calculator" window with WovenShell's title bar, close
control and taskbar entry, and correct content ("Expression: 2 + 2",
"Result: 4"). `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
--features hardware-demo -- -D warnings`, `cargo clippy --all-targets -- -D
warnings`, and `scripts/test-shell-qemu.py --hardware-demo` all pass on
1/2/4 CPUs.

**Limits.** No `WSIZ` (resize) handling yet, so the shell's resize-grip
affordance is non-functional for this window; no interactive input (buttons,
keypad) -- it displays a fixed `2 + 2` expression. The other nine Stage 19
catalog apps (woven-files, woven-settings, woven-monitor, woven-security,
woven-editor, woven-images, woven-update, woven-store, woven-terminal) are
still CLI-only and would need the same `share_with()`/`WOPN` treatment.

## Graphics track - bounded GSUB alternate substitutions (2026-10-19)

GSUB lookup type 3 alternate substitutions now select the first bounded
alternate from a checked coverage/set record; extension-wrapped type 3 lookups
use the same path. Missing coverage and empty/oversized alternate sets fail
closed. The 56-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Hardware GPU acceleration remains a separate boundary.

## Graphics track - saturating shared font metrics (2026-10-19)

The shared kernel font metrics used by WovenGraphics now saturate width and
line-count accumulation for arbitrary UTF-8 input. Normal measurements are
unchanged, while extreme text cannot wrap metric state before renderer
clipping. The 55-test `libwoven` suite, warning-denying library/kernel/
workspace Clippy, formatting, and Stage 19.2 compositor visual acceptance
passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded surface dimension casts (2026-10-19)

Canvas and DepthBuffer now cap each accepted non-empty dimension at
`i32::MAX`, in addition to checking the far coordinate. This matches the
internal `usize`-to-`i32` offset conversions used by shader and raster loops.
The 55-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded Canvas text origins (2026-10-19)

The shared `Canvas::draw_text` path now saturates glyph and newline cursor
arithmetic before clipped writes, matching the bounded `Renderer` text path.
Extreme text origins cannot wrap into unrelated pixels. The 55-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU
acceleration remains a separate boundary.

## Graphics track - bounded surface coordinate extents (2026-10-19)

Canvas and DepthBuffer constructors now reject non-empty rectangles whose
logical right or bottom edge cannot be represented by their `i32` coordinate
APIs. This keeps shader and raster offset arithmetic defined for every
accepted surface. The 54-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded affine transform arithmetic (2026-10-19)

WovenGraphics fixed-point scale and translation constructors now saturate
large inputs, and affine mapping uses `i128` intermediates before clamping to
`i32`. Extreme transform inputs therefore cannot overflow before clipped
triangle rasterization. The 53-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - widened path-fill intersections (2026-10-19)

WovenGraphics ordinary, compound, and supersampled path containment now use
`i128` edge-intersection arithmetic. Valid `i32` path coordinates cannot
overflow during clipped scan conversion. The 52-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded path stroke rasterization (2026-10-19)

WovenGraphics path segments now use widened integer Cohen-Sutherland
clipping before Bresenham rasterization. Extreme endpoints therefore produce
only the Canvas-visible segment instead of an unbounded walk. The 51-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware
GPU acceleration remains a separate boundary.

## Graphics track - shared bounded path geometry (2026-10-19)

Rounded rectangles and path scan conversion now use a shared saturating
rectangle-end helper for Canvas bounds; rounded-corner radii also use checked
conversion. Extreme radii and path geometry cannot wrap before clipped scans.
The 50-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded text layout (2026-10-19)

WovenGraphics centered and wrapped text now derive rectangle edges with
checked conversions, and scaled glyph placement saturates cursor arithmetic
before clipped fills. Extreme text rectangles cannot wrap into unintended
coordinates. The 49-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded transformed-rectangle corners (2026-10-19)

Transformed rectangles now construct their far corners with checked
`usize`-to-`i32` conversion and saturation before affine mapping. Extreme
dimensions therefore cannot wrap before reaching the bounded triangle
rasterizer. The 48-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded image blits (2026-10-19)

WovenGraphics opaque and blended image blits now iterate only over the Canvas
overlap and map clipped rows and columns back to the source image. Large
off-screen images therefore cannot create software work proportional to their
unseen area. The 47-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded stroke geometry (2026-10-19)

WovenGraphics rectangle outlines now derive their far edges with checked
`usize`-to-`i32` conversion and saturation before the existing clipped fills.
Extreme outline dimensions therefore cannot wrap into an unintended visible
edge. The 46-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded gradient rasterization (2026-10-19)

WovenGraphics vertical and horizontal gradients now clip iteration to the
Canvas before calculating pixels, while interpolation remains based on the
full logical rectangle. The shared Canvas clip arithmetic also handles
extreme `usize` dimensions without overflow. The 45-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded transformed-triangle clipping (2026-10-19)

The shared 2D triangle helper used by transformed rectangles now clips its
raster bounds to the Canvas before iterating. Extreme transformed vertices
therefore cannot cause unbounded software loops; widened edge arithmetic and
the existing pixel clipping remain in force. The 44-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded software-3D triangle clipping (2026-10-19)

The WovenGraphics software triangle path now clips raster iteration to the
intersection of the Canvas and caller-owned depth buffer before walking pixels.
Edge and barycentric arithmetic uses widened intermediates, so extreme
off-screen coordinates fail safely without overflow or unbounded iteration.
The 43-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded chaining format-2 target sequence positioning (2026-10-19)

Nested single-positioning records under GPOS chaining format 2 now resolve
coverage against target sequence indices beyond zero, while the outer class
coverage remains anchored to the first input. The bounded class-rule,
context, nested lookup and value-record checks remain enforced. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU
acceleration remains a separate boundary.

## Graphics track - bounded chaining format-1 nested pair extension wrapper (2026-10-19)

GPOS chaining format 1 now regression-tests a type-9 extension wrapper around
nested lookup-2 format-1 pair positioning. Wrapper format, nested offsets,
target coverage, partner glyph, pair-set bounds and value records remain
checked; stateful attachment lookups fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded chaining format-1 nested pair positioning (2026-10-19)

Nested lookup-2 format-1 pair positioning is now available from GPOS chaining
format 1 through the shared checked nested-record path. Target glyph coverage,
partner selection, pair-set offsets, value-record sizes and table extents are
validated; malformed or unsupported stateful nested lookups fail closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded chaining format-1 target sequence positioning (2026-10-19)

Nested single-positioning records under GPOS chaining format 1 now resolve
coverage against the selected target glyph for sequence indices 0 through 4;
the outer chaining coverage remains anchored to the first input glyph. The
bounded glyph-rule, context and value-record checks remain unchanged. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded chained class-matrix pair extension wrapper (2026-10-19)

The bounded GPOS chaining class-rule path now regression-tests a type-9
extension wrapper around nested lookup-2 format-2 class-matrix pair
positioning. Extension format, nested offsets, class definitions, matrix
dimensions and value-record extents remain checked; malformed or unsupported
forms fail closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded contextual GPOS class-matrix pair positioning (2026-10-19)

Nested lookup-2 format-2 class-matrix pair positioning is now supported from
the bounded GPOS chaining format-2 class-rule path. Coverage, class
definitions, matrix dimensions, pair value-record extents and the selected
partner class are checked before applying the first-glyph adjustment; malformed
or unsupported nested forms fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded contextual GPOS class-rule pair positioning (2026-10-19)

The bounded GPOS chaining format-2 class-rule path now applies nested lookup-2
format-1 pair positioning, including checked pair-set, value-record and
partner-glyph bounds. Direct nested pair lookups are supported; nested
extension-wrapped pair lookups are supported through the common type-9 path.
Other nested pair/class forms remain fail-closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded contextual GPOS chaining format 2 (2026-10-19)

GPOS lookup type 8 chaining-context format 2 now matches bounded backtrack,
input and lookahead class rules with up to five input glyphs and two context
classes on either side, then applies nested single-positioning lookup-1
records through direct or type-9-wrapped lookups. Class definitions, class-set
and record bounds are checked; unsupported nested pair/class forms fail closed.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS chaining format 1 (2026-10-19)

GPOS lookup type 8 chaining-context format 1 now matches bounded glyph-rule
records with up to five input glyphs and two backtrack/lookahead glyphs, then
applies nested single-positioning lookup-1 records, including type-9 wrappers.
Malformed rules and unsupported nested pair/class forms fail closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS chaining format 3 (2026-10-19)

GPOS lookup type 8 chaining-context format 3 now reuses the checked bounded
contextual dispatcher, including one- to five-input records, up to two
backtrack/lookahead coverages, nested single/pair positioning, class-pair
matrices, and type-9 extension wrappers. Unsupported or malformed records
remain fail-closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded contextual GPOS class-pair positioning (2026-10-19)

Nested contextual GPOS type-2 format-2 pair positioning now supports checked
class definitions and bounded class matrices, reusing the direct class-pair
value-record semantics for the selected contextual target and pair partner.
Class counts, matrix extents, coverage and table bounds remain validated;
broader nested lookup semantics remain a separate boundary. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU
acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS extension wrappers (2026-10-19)

Contextual GPOS lookup dispatch now unwraps checked type-9 extension records
that contain nested type-7 contextual subtables, reusing the bounded one- to
five-input matching and nested single/pair positioning paths. Extension
format, offsets and nested subtable bounds remain validated; broader lookup
semantics remain a separate boundary. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded two-coverage contextual GPOS guards (2026-10-19)

Contextual GPOS format-3 matching now supports up to two checked backtrack
coverages and two checked lookahead coverages, supplied from adjacent shaped
glyphs at each bounded input pass. Missing neighbors fail closed; longer
context and broader contextual lookup semantics remain separate boundaries.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded multi-input contextual GPOS pair positioning (2026-10-19)

Nested type-2 format-1 pair positioning now dispatches across the bounded
contextual input sequences: the target uses the following input glyph when it
is present, or the next shaped glyph when the partner lies outside the matched
sequence. Coverage, pair-set, partner availability and advance propagation
remain checked; broader lookup semantics remain a separate boundary. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs;
the 4-CPU run required one retry after an early-boot timeout and its rerun
passed. Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS sequence-index-1 pair positioning (2026-10-19)

Two-input contextual GPOS format-3 records targeting sequence index one can
now dispatch bounded nested type-2 format-1 pair positioning using the next
shaped glyph as the pair partner. Coverage, pair-set and partner availability
remain checked; nested pair positioning for later input indices and broader
contextual semantics remain separate boundaries. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded five-input contextual GPOS positioning (2026-10-19)

Contextual GPOS format-3 subtables with five input coverages now apply
bounded nested type-1 single adjustments to sequence index four, including
saturating advance propagation to following glyphs. One- through four-input
paths remain supported; nested pair positioning remains limited to the
two-input sequence-index-zero path, while longer context and broader lookup
semantics remain separate boundaries. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded four-input contextual GPOS positioning (2026-10-19)

Contextual GPOS format-3 subtables with four input coverages now apply
bounded nested type-1 single adjustments to sequence index three, including
saturating advance propagation to following glyphs. One-, two- and three-input
paths remain supported; nested pair positioning remains limited to the
two-input sequence-index-zero path, while longer context and broader lookup
semantics remain separate boundaries. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded three-input contextual GPOS positioning (2026-10-19)

Contextual GPOS format-3 subtables with three input coverages now apply
bounded nested type-1 single adjustments to sequence index two, including
saturating advance propagation to following glyphs. One- and two-input paths
remain supported; nested pair positioning remains limited to the two-input
sequence-index-zero path, and longer context/broader lookup semantics remain
separate boundaries. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded single-input contextual GPOS positioning (2026-10-19)

Contextual GPOS format-3 subtables with one input coverage now apply bounded
nested type-1 single adjustments through the shaped-glyph pass, including
saturating advance propagation and the existing adjacent backtrack/lookahead
guards. Two-input contextual positioning remains separately bounded; nested
pair positioning and broader contextual semantics remain separate boundaries.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Hardware GPU acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS sequence-index-1 positioning (2026-10-19)

Contextual GPOS format-3 records targeting sequence index one now apply
bounded nested type-1 single adjustments to the second input glyph, including
saturating advance propagation to following glyphs. Nested type-2 pair
positioning remains limited to sequence index zero in this path; indices beyond
one and broader contextual dispatch remain separate boundaries. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU
acceleration remains a separate boundary.

## Graphics track - bounded contextual GPOS multi-record composition (2026-10-19)

Contextual GPOS format-3 records targeting sequence index zero now compose
multiple nested type-1 single or type-2 format-1 pair adjustments with
saturating placement/advance accumulation. Nested lookup/table bounds remain
validated; sequence indices other than zero and broader contextual dispatch
remain separate boundaries. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains a separate
boundary.

## Graphics track - bounded contextual GPOS context guards (2026-10-19)

The contextual GPOS format-3 path now validates and matches up to one
backtrack and one lookahead coverage using adjacent shaped glyphs when
available. Context-free runs remain supported, and only sequence index zero
is dispatched to the nested single/pair lookup path. Longer context,
multi-record dispatch and broader lookup ordering remain open. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU
acceleration remains a separate boundary.

## Graphics track - nested contextual GPOS pair adjustment (2026-10-19)

The bounded format-3 contextual GPOS path now accepts a nested type-2
format-1 pair lookup in addition to nested type-1 single positioning. Two
input coverages, sequence index zero, pair-set bounds and optional extension
wrappers are validated. Broader contextual records, backtrack/lookahead and
multi-record composition remain open. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded contextual GPOS single adjustment (2026-10-19)

GPOS lookup type 7 format 3 now matches two input coverages with no backtrack
or lookahead and applies a nested type-1 format-1 single adjustment to the
first input glyph; nested type-9 wrappers are accepted. Coverage, lookup
record, sequence-index and table bounds are checked. Contextual pair/nested
multi-record semantics remain open. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration
remains a separate boundary.

## Graphics track - bounded GPOS mark-to-ligature attachment (2026-10-19)

GPOS lookup type 5 format 1 now resolves mark classes, ligature coverage,
component records and anchors, including type-9 wrappers and script/LangSys
selection. Component and anchor bounds are checked. The current shaped-glyph
ABI has no explicit ligature-component index for a following mark, so this
bounded path selects the final ligature component deterministically. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Contextual GPOS lookup types and hardware GPU acceleration remain separate
boundaries.

## Graphics track - GPOS attachment extension wrappers (2026-10-19)

Bounded type-9 GPOS extension records now unwrap mark-to-base, mark-to-mark
and cursive attachment lookups when their nested lookup type matches. Wrapper
offsets, extension formats and nested table bounds are validated; mismatched
wrappers fail closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Broader GPOS lookup/type semantics and
hardware GPU acceleration remain separate boundaries.

## Graphics track - script/LangSys-scoped GPOS attachments (2026-10-19)

Mark-to-base, mark-to-mark and cursive attachment now use the selected
script/LangSys feature records when shaping options provide them. Missing
scripts or languages fail closed; default shaping retains bounded all-feature
behavior. The 42-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Broader GPOS lookup/type semantics and hardware GPU acceleration
remain separate boundaries.

## Graphics track - bounded GPOS lookup-order composition (2026-10-19)

Selected `kern` GPOS single and pair adjustments now compose in lookup order
with saturating accumulation, while query-kind filtering, table bounds and
malformed records remain fail-closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Broader GPOS feature/type
coverage and hardware GPU acceleration remain separate boundaries.

## Graphics track - outline-resolved GPOS format-2 anchors (2026-10-19)

GPOS format-2 anchors now resolve their referenced contour point through the
bounded glyph outline decoder for mark-to-base, mark-to-mark and cursive
attachment. Point indices, glyph outlines and table offsets are validated;
malformed or unavailable outlines fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Broader GPOS lookup ordering
and hardware GPU acceleration remain separate boundaries.

## Graphics track - format-3 attachment anchors across GPOS (2026-10-19)

The checked format-3 anchor decoder with optional ppem device deltas now feeds
mark-to-base, mark-to-mark and cursive attachment paths. Format-1 and
outline-resolved format-2 anchors remain supported across those paths. The
42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Broader GPOS lookup ordering
and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GPOS format-3 anchors (2026-10-19)

GPOS mark-to-base attachment now accepts format-3 anchors and applies their
bounded x/y device-table deltas when ppem is supplied, reusing the checked
device-delta decoder. Format-1 and outline-resolved format-2 anchors remain
supported. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Broader GPOS lookup
ordering and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GPOS mark-to-mark attachment (2026-10-19)

GPOS lookup type 6 format 1 now supports bounded `mkmk` mark-to-mark
attachment. Mark1/mark2 coverage, class arrays and format-1 anchors are
validated; the second mark receives the first-anchor-minus-second-anchor delta
without changing advances. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Alternate anchor formats, broader GPOS lookup
ordering and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GPOS cursive attachment (2026-10-19)

GPOS lookup type 3 format 1 now supports bounded cursive entry/exit anchors
for `curs` features. Adjacent glyphs are matched through checked coverage and
entry/exit records; the resulting connection delta adjusts the right glyph
origin without changing advances. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Mark-to-mark attachment,
alternate anchor formats, broader GPOS lookup ordering and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded GPOS mark-to-base anchors (2026-10-19)

GPOS lookup type 4 format 1 now supports bounded mark-to-base attachment for
`mark` and `mkmk` feature records. Mark/base coverage, class arrays and format
1 anchors are validated; mark origins are adjusted without changing advances.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Mark-to-mark, cursive attachment, alternate anchor formats,
broader GPOS lookup ordering and hardware GPU acceleration remain separate
boundaries.

## Graphics track - bounded GPOS single adjustments (2026-10-19)

GPOS lookup type 1 now supports bounded single-adjustment formats 1 and 2,
including placement, advance and existing ppem device-table deltas. Single
adjustments are applied to every shaped glyph, including the final glyph, and
advance changes propagate to following origins. Value formats, coverage,
value-record arrays and table bounds are checked. The 42-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance passed on 1/2/4 CPUs. Mark/cursive/
attachment positioning, broader GPOS lookup ordering and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded GPOS pair extensions (2026-10-19)

GPOS lookup type 9 now unwraps bounded extension records for nested pair
positioning lookup type 2, including format 1 and format 2 value records and
existing ppem device deltas. Extension offsets, nested formats and pair-table
bounds are checked; unsupported extension forms fail closed. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Broader
GPOS lookup ordering, additional positioning lookup types, and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded GSUB extension class lookups (2026-10-19)

GSUB lookup type 7 now unwraps extension records for class-based contextual
and chaining substitutions (lookup types 5 and 6, format 2), reusing the
bounded class-definition and nested single-lookup matchers. Extension offsets,
class sets, rules, lookup references and glyph bounds remain fail-closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Complete OpenType lookup ordering and hardware GPU acceleration remain
separate boundaries.

## Graphics track - bounded GSUB extension contextual/chaining format 3 (2026-10-19)

GSUB lookup type 7 now unwraps extension records for bounded format-3
contextual and chaining substitutions in addition to single and ligature
lookups. Backtrack, input, lookahead coverage and nested lookup bounds reuse
the existing fail-closed matcher. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Extension-wrapped
class-based contextual/chaining forms, complete OpenType lookup ordering, and
hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GSUB extension ligatures (2026-10-19)

GSUB lookup type 7 now unwraps bounded extension records for both single and
ligature substitutions. Extension-wrapped ligatures preserve component
consumption in the eight-pass shaping pipeline; extension offsets, formats,
coverage, component and glyph bounds are checked, while other extension lookup
types fail closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Extension-wrapped contextual and chaining
lookups, complete OpenType lookup ordering, and hardware GPU acceleration
remain separate boundaries.

## Graphics track - bounded GSUB extension lookups (2026-10-19)

GSUB lookup type 7 now unwraps bounded extension records for single
substitution lookups, allowing extension-wrapped format 1/2 substitutions to
participate in the existing ordered shaping pipeline. Extension offsets,
formats and nested coverage/substitute bounds are checked; unsupported
extension lookup types fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Extension-wrapped
contextual, chaining and ligature lookups, complete OpenType lookup ordering,
and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GSUB feature-order composition (2026-10-19)

GSUB shaping now applies selected lookup results in a bounded eight-pass
pipeline, allowing ordered single substitutions and contextual/chaining
results to compose instead of stopping after the first applicable lookup.
Ligature consumption still terminates the local pipeline, and repeated
no-op/cyclic substitutions are bounded. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Full OpenType lookup
ordering across all lookup types, broader nested composition and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded class-based GSUB chaining format 2 (2026-10-19)

Added GSUB lookup type 6 format 2 class-based chaining. Backtrack, current
input and lookahead class definitions are matched against bounded chain rules;
a matching record can invoke a nested type-1 substitution at the current
glyph. Class-set, rule, sequence, lookup and glyph bounds are checked, with
unsupported nested forms failing closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Broader nested lookup
composition, complete feature ordering and hardware GPU acceleration remain
separate boundaries.

## Graphics track - bounded GSUB chaining format 3 (2026-10-19)

Added GSUB lookup type 6 format 3 chaining-context matching over bounded
backtrack, input and lookahead coverage sequences. Matching records may invoke
a nested type-1 substitution at the current glyph; lookup, sequence, coverage
and glyph bounds remain checked and unsupported nested forms fail closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Class-based chaining formats 1/2, broader nested lookup composition, complete
feature ordering and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded class-based contextual GSUB format 2 (2026-10-19)

Added GSUB lookup type 5 format 2 class-based contextual matching. Bounded
class definitions select rule sets, subsequent input classes are matched, and
a matching rule can invoke a nested type-1 substitution at the current glyph.
Class-set, rule, nested-lookup and glyph bounds are checked; unsupported
chaining and nested forms remain fail-closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Chaining substitution,
full feature ordering and hardware GPU acceleration remain separate
boundaries.

## Graphics track - bounded contextual GSUB format 3 (2026-10-19)

Added bounded GSUB contextual lookup type 5 format 3 matching for backtrack,
input and lookahead coverage sequences. A matching substitution record can
apply a nested type-1 single substitution to the current glyph; coverage,
sequence, lookup and glyph bounds are checked, while unsupported nested lookup
forms remain fail-closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Class-based/contextual format 1/2, chaining
substitution, full feature ordering and hardware GPU acceleration remain
separate boundaries.

## Graphics track - bounded GSUB single substitutions (2026-10-19)

Added bounded GSUB lookup type 1 single substitutions in formats 1 and 2 for
the selected Script/LangSys feature set, covering common features including
`ccmp`, `locl`, `smcp`, `subs`, `sups`, `case` and `cpsp`. Coverage,
substitute-array and glyph-range bounds are checked; unsupported lookup forms
fail closed. The 42-test `libwoven` suite, warning-denying library/kernel/
workspace Clippy, formatting, and Stage 19.2 compositor visual acceptance
passed on 1/2/4 CPUs. Contextual/chaining GSUB, full feature ordering and
hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded named LangSys selection (2026-10-19)

Added checked named language-system selection within a requested OpenType
script. `TextShapingOptions` makes script tag, language tag and optional ppem
inputs explicit; GSUB `liga`/`rlig` and GPOS `kern` now use the selected
LangSys, while an absent language tag selects no alternate feature set. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Contextual/complex GSUB/GPOS, broader script data and hardware GPU acceleration
remain separate boundaries.

## Graphics track - bounded script-scoped GPOS (2026-10-19)

Extended script-aware shaping so the requested ScriptList/default LangSys
scopes both GSUB `liga`/`rlig` substitutions and GPOS `kern` pair positioning.
`shape_text_ligatures_with_script_and_ppem` makes the combined script and
ppem inputs explicit; missing scripts select no alternate script, and all
script, feature, lookup and device-table bounds remain checked. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Named
language-system selection, contextual/complex GSUB/GPOS and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded GSUB script selection (2026-10-19)

Added `shape_text_ligatures_with_script`, which resolves a requested bounded
OpenType ScriptList record, its default language system, and the enabled
`liga`/`rlig` features before applying type-1 ligature substitutions. Script,
language-system, feature-index and lookup bounds are checked; the existing
legacy shaping path remains available, and missing requested scripts produce
no substitution rather than silently selecting another script. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. GPOS
script-scoped selection, language-specific systems, contextual/complex GSUB,
and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GPOS device-table deltas (2026-10-18)

Added bounded ppem-aware GPOS device-table decoding for formats 1, 2 and 3.
Device deltas are applied to the matching x/y placement or advance field and
then scaled through the existing shaped-glyph origin path. The legacy shaping
entry point fails closed when a device record requires ppem data; callers that
have an explicit ppem use `shape_text_ligatures_with_ppem`. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Complete
script shaping, variation-index device semantics and hardware GPU acceleration
remain separate boundaries.

## Graphics track - bounded GPOS y-value positioning (2026-10-18)

Extended GPOS pair adjustments to carry y-placement and y-advance fields in
addition to x-placement/x-advance. Values are scaled from font units, applied
to shaped glyph origins and propagated to following origins; all value-record
and table bounds remain checked. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Device-table deltas, complete script shaping
and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded GPOS class pair positioning (2026-10-18)

Extended the optional GPOS `kern` path to pair-positioning format 2 with
bounded class-definition formats 1/2 and class-matrix lookup. Class records
apply scaled x-placement/x-advance adjustments to shaped runs; class counts,
matrix extents and table bounds are checked. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Full GPOS value/device
semantics, script shaping and hardware GPU acceleration remain separate
boundaries.

## Graphics track - bounded OpenType GPOS pair positioning (2026-10-18)

Added optional GPOS `kern` feature handling for type-2 pair-positioning format
1. The shaped run now applies checked x-placement and x-advance adjustments in
font units, scales them to pixels, and shifts following origins; unsupported
formats and malformed records fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Full GPOS value/device
formats, script shaping and hardware GPU acceleration remain separate
boundaries.

## Graphics track - variation-aware shaped rasterization (2026-10-18)

Extended direct glyph-ID shaping/rasterization with normalized variation
coordinates: `advance_width_glyph_with_variation`, variation-aware outline and
compound paths, and `render_shaped_text_with_variation` now carry bounded gvar
metrics and outlines through GSUB-selected glyphs. Base rendering remains
separate from gvar application so deltas are applied exactly once. The
41-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Full script shaping and hardware GPU acceleration remain separate boundaries.

## Graphics track - shaped glyph-ID raster integration (2026-10-18)

Added direct glyph-ID outline and raster entry points plus
`TrueTypeFace::render_shaped_text`, preserving GSUB-selected glyph IDs through
the antialiased path instead of reducing them back to Unicode characters.
Glyph bounds, outline storage and renderer capacity remain checked. The
41-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Variation-aware shaped rasterization, full script shaping and hardware GPU
acceleration remain separate boundaries.

## Graphics track - bounded OpenType GSUB ligatures (2026-10-18)

Added `ShapedGlyph` and `TrueTypeFace::shape_text_ligatures`, parsing bounded
GSUB type-1 ligature substitutions linked by `liga`/`rlig` features. Coverage,
lookup, component-count, source-span, metric and table-end limits are checked;
fonts without those features use one glyph per character and malformed shaping
data fails closed. The 41-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Arbitrary shaped-glyph raster integration,
full script shaping and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded directional text runs (2026-10-18)

Added `TextDirection::classify` and `TrueTypeFace::layout_text_bidi` for
bounded strong RTL/LTR classification, neutral inheritance, and visual run
ordering over caller-owned placements. The API explicitly does not claim the
full Unicode Bidirectional Algorithm or script shaping. The 40-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance passed on 1/2/4 CPUs. Full UAX#9 behavior,
OpenType shaping and hardware GPU acceleration remain separate boundaries.

## Graphics track - bounded hint control flow (2026-10-18)

Added instruction-span-aware `IF`/`ELSE`/`EIF` selection and bounded relative
`JMPR`/`JROT`/`JROF` branches to the TrueType hint subset. Branch scanning
skips PUSH payloads, validates targets and nesting, and retains the 1024-step
execution cap; malformed or unsupported control flow fails closed. The
40-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Full projection math, hint interpolation/function calls, shaping and bidi
remain separate boundaries.

## Graphics track - bounded hint storage/CVT resources (2026-10-18)

Added `execute_glyph_hints_with_resources` with checked caller-owned storage
for `WS`/`RS` and CVT-backed `MIAP`/`MDAP` execution. The no-storage wrapper
continues to fail closed when a program requests storage. The 40-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. Full
projection math, hint interpolation/control flow, shaping and bidi remain
separate boundaries.

## Graphics track - bounded hint point/CVT execution (2026-10-18)

Extended `execute_glyph_hints_with_cvt` with checked `MDAP` point selection
and `MIAP` CVT placement along the active freedom axis. A synthetic
`PUSHB/PUSHB/MIAP` program moves a glyph point to a caller-owned CVT value;
the no-CVT wrapper fails closed. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Full projection math, storage/control flow,
hint interpolation, shaping and bidi remain separate boundaries.

## Graphics track - bounded hint-state execution (2026-10-18)

Added `HintState` and `TrueTypeFace::execute_glyph_hints` for a bounded
coordinate-independent TrueType instruction subset: vector-axis selection,
rounding-state changes, stack primitives and PUSHB/PUSHW forms. Execution is
limited to 1024 steps and a 256-entry stack; point movement, storage, CVT and
control-flow instructions fail closed. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Full hinting, shaping,
bidi and GPU acceleration remain separate boundaries.

## Graphics track - bounded TrueType hint-program boundary (2026-10-18)

Added `TrueTypeFace::glyph_hint_program`, which validates and copies simple
glyph instruction bytes into caller-owned storage with checked glyph bounds,
instruction lengths and a 256-byte cap. Composite programs and instruction
execution fail closed intentionally; the rasterizer remains deterministic and
does not claim hinting yet. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. GPU output retains the QEMU/VirGL capability
boundary.

## Graphics track - gvar phantom-point metrics (2026-10-18)

Added `TrueTypeFace::advance_width_with_variation`, retaining the four phantom
points internally while applying simple-glyph `gvar` deltas, then deriving the
varied advance from the right/left phantom pair. Full-strength and half-scale
synthetic metrics are covered. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Missing legacy LSB records preserve the
existing base advance API and fail closed for variation metrics; hinting,
shaping, bidi and GPU acceleration remain separate boundaries.

## Graphics track - composite gvar component variation (2026-10-18)

Extended variation rendering to composite glyphs using component-index
variation deltas: nested components are decoded recursively, component output
ranges are tracked, and each component’s bounded X/Y placement delta is applied
before returning the flattened outline. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. This follows the OpenType
composite rule that packed variation point numbers identify components rather
than simple-outline points; phantom metrics, hint execution, shaping and bidi
remain separate boundaries. GPU output retains the QEMU/VirGL capability
boundary.

## Graphics track - shared-point gvar tuples (2026-10-18)

Added glyph-level packed shared-point handling for `gvar` tuples, including
the valid all-points form, with checked run expansion, point limits and tuple
flag validation. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Composite glyph variation, hint execution,
shaping and bidi remain explicit follow-on boundaries; GPU output retains the
QEMU/VirGL capability boundary.

## Graphics track - intermediate gvar regions (2026-10-18)

Extended simple-glyph `gvar` application to embedded-peak intermediate
regions, with checked start/peak/end ordering and piecewise scalar blending.
The synthetic outline fixture verifies full-strength and half-strength tuple
application. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Shared-point sets, composite variation,
hinting, shaping and bidi remain explicit follow-on boundaries; GPU output
retains the QEMU/VirGL capability boundary.

## Graphics track - simple gvar tuple application (2026-10-18)

Added `TrueTypeFace::outline_points_with_variation`, applying bounded embedded
peak `gvar` tuples to simple glyph outlines at normalized F2DOT14 coordinates.
Packed delta runs, tuple scalars, point bounds and coordinate cardinality are
checked; composite glyphs and shared-point sets fail closed until their
dedicated support exists. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hint execution, complex
shaping, bidi and GPU acceleration remain separate; GPU output retains the
QEMU/VirGL capability boundary.

## Graphics track - bounded gvar payload boundary (2026-10-18)

Added `TrueTypeFace::glyph_variation_data`, validating the `gvar` version,
axis/shared-tuple metadata, glyph-count match, offset array width, table
extents and per-glyph slice before returning a borrowed payload. A synthetic
fixture covers empty and non-empty glyph data. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Tuple delta application,
hint execution, shaping and bidi remain separate slices; GPU output retains
the QEMU/VirGL capability boundary.

## Graphics track - accepted fvar axis fixture (2026-10-18)

The bounded `fvar` metadata API now has a positive synthetic-font fixture
covering the `wght` tag and checked 16.16 minimum/default/maximum values.
The 40-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. This remains metadata-only: `gvar` deltas, hint execution,
shaping and bidi are outside this slice, and GPU output retains the QEMU/VirGL
capability boundary.

## Graphics track - bounded variable-font axis metadata (2026-10-18)

Added `VariationAxis` and `TrueTypeFace::variation_axes` for checked `fvar`
axis records and 16.16 min/default/max metadata. Missing tables return an
empty axis set; truncated records and output overflow fail closed. This slice
does not apply `gvar` deltas, so rendering remains on the base outline until a
variation backend exists. The 39-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs; GPU output retains the documented QEMU/VirGL
capability boundary.

## Graphics track - explicit RTL placement (2026-10-18)

Added `TextDirection` and `TrueTypeFace::layout_text_direction`, providing
deterministic bounded LTR/RTL placement from validated advances. RTL mirrors
the produced placements but intentionally does not claim Unicode bidi
resolution, shaping, or script behavior. The 38-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. GPU output remains bounded
by the documented QEMU/VirGL capability boundary.

## Graphics track - Unicode format-12 cmap (2026-10-18)

`TrueTypeFace` now accepts optional cmap format 12 groups and resolves
non-BMP Unicode code points before falling back to the validated BMP format-4
path. Group bounds, glyph IDs and table extents are checked; malformed data
fails closed. A synthetic non-BMP fixture verifies mapping and advance lookup.
The 38-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Full script shaping, bidi and variation tables remain open; GPU output retains
the documented QEMU/VirGL capability boundary.

## Graphics track - TrueType point-matched composites (2026-10-18)

Composite glyph decoding now supports both XY-offset and point-matching
component arguments, applying component transforms before anchoring to an
already-emitted parent point. Invalid parent/component indices fail closed;
recursion and output remain bounded. The synthetic mapped glyph now contains
both component forms. The 37-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Variation deltas, hint execution, modern
shaping and scripts remain open; GPU output retains the documented QEMU/VirGL
capability boundary.

## Graphics track - bounded TrueType composite glyphs (2026-10-18)

The TrueType outline decoder now supports bounded composite glyph components
with XY offsets, uniform/non-uniform/two-by-two transforms, recursive depth
limits, instruction skipping and caller-buffer capacity checks. The synthetic
mapped glyph fixture is now composite and renders through the antialiased path.
Point-matching composite arguments, variation deltas and hint execution remain
unsupported and fail closed. The 37-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs; GPU output remains bounded by the documented
QEMU/VirGL device capability.

## Graphics track - bounded TrueType word wrapping (2026-10-18)

Added `TrueTypeFace::layout_text_word_box`, preserving whole words when
wrapping at spaces while retaining explicit newline handling, checked advances
and optional kerning. Words wider than the box remain intact; capacity and
malformed/unsupported input fail closed. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Script shaping, bidi,
hinting and compound glyph components remain open; GPU output retains the
documented QEMU/VirGL capability boundary.

## Graphics track - TrueType bitmap fallback (2026-10-18)

Added `TrueTypeFace::render_text_with_bitmap_fallback`: mapped glyphs use the
validated antialiased outline pipeline, while unmapped or unsupported glyphs
use the established bounded shared bitmap font. Mixed outline/fallback output
is covered by the synthetic face fixture. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. This remains single-line;
modern shaping, bidi, hinting and compound glyph components remain open, and
GPU output retains the documented QEMU/VirGL capability boundary.

## Graphics track - bounded TrueType line breaking (2026-10-18)

Added `TrueTypeFace::layout_text_box` for caller-owned single-line placements
with explicit newline handling, width wrapping, line height, checked advances
and optional kerning. Invalid dimensions, unsupported glyphs and capacity
overflow fail closed. The 37-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Script shaping, bidi, word-level breaking,
hinting and fallback remain open; GPU presentation remains bounded by the
documented QEMU/VirGL device capability.

## Graphics track - optional TrueType kerning (2026-10-18)

`TrueTypeFace` now recognizes optional legacy TrueType `kern` format-0
horizontal subtables and applies checked pair adjustments during bounded
single-line layout. Fonts without a kerning table preserve prior placement;
malformed optional data fails closed. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Modern shaping, script
support, line breaking and fallback remain open, and GPU presentation remains
bounded by the documented QEMU/VirGL device capability.

## Graphics track - bounded TrueType text layout (2026-10-18)

Added `GlyphPlacement`, `TrueTypeFace::layout_text`, and `render_text` for
allocation-free single-line placement and antialiased glyph rendering from
validated `hmtx` advances. Unsupported glyphs, newlines and output overflow
fail closed without partial layout. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Kerning, script shaping,
line breaking, hinting and fallback remain explicit follow-on work; GPU output
is still bounded by the documented QEMU/VirGL device capability.

## Graphics track - TrueType glyph raster entry point (2026-10-18)

`TrueTypeFace::render_glyph` now connects validated simple-glyph contour
assembly to the integer-antialiased `CompoundPath` renderer. A synthetic face
fixture proves a mapped glyph reaches a caller-owned Canvas. The 37-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs. This remains
a glyph raster entry point rather than a shaping engine: compound components,
kerning, scripts, hinting and font fallback remain open; GPU presentation is
still bounded by the documented QEMU/VirGL device capability.

## Graphics track - bounded antialiased fills (2026-10-18)

Added deterministic 4x4 supersampled coverage for `Path` and `CompoundPath`
fills, including source-over opacity composition and even-odd holes. The
implementation uses integer arithmetic, clips to the Canvas, and keeps work
bounded by the raster bounds and 16 samples per pixel; `hello-gui` exercises
the antialiased badge. The 37-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Hardware VirGL presentation remains subject
to the documented QEMU/device capability boundary.

## Graphics track - bounded multi-contour paths (2026-10-18)

Added `CompoundPath` with eight bounded closed contours, even-odd filling,
compound stroking, and `TrueTypeFace::outline_compound` for assembling all
simple contours of a glyph. Nested contours render as holes; overflow and
compound TrueType components fail closed. The 36-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Hardware GPU presentation
remains separately bounded by the previously documented QEMU VirGL/device
capability boundary.

## Graphics track - TrueType contour assembly (2026-10-18)

`TrueTypeFace::outline_contour` now assembles one decoded simple-glyph contour
into a bounded closed `Path`, mapping font units from a baseline origin and
handling on-curve/off-curve transitions plus implied midpoints through the
quadratic primitive. Path construction is transactional on capacity failure.
The 35-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
The API intentionally selects one contour because multi-contour fill/hole
semantics are not yet part of `Path`; compound glyphs, shaping and antialiasing
remain open.

## Stage 21.4 WovenStore dependency-ordered batches (2026-10-20)

WovenStore transactions now accept bounded package batches in arbitrary input
order and resolve dependencies through repeated bounded passes. Missing or
cyclic dependencies fail without committing a partial catalog, while the
existing capability policy, monotonic update, dependent-removal, and explicit
rollback rules remain active. Host tests, warning-denying Clippy, and QEMU
acceptance passed on 1/2/4 CPUs. Evidence is recorded in
`docs/audit-stages-consolidated.md`.

Persistent WovenFS publication, power-loss recovery, service/IPC authority,
and physical USB qualification remain open.

## Stage 21.5 durable WovenStore record codec (2026-10-20)

WovenStore now defines a bounded checksummed `WSTC` v1 catalog record codec.
It round-trips capability policy, package versions, application digests, and
dependency metadata, and rejects corruption, malformed lengths, invalid names,
duplicates, and invalid dependency fields. Host tests, warning-denying Clippy,
and QEMU acceptance passed on 1/2/4 CPUs. Evidence is recorded in
`docs/audit-stages-consolidated.md`.

The record is not yet wired to native WovenFS publication; crash/power-loss
service integration, IPC authority, and physical USB qualification remain open.

## Stage 21.6 native WovenFS catalog persistence (2026-10-20)

The package catalog record is now persisted through the capability-gated native
WovenFS writer at `/wfs/packages/catalog.wstc`, read back from the mounted
volume, and decoded after journaled commit. The Stage 21.6 disposable ATA QEMU
gate passed on 1/2/4 CPUs. Evidence is under `audit-artifacts/stage21.6-{1,2,4}cpu`;
host/freestanding warning-denying Clippy also passed.

This does not yet publish package payload/resource files atomically with the
catalog, recover interrupted install/update/remove transactions, or provide a
long-lived service/IPC boundary. Physical USB qualification remains open.

## Stage 21.7 authenticated WovenPackage payload publication (2026-10-20)

The kernel package service now publishes authenticated WPKG application and
resource payloads as bounded, versioned WovenFS chunks and commits the WSTC
catalog only after the chunks are durable. A failed chunk/catalog write cleans
up chunks created by that attempt. The disposable ATA QEMU gate passed on
1/2/4 CPUs; host/freestanding warning-denying Clippy passed. Evidence is
recorded in `docs/audit-stages-consolidated.md`.

This is not yet a complete package manager: multi-package crash recovery,
persistent update/removal transactions, package-service userspace IPC, and
physical USB qualification remain open.

## Stage 21.8 WovenPackage install-intent recovery (2026-10-20)

The package service now persists a checksummed `WPKJ` prepared/committed intent
around payload publication. Recovery reconciles the intent with the WSTC
catalog and removes orphaned application/resource chunks from interrupted
prepared installs. The disposable ATA QEMU recovery gate passed on 1/2/4 CPUs;
host/freestanding warning-denying Clippy passed. Evidence is recorded in
`docs/audit-stages-consolidated.md`.

Multi-package recovery, persistent update/removal transactions, full power-loss
injection, userspace package-service IPC, and physical USB qualification remain
open.

## Stage 21.9 WovenPackage update/removal transactions (2026-10-20)

WSTC v2 now records bounded application/resource chunk counts. WPKJ records
both the incoming and superseded generation. Package removal uses a
catalog-first journaled commit while the prior generation remains available
for the bounded rollback window. The Stage 21.9 disposable ATA QEMU gate
passed on 1/2/4 CPUs, and the
Stage 21.6/21.7/21.8 regression gates passed again on 1/2/4 CPUs after the ABI
and journal changes. Evidence is under timestamped `audit-artifacts`
directories. Host and affected freestanding warning-denying Clippy passed.

Multi-package recovery, power-loss injection at every boundary, sandbox
enforcement, userspace package-service IPC/ownership, and physical USB
qualification remain open.

## Stage 21.10 WovenPackage bounded rollback (2026-10-20)

The package service now persists one previous WSTC catalog generation before
each install or removal. It retains the corresponding payload generation,
records rollback intent in a checksummed WRBK record, restores the catalog and
payload authority idempotently, and retires only generations no longer active
or rollback-capable. The Stage 21.10 disposable ATA QEMU gate passed on 1/2/4
CPUs; host tests and warning-denying host/freestanding Clippy passed.

Multi-package rollback, power-loss injection at every boundary, sandbox
enforcement, userspace package-service IPC/ownership, and physical USB
qualification remain open.

## Stage 21.11 WovenPackage sandbox launch boundary (2026-10-20)

Package capability declarations now produce an immutable WovenGuard user
profile before launch. Declarations outside the user-domain ceiling are
rejected; file access is narrowed to system/temporary/mounted/home scopes,
device policy follows declared device capabilities, and service discovery is
limited to application-class services. The profile-aware spawn path destroys
invalid images before address-space publication. The Stage 21.11 disposable
ATA QEMU gate passed on 1/2/4 CPUs; host and freestanding warning-denying
Clippy plus the 63-test `libwoven` suite passed.

This closes the package launch-policy boundary but not a complete userspace
WovenPackage IPC broker, multi-package power-loss recovery, or physical USB
qualification.

## Graphics track - bounded quadratic paths (2026-10-18)

`Path::quadratic_to` now flattens quadratic Bézier segments into four
deterministic line segments using bounded fixed-point integer arithmetic.
Capacity checks are atomic, and `hello-gui` exercises the curve in the visual
frame. The 35-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4
CPUs. This prepares curve geometry for TrueType contours; curve-aware font
contour assembly and antialiased rasterization remain open.

## Graphics track - TrueType simple outlines (2026-10-18)

`TrueTypeFace` now validates optional `glyf`/`loca` storage and decodes
bounded simple-glyph point streams into caller-owned `OutlinePoint` buffers.
Contour endpoints, repeated flags, delta streams and short/long loca formats
are covered; compound, oversized and malformed glyphs fail closed. The
34-test `libwoven` suite, warning-denying Clippy, formatting, and Stage 19.2
visual acceptance passed on 1/2/4 CPUs. Curve expansion, rasterization,
antialiasing and shaping remain open.

## Graphics track — TrueType horizontal metrics (2026-10-18)

The validated `TrueTypeFace` boundary now reads `hhea`/`hmtx` horizontal
metrics and returns checked advance widths for mapped glyphs, including the
TrueType final-metric rule for glyphs beyond the long-metrics array. The
33-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Outline extraction and rasterization remain open.

## Graphics track — TrueType face boundary (2026-10-18)

`libwoven::graphics::TrueTypeFace` now validates a borrowed TrueType sfnt
header, bounded `head`/`maxp`/`cmap` tables, and format-4 BMP cmap mappings
without allocation or unsafe code. Invalid signatures, truncated directories,
missing tables and unsupported cmap formats fail closed; the existing shared
bitmap font remains the rendering fallback. The 33-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance passed on 1/2/4 CPUs. Outline extraction,
antialiasing and complex shaping remain subsequent font slices because no
font asset is bundled in the repository.

## Graphics track — alpha image blits (2026-10-18)

`Renderer::image_blended` now provides bounded row-major image uploads with
uniform source-over alpha, using the same clipped Canvas blend path as panels
and polygons. `hello-gui` exercises the alpha-aware icon upload. The 32-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.

## Graphics track — rounded alpha panels (2026-10-18)

`Renderer::rounded_rect` and `rounded_rect_blended` now provide bounded
rounded-panel/button fills with Canvas clipping and the shared source-over
alpha formula. `hello-gui` exercises a translucent rounded panel in its
visible frame. The 32-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs.

## Graphics track — bounded text layout (2026-10-18)

`Renderer::text_box` now wraps UTF-8 text at the available width and clips it
to a bounded rectangle, using the shared glyph provider, replacement fallback,
and integer scaling. `hello-gui` exercises the text-box path in its visible
frame. The 31-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance passed on
1/2/4 CPUs. Word shaping and outline fonts remain separate font-engine work.

## Graphics track — alpha-blended path fills (2026-10-18)

Closed paths now support uniform source-over alpha through
`Renderer::fill_path_blended`, sharing the Canvas blend formula used by the
existing rectangle/compositor paths. `hello-gui` exercises a translucent
filled polygon. The 30-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs.

## Graphics track — horizontal gradients (2026-10-18)

`Renderer::horizontal_gradient` now complements the existing clipped vertical
gradient, with deterministic endpoint interpolation and Canvas clipping.
`hello-gui` exercises both directions in its visible frame. The 29-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.

## Graphics track — bounded path filling (2026-10-18)

Closed `Path` geometry now supports clipped even-odd filling through
`Renderer::fill_path`, with iteration restricted to the Canvas bounds so large
coordinates remain bounded. `hello-gui` exercises a filled polygon alongside
the existing stroked path. The 28-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs.

## Graphics track — bounded paths (2026-10-18)

`libwoven::graphics` now provides allocation-free bounded `Path` geometry with
move, line and close operations, plus a clipped integer line rasterizer through
`Renderer::stroke_path`. `hello-gui` exercises an open chevron path in its
visible frame. The 27-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance passed on 1/2/4 CPUs. Path filling and richer outline/font work
remain subsequent increments.

## Graphics track — integer font scaling (2026-10-18)

The shared-glyph `Font` provider now supports bounded integer scaling from 1×
through 16×. `Renderer::text_scaled` and `text_centered_scaled` rasterize
nearest-neighbour glyph blocks through the same clipped Canvas path, preserving
UTF-8 replacement fallback and deterministic layout. `hello-gui` exercises a
scaled glyph in its visible frame.

The 26-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
TrueType/OpenType outlines, hinting and shaping remain future font-engine
increments.

## Graphics track — shared font metrics (2026-10-18)

`libwoven::graphics` now exposes a `Font` provider and `TextMetrics` backed by
the shared kernel glyph table. Multiline UTF-8 text is measured with bounded
replacement-glyph fallback, and `Renderer::text_centered` uses those metrics
while retaining Canvas clipping. `hello-gui` exercises centered text in its
visible WovenGraphics frame.

The 25-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
Full TrueType/OpenType shaping and richer font fallback remain the next font
engine increments; this slice preserves pixel identity with the compositor.

## Graphics track — fixed-point transforms (2026-10-18)

`libwoven::graphics` now provides a deterministic signed 16.16 `Transform`
and `Renderer::transformed_rect`, which fills transformed rectangles as clipped
triangles through the existing caller-owned `Canvas`. This adds scene-level
translation, scaling and affine shear without changing the surface ABI or
introducing unsafe code. `hello-gui` exercises the transformed primitive in
its visible WovenGraphics frame.

The 24-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance passed on 1/2/4 CPUs.
The accepted software fallback remains the display path; VirGL blob-backed
presentation is still an explicit QEMU capability boundary documented below.

## Graphics track — WovenGraphics abstraction and visual acceptance (2026-10-18)

The first WovenGraphics boundary is now present in `libwoven::graphics`.
Typed `Color` and clipped `Rect` geometry are format-independent, while
`Renderer` draws through the existing safe software `Canvas`. The renderer
does not own the framebuffer or bypass compositor authority; applications
upload its bounded pixel output through the existing `GraphicsSurface` ABI.

`hello-gui` now routes a visible title/content frame through this abstraction
and emits `[S16G] WovenGraphics visual frame: PASSED`. The Stage 19.2 QEMU
visual harness requires that marker and still verifies the compositor's
screen pixel and retained window state. The acceptance passed on 1, 2 and 4
CPUs with evidence under `target/hello-gui-compositor-{1,2,4}cpu/`.

Host `libwoven` tests (20), warning-denying application Clippy, formatting and
Python validation passed. This is a software-rendering abstraction and visual
composition checkpoint; font shaping, paths/gradients, GPU acceleration,
window policy and the physical display path remain future work. The existing
repository Stage 16.1 application-loader gate is a separate historical
boundary and is not relabeled by this graphics-track increment.

### Graphics track - 2D primitives increment (2026-10-18)

`Renderer` now also provides clipped vertical gradients, one-pixel strokes,
and bounded row-major image blits. `hello-gui` exercises all three in its
visible frame and emits `[S16G2] WovenGraphics 2D frame: PASSED`; the Stage
19.2 harness requires both WovenGraphics markers. The renderer remains safe
Rust over the existing caller-owned `Canvas`, with no new ABI or unsafe code.

The 21-test `libwoven` suite, warning-denying workspace and application
Clippy, formatting, and harness syntax checks passed. Stage 19.2 QEMU visual
acceptance passed on 1/2/4 CPUs with evidence in
`target/hello-gui-compositor-{1,2,4}cpu/`. Paths, font shaping, GPU backends,
window policy, and the physical display path remain future work.

### Graphics track - VirtIO-GPU capability boundary (2026-10-18)

The modern VirtIO transport now retains negotiated device features. The
VirtIO-GPU driver requests and exposes a typed capability record including
scanout count and the optional VIRGL/3D feature bit. Existing 2D scanout
resource creation and presentation are unchanged, and software rendering
remains the selected fallback until a VIRGL command path exists.

Kernel target Clippy with `stage20-14-test`, workspace all-target Clippy, and
the Stage 20.14 two-monitor pixel-verified acceptance passed on 1/2/4 CPUs.
Evidence is preserved under `audit-artifacts/stage20.14-{1,2,4}cpu-*`.

The VIRGL lifecycle slice now follows the VirtIO contract for bounded context
creation and destruction when the device advertises VIRGL. The current QEMU
two-monitor device does not expose a VIRGL context path, so the lifecycle is
capability-gated and the tested 2D fallback remains unchanged; no shader or
GPU rendering claim is made yet.

The driver now also reads the advertised capset count and can query bounded
capset metadata before context creation. This prevents selecting a rendering
protocol without a device-reported capset version and size. Stage 20.14
remained pixel-verified on 1/2/4 CPUs after the change.

The acceptance harness now has a `--virgl` mode using QEMU
`virtio-gpu-gl-pci` with `egl-headless`. VIRGL capability discovery and
context lifecycle passed on 1/2/4 CPUs. This is a separate discovery gate:
resource creation, command streams, and GPU-backed compositor presentation
are still open.

The VIRGL GL-QEMU path now verifies a bounded 3D resource lifecycle:
resource creation, context attachment, detachment, and release. This passed
on the 1-CPU discovery run after the strict kernel Clippy checks. A valid
VIRGL command stream and GPU-backed compositor presentation are still not
claimed.

The transport now submits the protocol-defined zero-payload VIRGL NOP command
after resource attachment, and the GL-QEMU discovery run passes that full
`SUBMIT_3D` framing/context-routing probe. This validates transport only; it
does not yet render a GPU frame.

The NOP packet is now produced by a bounded `VirglStream` encoder rather than
an ad hoc raw dword. Its checked header/payload capacity is covered by the
same 1/2/4-CPU GL-QEMU discovery gate, while visible GPU clear/draw output and
compositor handoff remain the next acceptance boundary.

The next probe now encodes a VIRGL surface, framebuffer binding, RGBA clear,
and surface destruction in one bounded stream; GL-QEMU accepts this command
transport sequence on 1/2/4 CPUs. A direct `SET_SCANOUT` attempt with the
3D resource was rejected by QEMU with device status `0x1203`, so the visual
scanout gate remains explicitly open and the accepted software/2D fallback is
unchanged. The failed probe is preserved under
`audit-artifacts/stage20.14-1cpu-1792313193416105600`.

The next presentation probe added capability-gated VirtIO resource-blob
creation and `SET_SCANOUT_BLOB`, with QEMU blob negotiation available through
the harness's opt-in `--blob` flag. This QEMU build accepts guest blob
creation, but rejects the blob as a VirGL context resource; its host-3D-plus-
guest variant is rejected at creation. The default VIRGL gate therefore stays
blob-disabled and passes on 1/2/4 CPUs; blob-backed visual presentation is
still open. Retesting with QEMU's explicit `hostmem=64M` window produced the
same `0x1200` host-3D blob-creation failure.

### Graphics track - software 3D foundation (2026-10-18)

`libwoven::graphics` now includes caller-owned `DepthBuffer`, projected
`Vertex3` values, and a clipped depth-tested triangle rasterizer with
interpolated vertex colour. `hello-gui`
renders a small two-face software 3D frame and emits
`[S16G3D] WovenGraphics software 3D frame: PASSED`; Stage 19.2 requires that
marker alongside the 2D markers. The 23-test `libwoven` suite, warning-denying
Clippy, formatting, and QEMU visual acceptance passed on 1/2/4 CPUs. This is
deterministic software rendering, not GPU acceleration or a shader API; the
next hardware-facing step is a VirtIO-GPU capability/backend boundary with a
software fallback preserved.

## Stage 10.7 exact Windows acceptance — passed 2026-10-18

The authoritative `RUN-STAGE10.7.ps1` launcher exited 0 on the current
working tree after regenerable build caches were reclaimed. Host acceptance,
host lint, build, warning-denying Clippy, host tests, repeated 1-CPU memory
regressions, Stage 7.6 2/4-CPU closure, 1/2/4-CPU host-verified network
regressions, and isolated Stage 10.1–10.7 production boots all passed. The
Stage 10.7 TCP gate verified connect, two transmissions, receive, and FIN/EOF
at 1/2/4 CPUs. Full transcript and preserved serial evidence:
`audit-artifacts/acceptance-20261018-091013-150/`.

This is deterministic QEMU/host acceptance only; physical USB/UEFI boot and
production-media qualification remain open. The shared tree still contains
uncommitted work from both agents, so this acceptance must not be represented
as a standalone commit until the tree is intentionally staged and reviewed.

## AX201 firmware provider and physical DMA queue owner — 2026-10-17

The AX201 lifecycle now accepts a named provider source and rejects a
non-AX201 firmware filename before parsing. The exact API-77 provider name is
`iwlwifi-QuZ-a0-hr-b0-77.ucode`; the external blob remains ignored and is not
embedded or committed. `Ax201QueueOwner` now allocates the 22000 MQ RX
free/used/status rings, command ring and one owned RX buffer per 512-entry MQ RX descriptor, initializes
the free descriptors with physical addresses and buffer IDs, and feeds those
addresses into context-info. Startup refuses to proceed without attached
queue ownership, and teardown retains queues until explicit hardware
quiescence. Startup also rings the Intel 22000 RFH shadow write-index doorbell
(`0x1c80`) for the initialized 512-entry RX ring before `CPU_INIT_RUN`. Stage 13.9
passed on 1/2/4 CPUs; kernel target and host/all-target warning-denying
Clippy passed.

The live RX reclaim path now consumes completions in device order and validates every device-produced AX201 completion
rbid against the kernel-owned buffer table before copying data. AX201's MQ
free descriptors are 16-byte transfer descriptors; its used completions are
32-byte records containing the rbid/flags rather than a returned address.
Each consumed buffer is returned to the free ring with its original physical
address, and the physical poller rings the RFH write pointer at the
eight-descriptor cadence.
The bounded four-command host-command path now builds short or wide headers into owned DMA,
writes a 256-byte 22000 TFD on command queue 9, rings the command pointer, and
retains the payload until the matching response sequence is observed. The
queue-owner self-test rejects stale command completions.
The TFD construction is now a shared checked encoder: it bounds the transfer
buffer list to 25 entries, validates 36-bit addresses and non-zero lengths,
and is covered by the queue-owner self-test before the descriptor is published.
The driver now also has a typed scan-control wire boundary: API-defined scan
configuration and abort commands are encoded into owned command payloads, and
scan-complete notifications are validated and decoded by group, command,
versioned payload size, UID, and status fields. This is command/notification
plumbing only; a full channel/SSID scan request and radio result are not yet
claimed.
The latest increment adds explicit API-17 passive and active UMAC scan-request
encoders bounded to 67 channels. The active path owns a real broadcast probe
template, supports up to 20 checked directed SSIDs, and sets the firmware's
per-channel SSID-selection bits. The per-iteration completion parser checks
the result-record count before decoding.
The next increment adds the checked API-1 `PHY_CONTEXT_CMD` wire boundary:
the packed 40-byte context is encoded with explicit channel, band, and width
validation and is available through the driver command path. It is not sent by
the physical probe yet; MAC context, binding context, command-status handling,
and teardown sequencing must be completed before radio commands are enabled.
The following increment adds the API-1 managed-station MAC context and API-2
binding context encoders, plus strict generic command-response status parsing.
The physical activation probe now submits PHY, MAC, and binding contexts in
that order after ALIVE and requires a zero firmware status for each response.
It still stops before scan/association and remains fail-closed on any context
timeout or rejection.
After those contexts succeed, it now issues a bounded API-17 active broadcast
scan on channels 1, 6, and 11, consumes iteration/completion notifications,
and sends the matching abort command on timeout. Directed SSIDs are supported
by the wire encoder; association, WPA, and returned-frame ingestion remain
unimplemented. The RX path now also decodes AX201 `REPLY_RX_MPDU_CMD`
descriptors, validates CRC/status and MPDU bounds, and feeds valid beacon or
probe-response frames into the bounded `Manager` scan table with bounded
metadata-derived RSSI.
Management-frame TX is now wire-encoded against the 22000 TX_CMD v9 layout;
dedicated data-TX queue ownership and TX completion handling remain open.
The final command-response reclamation correction also passed the Stage 13.9
1/2/4-CPU chain; the retained evidence IDs are
`stage13.9-1cpu-1792256412816237100`,
`stage13.9-2cpu-1792256436910132400`, and
`stage13.9-4cpu-1792256460770673800`.
The subsequent passive-scan increment also passed on 1/2/4 CPUs with evidence
in `stage13.9-1cpu-1792256636696324900`,
`stage13.9-2cpu-1792256661182426300`, and
`stage13.9-4cpu-1792258543111313700`.
The active-probe encoder and physical call-site also pass the fresh matrix in
`stage13.9-1cpu-1792259066847956700`,
`stage13.9-2cpu-1792259146012605800`, and
`stage13.9-4cpu-1792259181589741900`.
The RX MPDU/scan-table increment also passed on 1/2/4 CPUs with evidence in
`stage13.9-1cpu-1792259384946552100`,
`stage13.9-2cpu-1792259451253593400`, and
`stage13.9-4cpu-1792259489563484400`. The current physical image is
42,008,576 bytes with SHA-256
`f0c1d430652071c8f917169da37acc281d2493218db75390ee9253a14f286073`.
The TX boundary increment passed on 1/2/4 CPUs with evidence in
`stage13.9-1cpu-1792259723636441300`,
`stage13.9-2cpu-1792259774430287500`, and
`stage13.9-4cpu-1792259815230643000`. The current physical image is
42,008,576 bytes with SHA-256
`bddc86b900dfd7525cf59f7ba9917e8a7e71f6de1cd387574c20c7194828d75b`.
The following increment adds a separate bounded AX201 data-TX queue owner:
its 32-entry descriptor ring and encoded TX payloads are retained in owned
DMA until the exact queue/index completion is reclaimed; only Intel's
management queues 5–8 and data queues 10–30 are accepted, while queue 9 and
the RX queue are rejected as TX targets. The driver exposes submit/reclaim methods,
but physical DQA queue configuration and device-produced TX completion
decoding remain prerequisites before the physical probe may transmit.
Stage 13.9 passed again on 1/2/4 CPUs with evidence in
`stage13.9-1cpu-1792260873010961800`,
`stage13.9-2cpu-1792260907652843800`, and
`stage13.9-4cpu-1792260941804531300`. The rebuilt physical candidate is
43,057,152 bytes with SHA-256
`3c8220bd850e82171863c13daed3861b92184fdeb1db74109019dec645c727bc`.
The latest increment corrects the TX_CMD v9 field offsets to the Intel
22000 layout, adds the 4-byte DQA-enable and 36-byte API-v3 scheduler-queue
encoders, owns the required byte-count DMA table, and decodes bounded single-
frame `REPLY_TX` notifications using the RX header's queue/index sequence.
The physical probe now attempts DQA enable and scheduler queue allocation
after ALIVE and continues only after a valid queue response; it still does not
send an arbitrary management frame. Stage 13.9 passed again with evidence in
`stage13.9-1cpu-1792261608259229200`,
`stage13.9-2cpu-1792261642569914800`, and
`stage13.9-4cpu-1792261677212040100`. The current physical candidate is
43,057,152 bytes with SHA-256
`60c7323329d7eeb11407384c087cd40a4cada154186036041660416202cefd29`.
The physical path now exercises one host-owned management TX after a
successful scan: it builds an Open-System authentication request for the
first bounded scan result, submits corrected TX_CMD v9 through the configured
queue, waits for `REPLY_TX`, reclaims the exact queue/index slot, and resets
the link state without attempting association or credentials. Stage 13.9
passed again on 1/2/4 CPUs with evidence in
`stage13.9-1cpu-1792265662237988600`,
`stage13.9-2cpu-1792265696327245000`, and
`stage13.9-4cpu-1792265730806233400`. The physical candidate is
43,057,152 bytes with SHA-256
`20bc396f058d6b4a120fbbad5008c2c35b227d585ad11b8ba38b6df57dd233b4`.
The authoritative Stage 13.9 acceptance was rerun after the TX probe
increment and passed again on 1/2/4 CPUs with evidence in
`stage13.9-1cpu-1792265958428749200`,
`stage13.9-2cpu-1792265991087909700`, and
`stage13.9-4cpu-1792266024653575700`. Warning-denying physical-feature and
all-target host Clippy also pass. These are synthetic/QEMU gates; no new
device-produced ALIVE, scan, interrupt, or `REPLY_TX` hardware evidence has
been captured yet.
After the documentation-contract cleanup, the full Stage 13.9 matrix passed
again with evidence in
`stage13.9-1cpu-1792266232912824300`,
`stage13.9-2cpu-1792266283105118400`, and
`stage13.9-4cpu-1792266340243916800`. The rebuilt physical candidate is
43,057,152 bytes with SHA-256
`ad6f4de25a53fddafee14ed30b0133a391035b081ff933d74c68139fc444bb3f`.
The candidate was then written from an elevated PowerShell to the verified
AX201 test USB. The writer read back the full image region with the same
SHA-256, created the FAT32 `WOVENLOG` partition, and reported `DONE` in
`audit-artifacts/ax201-physical-write-20261017.txt`. This is media-preparation
evidence; the laptop still must boot the image and return its `LOGS/BOOT*.TXT`
hardware log.
Scan ownership is now explicit: only one UID may be active, aborts must name
that UID, unsolicited iteration/completion notifications require the active
UID, and a matching completion retires the session. This prevents stale scan
notifications from changing a newer request's lifecycle.
After ALIVE, the physical image now submits the read-only statistics command
(`0x9c`, disabling periodic notifications) and logs whether its response
sequence completes. It deliberately does not claim scan, association, or
network qualification from that transport probe.
The rebuilt physical image was written to the guarded USB target and its raw
read-back matched SHA-256
`5b5fd671fc67b48cc3e55cb778c5cab3cdd56cc3e92512d67657857f8eacfaf9`.
The scan-session image was written and raw-read back again in
`audit-artifacts/ax201-physical-activation-usb-write-scan-session-20261017.txt`.
The PHY-context source increment built a new physical candidate image with
SHA-256 `8774dc764c9042acbf7f9b94258014707ee91b63a8e3addb5d35a26f89bcd180`.
Its guarded USB write was attempted but refused by the non-Administrator
shell; the failed privilege transcript is retained in
`audit-artifacts/ax201-physical-activation-usb-write-phy-context-20261017.txt`.
The last successfully written image remains the scan-session image above.
The current context-sequence candidate image is 42,008,576 bytes with SHA-256
`bde089b3b692fc585ff6d9cda73a81a765f69aca321f9f0aed5cf8722665c312`.
The physical startup path now reads `CSR_HW_REV` at BAR offset `0x28` for
context-info `mac_id`; it no longer substitutes the PCI revision byte.
Before PCI binding, the physical image also verifies the external firmware
against the approved API-77 SHA-256
`f866c78d5bc3b8f53964bd4c2fcfdef5b06ea94f6657e00adc570ca1b4904104`.
The activation probe also reports the six-byte hardware MAC read from the
reviewed CSR pair `0x380/0x384`; this is diagnostic identity only and does not
claim radio or network qualification.
The command-queue TFD stride is now the 22000 hardware value of 256 bytes,
not the former 128-byte placeholder. Its context-info queue-size field now
uses the Intel `TFD_QUEUE_CB_SIZE` encoding (`2` for the 32-entry ring),
while ring bounds continue to use the actual 32-entry count.
The physical activation path now enters only after IDT/APIC initialization,
explicitly enables PCI memory decoding and bus mastering, programs the AX201
MSI vector, enables CSR RX causes, services the deferred interrupt boundary,
and disables MSI/CSR delivery before restoring the original PCI command word.
Before the RX doorbell, physical startup now programs the RFH free/used/status
queue bases and resets the device cursors; firmware remains responsible for
enabling RX DMA. Upstream 22000 behavior was checked before adding a command
path: the `RFH_QUEUE_CONFIG_CMD` is for additional RX queues, while this
single-queue owner uses the context-info default queue and therefore does not
send that command speculatively.

This is still not accepted physical activation: the new queue doorbells,
physical BAR/MSI execution, device-produced ALIVE, interrupt recovery, and
scan/association have not yet produced a returned hardware log. The current
USB physical image is awaiting the user's boot photograph/log; the read-only
inventory photograph remains inventory-only.

## AX201 integrated 22000 activation boundary — 2026-10-17

Stage 13.10AE/AF/AG now pass on 1/2/4 CPUs. The AX201 owner validates the
API-77 runtime container, copies LMAC/UMAC/paging payloads into owned DMA,
configures queue addresses, publishes context-info through the 22000
MMIO/PRPH adapter, issues `CPU_INIT_RUN`, accepts a versioned ALIVE
notification, and has an explicit stop path that releases its DMA/context
resources. This is still a synthetic hardware contract.

The AX201 PCI identity `8086:A0F0` is now admitted to the shared Intel
22000-family CSR path as `ActivationReady`, with the integrated
`iwlwifi-QuZ-a0-hr-b0` API-77 firmware profile. AX201 has a distinct MSI
binding contract and Stage 13.10AD self-test; it cannot match the AX200 MSI
contract by accident. Stage 13.9 WovenWiFi acceptance passed on 1/2/4 CPUs,
host/all-target Clippy passed, and the Wi-Fi TLV tests passed.

This remains a driver activation boundary, not a completed radio driver. The
tree still has no production firmware-provider/filesystem path, physical BAR
mapping and MSI attachment for this owner, device-produced `ALIVE` proof,
real RX/TX/command ring doorbells, interrupt teardown/recovery against the
device, or scan, association, security and physical network tests.

## Stage 10.7/release and AX201 identity increment — 2026-10-17

The supplied Stage 10.7 transcript is recorded in
[the Stage 10.7 audit](docs/audit-stages-consolidated.md). The full
serialized release validator exited 0; its report is
`target/release-validation/results.json` and includes host lint/tests, debug
and release 1/2/4-CPU memory/storage/network, shell, application, system-app
and catalog checks. Stage 13.9 WovenWiFi synthetic acceptance also passed on
1/2/4 CPUs after the AX201 identity increment.

After the AX201 firmware-policy metadata was added, the full release
validator was rerun with the local Cargo cache (`CARGO_NET_OFFLINE=true`) and
again exited 0 with `All release gates passed`. The first online attempt's
crates.io DNS failure was environmental and is retained in the terminal
evidence, not treated as a code result.

The selected machine's Intel AX201 `8086:A0F0` is now explicitly recognized
as `ActivationReady`, with an explicit integrated 22000-family firmware
profile (`iwlwifi-QuZ-a0-hr-b0`, API 77 only). The AX201-specific activation
contract can reach PCI bus mastering/MSI only through its reviewed entry
point; it cannot fall through the AX200 identity path. No firmware is
embedded or loaded. Native AX201 firmware transfer, DMA queues, interrupts,
teardown, scan, association and physical network acceptance remain open.

## Physical inventory result received — 2026-10-17

The returned probe photograph (`IMG_5963.jpeg`) reaches the explicit
`PHYSICAL COMPLETE - INVENTORY ONLY` marker. It records Intel PCI inventory
including xHCI `8086:9A13`; all five displayed xHCI ports report no connected
device (`CCS=0`, `PED=0`, `RESET=0`, `SPEED=0`). It also reports one AX201
target, `DMA=NOT-ACTIVATED`, and `RADIO=UNIMPLEMENTED`. This confirms the
read-only inventory completed, but it does not qualify USB storage discovery,
DMA, interrupts, Wi-Fi, Bluetooth, or any physical radio path. The existing
hardware/logging blocker and serial/xHCI evidence requirement remain open.

## Live boot log startup candidate (2026-10-16; physical retry correction 2026-10-17)

The production boot used to synchronously scan FAT, allocate a 512 KiB log
file, and flush USB writes before starting WovenShell. A slow physical USB
device could therefore delay the desktop before a log file existed. Live-log
initialization is now queued as a background I/O service immediately before
session start, with a short scheduling delay and retry diagnostics. The Stage 13.19
synchronous self-test path is unchanged. The exact Stage 10.7 Windows gate,
full release-validation chain, warning-denying stage feature lints, and the
Stage 13.19 production/test/unlabelled and Stage 20.15 desktop gates on 1/2/4
CPUs all pass. The
worker is queued even when USB storage is not yet enumerated, so discovery
failures are retried rather than silently disabling logging. Physical USB
enumeration now retries until storage is published instead of exiting after
one not-ready probe; an existing HID interrupt worker no longer blocks later
storage discovery. The earlier
driver path has init-begin and xHCI phase markers; UART polling is
bounded in all builds while preserving bytes in the in-memory log. These
checkpoints diagnose, but cannot persist to the USB stick if xHCI itself never
becomes usable. The validated image has been written to the authorized VendorCo
USB device and raw-readback verified. Physical boot and returned-log inspection
remain pending, so hardware behavior is not yet accepted. Focused Clippy,
Stage 13.18 (1 CPU), and Stage 13.19 production (1/2/4 CPU), test and
unlabelled (1 CPU) gates pass after the correction. See the
[live boot startup audit](audit-live-boot-log-startup-2026-10-16.md).

## Foundation preservation failure — 2026-10-16

Session 71482 exited 1 on a 1-CPU live DNS timeout inside exact Stage 10.7
preservation. Independent inspection matched all 861 frozen inputs and all
sixty stress logs' overall pass markers. Prior focused 1/2/4-CPU foundation,
debug/optimized storage/memory and 2/4-CPU hotplug checks passed, but this is
not full acceptance. See the foundation validation audit. The next ATA
durability candidate is isolated in `codex/foundation-ata-flush-20261016`;
no unvalidated kernel patch is integrated here. Release finalisation remains
blocked by these and the previously documented durability/hardware/guard gaps.

## Merge integration checkpoint — 2026-10-16

The foundation validator preservation run was invalidated by a concurrent
VS Code pull of `d6fcf854`: session 54720 exited 1 on tracked inventory change.
Its successful partial checks are not full acceptance. Merge repair retains
both the accepted compiler graph/TLS loader and the peer security/driver
additions. The user-directed main.rs restoration is backed up. All earlier
release-pass claims below retain their original peer/baseline scope; none
qualifies this newly combined tree. A fresh frozen chain is required.

## Current priority: stages 1–10 release qualification — 2026-10-16

The user requested desktop/laptop release preparation for stages 1–10. See
[the ordered release ledger](stage1-10-release-readiness.md). No release-ready
claim is made. The Stage 11 layout draft is preserved separately, not integrated;
the known guard vulnerability remains a release blocker. Codex is beginning
with foundation evidence integrity and stages 1–5 validation before later
closure decisions. Hardware, durability and failure-stress gaps remain explicit.

## IPC message receive race (2026-10-16, source fix)

Two threads of one process receiving messages at once could deliver one
message twice and silently drop the next. Receive peeks, copies out and
then dequeued whatever was at the head. It now dequeues only the message it
peeked, using the mailbox's dequeue serial, and a losing thread takes the
next message. Every boot's IPC self-test covers the race. See the
[IPC receive race audit](audit-ipc-receive-race-2026-10-16.md).

## Signal authority and handler validation (2026-10-16, source fix)

The syscall-boundary audit found two more defects, both fixed on PR #80.
- **Any process could kill any other.** `kill` had no permission check.
  Ring 3 may now signal only itself and its own children, unless it holds
  `TaskControl`.
- **`sigaction` could halt the kernel.** It accepted any handler address.
  A non-canonical one made `iretq` raise #GP in ring 0, and the kernel
  halted. Handlers must now be executable code in the caller's own image.

Ring-3 probes in the program every boot runs cover both fixes. See the
[signal authority audit](audit-syscall-signal-authority-2026-10-16.md).

**Allocation failure.** Fixed separately: syscall-path allocations sized
by user arguments are now fallible. Exhaustion fails the call instead of
halting the kernel.

## Syscall user-pointer bounds (2026-10-15, source fix, QEMU pending)

Any Ring-3 process could panic the kernel with one syscall.
`copy_from_current_user` and `copy_to_current_user` passed raw user
addresses to `VirtAddr::new`, which panics on non-canonical values. For
example, `write(1, 0x0001_0000_0000_0000, 1)` stopped the whole kernel.
- **Fix:** both routines now bound every range to the canonical lower half
  first (`user_address::check_user_range`, like Linux `access_ok`).
- **Defence in depth:** `translated_chunk` uses `VirtAddr::try_new`.
- **ABI:** unchanged.

**Evidence.** The host test `user_address` passes 6 tests, the host suite
passes 154, and Clippy is clean on 5 kernel feature sets and all host
targets.

**Ring-3 probes.** The integrated user program run by every boot now
passes bad pointers to `write` and `open`, and fails the boot unless each
call returns `-1`.

**Open.** 1/2/4-CPU QEMU and release-chain evidence (CI lanes on PR #80;
the local runs wait on the acceptance hold). See the
[user-pointer bounds audit](audit-syscall-user-pointer-bounds-2026-10-15.md).

## Stage 20.15 — the desktop as the session (2026-10-15)

A system-apps (release) boot now starts the WovenShell desktop instead of
the kernel console.
- **F12** switches to the diagnostic console and back.
- **Quick Settings** has Power off and Restart. They use capability
  PowerControl (25, system services only) and syscall 209.

**Also fixed.**
- **Stage 11 loader:** it relocated ET_EXEC images under ASLR, which
  crashed WovenShell on its first production boot. Only ET_DYN images
  are randomised now.
- **Shell image size:** WovenShell outgrew the VFS's 64 KiB file limit;
  it is now stripped.

**Evidence.**
- 20.15 passes on 1/2/4 CPUs, and three negative controls were caught.
- Stage 20 regression, 13.12 production and the system-apps desktop boot
  pass.
- The release gate passes.

See the [audit](docs/audit-stages-consolidated.md).

## Stage 13.20 — ACPI power-off and reset (2026-10-15)

- **Power-off:** S5 from the FADT PM1 control blocks and `\_S5`.
- **Reset:** the FADT reset register, then the 8042, 0xCF9 and a triple
  fault.
- **Shell commands:** `poweroff` and `reboot`.
- **Live-test log:** the `WOVENLOG` label is also accepted from the root
  directory, as Windows writes it.

**Evidence.**
- Power-off and reset pass on 1/2/4 CPUs, and three negative controls
  were caught.
- The release gate passes 102/102.

See the [audit](docs/audit-stages-consolidated.md).

## Stages 13.17–13.19 — live hardware testing (2026-10-15)

Booting a WovenHat USB stick on another machine is now safe, and the boot
reports back on the stick.
- **13.17: disks WovenHat does not own are never written.** A root device
  is writable only if its FAT32 volume is labelled WOVENHAT or WOVENLOG.
  Before this, a normal boot mounted another PC's EFI system partition
  writable.
  - Also fixed: AHCI takeover from firmware. A port the firmware stopped
    was skipped (`NoSataPort`), which likely affected every real SATA
    machine.
  - Driver bring-up now logs specific errors.
- **13.18: USB mass storage.** Bulk-Only Transport with SCSI commands over
  xHCI, as a block device. Input reports are routed during storage I/O.
  - Also fixed: a TRB ring-wrap ordering bug that could stop a USB
    keyboard after 255 reports on real controllers.
- **13.19: the boot's log on the stick.** Each boot writes
  `LOGS/BOOTnnnn.TXT` on the stick's FAT32 WOVENLOG partition: a summary
  of every reported failure, a hardware inventory, and the full kernel
  log, flushed every 3 s and on panic. The kernel shell has `note <text>`
  and `savelog`.

**Evidence.** All three gates pass on 1/2/4 CPUs, and seven negative
controls were caught (one AHCI control was faulty and was redone).
Regression passes for 13.3–13.6, 13.12–13.14, 13.16, storage, 20.1,
20.14b and the release gate (100/100).

See the
[audit](docs/audit-stages-consolidated.md).

## Stage 1–19 production closure review — 2026-10-15

**ASLR guard defect confirmed (2026-10-16):** forced supported stack draw 16 /
mmap draw 0 CPU execution mapped all sixteen full slots, then successfully
wrote the intended stack guard (exit 99 rather than required fault -11).
The new independently guarded regression remains deliberately failing; its
serial and exact 3488/3488 frame teardown are preserved in the ASLR geometry
audit. A joint image/arena/TLS/guard/stack planner is required before acceptance.

The current synchronized branch has extensive bounded software/QEMU coverage,
but Stages 1–19 are **not collectively production-ready**. The exact closure
decision and remaining blockers are recorded in the
[Stage 1–19 closure gate](stage1-19-closure-gate.md). In particular, shared
libraries/dynamic TLS, native hardware and power-loss qualification, firmware
security provisioning, persistent update rollback, and several driver/service
boundaries remain open. The read-only host preflight found two USB disks but was
not elevated and performed no physical test.

**Signed-library increment:** the bounded signed `/lib` dependency graph and
transactional teardown passed Stage 10.8–11.5 on 1/2/4 CPUs, warning-denying
host/kernel/affected-feature checks and full Stage 10.7 preservation. The
formatter-overlapped phase was repeated successfully on exact restored sources.
See the shared-library audit for retained evidence. This accepts the bounded
increment, not full Stage 11.5 or a newly qualified release image.

**Loader interoperability finding:** a separate unmodified
Rust/LLD executable probe identified null `DT_DEBUG` rejection and an incorrect
compiler TLS/TCB layout, including live-state copying when creating threads.
The debug-metadata prerequisite and bounded main-image compiler local-exec TLS
are now accepted below after actual compiler execution and full preservation,
not merely synthetic metadata tests. Broader TLS models remain unresolved.
See the shared-library and compiler-TLS implementation audits for scope.
The Stage 11.2 thread-generation saturation correction is accepted: permanent
retirement and five actual-source host regressions pass, as do full Stage 10.7
preservation and Stage 10.8–11.5 matrices on 1/2/4 CPUs on the unchanged frozen
sources. The generation audit retains the original three retirement-path
failures and final serialized evidence. Compiler TLS, loader interoperability
and full Stage 11 production closure remain open.

**IPC correction accepted:** permanent exhausted-slot skipping passed durable
actual-source allocator regressions, real-state installation/rollback/teardown
probes on 1/2/4 CPUs, host/production/fourteen-feature warning-denying lint,
host/libwoven/Python tests, full Stage 10.7 preservation and Stage 10.8–11.5
matrices. Session 63244 exited 0 with the final pass marker; all 488 frozen
inputs matched at acceptance. The handle-generation audit retains final evidence and
the earlier test-ordering failure. This accepts the bounded IPC correction,
not Stage 7–11 production closure or a newly qualified release image.

**COW ancestor correction accepted:** the frozen 500-input combined chain exited
0 after complete host/lint/runtime preservation. See the COW and combined
acceptance audits. This is a bounded foundation correction, not full-stage closure.

**Wait-ownership correction accepted:** parent ownership is checked before
scheduler liveness and revalidated in the original reap transaction, without
inverting lock order. The common 500-input full acceptance and real 1/2/4-CPU
authority/error tests pass. See the process-wait ownership audit for limits.

**Signed-status correction accepted:** after the two earlier prerequisite
decisions, zero-extended 32-bit process-wait/thread-join payloads passed actual
kernel/libwoven host and Ring-3 tests on 1/2/4 CPUs, with original thread
assertions and exact resource baselines preserved. Session 94658 exited 0 after
full host/lint QA, Stage 10.7 and Stage 10.8–11.5 preservation, with all 500
frozen inputs unchanged. See the signed-status and combined acceptance audits.
This is not full Stage 11 or a release qualification. Compiler TLS, ET_EXEC
placement/interoperability, combined-tree, persistent security/recovery and
native hardware remain open. Earlier manifests are retained historical baselines.

**Executable placement correction accepted:** Claude's fixed-ET_EXEC policy is
adapted to the signed graph loader. After reproducing the nonzero-bias fault,
source `a8acbaaf` passed the expanded 1/2/4-CPU fixture, 159 host Rust tests,
nineteen libwoven/Python tests each, warning-denying host/production/fifteen-
feature lint, exact full Stage 10.7 and Stage 10.8–11.5 preservation. Session
62865 exited 0; independent inspection verified all 500 frozen inputs and sixty
stress logs. Nonzero PIE execution and original assertions remain intact.
See the executable-placement audit. Compiler TLS, general interoperability,
combined-tree, release and hardware qualification remain open.

**Compiler shared-PIE correction accepted:** source `da6d16e6` explicitly handles
inert null DT_DEBUG metadata while rejecting nonzero/duplicate values. Session
73596 exited 0 after zero/nonzero-bias compiler execution, signed-status and
all original assertions on 1/2/4 CPUs; 162 root host, nineteen libwoven and
twenty Python tests; warning-denying host/production/sixteen-feature lint;
exact full Stage 10.7 and Stage 10.8–11.5 preservation. Independent inspection
verified all 505 frozen inputs and sixty stress logs. See the compiler-PIE
audit. Compiler TLS, general dynamic linking, merged-tree, release and hardware
qualification remain open.

## Stage 21.3 bounded WovenStore transaction core (2026-10-20)

`libwoven::store` now implements a fixed-capacity catalog transaction core.
It enforces a sandbox capability mask, requires dependencies before install,
allows only monotonic version updates, protects dependencies from removal, and
provides explicit commit/rollback snapshots. Host tests, host/freestanding
warning-denying Clippy, and the Stage 21.3 QEMU gate passed on 1/2/4 CPUs.
Evidence is recorded in
`docs/audit-stages-consolidated.md`.

The core is not yet persistent: WovenFS publication, payload/catalog atomicity
across reboot, package service ownership/IPC, and physical USB qualification
remain open.

**Compiler main-image local-exec TLS accepted (2026-10-16):** checked Variant-II layout, a mapped TCB
self pointer and an immutable post-relocation template fix the retained
compiler FS/guard fault. Host parser/layout tests and first expanded Ring-3
execution pass on 1/2/4 CPUs, including fresh children, page/congruence edges, fork mapping
protection and rollback. Expanded compiler fork/cancel/capacity/owner-exit and
relocated-pointer cases pass on every online CPU in 1/2/4-CPU boots. Session
37505 passed full inherited acceptance: 515 unchanged inputs, nine focused
boots / 28 TLS executions, twenty-one inherited boots, seventeen feature lints,
168 host / nineteen libwoven / twenty-one Python tests and sixty stress logs.
Review also found ASLR mmap/stack-range
overlap, hidden by zero-draw QEMU boots; that needs a separate validated layout
gate. Pure-BSS runtime, broader/library TLS and physical qualification remain
open. See the compiler-TLS implementation audit. No full TLS/stage claim.

## Presentation and repaint performance (2026-10-15)

A fix to accepted Stage 19/20 behaviour, found by the Stage 20.7 fade gate.
On 1 CPU the fade showed only 4–6 of its frames, because every frame
over-drew. Three changes:
- **WovenShell.** It repaints up to four separate dirty areas instead of
  one bounding box. Window opening and notifications report their parts
  separately.
- **Damage.** Kernel `Damage` merges regions that form exactly a
  rectangle, so tiled repaints no longer overflow the list into a bounding
  box.
- **Present.** It converts each row once and copies it to the physical
  rows it covers; the virtio-gpu path does the same.

**Results.**
- A fading frame presents 52 k pixels instead of 167 k.
- The window-open frame dropped from about 1.8 G to 60 M cycles.
- 20.7 passed six consecutive 1-CPU runs.
- 20.1–20.14, 17.1–19.14 and 13.12 production pass on 1/2/4 CPUs.

See the
[performance audit](audit-presentation-performance-2026-10-15.md).

## Stage 20.14b — the desktop across two monitors (2026-10-15)

Roadmap Stage 18.9, Multi-monitor Desktop.
- **Compositor.** It drives a multi-head virtio-gpu, with heads side by
  side, and presents each damaged region to the heads it covers.
- **Syscall 207.** It reports the monitors.
- **Shell.** The taskbar and panels stay on the primary monitor; the
  wallpaper, pointer and window dragging span every monitor.

**Fixes found by the gate.**
- A drop on the second monitor no longer snaps back to the primary.
- Windows on the second monitor are no longer clipped.
- The scale toggle no longer resets the primary.

**Evidence.**
- **Gate `[S20.14B] desktop across monitors: PASSED`:** passes on 1/2/4
  CPUs. Host captures of both heads match the kernel's rendering after a
  window is dragged onto the second monitor.
- **Negative controls:** all five failed the gate as intended.
- **Regression:** 20.1–20.14a, 17.1–19.14 and the 13.12 production boot
  pass on 1/2/4 CPUs.

See the
[Stage 20.14b audit](docs/audit-stages-consolidated.md).

**Stage 18 is complete in software.** Roadmap 18.1–18.9 are Stages 20.1
to 20.14b.
## Stage 19.8 timer/event sources — 2026-10-15

The unified wait path now has append-only syscall 206, `WaitAnyObjects`, for
up to eight generation-safe timer/event objects. The kernel copies the source
array before registration, enforces capability and owner/generation checks,
uses event-driven task wakeups, consumes one-shot timer expiry exactly once,
preserves manual-reset event level semantics, and clears every registration on
all return paths. A live completion-handle waiter is refused rather than
replaced; manual-reset events support four bounded task waiters with broadcast
wakeups, while one-shot timers retain a single consuming task waiter. The safe `libwoven::wait_any_objects` wrapper
uses the same bounded ABI. The existing syscall 111 remains ABI-compatible.

The Stage 19.8 feature gate now includes an object-registration self-test and
the `hello-gui --timer-event` userspace scenario and an event waiter fan-out
self-test. Host/all-target Clippy,
freestanding Stage 19.8 and neighboring Stage 19.13 warning-denying gates, and
full 1/2/4-CPU QEMU acceptance all pass. The scenario verifies one-shot timer
wake, manual-reset event wake, cleanup, bounded event fan-out, and the existing
multiplexed input/message/damage/timeout behavior. Timer expiry remains
single-consumer by design. See [the ABI contract](wait-any-objects-abi.md).

## Stage 20.14a — two monitors through virtio-gpu (2026-10-15)

Roadmap Stage 18.9, first part.
- **Transport.** A modern virtio 1.x PCI transport, `virtio_pci.rs`.
- **Driver.** A virtio-gpu 2D driver with several scanouts,
  `virtio_gpu.rs`. It creates, backs, binds, transfers and flushes one
  resource per head.

**Evidence.**
- **Gate `[S20.14] two monitors: PASSED`:** two heads show exact kernel
  patterns, and host captures of both heads match pixel for pixel on
  1/2/4 CPUs.
- **Negative controls:** all four failed the gate as intended.

See the [Stage 20.14a audit](docs/audit-stages-consolidated.md).

**Next: 20.14b.** The compositor will present the desktop across the
heads, and the shell will use the second monitor.

## Stage 20.13 — workspaces (2026-10-15)

Roadmap Stage 18.8, Multiple Workspaces.

**Shell.** There are four workspaces, each with its own windows and
stacking order.
- **The switcher.** Four numbered boxes left of the Quick Settings button
  show them, with the current workspace highlighted.
- **Switching.** Clicking a box, or pressing F3, switches workspace. F4
  moves the focused window to the next workspace and follows it.
- **Per-workspace behaviour.** New windows open on the current workspace.
  The taskbar's window entries and tiling cover only the current workspace.
- **Implementation.**
  - The shell's stack holds only the current workspace's windows.
  - Switching saves that stack and restores the target's.
  - A window that closes, or whose owner exits, on a hidden workspace is
    removed from that workspace's saved stack.

**Keys.** F3 and F4 are new keys through WovenInput: PS/2 scancodes
0x3D/0x3E and USB usages 0x3C/0x3D. They reach userspace as input kinds 14
and 15 and libwoven's `Key::F3`/`Key::F4`. The kernel's own consumers
ignore them.

**USB F2 fix.** The USB keyboard map, from Stage 13.13, also lacked F2,
which is added.

**Gate changes.**
- The Stage 20.1 gate now checks the switcher, each box and digit exactly,
  and keeps checking the bare taskbar on both sides.
- The Stage 20.12 gate expects the signed-in name left of the switcher.

Gate `[S20.13] workspaces: PASSED`, driven by real i8042 input and checked
against the kernel's own rendering:
- Four workspaces, with the first current, and window A opens on it.
- F3 moves to workspace 2, and A and its taskbar entry disappear.
- Window B opens on workspace 2.
- Clicking box 1 brings A back and hides B.
- F4 moves A to workspace 2 and follows it, with A on top and both listed.
- F3 moves to workspace 3, which shows neither window.
- B is killed while hidden; back on workspace 2, only A remains.
- Killing the shell returns the display.

B lies inside A's shadowed footprint, so the gate judges "B hidden" by
B's colour being absent and the taskbar listing one window.

**Validation**, in the first worktree on `stage20-13`:
- **Lint and host tests:** kernel Clippy for production, every Stage 20
  feature, `stage13-13-test`, `stage13-16-test` and `stage19-12-test`, plus
  woven-shell and host Clippy, all with warnings denied, pass; the libwoven
  tests pass.
- **Gates:** 20.1–20.13 pass on 1/2/4 CPUs, and 13.13 and 13.6 pass on
  1/2/4 CPUs.

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each failed the gate as intended:
- Taskbar entries not filtered by workspace.
- Saved stack not restored on switching.
- F4 doing nothing.
- Switcher clicks ignored.
- Windows always opening on workspace 1.

Open:
- **No transition animation.** Workspace transitions are not animated yet.
- **No overview.** There is no workspace overview.
- **Fixed count.** There are always four workspaces.

## Stage 13.16 — tablets, pens, touch and game controllers (2026-10-14)

HID devices that describe their own reports now work.
- **Parser and decoder.** A pure report-descriptor parser and decoder
  (`kernel/src/hid.rs`) covers pointers, tablets, pens, touch screens and
  game controllers.
- **xHCI.** It fetches and parses each such device's descriptor.
- **Userspace.** Scroll, absolute-pointer, touch, pen and game-controller
  events reach userspace as input kinds 9–13.

**Evidence.**
- **Host tests:** pass against real descriptors for a tablet, a game pad,
  a pen and a touch screen. They caught a lost-axis bug.
- **Gate `[S13.16] report-protocol HID input: PASSED`:** QEMU's USB tablet
  delivers exact absolute positions, a click and wheel steps on 1/2/4
  CPUs.
- **Negative controls:** all four failed the gate as intended, one after a
  harness gap was closed.
- **Regression:** 13.5, 13.6, 13.7, 13.13 and 19.12 pass on 1/2/4 CPUs.

See the [Stage 13.16 audit](docs/audit-stages-consolidated.md).

**Open.** Multi-application devices, a virtio-input driver, and physical
qualification of digitizers and controllers.

## Stage 13.15 — WovenAudio playback for userspace (2026-10-14)

Applications can now play sound.
- **New capability.** `AudioPlayback` (24), which the permission broker
  can grant as `Permission::Audio`.
- **Syscalls.** 203 `AudioInfo`, 204 `AudioPlay` and 205
  `AudioSetVolume`, with libwoven wrappers.
- **Interrupt-driven HDA output.** The player sleeps until the stream's
  completion interrupt wakes it.
- **Ledger.** Every play attempt is recorded in the security ledger.

**Evidence.**
- **Gate `[S13.15] WovenAudio playback: PASSED`:** passes on 1/2/4 CPUs.
  QEMU's recording of the output holds the exact square wave the Ring-3
  client played, and a client without the capability is refused.
- **Negative controls:** all five failed the gate as intended.
- **Regression:** 13.8, 15.3, 15.4 and 15.8 pass on 1/2/4 CPUs.

See the [Stage 13.15 audit](docs/audit-stages-consolidated.md).

**Open.** Streaming and mixing, other sample formats, recording through
`Microphone`, and physical codecs.

## Stage 13.14 — storage on NVMe and AHCI (2026-10-14)

A machine with no ATA disk now mounts its NVMe or AHCI disk at `/mnt`.
- `block_io` routes all primary storage I/O, the FAT32 mount, persistence,
  journals and swap to a selectable root device.
- A normal boot selects the first ready `nvme0` or `ahci0` after
  bring-up.
- ATA stays the default, so existing configurations are unchanged.

**Evidence.**
- **Gate `[S13.14] NVMe/AHCI storage: PASSED`:** passes for NVMe and for
  AHCI on 1/2/4 CPUs. A persisted file survives a remount and is found in
  the disk image on the host.
- **Production:** a production boot mounts the NVMe volume and reaches
  the shell, on 1/2/4 CPUs.
- **Negative controls:** all four failed the gate as intended.
- **Regression:** the ATA storage regression, the WovenFS device gate, and
  1-5, 10.8, 12.1, 13.3 and 13.4 pass on 1/2/4 CPUs.

See the [Stage 13.14 audit](docs/audit-stages-consolidated.md).

**Open.** WovenFS on NVMe or AHCI, multiple volumes, interrupt-driven
NVMe and AHCI I/O, and physical-device qualification.

## Stage 13.13 — USB keyboards and mice into WovenInput (2026-10-14)

USB keyboards and mice now work in every normal boot.
- The xHCI driver enumerates each connected port, with its own slot per
  device.
- It binds an MSI-X or MSI vector through the driver manager.
- A worker sleeps until that vector fires, drains reports under the
  controller lock and publishes after releasing it.
- USB input reaches the same queue as PS/2.

**Two existing bugs fixed.**
- **MSI-X never delivered on any device.** Activation left Function Mask
  set.
- **The USB mouse Y axis was inverted.**

Gate `[S13.13] USB input: PASSED`. Input injected through QEMU's USB
keyboard and mouse arrives exactly, through device interrupts, with the
worker sleeping between them.
- **Runs:** passes on 1/2/4 CPUs.
- **Negative controls:** all six failed the gate as intended.
- **Regression:** 13.12 in all modes, and 13.2, 13.5, 13.6, 13.7, 13.9 and
  13.10, pass on 1/2/4 CPUs.

See the [Stage 13.13 audit](docs/audit-stages-consolidated.md).

**Open.** Report-protocol HID devices (touchpads; Stage 13.16), USB
hotplug, a fuller keymap, and physical-device qualification.

## Stage 13.12 — boot-time driver bring-up (2026-10-14)

Until now, every Stage 13 driver ran only inside its own test build. A
normal boot contained no driver manager, NVMe, AHCI, xHCI or HDA code.
Every normal boot now runs `drivers::bring_up()`, which:
- matches each controller through the WovenDriver manager;
- binds its PCI function and MMIO BARs;
- initializes the controller at that address;
- rolls the binding back if initialization fails.

A missing or failing device never stops the boot. Ready drivers appear in
the device registry as `nvme0`, `ahci0`, `xhci0` and `hda0`.

**Evidence.**
- **Gate `[S13.12] driver bring-up: PASSED`:** passes in its three
  machine modes (all devices, AHCI without a disk, no devices) on
  1/2/4 CPUs.
- **Normal build:** a kernel built with no test feature binds all four
  drivers and reaches the shell, on 1/2/4 CPUs.
- **Negative controls:** four controls each failed the gate as intended,
  and the gate passed again once the source was restored.
- **Prior gates:** 13.1–13.10 pass on 1/2/4 CPUs.

See the
[Stage 13.12 audit](docs/audit-stages-consolidated.md).

**Stage 13 is not closed by this.** Still to do:
- mount storage from NVMe or AHCI;
- deliver USB keyboard and mouse input through an interrupt-driven xHCI
  worker into WovenInput;
- expose WovenAudio to userspace;
- move touchpad, touchscreen, pen and game-controller events out of the
  Stage 13.7 probe;
- physical qualification of Wi-Fi and Bluetooth.

## 2026-10-14 revalidation

The Stage 19 system applications now build outside the kernel, sign into WAPP,
and execute in Ring 3 through the existing verified application boundary.
WovenCalculator, WovenTerminal, and WovenFiles retain their bounded 1/2/4-CPU
acceptance; see their individual audits. The remaining roadmap applications,
catalog, and multi-package boot-image installation are now covered by the
ten-application release chain. WovenFiles now adds a read-only bounded
directory/file browser; see the
[WovenFiles audit](docs/audit-stages-consolidated.md).
WovenMonitor adds bounded read-only clock, display, and storage diagnostics
and passes signed 1/2/4-CPU execution; see the
[WovenMonitor audit](docs/audit-stages-consolidated.md).
WovenSettings adds bounded read-only identity, clock, and display-policy
diagnostics and passes signed 1/2/4-CPU execution; see the
[WovenSettings audit](docs/audit-stages-consolidated.md).
WovenSecurity adds bounded read-only identity and security-ledger reporting and
passes signed 1/2/4-CPU execution; see the
[WovenSecurity audit](docs/audit-stages-consolidated.md).
WovenEditor adds bounded text preview and capability-enforced note writing and
passes signed 1/2/4-CPU execution; see the
[WovenEditor audit](docs/audit-stages-consolidated.md).
WovenImages adds bounded image inventory and header inspection and passes
signed 1/2/4-CPU execution; see the
[WovenImages audit](docs/audit-stages-consolidated.md).
WovenUpdate adds bounded read-only update preflight and passes signed 1/2/4-CPU
execution; see the
[WovenUpdate audit](docs/audit-stages-consolidated.md).
WovenStore adds bounded read-only installed-catalog browsing and passes signed
1/2/4-CPU execution; see the
[WovenStore audit](docs/audit-stages-consolidated.md).

The complete `scripts/test-release.py` chain now passes on the current tree.
The debug QEMU exit path was corrected to issue the `isa-debug-exit` byte-width
write explicitly; the full chain then passed lint, host tests, 1/2/4-CPU debug
and release gates, all ten applications, catalog acceptance, and production
system-app boot. See the [release validation audit](audit-release-validation-2026-10-14.md).

This is software/QEMU qualification only; native hardware and destructive USB
media limits remain open.

The authoritative release chain now includes app-specific Clippy, the signed
ten-application catalog gate on 1/2/4 CPUs, and release packaging of WAPP
payloads, catalog descriptors, and per-artifact hashes. See the
[system-app release audit](audit-release-system-apps-2026-10-14.md). The
feature-gated release image now claims multi-package RAM installation; physical
media qualification remains open.

The `system-apps` production-image feature now installs the ten signed
packages during normal boot, with pre-mutation signature/ELF validation,
collision checks, and rollback of newly created VFS entries on publication
failure. It requires an explicitly provisioned application trust key; the
acceptance harness supplies the test key only for QEMU. The 4-CPU shell gate
confirmed installation before shell startup. Persistent media updates,
power-loss rollback, key rotation/revocation, and native hardware
qualification remain open. See the
[production system-app boot audit](docs/audit-stages-consolidated.md).

Stage 14.5D was rerun on 1/2/4 CPUs. DHCP, DNS, ICMP, host-verified UDP/TCP,
SMP/TLB, and firewall transport/path checks passed with exit 33. See the
[Stage 14.5D revalidation audit](docs/audit-stages-consolidated.md).
The evidence remains QEMU virtio-only and does not claim native NIC/radio
qualification.

Stages 13.4, 13.5, 13.6, and 13.7 were independently rerun on 1/2/4 CPUs
and all passed with exit 33. See the
[Stage 13.4–13.7 validation audit](docs/audit-stages-consolidated.md).
These QEMU gates validate software and protocol contracts, not native-device
interoperability. The Stage 13.6 fixture exercises USB HID keyboard support;
The bounded boot-mouse report decoder is now self-tested, but live xHCI mouse
polling/publication and physical USB mouse interoperability remain open. See
the [USB mouse decoder audit](docs/audit-stages-consolidated.md).

Stage 16.1, 17.1, 18.1, 19.1, 19.3, and 19.10 were rerun on 1/2/4 CPUs
after the remote-termination cleanup change; all emitted their required pass
markers and exit 33. See
[the revalidation audit](docs/audit-stages-consolidated.md).

The complete Stage 12 acceptance chain was independently rerun on the current
accepted tree. Stages 12.1 through 12.5 all passed on 1/2/4 CPUs with exit 33;
the retained evidence directories are `stage12.1-1cpu-1791977944719919100`,
`stage12.1-2cpu-1791978000197697900`,
`stage12.1-4cpu-1791978038096642600`,
`stage12.2-1cpu-1791978076763178400`,
`stage12.2-2cpu-1791978128405584500`,
`stage12.2-4cpu-1791978166570188900`,
`stage12.3-1cpu-1791978204787637600`,
`stage12.3-2cpu-1791978254549661600`,
`stage12.3-4cpu-1791978294758980400`,
`stage12.4-1cpu-1791978335142454100`,
`stage12.4-2cpu-1791978387714722500`,
`stage12.4-4cpu-1791978427671708800`,
`stage12.5-1cpu-1791978468908251500`,
`stage12.5-2cpu-1791978521938978000`, and
`stage12.5-4cpu-1791978563395332900`.

The ELF dynamic metadata boundary was hardened before shared-library work.
`DT_NEEDED` is now bounded and string-table validated, while the loader rejects
dependency-bearing images until dependency mapping, cross-image symbol scope,
dynamic TLS, and shared-image teardown exist. The Stage 11.3–11.5 1/2/4-CPU
matrix passed again with the new parser self-test; see the
[dynamic dependency metadata audit](docs/audit-stages-consolidated.md).

## Host-test boundary repair — 2026-10-14

The freestanding kernel now has an explicit host-test boundary: test builds no
longer hide mutex/heap symbols needed by compiled production modules, and they
do not install a second panic or allocator handler. Warning-denying kernel
Clippy, all-targets Clippy, and the complete `wovenhat-os` integration-test
gate pass. The unsupported `cargo test --workspace --all-targets` command is
documented as a host-assembler mismatch rather than treated as a production
failure. See the [host-test audit](audit-host-test-boundary-2026-10-14.md).

## Stage 20.12 — sign-in screen and local accounts (2026-10-14)

Roadmap Stage 18.7, Login Manager.

**Accounts.** `libwoven::accounts` reads an accounts file with one local
account per line: `name:salt-hex:hash-hex`.
- The hash is `SHA-256(salt || PIN)`. It uses the same freestanding,
  software-only `sha2` crate as the kernel's boot measurement.
- No PIN is stored anywhere.
- Names are 1–16 printable ASCII characters other than `:`. Salts are
  1–32 bytes, and hashes exactly 32. Any other line is refused.
- Hashes are compared in constant time.
- Host tests check FIPS 180-2 vectors split across salt and PIN, and the
  parser's refusals.

**Shell.** With `--accounts <file>`, the desktop starts at a sign-in
screen. The screen shows "Sign in" and the accounts, with the chosen one
marked.
- Tab chooses the next account, digits enter its PIN, and Enter signs in.
  Masking and the three-try lockout work as on the lock screen.
- Once signed in, the account's name appears on the taskbar, left of the
  Quick Settings button.
- The lock screen then asks for that account's PIN, not `--pin`.
- If `--accounts` is given but no account can be read, the shell refuses
  to start rather than open the desktop to anyone.

**Fixes found by the first runs.**
- **The shell overflowed its user stack.** A user stack is 8 KiB
  (`UserStack::SIZE`). The desktop state was built in `main`'s 5.8 KiB
  frame, and the catalog loader's 2.3 KiB frame sat on top. Adding the
  accounts pushed the total past the guard page: a page fault at the
  shell's first instruction in `load_catalog`. The state now lives in a
  static (.bss), like the tile buffer, and the loaders fill it in place.
  `main` is down to 1.5 KiB and `load_catalog` to 1.1 KiB. The kernel's
  stack size is unchanged.
- **The gate tried to create `/etc`, which boot already creates.** It now
  accepts an existing directory.
- **The Stage 20.1 gate failed after Stage 20.10.** 20.10 put the Quick
  Settings button inside the strip the 20.1 gate required to be plain
  taskbar. CI's desktop-userspace job on `stage20-10` failed there. The
  20.1 gate now checks the strip on both sides of the button, and the
  button's fill and "Q" exactly. The fix is on `stage20-10` and merged
  forward.

Gate `[S20.12] sign-in: PASSED`. The kernel writes an accounts file with
`ada` (PIN 1234) and `lin` (PIN 9876), salted and hashed with the kernel's
own SHA-256. The shell runs with `FileRead` and real i8042 input. Screens
are compared row by row with the kernel's rendering.
- The desktop opens at the sign-in screen, with ada chosen.
- Tab chooses lin.
- ada's PIN is refused for lin.
- lin's PIN signs in, and lin's name is on the taskbar.
- Locked through Quick Settings, the lock screen refuses ada's PIN and
  takes lin's.
- Killing the shell returns the display.

Validation in the second worktree, on `stage20-12`:
- **Lint and tests:** kernel Clippy for every Stage 20 feature, host
  `--all-targets` Clippy and woven-shell Clippy, all with warnings denied,
  pass. libwoven host tests pass (16).
- **Gates 20.2–20.12:** pass on 1/2/4 CPUs.
- **Gate 20.1:** failed on 1/2/4 CPUs before the Quick Settings fix
  above. After the fix, it was rerun on 1/2/4 CPUs; see the commit.

Negative controls, each run on 1 CPU, with the source restored byte for
byte afterwards and the gate passing again:
- Any PIN accepted: fails at "ada's PIN is refused for lin".
- Tab ignored: fails at "Tab chooses lin".
- Lock forgets the session: fails at "the lock screen refuses ada's PIN
  and takes lin's".
- No sign-in screen: fails at "the desktop opens at the sign-in screen".
- Name not shown: fails at "lin's PIN signs in; lin's name is on the
  taskbar".

Open:
- **Every process still runs with the same kernel credentials.** Per-user
  sessions need the kernel to start processes as another user.
- **Accounts are read from a file given at startup.** There is no account
  creation, password change, or protected credential store; the file's
  integrity rests on the filesystem.
- **A 4-digit PIN behind a salted fast hash protects only against casual
  access.** An attacker who can read the accounts file can try all 10,000
  PINs instantly. A slow KDF and longer secrets belong with a real
  credential store.

## Stage 20.11 — lock screen (2026-10-14)

Roadmap Stage 18.6, Lock Screen.

**Shell.** Quick Settings gains a fourth row, "Lock screen", which locks
the desktop.
- **What locking hides.** No window is drawn, no keystroke reaches an
  application, and a notification is remembered and counted but never
  shown. The pointer still moves, unseen.
- **What it shows.** The wallpaper dimmed (160/255), with "Locked", the
  time, a prompt, and one masked dot per digit typed, all centred.
- **Unlocking.** The PIN is given with `--pin <digits>`, 1–16 digits.
  - Attempts are compared in constant time, and typed digits are wiped
    after each one.
  - A wrong PIN says "Wrong PIN".
  - Three wrong PINs in a row lock entry out for 5 s, during which even
    the right PIN is refused ("Try again shortly").
  - Without `--pin`, Enter unlocks.

**Why the PIN is not in the secret vault.** The PIN is in the shell's own
memory, not the kernel's secret vault. The vault needs the `Secrets`
capability. Codex's security model (Stage 15.3) reserves it for
applications the permission broker grants it to, and keeps it out of the
system-service ceiling, and the shell is a system service, because it
needs the display and input. Widening that ceiling to pass a gate would
be wrong, so this stage does not. Real accounts and credentials belong
with the login manager (18.7).

**Gate changes.**
- The Stage 20.10 gate now expects the fourth row, and passes.
- In the 20.11 gate, the lockout check first raced the shell. It began
  its one-second stability check while the typed digits were still
  showing. It now waits until the shell's input queue is empty and the
  lockout screen has settled.

Gate `[S20.11] lock screen: PASSED`. The shell runs with `--pin 2468` and
no more authority than the other desktop gates; real i8042 input. The lock
screen's rows are compared with the kernel's rendering: the dimmed
wallpaper and the text in the kernel font.
- Quick Settings locks the screen, and the window is hidden.
- `x` while locked reaches no application: A, waiting for `x`, is still
  running a second later.
- Two digits show as two dots.
- A wrong PIN says so and stays locked.
- 2468 unlocks, the window returns, and A then gets `x` and exits.
- After locking again, three wrong PINs lock entry out, and 2468 is
  refused for a full second. After the 5 s lockout, 2468 unlocks.
- Killing the shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.11-negative-control-*`:
- With the PIN check bypassed, the gate fails at "a wrong PIN says so and
  stays locked".
- With keys passed through while locked, it fails at "a key while locked
  reaches no application".

Open:
- **No idle auto-lock.** There is also no lock key beyond Quick Settings.
- **The PIN is a startup argument held in the shell's memory.** There is
  no credential store or account model yet; that is 18.7.
- **Nothing is protected against a process that can already read the
  shell's memory.**

## Stage 20.10 — Quick Settings (2026-10-14)

Roadmap Stage 18.5, Quick Settings. The roadmap lists Wi-Fi, Bluetooth,
sound, brightness, power, VPN and security. Most of these have no service
userspace can act on yet. This stage offers only toggles that act on the
system today, and the others are not faked.

**Shell.** A "Q" button left of the tray's last slot opens Quick Settings,
a translucent panel above the taskbar's right end. Like the other panels
it is drawn as a blended background plus a keyed overlay, and it, the
launcher and the Notification Center close each other. Its toggles:
- **Display scale, 2× or 1×.** The compositor surface is reallocated at
  the new geometry (syscall 117). Every window is kept on screen, and
  re-tiled if tiling is on. The pointer is clamped, and the whole desktop
  is repainted.
- **Do Not Disturb.** Notifications are still remembered and counted by
  the Notification Center badge, but raise no toast.
- **Tiling.** The same switch as F2.

hello-gui `--desktop-client` gains a `q` variant, Quiet, which posts a
notification each time it is sent `p`.

Gate `[S20.10] quick settings: PASSED`. Real i8042 input; each panel state
is compared with the kernel's own rendering over what lies beneath.
- The button opens the panel: scale 2×, Do Not Disturb off, tiling off.
- With Do Not Disturb on, `p` makes Q post a notification. The badge reads
  1, and the toast area stays clear for a full second.
- With tiling on, the only window fills the work area under the panel.
- Scale 1× makes the desktop 1280×800, and the panel follows the new
  geometry. Scale 2× from the panel at that geometry restores 640×400.
- The button closes the panel, Q takes its key and exits, and killing the
  shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.10-negative-control-*`:
- With Do Not Disturb ignored, the first version of the gate still
  passed. It checked the toast area once, at the instant the badge
  appeared. The shell paints the desktop layer, with the badge, before the
  toast layer within one repaint, so that instant can come before the
  toast is drawn. The gate now requires the toast area to stay clear for
  a second, and the control then fails at "a notification under Do Not
  Disturb is counted but raises no toast".
- With the scale toggle stuck, it fails at "display scale 1x makes the
  desktop 1280x800".

Open:
- Wi-Fi, Bluetooth, sound, brightness, power, VPN and security toggles
  await services userspace can reach.
- At 1×, a window cannot be resized to the whole work area. Its owner
  needs the old and new surfaces at once, and together they exceed the
  4 MB per-process budget, so the window keeps its size.
- Settings are not persisted.

## Stage 20.9 — Notification Center (2026-10-14)

Roadmap Stage 18.4, Notification Center.

**Shell.**
- **History.** Every notification is kept after its toast expires: the
  last 8, newest first. Each is stored as one row, `source: text`, where
  the source is the sending application's window title ("app" for a
  sender without a window), cut to what fits.
- **Unread badge.** A 10×11 accent box just left of the clock counts the
  notifications that arrived since the Notification Center was last
  opened, up to 9.
- **Panel.** Clicking the clock, or the badge, opens the Notification
  Center: a translucent panel above the taskbar's right end, with a title
  row holding "Clear", one row per notification, or "No notifications".
  - Opening it marks everything read, and the badge goes.
  - Clear empties the history.
  - A click outside the panel, or on the clock again, closes it.
- **One panel at a time.** It and the launcher close each other.
- **Rendering.** It is drawn like the launcher: a region-blended
  background, with an overlay copied by the keyed blit.

hello-gui `--desktop-client` gains an `n` variant, Notes, which posts
three notifications 10 ticks apart.

Gate `[S20.9] notification center: PASSED`. Real i8042 pointer and
keyboard input, and every pixel compared with the kernel's own rendering
(the background blended over the wallpaper, the border, title row and text
in the kernel font):
- Window N opens and posts "Build done", "Mail: 3 new" and "Battery 80%".
- The toast shows the latest, and the badge reads 3.
- A click on the clock opens the center, listing `Notes: Battery 80%`,
  `Notes: Mail: 3 new` and `Notes: Build done` in that order, and the
  badge goes.
- Clear leaves "No notifications", and a click on the clock closes the
  panel, showing the wallpaper again.
- N takes its key and exits, and killing the shell returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.9-negative-control-*`:
- With notifications not remembered, the gate fails at "clicking the
  clock opens the center, newest first, and clears the badge".
- With the badge left set on opening, it fails at the same check.

Open:
- No per-notification actions: a click on a row does not focus the
  sender.
- The history is not persisted across shell restarts.
- The kernel's own lifecycle notifications (`NotificationPoll`) are not
  shown.

## Syscall numbering for the desktop (2026-10-14)

Twice, the desktop's compositor syscalls took numbers that work on
`phase2` took at the same time, and merging conflicted:
- region blend and keyed blit at 122–124 collided with permissions, the
  audit ledger and secrets (122–129);
- the same calls renumbered to 130–132 collided with threads (130–135).

They now live at **200–202**:

| Number | Syscall |
|---|---|
| 200 | `GraphicsCompositorBlendRegion` |
| 201 | `GraphicsCompositorBlendRegionShared` |
| 202 | `GraphicsCompositorBlitKeyed` |

New desktop and compositor syscalls take numbers from the 200 block, so
the two lines of work cannot collide. The syscalls the desktop added
earlier, 116–121, were never contested and keep their numbers.

## Stages 20.4–20.8 — validation after rebasing onto phase2 (2026-10-14)

The Stage 20.4–20.8 commits were rebased onto `phase2` at `c5b13392`,
which includes the Stage 1–19 hardening (`b4f00fd3`) and the Stage 13.2
merge. They were then validated as one stack in an isolated worktree.
- Kernel Clippy is clean across all 77 feature configurations, including
  `stage13-2-test`, which compiles again.
- Clippy for woven-shell, hello-gui, libwoven and the host is clean, and
  the host tests pass.
- **102 QEMU gates pass across 1, 2 and 4 CPUs**:
  - every Stage 20.1–20.8 desktop gate;
  - all Stage 19.x application gates;
  - the kernel runtime stages back to 1–5;
  - the legacy-IRQ memory suite;
  - Stage 6 hotplug on 2 and 4 CPUs.
- In the rebase, syscalls 122–124 were renumbered to 130–132, and later
  to 200–202 (see "Syscall numbering for the desktop" above); see Stages
  20.5 and 20.6.

## Stage 20.8 — tiling: resize protocol, snap to edges, master and stack (2026-10-13)

Roadmap Stage 18.3, Window Experience: intelligent tiling. This completes
the design's 20.4–20.8 sequence. Workspace transitions are deferred to
roadmap 18.8.

**Input.** F2 is decoded (scancode 0x3C) and reaches applications.
- `woven_input::Key::F2` is encoded as input kind 8 and decoded by
  libwoven.
- The kernel's own terminal paths ignore it.
- This is an additive change to the `InputPoll` encoding.

**libwoven `layout`.** Pure geometry, host-tested:
- `tile(count, area)` places master and stack: the first window takes the
  left half, and the rest share the right half in equal rows, the last
  taking the remainder.
- `half(area, right)` gives the left or right half.
- `Frame::client` gives the client area under a title bar.
- The tests show frames cover the area exactly, with no overlap, for 1 to
  10 windows (capped at 8), and check the halves.

**Resize protocol.** The shell sends the owner `WSIZ` with a client size,
and the owner answers `WRSZ` with a new surface of that size (token,
width and height, owner-checked like `WOPN`), then releases the old one.
- The shell moves the window at once and swaps the surface when the reply
  arrives.
- For one second after a request, a vanished surface does not count as an
  exit: the owner may release the old surface before its reply is
  processed.
- hello-gui `--desktop-client` answers `WSIZ`.

**Shell.**
- Dropping a dragged window on the left or right screen edge snaps it to
  that half.
- F2 toggles tiling: master and stack over the screen above the taskbar,
  in window order, applied again whenever a window opens or closes.

Gate `[S20.8] tiling: PASSED`, driven by real i8042 input:
- Window A is dragged by its title bar onto the left edge and dropped. It
  snaps and is resized: its 320×362 client area and its title bar are
  exact.
- F2 tiles: A keeps the left half and B takes the right half, resized,
  with both title bars exact (A focused, B not).
- F2 turns tiling off. Both applications take their keys and exit, and
  killing the shell returns the display.

The first run failed at the tiling check because of the gate, not the
shell. A, focused after its drag, correctly casts its deep shadow across
the first 8 columns of B beside it, and the gate had expected plain blue
there. The gate now applies A's shadow bands to B's expected pixels, the
same stacking the Stage 20.5 gate computes.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.8-negative-control-*`:
- With F2 ignored, the gate fails at "F2 tiles the windows: master on the
  left, stack on the right".
- With snapping disabled, it fails at "dropping A on the left edge snaps
  it to the left half, resized".

Open:
- No minimum sizes: windows are never refused a tile size.
- No resizing by dragging window edges; resizing happens only through
  snapping and tiling.
- No keyboard tiling beyond F2. Real modifier keys (Ctrl, Alt, Super) are
  still missing.

## Stage 20.7 — physics-based animation: windows fade in on a spring (2026-10-13)

Roadmap Stage 18.3, Window Experience: smooth physics-based animation.

**libwoven `Spring`.** The spring eases a value toward a target, stepped
once per frame.
- It is fixed-point (16 fractional bits): no floating point, and every run
  is identical.
- Stepping is semi-implicit Euler with stiffness 0.08 and damping 0.5 per
  frame. That keeps both roots of the discrete system real and positive:
  the value approaches its target monotonically, with no overshoot, and
  settles within about 30 frames of a 255-step move.
- `step(skipped + 1)` keeps motion on time when frames are dropped.
- Host tests cover:
  - monotone approach with no overshoot, for three moves including
    negative values, settling within 60 frames, with at least 10
    intermediate values (a motion, not a jump);
  - stepping 7 frames at once equals 7 single steps;
  - determinism;
  - rest staying at rest, and retargeting starting from the current value.
- An integer Python model of the same arithmetic agreed before the first
  build: 28–30 frames, monotone.

**Shell.** A new window fades in: its opacity follows the spring from 0 to
255.
- Its title bar is drawn into the staging surface and region-blended at
  that opacity.
- Its client area is region-blended with the shared-surface blend (123).
- Its shadow bands are scaled to `opacity * alpha / 255`.
- While anything animates, the frame clock runs at 20 ms (2 ticks); at
  rest, at 250 ms.

Gate `[S20.7] spring animation: PASSED`. The kernel samples one pixel of
the appearing window as fast as it can.
- It samples the **physical framebuffer**, which changes only when a fully
  composed frame is presented. The compositor surface is updated layer by
  layer, and sampling it could catch a frame half built.
- For every opacity `a` it computes what that pixel must read: the
  wallpaper, the window's own shadow band at `48a/255`, then the client
  colour blended at `a`.
- Every sample must be one of those values, the opacities they imply must
  never decrease, at least three intermediate opacities must be seen, and
  the pixel must end fully opaque.
- Then A takes its key and exits, and killing the shell returns the
  display.

The first run sampled 11 distinct frames over 69 ticks, at opacities
0, 20, 80, 135, 178, 217, 244, 248, 251, 253 and 255: the spring's curve.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.7-negative-control-*`:
- With windows appearing at full opacity, only opacities 0 and 255 are
  seen, and the gate fails at "the window passes through at least three
  intermediate opacities".
- With the shadow not scaled by the fade, the samples are no longer
  explained, and it fails at "every sample is the window blended at some
  opacity, never decreasing".

Open:
- Only window opening is animated. Closing cannot fade, because the
  surface is gone when the owner exits. Minimise does not exist yet.
- The spring constants are fixed.

## Stage 20.6 — translucent overlays over windows (2026-10-13)

Roadmap Stage 18.3, Window Experience: subtle translucency.

**Kernel: keyed blit.** Syscall 202 (first numbered 124, then 132; renumbered on
rebasing onto the Stage 1–19 hardening), `GraphicsCompositorBlitKeyed`, is the
region blit of one of the caller's own surfaces, except that source pixels
equal to `display::TRANSPARENT_KEY` (magenta, `0x00FF00FF`, which no
WovenHat chrome uses) leave the destination as it was.
- It uses the same region snapshot and authority as the other region
  blits.
- `display::Surface::blit_region_keyed` does the work, with two new host
  tests: key skipped and everything else copied; without keyed pixels,
  exactly the region blit, clipping included.
- libwoven adds `TRANSPARENT` and `GraphicsSurface::present_region_keyed`.

**Shell.**
- The launcher panel and the toast are translucent, 216/255, over whatever
  lies beneath, windows and shadows included. Their background is
  region-blended. Their border, title row, selection, and text are an
  overlay drawn on the transparent key and copied with a keyed blit.
- The pointer is now an overlay too. It no longer reads the screen back
  pixel by pixel through `screen_pixel`, which cost about 70 system calls
  per pointer repaint.

**Gate changes.** The Stage 20.2 and 20.3 gates now expect the toast's and
the panel's backgrounds blended over the wallpaper. This is an intended
change of appearance; both pass.

Gate `[S20.6] translucent overlays: PASSED`. The kernel recomputes each
pixel: what lies beneath (wallpaper, the focused window's shadow bands, or
the window), the background blended over it, and the overlay rendered with
its own font and outline.
- A's toast is translucent over the wallpaper.
- A is dragged onto the taskbar's edge. The pointer rests over A's client
  area: its outline and fill are exact, and its transparent cells show A.
- F1 opens the empty launcher over A. The panel is translucent over A's
  client area, A's shadow and the wallpaper, every pixel except the
  pointer's cells, and the pointer stays above it.
- The panel closes, A takes its key and exits, and killing the shell
  returns the display.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.6-negative-control-*`:
- With overlays copied without the key, the gate fails at "the toast is
  translucent over the wallpaper", because the key colour lands.
- With an opaque panel background, it fails at "the launcher panel is
  translucent over a window and its shadow".

**Physical hardware check.** On 2026-10-13 the Stage 15.2 physical WovenFS
qualification ran again on the USB flash drive. The drive is identified by
model `VendorCo ProductCode USB Device`, 8,052,549,120 bytes, and serial
5474001281346643368; the system NVMe disk was never addressed. The run
used the Stage 20.5 commit, in an isolated worktree, with the device
passed raw to the kernel's ATA driver.
- Formatting, writing, remounting, VFS mount, reading, unlink, and unlink
  surviving a remount passed on 1, 2 and 4 CPUs, each with exit code 33.
- Windows then read `WOVENFS1` at LBA 0 with checkpoint sequence 5.
- Evidence is in the worktree's `audit-artifacts/wovenfs-physical-*`.
- This shows the storage stack still works on real flash with the desktop
  stages in place. It is not a native boot of the desktop on physical
  hardware.

Open:
- Translucency is uniform per layer. There is no blur behind translucent
  surfaces.
- Adaptive surfaces (chrome tinted from the wallpaper) were not done in
  this stage. They wait for selectable wallpapers, since there is
  currently one fixed wallpaper.

## Stage 20.5 — depth: window shadows and elevation (2026-10-13)

Roadmap Stage 18.3, Window Experience: dynamic depth.

**Kernel: region blend.** Syscalls 200 and 201 (first numbered 122/123, then 130/131;
renumbered on rebasing onto the Stage 1–19 hardening, which took 122–129),
`GraphicsCompositorBlendRegion` and its shared variant, are the region blit
(120/121) with a uniform opacity in place of the copy. They use the same
region snapshot, so only the exposed part is copied, and the same authority
rules.
- Four values have to fit in three registers. The destination word packs x
  and y as 24-bit signed fields, ample for a compositor bounded at
  1920×1200, with the opacity in the top byte.
- The byte between them is reserved and must be zero; anything else is
  refused.
- libwoven adds `GraphicsSurface::present_region_blended`,
  `compositor_blend_region_shared` and `pack_blend_destination`, with a
  host test of the packing.

**Shell.**
- Every window casts a soft shadow: two bands of black blended over what
  lies beneath, offset down and right, outer band lighter.
- The focused window sits higher: bands at (8 px, 24/255) and
  (4 px, 48/255), against (4, 20) and (2, 40) for the others. Focus changes
  repaint both windows' shadows.
- Shadows are clipped at the taskbar.
- Repaint areas follow a window's footprint (its frame plus its deepest
  shadow), not just its frame, so opening, closing, moving and refocusing
  leave no stale shadow.
- Shadow bands are rendered as solid chunks in the staging surface and
  region-blended into the exposed rectangle.

Gate `[S20.5] window depth: PASSED`. Every shadow pixel checked is
recomputed by the kernel: the wallpaper, then each window's bands, bottom
to top, with `display::blend_pixel`.
- With B focused, B's right and bottom strips carry the deep bands, and
  A's exposed strips carry the shallow ones.
- A shadow pixel really differs from the bare wallpaper.
- Clicking A's title bar swaps the shadows, including where both windows'
  bands overlap.
- Dragging A onto the taskbar's edge: its shadow beside it stops at the
  taskbar, and the accent line and taskbar rows below it are untouched.
- Both applications exit, taking their windows and shadows with them.
- Killing the shell returns the display.
- The first run showed the gate typing to B before the shell had seen A
  go. A key there goes to the still-focused, exited A, so the gate now
  waits for A's window to disappear first, as the Stage 20.2 gate does.

Negative controls, with evidence in the worktree's
`audit-artifacts/stage20.5-negative-control-*`:
- With the focused window given the shallow shadow, the gate fails at
  "focused window B casts the deeper shadow".
- With shadows allowed onto the taskbar, it fails at "shadows stop at the
  taskbar".

Open:
- Shadows are rectangular bands, without blur or rounded corners.
- No dimming of unfocused windows yet.
- A key typed in the instant between an application exiting and the shell
  noticing, within one 250 ms frame, goes to the exited application and is
  lost.

## Stage 20.4 — windows that move and close; budgeted client surfaces (2026-10-13)

Roadmap Stage 18.3, Window Experience, first step: the prerequisite in
`docs/design-stage18-3-window-experience.md`.

**Surface budgets (kernel).** The 320×240 per-surface cap is gone.
- A surface may be up to 1920×1200, the compositor's own bound.
- Pixels are budgeted: one process may hold `OWNER_PIXEL_BUDGET` (1 M
  pixels, 4 MB) across its surfaces, and all processes together
  `SYSTEM_PIXEL_BUDGET` (4 M pixels, 16 MB).
- The existing counts stay: 4 surfaces per process and 16 in total.
- Going over a process's budget is `OwnerFull`, going over the system's is
  `SystemFull`, and a destroy returns its budget.
- The sizes are defaults pending the owner's decision on a desktop
  session's memory target.

Larger surfaces needed three changes to how the registry holds its lock:
- **The registry lock disables preemption, not interrupts.** Copying a
  9 MB surface must not hold interrupts off. Nothing takes the lock from
  interrupt context. The compositor already nests its preemption lock
  under the scheduler's interrupt lock on the exit path, so the same rank
  order holds.
- **Allocation and freeing happen outside the lock.** A new surface is
  allocated between a budget check before and a re-check after, so a
  create that lost a race frees its surface after unlocking. A destroyed
  surface is freed after unlocking. On the exit path, which runs under the
  scheduler lock, it is freed in place.
- **Region blits copy only the region.** The new
  `user_graphics::snapshot_region` copies just the exposed part of a
  surface, so a small repaint of a large window stays small. Previously
  every region blit copied the whole surface.

**Shell.**
- Windows may be as large as the desktop above the taskbar.
- A drag on a title bar moves the window, raised and focused. The frame is
  kept on screen and above the taskbar, and the old and new places are
  repainted on every motion step.
- Each title bar has a close control. A click on it sends the owner
  `WCLS`, without focusing the window. The window goes when the owner
  releases its surface, so an application can finish first or decline.
- hello-gui `--desktop-client` honours `WCLS`, and gains a `c` variant
  with a 400×260 window.

Gate `[S20.4] windows that move and close: PASSED`.
- The kernel checks the budgets directly:
  - oversized dimensions are `OutOfRange`;
  - a 1024×1024 surface fills a process's budget, so a further 1×1 is
    `OwnerFull`, and is admitted once the large surface is destroyed;
  - four processes with 1024×1024 each fill the system, so a fifth is
    `SystemFull`, and is admitted once one process releases.
- On the desktop, real i8042 input:
  - window A opens, then a 400×260 window opens on top with its title bar
    and close control exact;
  - A is dragged by its title bar by (300, 250). It lands at the computed
    place, above the wide window, focused, with its title bar and its
    damage patch exact. Its old place shows wallpaper, and the wide window
    shows where A left it;
  - the wide window's close control makes it exit 0, and it disappears;
  - A, still focused, takes `x` and exits;
  - killing the shell returns the display.
- Stage 20.2's title-bar checks were updated to include the close control,
  an intended change of appearance.

Negative controls, with evidence in
`audit-artifacts/stage20.4-negative-control-*`:
- With dragging disabled, the gate fails at "dragging the title bar moves
  the window and repaints both places".
- With the per-process budget lifted, it fails at "one process is held to
  its pixel budget".

Open:
- No resize: the protocol request is in the design. No minimise or
  maximise.
- At the time of this original gate, pointer events were not forwarded to
  applications; the later Graphics client pointer forwarding slice covers
  Calculator.
- The other compositor paths (whole-surface blit, blend) still copy whole
  surfaces, now under a preemption lock rather than an interrupts-off one.

## Stage 20.3 — WovenStart application launcher (2026-10-13)

Roadmap Stage 18.2, WovenStart: application launcher and search.

**Catalog.** `/share/applications/*.app` holds one small text file per
application, with a `Name=` line and an `Exec=` line. In the command, `%p`
becomes the shell's process id; that is how a launched application finds
the shell, which closes the discovery gap for launched applications. The
shell loads up to eight entries at startup and sorts them by name.
Directories, non-`.app` files and malformed entries are skipped. A missing
catalog, or no `FileRead`, leaves the launcher empty rather than stopping
the shell.

**Launcher.** The start button, or F1, opens a panel above the taskbar. It
has a title row, a search row showing the query, and one row per matching
application, the first highlighted.
- Typing filters by case-insensitive substring. Backspace edits, Tab moves
  the selection, Enter launches the selection, and F1 closes.
- A click on a row launches it; a click outside the panel closes it.
- While the panel is open, keys go to it rather than to the focused window.
- With no match, the panel says "No matches".
- The panel is a layer of its own in the exposed-region repaint, between
  the toast and the pointer.

**Launching and reaping.** The shell starts an application with
`SpawnCommand`, which needs `FileRead` and `ProcessCreate`. The application
is an ordinary user process with the default user capabilities, including
`Ipc` but not `DisplayIo`, and it is the shell's child.
- If the program cannot be started, the shell shows a "Could not start …"
  toast.
- `waitpid` does not block (it returns would-block while the child runs),
  so the shell polls its children every frame and reaps each as it exits.
  Without that, exited applications would stay zombies until the shell
  exited.
- libwoven adds `open`, `close` and `read_dir`. `read_dir` requires a
  255-byte name buffer, because the kernel copies the whole name.
- The kernel adds `task::child_count`, test-only, so the gate can see
  reaping.

**Fixed: double-encoded UTF-8 (since Stage 19.14).** The negative-control
runs for 19.14, 20.1 and 20.2 restored source files through Windows
PowerShell 5.1 `Get-Content`. It decoded UTF-8 as Latin-1 and wrote the
result back as UTF-8, so every `×` and `—` in doc comments in
`compositor.rs`, `libwoven/src/lib.rs` and `woven-shell` became mojibake,
and some was committed. No code was affected. The runs were repaired here.
Negative controls now edit and restore files byte for byte through Python.

Gate `[S20.3] application launcher: PASSED`. The catalog is Alpha, Beta and
Gamma, where Gamma names a missing program, plus a stray `notes.txt`. The
shell runs with `FileRead` and `ProcessCreate`, driven by real i8042 input.
Every panel state is compared with the kernel's own rendering.
- F1 opens the sorted catalog, without `notes.txt`. Tab moves the
  selection. Typing `be` filters to Beta.
- Enter launches Beta: its window opens, the panel closes, and the shell
  has one child. Beta takes `y` and exits, and the shell reaps it (child
  count 0).
- The start button opens the panel, and clicking Alpha's row launches it.
  Alpha takes `x` and exits, and is reaped.
- `g` then Enter shows "Could not start Gamma", with no child.
- `zz` shows "No matches", Enter does nothing, and F1 closes.
- Killing the shell returns the display.

Negative controls, with evidence in
`audit-artifacts/stage20.3-negative-control-*`:
- With reaping disabled, the gate fails at "Beta takes its key, exits, and
  is reaped".
- With the search ignoring the query, it fails at "typing filters the
  list".

Open:
- The catalog is read once at startup, with no reload, and up to 8 entries.
- No icons, no categories, and no search beyond names.
- Launched applications get the default user capabilities; there is no
  per-application manifest of capabilities.
- Not started at boot yet (see Stage 20.1).

## Stage 20.2 — desktop applications: windows, taskbar entries, tray, notifications (2026-10-13)

Roadmap Stage 18.1, Desktop, completed.

**Protocol.** Applications talk to woven-shell over IPC. Each message
starts with a four-byte tag:
- `WOPN`, token, width, height, title: open a window showing a surface the
  sender shared with the shell.
- `TRAY`, token: show a 16×16 surface in the system tray.
- `NOTE`, text: show a notification toast for 4 s.

A surface is accepted only if the sender owns it. `libwoven::surface_owner`
reads the owner from the token, and the kernel enforces that name on every
operation. A window or icon disappears when its surface does, normally
because its owner exited; the shell's per-frame damage check detects this.
Keystrokes go to the focused window's owner as one-byte messages.

**Shell.**
- Windows cascade from (40, 40) with a title bar above the client area,
  focused or unfocused, and never overlap the taskbar.
- The taskbar has one entry per window, highlighted when focused. Clicking
  an entry, or a window, raises and focuses it.
- Tray icons sit right to left from the clock.
- The toast is drawn top-right.
- The pointer is a two-tone arrow, hidden until the mouse first moves.
- The shell waits on input, messages and damage together, with a 250 ms
  frame clock.
- Without the `Ipc` capability it runs the desktop with applications
  disabled, rather than failing; this keeps Stage 20.1's minimal
  capability set working.

**Exposed-region redraw.** Every change becomes a repaint of one
rectangle, recomposed completely, bottom to top:
1. the desktop;
2. tray icons;
3. each window: its title bar, then its client surface;
4. the toast;
5. the pointer, drawn over what is already on screen.

Each layer contributes only its part inside the rectangle. This needs
syscalls 120, `GraphicsCompositorBlitRegion` (own surface), and 121,
`GraphicsCompositorBlitRegionShared`. They carry the same authority as the
whole-surface blits and are backed by `display::Surface::blit_region`, with
two new host tests. A whole-surface blit would paint over windows stacked
above outside the rectangle.

Client damage still goes through `GraphicsCompositorBlitDamage`. That blit
can land over a higher window, so the shell repaints the damaged rectangle
before presenting, and the overdraw never reaches the screen.

**Test-only change.** The i8042 loopback helpers (`mouse::loopback`,
`keyboard_loopback`) are now compiled for this gate too.

Gate `[S20.2] desktop applications: PASSED`. The shell runs with two
hello-gui `--desktop-client` applications, driven by real i8042 pointer
packets and keyboard scancodes.
- Application A opens a window. The kernel checks against its own
  rendering:
  - the window's frame, and the focused title bar with the kernel font;
  - A's taskbar entry;
  - the green tray icon;
  - the toast, with its border.
- A redraws a patch and reports damage, and the patch appears.
- B opens on top and focused: B covers A at the overlap, A's title bar
  turns unfocused, and both entries show the right state.
- The toast expires and the wallpaper shows through.
- The pointer moves onto A's entry and clicks: A is raised. The pointer is
  drawn at its position.
- `x` is typed: A receives exactly its key and exits 0. Its window, entry
  and tray icon disappear, and B is focused.
- A click in B, then `y`: B receives its key and exits, leaving the desktop
  empty.
- Killing the shell returns the display to the terminal.

The Stage 20.1 gate still passes with the extended shell.

Negative controls, with evidence in
`audit-artifacts/stage20.2-negative-control-*`:
- With keys sent to the bottom window instead of the focused one, the gate
  fails at "key x routed to focused window A".
- With taskbar entries ignoring clicks, it fails at "taskbar click raises
  window A".

Open:
- **Discovery.** Applications learn the shell's process id from their
  arguments; there is no name service yet.
- **Window controls.** No move, resize, close button or minimise. Windows
  are limited to the 320×240 surface cap, and the shell takes four windows
  and four tray icons.
- **Notifications** are one toast at a time; there is no history. The
  Notification Center is 18.4.
- **Input routing.** Only printable ASCII keys are forwarded. Pointer
  events are not forwarded to applications yet.
- **Not started at boot yet** (see Stage 20.1).

## Stage 20.1 — WovenShell desktop: wallpaper, taskbar, clock (2026-10-13)

Roadmap Stage 18.1, Desktop, first part. Repo stage 20 is roadmap Stage 18,
WovenShell. Stages 18–19 of this repo were roadmap Stage 17, WovenCompositor.

**Desktop program.** `apps/woven-shell` is a new userspace program, a
separate crate built like hello-gui.
- It takes the compositor lease, and switches to display scale 2 on a
  screen of at least 1280×800 so the 5×7 font stays legible.
- It draws:
  - a procedural wallpaper: a vertical gradient with a faint woven grid;
  - a translucent taskbar with an accent line;
  - the WovenStart button, a placeholder until 18.2;
  - a clock (`HH:MM`, or `HH:MM:SS` with `--seconds`).
- It wakes four times a second through `FrameClock` and repaints only the
  clock's tiles when the text changes. It drains input, since nothing on
  the desktop takes input yet.

Client surfaces are capped at 320×240, so the desktop is never one surface.
The scene is a pure function of position and state. The shell renders it
in software into a `libwoven::Canvas`, one 320×40 tile at a time, uploads
each tile into one staging surface and blits it into place. Any region is
repainted by rendering the tiles that cover it.

**Kernel.**
- `kernel/src/rtc.rs`: CMOS real-time clock.
  - Readings are taken only while update-in-progress is clear, and accepted
    when two consecutive readings agree.
  - BCD and binary, and 12- and 24-hour forms, are decoded. Invalid digits
    and out-of-range fields are refused.
  - It is serialised by its own lock, with bounded polling. Times are UTC,
    the clock's own zone.
- Syscall 119, `ClockRead`: returns the packed date-time. It needs no
  capability, like `Getticks`.
- Syscall 118, `GraphicsSurfaceWrite(handle, rect, pixels)`: uploads
  little-endian pixels into one of the caller's surfaces.
  - The rectangle is checked against the surface before anything is copied,
    which bounds the copy by the surface caps.
  - Pixels are copied from user memory with no lock held, and the handle is
    re-resolved for the write, so a surface destroyed in between is refused.
  - The unused high byte is cleared.
  - `display::Surface::write_pixels` does the write, with two new host
    tests.

**libwoven.** `GraphicsSurface::write` and `write_canvas`, `Canvas`
(clipped fill, blend, shade and text), `blend`, `clock` / `DateTime`, and
`font`. The font is the kernel's own source file, so client text matches
kernel text pixel for pixel.

**Defects found and fixed.**
- **Kill path skipped display teardown (since Stage 18).** A process
  killed from outside is torn down by `complete_process_termination`, not
  `exit_current_process`. That path never released the process's client
  surfaces or its compositor lease, so a killed compositor or desktop kept
  the screen forever. Both are now released there, under the same
  scheduler lock as on the exit path.
- **App linker script missed large-model sections.** The large code model
  emits `.ltext`, `.lrodata`, `.ldata` and `.lbss`, which `link.x` did not
  match. The linker placed them as orphans in the first, read-execute
  segment, so the shell's first write to a static faulted. No earlier app
  had a writable static. The fix is in all three apps.
- **Stale images.** Each app now has a `build.rs` that reruns when
  `link.x` changes, because Cargo does not track files named only in link
  arguments.

Gate `[S20.1] woven-shell desktop: PASSED`. QEMU's 1280×800 framebuffer
gives a 640×400 desktop at scale 2.
- The kernel checks the clock decoder against fixed vectors, then reads the
  hardware clock.
- It starts the shell as a system service (`Console`, `DisplayIo`,
  `InputIo`) and waits for it to take the display.
- It checks the geometry against the framebuffer and the scale rule.
- It checks three 40×40 wallpaper blocks against the formula restated in
  the kernel.
- It checks the accent line and the translucent taskbar body.
- It checks the start button against the label rendered with the kernel's
  own font.
- It checks the clock region against the kernel's own reading of the
  hardware clock, allowing up to 2 s of lag, then waits for a later
  reading, so the clock is seen to advance.
- It reads the physical framebuffer back at scale 2.
- It kills the shell, then checks that the lease is gone, presentation has
  stopped, and terminal text reaches the framebuffer again.

Negative controls, with evidence in
`audit-artifacts/stage20.1-negative-control-*`:
- With the kill-path teardown removed (the pre-fix behaviour), the killed
  shell keeps the display and the gate fails at "display returned to the
  terminal".
- With the shell's clock never repainting, it fails at "clock advances
  with the hardware clock".

Open:
- **Not yet started at boot.** The desktop runs under its acceptance gate.
  Starting it at boot belongs with session start: the login manager (18.7),
  or a boot option before that.
- **18.1 items still to come:** client windows on the desktop with a
  taskbar window list, the system tray, and notification toasts.
- **WovenStart** is a placeholder button; the launcher is 18.2.
- **Clock** shows UTC: there is no time-zone configuration.
- **Kill-path cleanup now matches explicit exit.** Scheduler-driven remote
  termination releases thread handles, queued notifications, terminal
  foreground ownership, and process sockets before publishing the zombie.
  Host Clippy and the complete host test suite pass after this fix; physical
  USB and multi-monitor qualification remain outside the QEMU gate.

## Stage 19.14 — integer display scaling and paced animation (2026-10-14)

Roadmap Stage 17's DPI-scaling and animation items. This closes the
compositor stage except multiple monitors, which is deferred (see Open).

**Scaling.** The compositor keeps an integer display scale, 1 to
`MAX_SCALE` (4).
- The surface is the framebuffer divided by the scale, clamped to the
  existing 1920×1200 bound. `display::present_scaled` draws each surface
  pixel as a scale×scale block, and scales the damage with it. `present`
  is now `present_scaled` at scale 1, so every earlier presentation path
  is unchanged. Three new host tests in `tests/display_compositing.rs`
  cover:
  - block expansion;
  - damage scaling;
  - screen clipping, plus scale 1 matching `present` byte for byte.
- Changing the scale reallocates the surface, cleared to black and wholly
  damaged; the holder redraws at the new geometry. The allocation and the
  free happen with the compositor lock released, and the lease is
  re-checked afterwards because the holder can exit in between.
- A new lease holder always starts at scale 1, whatever the previous holder
  chose. The reset happens in `claim`, not in `release_owner`, because the
  exit path holds the scheduler lock and must not allocate.

Two syscalls, both requiring `DisplayIo` and the lease:
- 116, `GraphicsCompositorInfo`: returns `width | height << 24 |
  scale << 48`.
- 117, `GraphicsCompositorScale(scale)`: returns the new geometry in the
  same packing, or ERROR for a scale outside 1–4.

libwoven adds `CompositorInfo`, `compositor_info` and
`compositor_set_scale`. Pointer motion stays relative; the window manager
tracks the pointer in surface coordinates.

**Frame pacing.** libwoven `FrameClock` paces at a fixed period on the
kernel timer, with no new kernel interface.
- Frame boundaries are absolute (`start + n * period`), so one late frame
  does not push back the rest.
- A late caller is told how many frames it skipped, so an animation stays
  on time rather than slowing down.
- `wait(mask)` uses the existing `WaitAny` deadline, so a window manager
  can sleep on input, messages or damage until the next frame. A mask of 0
  sleeps for the frame alone.

Gate `[S19.14] display scaling and frame pacing: PASSED`, on QEMU's
1280×800 framebuffer:
- The kernel checks that the boot compositor is at scale 1 and that scales
  0 and 5 are refused.
- It takes the display, sets scale 2 (640×400), and checks that the
  repaint writes all 1280×800 pixels. It blits an 8×8 surface at (10, 10)
  and reads the physical framebuffer back: the 16×16 block at (20, 20) has
  the colour, and its neighbours do not.
- The unprivileged probe is refused info and scale (exit 223 otherwise).
- hello-gui `--scaled-compositor`:
  - checks that info is refused before claiming and reads scale 1 after;
  - checks that out-of-range scales leave the geometry alone;
  - sets scale 2 and checks the exact half geometry, black corners, and an
    out-of-range read refused;
  - checks the first present writes the whole physical screen;
  - animates a square for 12 frames at 2 ticks per frame. No frame may
    arrive before its due tick, each frame writes exactly the 200×150
    backdrop region at 2×2, and the whole run takes at least 24 ticks.
    Observed: 12 frames in 25 ticks.
- The kernel then re-reads the final frame at scale 2 and checks that the
  next holder starts at scale 1.

Negative controls, with evidence in
`audit-artifacts/stage19.14-negative-control-*`:
- With presentation ignoring the scale, the gate fails at "rescale repaints
  the whole screen".
- With a `FrameClock` that never sleeps, hello-gui fails at step 176 (a
  frame arrived early).

Open:
- **Multiple monitors (deferred).** The firmware framebuffer is one
  output, and there is no GPU or virtio-gpu driver to enumerate more. The
  roadmap says native GPU support must not block the first GUI release, so
  this waits for a display-driver stage.
- Scale is whole-number only, and is not detected from the display: there
  is no EDID or physical-size source.
- No vsync. Frames follow the 100 Hz timer, so the shortest period is
  10 ms and frames are not aligned to the display's refresh.
- Rescaling transiently holds two surfaces (up to about 18 MB). This is
  bounded because only the holder can rescale.

## Stage 19.13 — translucent windows and shadows (2026-10-14)

Roadmap Stage 17's transparency and shadow items.

Pixels carry no alpha channel (`0x00RRGGBB`), and per-pixel alpha would have
made every existing surface, whose high byte is zero, transparent. So
translucency is a property of the blit. `display::Surface::blend` draws a
source with uniform opacity, using `display::blend_pixel`: per channel,
`(over * a + under * (255 - a) + 127) / 255`. That is exact at both ends:
opacity 0 changes nothing and reports no damage, and 255 is a plain blit.
Clipping matches `blit`. Three new host tests in
`tests/display_compositing.rs` cover:
- endpoints, rounding at the midpoint, and colour-over-itself identity;
- opacity 255 matching `blit` pixel for pixel, and opacity 0 touching
  nothing;
- clipping off the corner, so only the overlap is mixed.

Two syscalls carry the opacity in their third argument. The existing blits
(102, 110) ignore that argument, so it could not be repurposed without
breaking their ABI:
- 114, `GraphicsCompositorBlend`: one of the caller's own surfaces. This is
  how a window manager draws shadows and translucent chrome.
- 115, `GraphicsCompositorBlendShared`: a shared client window.

The authority is the same as the corresponding blits: `DisplayIo` plus the
lease, and the caller's own token for 114 or a grant naming the caller for
115. An opacity above 255 is refused rather than truncated. libwoven adds
`GraphicsSurface::present_blended` and `compositor_blend_shared`.

Gate `[S19.13] translucency and shadows: PASSED`:
- The probe is refused both blends.
- hello-gui `--alpha-compositor` draws an opaque backdrop, a black shadow
  from its own surface at 96/255 offset under the window, and the Stage 19.7
  `--client`'s shared window at 160/255 over both.
- It checks four regions against the formula, restated independently in the
  app: backdrop only, shadow only, window over the backdrop, and window over
  the shadow.
- It checks that opacity 0 leaves the screen unchanged, that the private
  surface is refused, and that opacity 256 is refused.
- The kernel recomputes the four regions with its own `blend_pixel` after
  both processes exit.

Local 1/2/4-CPU evidence is recorded in the
[Stage 19.13 transparency audit](docs/audit-stages-consolidated.md).

Negative control: with `blend` copying opaquely, the compositor fails its
region check (exit 167) and the gate fails. Evidence is in
`audit-artifacts/stage19.13-negative-control-*`.

Open:
- No per-pixel alpha, so no anti-aliased edges or soft shadows; a soft
  shadow needs several stacked blends.
- No animation: frame timing belongs to the window manager, and there is no
  vsync source.

## Stage 19.12 — PS/2 pointer input; keystrokes delivered at IRQ time (2026-10-14)

**Defect fixed (Stages 19.5/19.6).** The keyboard IRQ handler only queued raw
scancodes. They became WovenInput events only when a consumer called
`keyboard::poll` or `read_bytes`, which in practice meant the kernel shell
loop or a userspace `read`. A focus holder asleep in `InputWait` or `WaitAny`
polls nothing, so real keystrokes reached it only if something else happened
to be polling. The 19.5 and 19.6 gates published keys directly into WovenInput
and never exercised the hardware path. The handler now decodes and publishes
at IRQ time. Every lock on that path is an IRQ mutex at rank 10, and the two
`PreemptMutex`es an IRQ can interrupt are also rank 10, so neither inversion
nor recursion is possible. The polled fallback (`poll_legacy`) now leaves
auxiliary bytes for the mouse.

**Mouse.** `kernel/src/mouse.rs` drives the i8042 auxiliary port.
- Initialisation runs with interrupts off and polls with bounds: enable the
  port, set defaults, enable reporting (each acknowledged), then set the
  auxiliary IRQ bit and clear the auxiliary clock-disable bit in the
  controller configuration.
- IRQ12 is routed through the IOAPIC beside the keyboard. Unlike the keyboard,
  the mouse is optional: an unusable GSI leaves it unrouted rather than
  failing boot. Under the legacy PIC it is unmasked only if a mouse answered.
  Boot logs `[BOOT] ps2 mouse present=`.
- The handler takes only bytes flagged auxiliary and decodes three-byte
  packets: bit-3 resynchronisation, 9-bit signed deltas, Y flipped to screen
  orientation, overflow packets dropped whole, and button edges. It publishes
  `PointerMove` / `PointerButton`, which are now production WovenInput
  events rather than Stage 13.7 probe-only.

**Userspace.** `InputPoll` and `InputWait` encode pointer motion as kind 6
(two's-complement dx/dy halves) and buttons as kind 7. libwoven adds
`InputEvent` and `input_poll_event`; `input_poll` and `input_wait` keep
returning keys only, skipping pointer events. The window manager tracks the
pointer from relative motion; a left click on an uncovered part of the bottom
window raises and focuses it (click-to-focus).

Gate `[S19.12] pointer input: PASSED`. Every input travels the real i8042
path: command 0xD3 presents a byte as if the mouse sent it (IRQ12, the mouse
driver), and 0xD2 as if the keyboard did (IRQ1, the keyboard driver).
- The kernel checks the packet decoder, then the raw paths into a focus
  queue: a mouse packet arrives as motion plus press, then release; a
  keyboard make code arrives as a decoded key with no one polling.
- hello-gui `--wm` with two clients: the kernel moves the pointer into A
  only, clicks, and types `x`; then moves into B only, clicks, and types `y`.
- Each client must receive exactly its own key, and the kernel reads B on top
  at the overlap.

Negative control: with the IRQ-time decode removed, which was the pre-19.12
behaviour, the gate fails at "keyboard bytes through IRQ1 are decoded at IRQ
time". Evidence is in `audit-artifacts/stage19.12-negative-control-*`.

Open:
- USB boot and report-protocol HID pointer reports are decoded into
  `WovenInput` events by the xHCI worker; Stage 13.13 and 13.16 QEMU gates
  cover the software paths. Physical USB qualification, multi-application
  devices, and virtio-input remain open.
- The PS/2 boot-mouse path has no IntelliMouse wheel or acceleration; report
  protocol wheel events are supported by the HID decoder.
- The pointer position lives in the window manager. WovenShell draws a bounded
  software cursor overlay; there is no hardware cursor.

## Stage 19.11 — userspace window manager: stacking, raise, focus, input routing (2026-10-14)

**Roadmap mapping.** This repository's Stage 18–19 numbering has drifted
from `docs/master-development-roadmap.md`: the work from Stage 18.1 onward is
the roadmap's **Stage 17, WovenCompositor**. That stage lists windows,
surfaces, compositing, damage tracking, z-order, transparency, shadows,
animations, input routing, focus, multiple monitors and DPI scaling. Its
security rule is that applications must never directly control another
application's surface. The desktop shell is roadmap Stage 18 and follows it.
This increment covers z-order, focus and input routing.

**What it shows.** A complete userspace window manager runs on the existing
primitives, with no new kernel interface:
- the compositor lease and display (19.4, 19.3);
- input focus and `WaitAny` (19.5, 19.8);
- shared client surfaces (19.7);
- IPC.

hello-gui `--wm` takes the display, and with it focus. It receives each
client's shared-surface token, position and colour over IPC, and stacks the
windows, the older client at the bottom. It draws them bottom to top and, after
every redraw, checks its own screen: a pixel inside both windows must show the
top window, and an uncovered corner of the bottom window must show the bottom
window. Tab raises and focuses the other window. A typed character is forwarded
over IPC to the focused (topmost) window's client only. Clients never touch
each other's windows; the window manager reads them only through their grants.

Gate `[S19.11] window stacking and focus: PASSED`:
- `--wm` runs with `Console | DisplayIo | InputIo | Ipc`. Two ordinary
  `--wm-client` processes share overlapping 120×80 windows: A at (200,150),
  red; B at (260,190), blue.
- Whenever the window manager is asleep for input, the kernel publishes the
  next of Tab, `x`, Tab, `y` through the keyboard path.
- A must receive exactly `x` (it is on top after the first Tab), and B
  exactly `y` (on top again after the second Tab). Each client then waits a
  quiet period for stray keys. A misrouted key that arrives later still fails
  the gate: the window manager's send to an exited client fails, and the
  intended client never gets its key.
- The kernel reads the overlap (blue, B on top) and each window's private
  area from the compositor, and confirms lease and focus are released.

Negative control: with the window manager forwarding to the bottom window,
the wrong client receives the key (exit 156) and the gate fails. Evidence is
in `audit-artifacts/stage19.11-negative-control-*`.

The first run failed the window manager's own check (step 144): its "bottom
window still visible" probe used a corner the raised window covers in the
swapped layout. That was a test defect. The probe now picks an uncovered
corner. Evidence is in `audit-artifacts/stage19.11-probe-point-1cpu-*`.

Open (remaining roadmap Stage 17 items after the accepted Stage 19 gates):
- A window move or lower redraws all windows: the window manager tracks no
  exposed regions, and the kernel compositor has no z-order of its own,
  because stacking is policy, held in userspace.
- Multi-monitor output and per-display policy are not implemented.
- Scaling is integer-only and animation pacing is bounded software timing;
  fractional DPI and hardware GPU acceleration remain outside this gate.

Pointer-driven focus is closed by Stage 19.12: IRQ12 mouse packets and
IRQ1 keyboard input reach the event-driven window manager, clicks raise and
focus the selected window, and keys route only to that client. See the
[Stage 19.12 pointer audit](docs/audit-stages-consolidated.md).

## Stage 19.10 — compositor follows the framebuffer; one framebuffer writer (2026-10-12)

**Geometry.** The boot compositor is now sized to the framebuffer instead of
a fixed 640×480. `compositor::init_for_display` reads
`terminal::framebuffer_info` after the heap self-test and clamps to
1920×1200. A 4K display would otherwise cost a 33 MB kernel allocation; the
clamp bounds it near 9 MB, and a larger display shows the compositor in its
top-left corner. The self-tests that reset and re-initialise the compositor
still use the fixed default, so their expectations are unchanged.

**Single writer.** The early-boot `Console` no longer holds the framebuffer
slice. It keeps only the geometry, and each operation (character, clear,
scroll, backspace, desktop present) borrows the buffer once through
`terminal::with_text_framebuffer`. That accessor refuses while the
compositor owns the display. Every framebuffer write after `terminal::init`
now goes through the terminal lock: console, userspace terminal and
compositor. This closes the Stage 19.3 open item, which noted that the boot
console wrote outside the lock and had to move behind it before the
compositor could be activated at boot. `Console` lost its lifetime parameter;
its users in `shell.rs`, `smp.rs` and `physical_probe.rs` changed
accordingly.

Gate `[S19.10] display geometry and single writer: PASSED`:
- The boot compositor's dimensions equal the clamped framebuffer size
  (1280×800 on QEMU).
- Taking the display repaints exactly that many pixels, and the far corner of
  the framebuffer shows the compositor background, where before it was
  outside the compositor.
- Boot-console output leaves the framebuffer checksum unchanged while the
  compositor owns the display, and changes it after release.

Negative control: with `with_text_framebuffer` ignoring display ownership,
the gate fails at "boot console suppressed while compositing". The evidence
is labelled in `audit-artifacts/stage19.10-1cpu-1791825577739619200`.

Open:
- The production boot still does not activate the compositor. That is now a
  policy decision (when a desktop process starts) rather than a safety
  prerequisite.
- The kernel shell's desktop renderer keeps its own retained surface,
  presented through the same lock; it is not a compositor client.

Next prerequisite: a desktop/shell process that takes the lease at startup
and composites client windows, which is the Phase 5 desktop itself.

## Stage 19.9 — client damage notification and damage-only compositing (2026-10-12)

A client now tells the compositor what changed, and the compositor copies only
that.

- **Declaring damage:** syscall 112 (`GraphicsSurfaceDamage(handle, rect)`)
  lets the owner record a changed rectangle. The token is bound to the
  caller, and no capability is needed, since only the caller's own pixels are
  involved. Damage is clipped to the surface and unioned into one bounding
  rectangle per slot, so per-slot state stays fixed-size. It is cleared when
  the slot is freed. If the surface is shared, the grantee's damage waiter is
  woken after the registry lock is released.
- **Compositing damage:** syscall 113 (`GraphicsCompositorBlitDamage(token,
  origin)`) lets the lease holder take the pending damage of a surface shared
  with it, copy just that region, and blit it at its offset within a window
  placed at `origin`. It returns the region, or 0 when nothing was pending.
  The grant rules are the same as for the whole-surface shared blit.
- **Waiting on damage:** `WaitAny` gains source bit 2, damage on any surface
  shared with the caller, which requires `DisplayIo`. Waiters live in a
  bounded four-entry table in the registry. A grantee's exit drops its entry
  without waking, since that path holds the scheduler lock. The usual rules
  apply: one registration per grantee, live-holder refusal, and take-over
  from a dead holder.
- **libwoven:** `GraphicsSurface::damage`, `compositor_blit_damage ->
  Option<Region>`, and `wait::DAMAGE`.

Gate `[S19.9] damage compositing: PASSED`:
- The kernel first checks the damage rules: clipping, union, reads only
  through a grant, cleared on take, rectangles outside the surface ignored,
  damage dies with the slot.
- The probe is refused damage waits, damage on a foreign surface, and
  damage-only compositing.
- hello-gui `--damage-compositor` takes the lease and composites the client's
  declared full-window damage, (0,0,40,30); a second take finds nothing. It
  then sleeps in `WaitAny(DAMAGE)`.
- Once the kernel sees it `Blocked`, it tells `--damage-client` to go. The
  client recolours (10,5,4,3) and declares it, and recolours (30,20,2,2)
  without declaring it.
- The compositor must be woken before its 500-tick deadline and copy exactly
  (10,5,4,3). The screen must show the new colour there and the old colour
  where the undeclared change was. The kernel re-reads both pixels after
  both processes exit.

Negative controls (evidence in `audit-artifacts/stage19.9-negative-control*`):
- With the wakeup removed from `mark_damage`, the first version of the gate
  still failed, but only through the kernel's wait for the client timing out.
  The compositor itself passed: `WaitAny` is level-checked, so at its
  deadline it correctly reported damage that had arrived without a wakeup.
- The compositor now times its wait and fails if only the deadline woke it.
  The same control then fails crisply at that step (exit 111).

Open:
- One bounding rectangle per surface: two small distant changes copy the
  span between them.
- Damage is not tied to a particular blit position; the compositor supplies
  the window origin each time.
- No per-client damage queue: `WaitAny` says some shared surface has damage,
  and the compositor asks each of its clients.

Next prerequisites: the compositor surface following real framebuffer
geometry instead of a fixed 640×480, and boot-time activation once the early
boot console writes through the terminal lock.

## Stage 19.8 — one sleep over input and IPC (2026-10-12)

A process can now sleep until any of several sources is ready. Syscall 111
(`WaitAny(deadline, mask)`) takes a mask: bit 0 is input (`InputIo`, live only
while the caller holds focus), bit 1 is the caller's pid mailbox (`Ipc`). It
returns the mask of ready sources, or 0 when the timer reaches `deadline`.
Nothing is consumed: the caller follows up with `InputPoll` or
`MessageReceive`. Empty or unknown masks are refused, and so is waiting on
input alone without focus, which could never complete.

The pid mailbox gained a waiter. `LegacyEndpoint` holds one optional `TaskId`;
`ipc::send` takes it under the IPC lock and calls `signal_event` after
releasing it. Endpoint teardown drops the waiter without waking, as focus
withdrawal does. `woven_input` gained `register_waiter`, `clear_waiter` and
`pending`, so the same focus waiter slot serves both `InputWait` and
`WaitAny`.

The wait registers with every requested source, then re-checks readiness, then
sleeps. An event between registration and sleep latches the scheduler's
pending permit, so it cannot be lost. A slot held by another live task of the
same process is refused; a slot held by a dead task is taken over, with
liveness checked outside the source's lock. All registrations are dropped
before returning.

Gate `[S19.8] combined wait: PASSED`:
- An unprivileged probe is refused `WaitAny` on input, and refused empty and
  unknown masks.
- hello-gui `--multiplex <peer>` takes focus and sleeps over both sources.
- The kernel registers a stand-in peer mailbox. Once it sees the client
  `Blocked`, it sends a message; the client must report exactly `MESSAGE` and
  receive `ping` from the peer.
- Once the client is `Blocked` again, the kernel publishes a key; the client
  must report exactly `INPUT` and read `z`.
- A 20-tick timed wait then returns 0, no earlier than its deadline, and focus
  returns on exit.

Negative control: with the wakeup removed from `ipc::send`, the client sleeps
through the message and wakes on the key with both sources ready (exit 83), so
the gate fails. Evidence is in `audit-artifacts/stage19.8-negative-control-*`.

Two problems found while bringing this up:
- The first design had the client reply to the stand-in peer. `may_ipc_with`
  refused it, correctly: the peer is a bare mailbox with no process
  credentials. The reply step was removed instead of relaxing the policy.
  Evidence is in `audit-artifacts/stage19.8-reply-refused-1cpu-*`.
- **Harness defect.** The three app harnesses built the app with
  `cargo build`, inheriting `CARGO_TARGET_DIR` from the caller's shell. With
  the project's documented environment, the fresh binary went to the kernel's
  target tree, while the kernel embedded the stale binary in
  `apps/*/target`. Earlier gates were unaffected only because the app had
  been built by hand from the same source before each run. All three
  harnesses now build with the app's own target directory pinned and the
  kernel's build-directory override removed.

Open:
- The pid mailbox has one waiter; handle-based endpoints keep their existing
  multi-waiter queues and are not yet `WaitAny` sources.
- Syscall 206 now exposes timer/event objects as bounded `WaitAny` sources;
  the current focused gate covers registration safety, the full userspace
  timer/event scenario, and bounded manual-reset-event fan-out. One-shot timer
  fan-out remains intentionally single-consumer; handle-based endpoint sources
  remain outside syscall 206.

Next prerequisite: client-to-compositor damage notification, so a compositor
sleeping in `WaitAny` redraws only what a client changed.

## Stage 19.7 — client surfaces delegated to a compositor process (2026-10-12)

A separate compositor process can now draw other clients' windows, with each
client's consent.

**Sharing:** a client grants one process read access to one of its own
surfaces with syscall 109 (`GraphicsSurfaceShare(handle, grantee)`; grantee 0
revokes). Sharing needs no capability: a process may always decide who reads
its pixels. The token still goes through `caller_surface`, so only the owner
can share. Each surface carries at most one grant, in the slot. The grant is
cleared whenever the slot is freed, by destroy or by the owner's exit, so it
never outlives the surface or carries over to the slot's next occupant. Pids
are never reused, so a grant naming an exited process is inert.

**Compositing:** the lease holder draws a shared surface with syscall 110
(`GraphicsCompositorBlitShared(token, x | y << 32)`). It requires `DisplayIo`
and the lease. The token is the owner's, passed out of band, so it is
deliberately not bound to the caller. Instead
`user_graphics::snapshot_shared` requires slot, generation and owner to match
and the slot's grant to name the caller: a token alone confers nothing. The
snapshot-then-blit lock pattern is the same as `blit_for`.

**libwoven:** `GraphicsSurface::share_with` / `unshare`,
`compositor_blit_shared`, and typed IPC: `message_send` /
`message_receive` over the existing syscalls 11 and 12.

Gate `[S19.7] surface delegation: PASSED`:
- The kernel first checks the grant rules: nothing is shared by default; the
  grantee is allowed while a stranger and the owner itself are not; stale and
  forged tokens are refused even for the grantee; revocation works; a grant
  dies with its surface.
- hello-gui `--compositor` (`Console | DisplayIo | Ipc`) takes the lease.
  hello-gui `--client <pid>` runs as an ordinary process: it shares one
  surface with the compositor, keeps a second private, checks it cannot blit
  anything itself, and sends both tokens over IPC.
- The compositor must draw the shared surface and see the client's colour on
  the screen. It must be refused the private surface even with its genuine
  token, and refused the shared one through the owner-only blit path. It then
  presents and replies; the client waits for that reply so its surfaces stay
  alive until drawn.
- After both exit, the kernel reads the client's colour at two corners of the
  composited window, and checks that the client's surfaces and grants and the
  lease are gone.

Negative control: with the grant check bypassed in `snapshot_shared`, the gate
fails at the grant-rules check. Evidence is in
`audit-artifacts/stage19.7-negative-control-*`.

Open:
- A shared surface is copied whole at each blit, and there is no damage
  notification from client to compositor: the compositor redraws when it
  chooses.
- One grantee per surface.
- Token transfer is by plain IPC bytes. It cannot be forged into access,
  because the grant is the authority, but it is not a kernel-transferred
  handle.
- IPC receive is still non-blocking, so both test processes poll with a
  bounded deadline.

Next prerequisites: a blocking IPC receive (or a combined wait with input),
and client-to-compositor damage notification.

## Stage 19.6 — event-driven input wait (2026-10-12)

The focus holder can now sleep until input arrives instead of polling.
Syscall 108 (`InputWait(deadline)`) returns the next event, or `WOULD_BLOCK`
when the timer reaches `deadline` (`u64::MAX` waits indefinitely). It has the
same authority as `InputPoll`: `InputIo` plus focus. libwoven wraps it as
`input_wait(Option<Ticks>)`.

`woven_input::wait_for` registers the calling task as the focus waiter under
`FOCUS`, releases the lock, and sleeps with
`task::wait_for_event_until(deadline)`. `publish` takes the waiter out under
`FOCUS` and calls `task::signal_event` after releasing it. No wakeup is lost:
a publish between registration and sleep latches the scheduler's pending
permit, and the sleep consumes it. Unrelated wakeups simply re-check.

Lock order:
- The task-exit path takes the scheduler lock, then the compositor lock, then
  `FOCUS`. So `FOCUS` is never held while the scheduler lock is taken: waking
  and liveness checks happen with it released.
- Focus withdrawal forgets the waiter rather than waking it. It runs under the
  scheduler lock on the exit path, and only the owner's exit withdraws focus,
  which takes every task that could be waiting with it.

Waiter rules:
- One task per process may wait. A second concurrent waiter is refused, unless
  the registered one has died, in which case `task::task_live` (checked after
  releasing `FOCUS`) lets the newcomer take over by compare-and-set. Task ids
  are never reused, so a dead answer is final.
- A timed-out wait unregisters itself and returns any event that raced in
  after its last check.

Gate `[S19.6] blocking input wait: PASSED`:
- An unprivileged probe must be refused `InputWait`, with a bounded deadline
  so a broken check cannot hang the gate.
- hello-gui `--wait` gains focus and reads `o`, `k`, Enter through
  `input_wait(None)`. Before publishing each key, the kernel waits until the
  client's task is `Blocked`. A polling client is only ever `Ready`, so this
  proves it sleeps in the kernel and is woken by the publish itself.
- The app then makes a 20-tick timed wait with nothing pending. It must return
  `None`, and no earlier than the deadline.
- Focus returns on exit.

Negative control: with the sleep in `wait_for` replaced by `yield_now` (a
spinning wait), the gate fails at "client sleeps while waiting". Evidence is in
`audit-artifacts/stage19.6-negative-control-*`.

Open: the waiter slot is per process, not per thread, so a multi-threaded
client must funnel input through one thread. There is no combined wait on
input plus other events: `InputWait` is separate from the event objects, so a
client cannot yet sleep on "input or timer or IPC" in one call. Next
prerequisite: delegating client surfaces to the holder by capability, so a
separate compositor process can draw other clients' windows.

## Stage 19.5 — input focus follows the compositor lease holder (2026-10-12)

When the compositor lease holder takes the display (its first present), it
also takes input focus. From then on, `woven_input::publish` delivers every
event into a bounded 128-entry focus queue that only that process can read.
The terminal's queue gets nothing, because the user is looking at the
holder's screen, not the hidden terminal. When the holder exits,
`compositor::release_owner` withdraws focus and the terminal receives input
again. Every focus change discards the focus queue, so keys typed for one
owner never reach the next owner or leak to the shell. `compositor::reset`
also releases a holder's focus.

The focus state is an IRQ mutex, because the keyboard interrupt path
publishes through it. Lock order: compositor or keyboard decoder (rank 10),
then `FOCUS`, then the terminal `QUEUE`. `FOCUS` is held across the fallback
push so focus cannot change between the check and the push.

Syscall 107 (`InputPoll`) returns the next event as `kind << 56 | value`
(character, Enter, Backspace, Tab, F1), `WOULD_BLOCK` when the queue is empty,
or `ERROR`. It requires `InputIo`, and the caller must hold focus. libwoven
wraps it as `input_poll() -> Result<Option<Key>, Error>`. Pointer events exist
only in the Stage 13.7 probe build and have no userspace encoding yet.

Gate `[S19.5] input focus: PASSED`:
- The kernel first checks the focus rules: no focus routes to the terminal;
  focus routes only to the owner; non-owners are refused; the queue is bounded
  and overflow is refused; a release by a non-owner is ignored; a focus change
  discards pending events.
- An ordinary-process probe must be refused `InputPoll`, along with all the
  Stage 19.4 refusals, against a genuinely foreign surface.
- hello-gui `--input` runs with `Console | DisplayIo | InputIo`. Once it holds
  focus, the kernel publishes `o`, `k`, Enter through the keyboard interrupt's
  own path. The terminal queue must stay empty, and the app must receive
  exactly those keys in order with nothing extra. After exit, focus is gone
  and a keystroke reaches the terminal again.

Negative control: with `publish` changed to ignore focus, the gate fails at
the focus-rules check. Evidence is in
`audit-artifacts/stage19.5-negative-control-*`.

Two defects were found while bringing this up:
- The first `publish` returned `QUEUE.lock().push(event)` as its tail
  expression. Rust drops tail-expression temporaries after locals, so the
  `FOCUS` guard was released before `QUEUE`, out of nesting order. The
  lock-order tracker panicked on the first unfocused keystroke, which the boot
  sequence produces in every build. The push result is now bound and `FOCUS`
  dropped explicitly.
- `kernel_panic` parked forever, so under `qemu-test` that panic surfaced as a
  300-second host timeout. It now exits QEMU with the failure status after
  printing the panic report, as `halt()` already did. Every harness already
  treats `KERNEL PANIC` as a failure. The timeout evidence is in
  `audit-artifacts/stage19.5-timeout-1cpu-*`.

Open: `InputPoll` is non-blocking, so clients poll with `yield_now`. A
blocking wait belongs on the existing event objects, so a client sleeps until
input arrives. Focus is all-or-nothing per holder, with no per-window focus
yet; that is the holder's job once it composites several clients. Next
prerequisites: an event-driven input wait, then delegating client surfaces to
the holder by capability.

## Stage 19.4 — compositor authority and client handle binding (2026-10-12)

**Defect fixed from Stage 18.** The client surface registry compared a slot's
owner with the owner field inside the handle, but userspace supplies that
handle. Any process could forge a token naming another process as owner, and
the slot index (0–15), generation (low after boot) and pid are all small and
guessable. With that token it could read, fill, stroke, destroy or blit the
other process's surface. The Stage 18.1 impostor test only swapped the owner
field of a genuine token, so it never exercised a forgery. Every graphics
syscall now resolves its token through `caller_surface`, which requires the
token to name the calling process; the registry then checks slot, generation
and owner as before.

**Compositor authority.** Blitting onto the compositor, reading screen pixels
and presenting to the framebuffer now require the `DisplayIo` capability. They
also require the compositor lease, which exactly one process holds at a time:
`compositor::claim` (syscall 105) takes it, and claiming again as the holder
succeeds. Reading the screen carries the same authority as drawing on it,
because the screen holds every client's pixels. Syscall 106 presents
compositor damage to the framebuffer, taking the display from the terminal on
first use. The task-exit path calls `compositor::release_owner`, which drops
the lease and gives the display back, so a crashed holder cannot keep the
screen. Ordinary user processes do not carry `DisplayIo`; client surfaces
(syscalls 98–101, 104) still need no capability, since a process can only
reach its own.

**libwoven.** `libwoven::entry!(main)` defines `_start` as a three-instruction
assembly stub that passes the entry stack pointer to `main(Args)`. Rust code
cannot observe that pointer reliably, which is the only reason the stub is
assembly. `Args::get` bounds each string scan by the kernel's 256-byte argument
limit. Also new: `compositor_claim()` and `compositor_present()`.

**Gate.** `[S19.4] compositor authority: PASSED`. The kernel first checks the
lease rules directly: exclusivity, idempotent re-claim, non-holders refused
blit, read and present, and a release by a non-holder ignored. It then creates
a surface owned by another pid and runs hello-gui twice:
- `--probe <token>` as an ordinary user process. Claim, blit, screen read and
  present must all fail. Every operation on the foreign surface must fail,
  both with its genuine token and with a token forged to name the probe. The
  kernel then checks that the foreign surface still holds its colour and that
  the probe never held the lease.
- Normal mode with `Console | DisplayIo`. The app claims, blits, verifies the
  screen and presents. The kernel then reads the framebuffer itself and finds
  the window's content colour, and checks that the lease was released, the
  display returned to the terminal, and the app's surfaces were reclaimed.

A negative control was run: with `caller_surface` removed from the pixel
syscall alone, the probe's forged read succeeds, it exits 207 and the gate
fails. The evidence is in `audit-artifacts/stage19.4-negative-control-*`.

Stages 18.2 and 19.2 now start hello-gui through the system-service spawn path
with `Console | DisplayIo`, because the app needs display authority. Their
assertions are unchanged.

Open: `DisplayIo` is granted at spawn by the kernel; there is no service
manager or user-facing grant flow. The lease has no revocation other than the
holder's exit, and no hand-over to a successor compositor. The holder still
blits clients' surfaces itself, so there is no way yet for one process to
composite another's surface by capability. Next prerequisites: input events
routed to the lease holder, then delegating client surfaces to the holder by
capability so a separate compositor process can draw them.

## Stage 19.3 — compositor presentation and display ownership (2026-10-12)

The compositor surface now reaches the real framebuffer.
`compositor::activate_display()` takes the display from the text terminal and
marks the whole surface damaged. `compositor::present()` copies only the damaged
regions through `display::present` and clears the damage after a successful
copy, so a failed presentation loses nothing. `deactivate_display()` returns the
display to the terminal.

There is exactly one path to the framebuffer after boot.
`terminal::with_framebuffer` lends the buffer under the terminal's lock, so the
compositor keeps no pointer of its own and its writes cannot interleave with
terminal text. While the compositor owns the display, terminal rendering is
suppressed; when it releases the display, terminal rendering resumes. The
compositor lock changed from an IRQ mutex to a preemption mutex: copying a
1280×800 frame with interrupts disabled would stall device interrupts, and
nothing takes the lock from interrupt context. Lock order is compositor, then
terminal, both at rank 10. The terminal never calls the compositor.
`with_framebuffer` deliberately avoids `file_fault_io`, which takes the
scheduler lock: the framebuffer mapping is resident and cannot fault.

The self-test reads framebuffer bytes back itself, converting by the reported
pixel format, rather than trusting the compositor's state. It checks:
presentation is refused before takeover; takeover repaints exactly the visible
area (640×480 on QEMU's 1280×800 framebuffer); the screen shows the compositor's
black background where the terminal's background was dark blue; a second
present writes nothing; one 32×16 client blit presents exactly 512 pixels, with
the client colour at its offset and background next to it; terminal output
leaves the framebuffer checksum unchanged while the compositor owns the display;
presentation is refused after release; and terminal output changes the
framebuffer again afterwards. Marker: `[S19.3] compositor presentation: PASSED`.
Passed on 1, 2 and 4 CPUs.

Open: nothing in the production boot activates the compositor yet. The early
boot `Console` still writes the framebuffer directly, outside the terminal lock.
That is safe only while it runs single-threaded before the scheduler starts, and
it must move behind `with_framebuffer` before the compositor is activated at
boot. Userspace cannot present: activation and presentation are kernel calls,
pending a compositor capability. Terminal output while compositing is not shown
anywhere on screen; a terminal client of the compositor would replace that.
Next prerequisites: the compositor capability (a single holder may present),
then input events routed to the focused client.

## Stages 18–19 — client surfaces and the compositor primitive (2026-10-12)

A userspace process can now own a drawing surface, draw into it through
syscalls, and place it on a kernel-held compositor surface that outlives the
process.

**Stage 18.1 — client surface registry.** `kernel/src/user_graphics.rs` is a
bounded, owner-tagged registry of `display::Surface` objects: 16 surfaces
system-wide, 4 per process, at most 320×240 each. A handle packs slot (8 bits),
generation (24) and owner pid (32). Every operation checks all three, so a stale
handle or another process's handle is refused rather than resolving to a reused
slot. `release_owner(pid)` runs on the task-exit path in `task.rs`, next to the
other per-process registries. Syscalls 98–101 cover create, fill, pixel
readback and destroy, and 104 covers stroke. Fill and stroke pack x/y/w/h as four
u16 values in one register, because the ABI carries three arguments. A text
syscall was tried and removed: handle, origin, pointer, length and colour do not
fit in three arguments. Text drawing stays kernel-internal until the syscall ABI
gains an argument block. Marker: `[S18.1] client surface registry: PASSED`.

**Stage 18.2 — end to end through a real application.** `apps/hello-gui`
builds outside the kernel, like `apps/hello`. It allocates a surface, draws a
window (background, title bar, content, one-pixel border) and reads back seven
pixels, one in each region and edge, through the pixel syscall. Each mismatch
exits with its own status. The kernel then checks the app's exit status and that
`count_for(pid) == 0` after exit. Marker:
`[S18.2] client surface end-to-end: PASSED`.

**Stage 19.1 — compositor surface.** `kernel/src/compositor.rs` holds one
640×480 `Surface` plus a `Damage` list. It is initialised lazily after the heap
self-test, because a 1.2 MB allocation earlier in boot fails. `blit` takes a
snapshot of the client surface under the `user_graphics` lock, releases that
lock, and then takes the compositor's. Both locks are rank 10, so the two
subsystems never nest and need no ordering rule between them. The self-test
covers: refusal before init (NotReady regardless of the handle), idempotent
init, exact colour at the blitted offset and untouched pixels next to it,
edge-clipped touched rectangles, refusal of a bogus handle, and refusal of a
handle whose owner has exited. Syscalls 102 (blit, `x | y << 32` as i32) and
103 (screen pixel). Marker: `[S19.1] compositor surface: PASSED`.

**Stage 19.2 — application through the compositor.** `hello-gui` presents its
window at (60, 40), reads screen pixels back from userspace, and exits. The
kernel then reads `compositor::pixel(160, 100)` itself, after the process has
gone. A userspace check that only claimed success cannot pass this gate, and the
read also shows that composed pixels do not depend on the client's lifetime.
Marker: `[S19.2] hello-gui through the compositor: PASSED`.

Validation: Stages 17.1, 18.1, 18.2, 19.1 and 19.2 passed on 1, 2 and 4 CPUs.
The kernel Clippy matrix (49 feature configurations) is clean with
`-D warnings`, as are libwoven and host Clippy (`--all-targets`). The first
Stage 19.1 run failed on all CPU counts because `blit` snapshotted the client
before checking readiness. It returned `InvalidClient` where `NotReady` was
specified, and the fix checks readiness first.

Historical note: the following paragraph describes the pre-19.3 baseline; the
later Stage 19 gates supersede those limitations.

Open: the compositor surface is not yet presented to the real framebuffer. It is
a fixed 640×480 off-screen surface, because no global framebuffer accessor
exists for it. There is no stacking, focus or input routing: a blit lands where
the client asks, so this is the primitive a userspace window manager will drive,
not a window manager. Any process may blit its own surfaces anywhere. Before a
second graphical client is admitted, that authority must become a capability.
Blits copy the whole client surface (at most 300 KB) per call. Text drawing
from userspace needs the argument-block ABI. Next prerequisites: framebuffer
presentation of compositor damage, a compositor capability, and keyboard and
pointer events delivered to the focused client.

## Stage 17.1 — WovenHat font and software compositing (2026-10-12)

`kernel/src/font.rs` covers all 95 printable ASCII glyphs at 5×7. The old table
had 14, so "WOVENHAT DESKTOP" could not render. `kernel/src/display.rs` adds
`Surface`, bounded `Damage` (16 rectangles, then a bounding box) and a single
`present` path that resolves the pixel format once per run. Host tests in
`tests/display_compositing.rs` cover clipping, stride and damage coalescing.
Marker: `[S17.1] WovenHat font coverage: PASSED`. Passed on 1, 2 and
4 CPUs in the Stage 18–19 validation.

## Stage 16.1 — applications built outside the kernel (2026-10-14)

WovenHat runs a program produced by the ordinary Rust toolchain. Before this,
every userspace program was one of 32 `global_asm!` blocks in
`kernel/src/userspace.rs`, wrapped in a synthetic ELF by `build_stub_elf` at
runtime, so a program could not exist without editing the kernel. That was the
binding constraint on a userspace platform, and therefore on any desktop.

`apps/hello` is a separate crate, excluded from the kernel workspace, with its
own linker script and link flags. It compiles to a static ET_EXEC image; the
dedicated harness signs that image into a WAPP package before the kernel build.
The kernel verifies the package, installs it into the filesystem, reads it back,
and loads the verified payload through the same ELF loader every other
executable uses. The self-test loads the bytes read from the filesystem rather
than the embedded constant, so the gate proves signature enforcement and that
path rather than assuming either.

`libwoven` grew from two bound syscalls to the process, I/O, filesystem, time
and environment core, in two layers: one `unsafe` wrapper per kernel entry point
returning the kernel's value unchanged, and a safe surface returning
`Result<_, libwoven::Error>` where the unsafety can be discharged. The kernel
implements 98 syscalls; the remainder are reachable through
`libwoven::syscall::invoke` and should grow typed wrappers as they are used.

Three build settings follow from the loader's rules and the user address space,
each documented in [the application guide](writing-wovenhat-applications.md):
`code-model=large`, because `USER_REGION_START` is far outside the small code
model's ±2 GB; `build-std = ["core"]`, because the precompiled `core` uses the
small model and conflicts; and an explicit `PHDRS` linker script with
`-z norelro`, because the loader rejects `PT_GNU_RELRO` and any
writable-executable segment.

The marker is `[S16.1] out-of-kernel application: PASSED`, with per-step markers
for WAPP verification, tamper refusal, unsigned-external-image refusal, loader
acceptance, filesystem install, read-back, load, spawn, completion and exit
status. The dedicated application harness now passes on 1, 2 and 4 CPUs; see
the [Stage 16.1 audit](docs/audit-stages-consolidated.md).

The original Stage 16.1 boundary remains intentionally single-package and
VFS-backed; the release-level Stage 19 catalog/production-image gates now cover
the ten-package install plan and are recorded above. Shared libraries and
stack-size negotiation remain outside the bounded loader contract; static
initial-exec TLS and self-contained dynamic relocation are now supported.

The accepted Stage 19 applications now have a reproducible packaging boundary:
`scripts/package-system-apps.py` builds all ten applications, signs each WAPP
payload, and emits hashed `/bin` install paths plus `/share/applications`
descriptors in an install-plan manifest. The release image consumes that plan
and installs all ten packages transactionally in RAM after verification.

The feature-gated multi-application catalog gate now passes on 1/2/4 CPUs.
It consumes all ten signed packages, rejects tampered copies, verifies VFS
byte-for-byte read-back and normal ELF loading, and publishes matching catalog
descriptors. The normal-boot production image gate confirms installation
before shell startup. See the
[Stage 19 system-app catalog audit](docs/audit-stages-consolidated.md).
Persistent-media installation, power-loss rollback, and deployment-key
rotation/revocation remain separate production requirements.

## Stage 15 — WovenFS native filesystem (2026-10-12)

WovenFS is WovenHat's own on-disk filesystem, specified in
[the format document](wovenfs-on-disk-format.md). Its design constraint is that
no operation depends on more than one sector being written atomically: the
superblock alternates between two checkpoint slots rather than being overwritten
in place, and every multi-sector change is staged in a write-ahead journal whose
completion is decided by a single commit sector.

Stage 15.1 covers the filesystem itself — format, mount, create, write, read,
unlink, lookup, read_dir and a consistency check, with checksummed inodes and
file contents. Crash recovery is proven rather than asserted:
`tests/wovenfs_crash.rs` counts the writes each of format, create, write and
unlink performs, replays the operation once per write boundary against a fresh
image with the device refusing every later write, and at each point requires the
volume to mount, the consistency check to pass, and the operation to be entirely
absent or entirely present. Replay idempotence, checkpoint-slot fallback and
torn journal payloads are covered too.

Stage 15.2 covers WovenFS on a real block device through the ATA driver, with
its root published into the VFS at `/wfs`. It passed on 1/2/4 CPUs against a
scratch image and on an 8 GB USB device, verified independently from the host:
LBA 0 reads `WOVENFS1` with the checkpoint sequence in the slot the alternation
scheme predicts.

Physical qualification exposed an ATA defect invisible to emulation. The drive
wait was bounded by iteration count rather than time, and real flash stalls for
tens of milliseconds once its write buffer fills, so the driver read a normal
programming cycle as a fault and format failed at the 320th sector write. The
wait is now wall-clock bounded. This affected every ATA read and write in the
kernel, not only WovenFS.

Open: WovenFS is not the mounted system filesystem — FAT32 still holds `/mnt`
and the boot volume — and whether to make it one or to keep FAT32 and reclassify
Stage 12.2/12.4 is an open product decision. The bounded `/wfs/<name>` file
authority now supports durable create, write, read-back, and unlink through task
file syscalls; direct generic VFS mutation remains read-only. Native mutations
serialize their complete read-modify-write transactions. The adapter remains a
Nested native paths, directory creation, empty-directory removal, and rename
are now transactional; directory rename rejects descendant cycles with a
bounded fail-closed walk. Recursive import into the VFS mount and mounted-system
publication now walks validated nested directories with bounded depth and
visited-inode tracking; oversized files fail closed at the VFS capacity
boundary. `Volume::check()` validates the complete reachable tree and rejects
cycles, duplicate references, malformed names, and orphaned live inodes.
Mounted-system policy remains open. See the [Stage 15
VFS write-through audit](docs/audit-stages-consolidated.md).

Crash recovery is proven by fault injection, not actual power loss on a device.
Physical qualification remains incomplete: the inserted USB flash disk was
inspected read-only, while the destructive harness requires Administrator
elevation and explicit authorization to erase it.

The complete Stage 12 acceptance chain was revalidated on 2026-10-13: all
Stages 12.1–12.5 passed on 1/2/4 CPUs with exit 33. See the
[revalidation audit](docs/audit-stages-consolidated.md). This confirms the
bounded gates only; the integration and physical limits above remain open.

## Stage 15.3 — runtime permission broker (2026-10-13)

The bounded permission broker now exposes generation-safe request/poll/decide/
revoke operations through the userspace ABI. Decisions require `TaskControl`
and successful changes pass through WovenGuard grant/revoke lineage checks;
security-ledger events record each request and decision. The 1/2/4-CPU gate,
freestanding Clippy, and all-target Clippy pass. See the
[Stage 15.3 permission-broker audit](docs/audit-stages-consolidated.md).
Graphical prompts, signed application identity, persistent consent, secure or
measured boot, and WovenVault remain open Stage 15 work.

## Stage 15.4 — signed application cryptographic boundary (2026-10-13)

The pure-Rust `no_std` Ed25519 verifier and bounded versioned WAPP envelope
now reject malformed and tampered payloads. The exact 1/2/4-CPU gate and
warning-denying freestanding Clippy pass; see the
[Stage 15.4 audit](docs/audit-stages-consolidated.md).
Release builds now require `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX`; the RFC
self-test remains pinned to the development vector. Signed package
installation now has a real external-application path through
`tools/wapp-packager`, and Stage 16.1 verifies that package before and after its
VFS round trip. Multi-application catalogs and rollback-safe RAM publication
are now covered by the ten-app release gates. Deployment-key
rotation/revocation and firmware/installer-owned key slots remain open. User
`exec`/`spawn` now reject raw external ELF; only the finite kernel-owned system
image allowlist retains the explicit internal-image path.
Stage 15.5 software signed-kernel handoff is accepted with the exact 1/2/4-CPU
gate; see the [secure-boot audit](docs/audit-stages-consolidated.md).
Firmware UEFI Secure Boot state, TPM/event logs, release-key provisioning,
rollback/revocation, and signed-initrd enforcement remain hardware/platform
work and are not claimed complete.

## Stage 15.7 — WovenVault volatile secret handles (2026-10-13)

WovenVault now provides generation-safe per-owner secret handles with bounded
storage, validated copy-in/copy-out, zeroizing revoke, and owner teardown.
The `Secrets` capability is not granted by default. The 1/2/4-CPU gate and
warning-denying freestanding kernel Clippy pass. See the
[Stage 15.7 WovenVault audit](docs/audit-stages-consolidated.md).
The Stage 15.6 software measurement audit records the separate 1/2/4-CPU
kernel measurement gate; TPM PCR extension, event logs, and firmware Secure
Boot state remain hardware-dependent.
Persistent encrypted records and crash-safe successor rotation now pass at the
kernel boundary; TPM/firmware provisioning, deployment-key custody, and a
secrets UI remain open.

## Stage 15.8 — security-ledger read ABI (2026-10-13)

The security ledger now has a bounded newest-relative read syscall authorized
by `TaskInspect`, with validated userspace copies and a fixed 48-byte record
decoded by typed `libwoven` wrappers. The 1/2/4-CPU gate, build, and
warning-denying freestanding kernel Clippy pass. See the
[Stage 15.8 security-ledger audit](docs/audit-stages-consolidated.md).
This is an ABI boundary, not a complete security-center product: durable
retention/export, signed identity, graphical review, and secure/measured boot
remain open.

## Stage 15.9 — exploit-mitigation hardening (2026-10-13)

The existing ASLR, W^X, and user-stack guard-page checks are now joined by a
scheduler-validated canary on each static kernel task stack. The dedicated
1/2/4-CPU gate and warning-denying freestanding kernel Clippy pass. See the
[Stage 15.9 exploit-mitigation audit](docs/audit-stages-consolidated.md).
This closes the verified software-hardening subset only: true unmapped kernel
stack guards, SMEP/SMAP enablement and hardware qualification, shared-library
RELRO/TLS support, and scheduler-backed user threads remain open.

## Stage 1–5 bounded batch data journal — 2026-10-06

FAT32 persistence now exposes a separate versioned `WDJ2` journal for bounded
sixteen-file data batches. Prepared batches roll every partially-written file
back to its prior bytes and metadata, removing stale WMD1/WMD2 metadata
intents; committed batches are retired only after all intended checksums and
metadata snapshots match. The legacy `WDJ1`
single-file format and metadata ABIs remain readable. Kernel
build/check, warning-denying kernel Clippy, and the existing storage gate
passed on 1/2/4 CPUs, including two-file commit/readback and prepared,
partial-commit, and complete-commit recovery exercises. See the
[batch-journal audit](docs/audit-stages-consolidated.md).

Physical storage/DMA qualification, unclean-shutdown hardware testing,
metadata transactions, and arbitrary-size transactions remain open.

## Stage 1–5 immediate batch rollback — 2026-10-13

The bounded WDJ2 data-batch API now invokes durable recovery immediately when a
post-journal write, metadata, commit-record, or journal-retirement operation
fails. It invalidates file-page cache state before recovery, preventing a
transient runtime error from leaving partially committed data on disk or stale
pages available to the current boot's VFS; the original error is preserved and
a failed rollback remains recoverable from the durable journal. Direct FAT32
and MBR/GPT-partitioned paths are covered. The 1/2/4-CPU disposable storage
gates, host crash tests, and warning-denying Clippy passed. See the
[immediate-rollback audit](docs/audit-stages-consolidated.md).

This is a bounded hardening increment, not full Stage 1–5 completion. The
follow-up metadata transaction increment below closes metadata rollback within
this bounded format; arbitrary-size/full-volume rollback, physical storage/DMA
qualification, and unclean-shutdown hardware evidence remain open.

The WDJ2 capacity extension was revalidated on 1/2/4 CPUs after increasing the
bounded batch maximum from four to sixteen entries; see the [capacity audit](docs/audit-stages-consolidated.md).

## Stage 1–5 bounded metadata transactions — 2026-10-14

WDJ2 v2 snapshots old and new inode/sidecar ownership metadata while retaining
version 1 recovery compatibility. Prepared and incomplete committed batches
restore both bytes and prior metadata; committed recovery retires the journal
only after bytes and new metadata match. The live FAT32 recovery fixture and
storage/SMP QEMU gates passed on 1/2/4 CPUs. See the [metadata transaction
audit](docs/audit-stages-consolidated.md).

This closes bounded metadata transactions, not arbitrary-size/full-volume
rollback or physical unclean-shutdown qualification.

## Stage 1–5 sector-write fault injection — 2026-10-14

FAT32 mutation self-tests now inject a deterministic `DeviceFault` at every
bounded file-creation sector-write boundary. Failed mutations must leave the
volume structurally checkable, remove the new file, and preserve the original
file bytes; successful mutations are also checked. Host and freestanding
storage gates use the same self-test. This is software error-return evidence,
not physical power-loss or torn-sector evidence. FSInfo remains an advisory
hint and may be stale after an injected failure; physical flash/ATA/DMA,
torn-write, arbitrary-size/full-volume rollback, and reboot recovery remain
open.

The WDJ2 codec self-test also rejects every byte-truncated form of a valid
prepared record before recovery can interpret it. This strengthens software
journal-boundary evidence; it does not prove physical torn-sector atomicity or
power-loss recovery.

## Stage 1–5 DMA allocation cleanup — 2026-10-14

The existing AHCI and NVMe DMA-page owners now return allocator frames when
direct-map setup fails. AHCI also drops all pages on failed controller
initialization; successful controller ownership remains process-lifetime and
is held under the controller lock until no hardware command can reference the
pages. The `BlockDevice` ABI is unchanged.

The Stage 13.3/13.4 freestanding Clippy gates, host Clippy, and the complete
host test suite passed after this change. The NVMe and AHCI QEMU controller
gates also passed on 1/2/4 CPUs with exit 33. See the [DMA allocation audit](docs/audit-stages-consolidated.md)
for the six evidence directories.
This closes a software allocation-leak path only. Physical AHCI/NVMe and USB
flash qualification, IOMMU isolation, interrupt completion, torn-sector and
power-loss evidence, and arbitrary-size rollback remain open.

## Stage 14.2D — Router Advertisement state (2026-10-06)

Router Advertisement state now tracks a link-local router, bounded lifetimes,
and one validated prefix with expiration and withdrawal. It rejects invalid
preferred/valid lifetimes and global router sources. Check the remote D audit
for its exact acceptance evidence.

## Stage 14.2E — ICMPv6 checksum boundary (2026-10-06)

The IPv6 foundation now has production-reusable address and prefix primitives:
prefix containment, link-local/multicast classification, and RFC 4291
solicited-node multicast derivation. The 1/2/4-CPU Stage 14.2 QEMU gate
exercises these exact operations and requires `[S14.2A]` and `[S14.2B]`
markers. This remains a deterministic foundation boundary: ICMPv6 Neighbor
Discovery packets, Duplicate Address Detection, router discovery, DHCPv6,
live IPv6 sockets, and physical IPv6 qualification remain open. See the
[Stage 14.2B audit](docs/audit-stages-consolidated.md).

The bounded parser now accepts only ICMPv6 Router/Neighbor Solicitation and
Advertisement envelopes with code zero, valid fixed bodies, non-multicast
targets, and complete non-zero-length options. It caps input at 1 KiB and does
not retain packet ownership. The 1/2/4-CPU gate exercises valid and malformed
messages. IPv6 pseudo-header checksum verification, Duplicate Address
Detection, router state, DHCPv6, live IPv6 sockets, and physical IPv6
qualification remain open.

See the [Stage 14.2C audit](docs/audit-stages-consolidated.md).

The parser now has a checked entry point that authenticates the ICMPv6
pseudo-header using the IPv6 source/destination, payload length, next-header
value, and packet checksum. Tampered packets are rejected before Neighbor
Discovery parsing. Router state, Duplicate Address Detection, DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2E audit](docs/audit-stages-consolidated.md).

## Stage 14.2F — Neighbor cache and DAD state (2026-10-06)

The bounded state layer now accepts only checksum-validated Neighbor
Advertisements, stores up to eight expiring link-layer records, and exposes
explicit Duplicate Address Detection pending/unique/duplicate outcomes. It
does not yet transmit probes or integrate live interface state. DHCPv6, live
IPv6 sockets, and physical IPv6 qualification remain open.

See the [Stage 14.2F audit](docs/audit-stages-consolidated.md).

Next: connect Neighbor Discovery state to bounded live IPv6 interface traffic.

## Stage 14.2G — SLAAC lifecycle (2026-10-06)

The bounded SLAAC lifecycle now derives an address from an accepted /64
prefix, tracks tentative/preferred/deprecated/expired/duplicate states, and
applies DAD completion plus valid/preferred lifetimes. Live router egress,
DHCPv6, sockets, and physical qualification remain open.

## Stage 14.2H — DHCPv6 protocol foundation (2026-10-06)

The bounded DHCPv6 foundation validates supported message types, 24-bit
transaction identifiers, complete non-zero-length options, and bounded
serialization. It is protocol framing only: live lease exchange, retransmit
timers, address installation, and persistent lease state remain open.

## Stage 14.2I — DHCPv6 client state machine (2026-10-06)

The bounded DHCPv6 client state machine validates Solicit/Advertise/Request/
Reply progression, binds replies to the transaction and server DUID, and
applies bounded retry timing. It does not perform live lease installation or
persistent configuration.

## Stage 14.2J — IPv6 interface ingress boundary (2026-10-06)

The ingress coordinator verifies ICMPv6 checksums before dispatching Router
Advertisement, Neighbor Advertisement, and Neighbor Solicitation packets into
bounded interface state. It emits explicit events for accepted traffic, DAD
conflicts, and rejected packets. No packet slice is retained and no physical
NIC activation is claimed.

See the [Stage 14.2J audit](docs/audit-stages-consolidated.md).

Next: bounded IPv6 interface address configuration and controlled egress.

## Stage 14.3 — WovenNet socket API boundary (2026-10-06)

Socket endpoint admission now rejects zero-port, unspecified, and IPv4
multicast destinations before smoltcp state mutation. Packed IPv4 endpoint
decoding and encoding have a deterministic round-trip contract, while the
existing owner/generation socket lifecycle remains unchanged. This is an API
boundary; live IPv6 sockets, routing policy, and physical NIC qualification
remain open.

See the [Stage 14.3 audit](docs/audit-stages-consolidated.md).

Next: bounded routing-table ownership above the accepted socket API.

## Stage 14.4 — WovenNet routing-table foundation (2026-10-06)

The bounded routing table now supports eight generation-tagged routes, rejects
invalid prefixes and zero gateways, performs longest-prefix matching, and uses
metric then generation tie-breaking. Removal requires the exact route handle,
so stale removal cannot delete a replacement. Live route installation into the
network interface and physical NIC qualification remain open.

See the [Stage 14.4 audit](docs/audit-stages-consolidated.md).

## Stage 14.5A — WovenGuard firewall policy foundation (2026-10-07)

The bounded WovenGuard firewall policy foundation is accepted. It supports
inbound, outbound, and forward directions; allow/deny actions; protocol,
dual-stack CIDR, and port matching; deterministic priority/ID ordering; a
default policy; bounded rule storage; and synchronized global policy access.
This foundation is policy-only: live packet-path enforcement remains Stage
14.5B work.

The Stage 14.5A acceptance marker is
`[S14.5A] firewall policy foundation PASSED`, with the inherited WovenNet
networking gates retained. GitHub Actions run #1267 passed on commit
`f8b79ac7`.

Next: Stage 14.5B live packet-path enforcement without weakening the accepted
DHCP, IPv4/IPv6, socket, or routing behavior.

## Stage 14.5B — WovenGuard live packet enforcement (2026-10-07)

Stage 14.5B is accepted. WovenGuard now evaluates real Ethernet-frame traffic
at the virtio network boundary: inbound denial occurs before an RX token is
returned to smoltcp, and outbound denial occurs after frame construction but
before hardware submission. The shared parser derives IPv4/IPv6 TCP, UDP,
ICMP, and ICMPv6 policy metadata while preserving non-IP Layer-2 control
traffic.

Live enforcement has an explicit activation lifecycle. It remains disabled
during network bootstrap, is enabled for controlled policy enforcement, and
does not replace the policy's default-deny semantics with a permissive default.
This prevents DHCP/bootstrap deadlock while keeping firewall activation an
intentional security transition.

The Stage 14.5B acceptance marker is
`[S14.5B] live packet enforcement PASSED`. GitHub Actions run #1277 passed
the dedicated build, Clippy `-D warnings`, inherited Stage 14.1-14.5A network
regressions, and QEMU 1/2/4-core acceptance on commit `1d1ce646`.

Next: extend WovenGuard beyond the accepted virtio live boundary with explicit
policy-management authority and transport coverage before claiming a
production firewall.

## Stage 14.5C — WovenGuard firewall policy authority (2026-10-07)

Stage 14.5C is accepted. Firewall administration is separated from ordinary
network use through the dedicated `NetworkAdmin` capability. Normal users keep
`NetworkIo` for socket traffic but cannot add, remove, replace, or otherwise
change global firewall policy, the default action, or live-enforcement state.
`NetworkAdmin` is available only within the Kernel/SystemService domain
ceiling.

The acceptance marker is `[S14.5C] firewall policy authority PASSED`.
GitHub Actions run #1287 passed the dedicated build, Clippy `-D warnings`,
inherited Stage 14.1-14.5B regressions, and QEMU 1/2/4-core acceptance on
commit `b81b3a76`.

Next: complete firewall transport/path coverage and parser hardening before
adding stateful connection tracking.

## Stage 1–5 long-name mutation hardening — 2026-10-01

The FAT32 create/overwrite and rename paths now compare the user-visible long
name before selecting an 8.3 alias. Existing long-name files are overwritten
through their existing directory entry, and attempts to create a directory or
rename another file onto that name fail closed. The strengthened FAT32
self-test and complete Stage 1–5 QEMU gate passed on 1/2/4 CPUs with exit 33.
See [the hardening audit](docs/audit-stages-consolidated.md).

This is a bounded software hardening increment, not a claim that the remaining
hardware qualification, DMA storage completion, or full data rollback gaps are
closed.

## Stage 1–5 durable data journal — 2026-10-01

FAT32 persistence now uses a bounded on-volume prepared/committed data journal
to recover interrupted single-file replacements, including restoration of the
previous bytes or removal of a newly-created target. Payload tampering fails
checksum validation. The journal self-test and 1/2/4-CPU QEMU gate passed.
See [the data-journal audit](docs/audit-stages-consolidated.md).

Multi-file atomicity, larger-than-VFS files, and physical power-loss testing
remain open.

The journal now writes version 2 records with the intended replacement length
and checksum. Mount recovery validates committed records before retiring them
and restores the previous bytes if the committed target does not match.
Version 1 records remain readable for compatibility. Multi-file atomicity and
physical power-loss testing remain open.

## Stage 1–5 metadata capacity extension — 2026-10-01

The ownership sidecar preserves the WMD1 four-sector ABI and now uses a
versioned WMD3 extension in eight additional reserved sectors when the volume
geometry permits it. Capacity increases from 84 to 252 records without
relocating or rewriting legacy metadata. The strengthened self-test and
1/2/4-CPU QEMU gate passed with exit 33. See the
[metadata-capacity audit](docs/audit-stages-consolidated.md).

The table remains intentionally bounded; a dynamically growing metadata
format and full multi-operation transaction log are still future work.

Production completion is tracked separately from bounded foundation acceptance
in [the Stage 1–12 gap audit](stage1-12-production-gap-audit.md). No stage is
fully complete while a roadmap requirement remains open.

## Physical qualification tooling — 2026-10-01

The read-only physical-probe image now records CPU features, ACPI/ECAM state,
PCI class information, and every retained PCI function before stopping ahead
of driver activation. Its 1/2/4-CPU QEMU inventory smoke passed. This improves
lab evidence collection but does not close physical storage, DMA, interrupt,
radio, or unclean-shutdown qualification. See the
[physical inventory audit](audit-physical-inventory-2026-10-01.md).

Authoritative development sequence: [supplied Stage 10.7–36 roadmap](master-development-roadmap.md).

## TCP close/drain preservation repair - 2026-09-24

This-machine preflight exposed an intermittent 4-CPU Stage 10.7 failure:
last-reference release removed a TCP socket with 16 bytes still queued.
Graceful bounded retirement now preserves that transport. Ten corrected
4-CPU repeats passed, including queued-close cases; the full 75-boot Stage
10.7 preservation gate passed. Related 10.8/10.9/13.9 checks on 1/2/4 CPUs
and normal release shell/SMP smoke also passed. Assertions and existing test
timeouts are unchanged; cleanup now requires transport slots at baseline. See
[the TCP repair audit](audit-tcp-close-drain-2026-09-24.md).

## Physical integration request - 2026-09-24

Physical integration is incomplete. The latest user instruction selects this
HP Pavilion x360 as the actual test machine, superseding the earlier AX200
target choice. Its adapter is AX201 `8086:A0F0`, subsystem `8086:0074`, revision
`20`. Native AX201 startup and transport remain unimplemented; adding its PCI
ID to the AX200 activation list is not a valid implementation.

The authorized Kingston USB was cleared and written with the verified inventory
image. An elevated helper read back all 8,454,144 image bytes and matched the
SHA-256. Its new GPT layout contains an 8 MiB EFI system partition. The internal
system disk was not written. Secure Boot is enabled and the image has no PE
Authenticode certificate; native boot requires a local firmware decision.
Windows C: is fully encrypted with BitLocker protection on, so the recovery key
must be available before changing firmware security settings. Neither BitLocker
nor firmware settings were changed, and no reboot or physical test has run.
The inventory image passed 1/2/4-CPU configured QEMU boots plus a no-UART
framebuffer check. No AP startup or physical radio is tested by that mode.
See [the AX201 machine audit](audit-ax201-machine-2026-09-24.md).

The runtime firmware loader now validates the complete AX200 section layout
before DMA allocation and copies payloads into an unpublished owned context.
It skips separators and init-image records and preserves existing DMA bounds.
Build, warning-denying host/kernel/feature Clippy, all 35 Rust tests and all
54 Wi-Fi markers on 1/2/4 CPUs passed for this loader increment.
See [the runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

A real AX200 firmware container exposed parser defects. The corrected
allocation-free TLV parser passes seven new host tests and the real-image
probe; all 54 Wi-Fi markers still pass on 1/2/4 CPUs. Build, host-test and
kernel Clippy, and all 31 Rust tests pass. The full Stage 10.7 preservation
gate passed after the heap lint cleanup: nine Python tests and 75 QEMU
boots, plus three focused Wi-Fi boots (78 total for this prerequisite).
See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md).
This is prerequisite work, not physical firmware/IRQ/DMA/RF acceptance.

## Current continuation - 2026-09-24

Base commit `0563d0b` introduced the Stage 13.10AC deferred AX200 interrupt
service candidate. This continuation passed preservation and bounded QEMU
acceptance; it does not establish production completion of Stage 13.

- Ordinary build and host/freestanding-kernel Clippy passed with warnings
  denied; 24 Rust host tests and nine Python harness tests passed.
- The complete Stage 10.7 preservation gate passed, exit code 0: 75 QEMU boots
  covering 1/2/4 CPUs, live networking, and async block/file/UDP/TCP.
- Stages 1-5, 10.8-10.9, 11.1-11.5, 12.1-12.5, and 13.1-13.9 passed
  per-feature freestanding Clippy and 1/2/4-CPU QEMU (66 additional boots).
  Wi-Fi required all 54 markers through 13.10AC on every CPU configuration.
- CPU hotplug passed on 2/4 CPUs. The normal release build and 4-CPU
  shell/PS2/IOAPIC/SMP smoke passed. Total: 144 successful QEMU boots.

The continuation corrected feature boundaries, a misplaced audio check in
USB HID's error branch, and a nested Cargo release-build artifact-lock
conflict in the shell harness. Failed/interrupted attempts remain preserved.
See [the continuation audit](docs/audit-stages-consolidated.md)
for exact evidence directories and validation scope.

Next prerequisites remain production Wi-Fi worker/lifecycle integration,
secure entropy provisioning, physical AX201 firmware/IRQ/DMA/RF qualification,
and the earlier unfulfilled roadmap requirements. Do not advance to a later
stage or treat these synthetic Wi-Fi results as working physical Wi-Fi.

## Stage 6 — accepted on 2026-09-16

The bounded SMP foundation passed the complete release matrix: warning-denying
kernel/host Clippy, all Rust host tests, 1/2/4-CPU memory/storage/network
gates, legacy PIC fallback, 4-CPU release gates, and shell/SMP smoke. See
[the Stage 6 audit](docs/audit-stages-consolidated.md).

General multicore userspace, unrestricted concurrent device/filesystem service
throughput, multi-node NUMA page-placement qualification, non-contiguous
hotplug hardware qualification, APIC-ID-above-255 hardware qualification, and
broader cross-layer lock-path qualification and priority inheritance
remain explicit deferred requirements in the Stage 6 contract. CPU-domain
placement, audited Ready-state I/O service migration, and the x2APIC MSR transport are
implemented; bounded contiguous-prefix AP offline/re-online control is also
implemented; interrupt-safe scheduler, IPC, WovenGuard, teardown, and worker
locks now enforce bounded rank and nesting
checks, and VirtIO network DMA uses an allocator-reserved physically
contiguous arena; see the [NUMA/x2APIC audit](docs/audit-stages-consolidated.md),
[hotplug audit](docs/audit-stages-consolidated.md), and
[lock-order audit](docs/audit-stages-consolidated.md) and
[DMA audit](docs/audit-stages-consolidated.md).

The CPU lifecycle follow-up supports a non-contiguous online mask in QEMU:
CPU 1 can park while CPU 3 continues scheduling work and acknowledging TLB
shootdowns, then rejoin its stable slot. Offline preparation validates every
task move before committing any; draining CPUs reject new placement. See the
[non-contiguous hotplug audit](docs/audit-stages-consolidated.md).
Physical non-contiguous hotplug, high APIC-ID, and multi-node NUMA hardware
qualification remain open.
The final source passed the complete release matrix and dedicated twice-cycled
2/4-CPU hotplug gates after this follow-up.
The cancellation follow-up distinguishes unclaimed requests from AP-owned
transitions, rearms rejected states, and keeps further hotplug disabled if an
AP never finishes a claimed transition. Forced AP rejections and unclaimed
timeouts recover and retry in the 2/4-CPU QEMU gates; broad concurrent-work
and physical-fault stress remains open.

The 2026-09-17 lock follow-up made the file-frame cache IRQ-safe, moved
fork/dup reference retention outside the process-table lock, and added
generation-tagged VFS and pipe handles to reject stale slot reuse. The VFS
read and materialization paths now release registry/open-description guards
for disk I/O and revalidate node identity, version, backing, and shared seek
position before committing results. Both VFS tables now use ranked IRQ-safe
locks. See the
[lock-order audit](docs/audit-stages-consolidated.md). The full release
matrix and twice-cycled 2/4-CPU hotplug gates passed after these changes.
The pipe follow-up also uses scheduler-latched wakeups and rejects full waiter
tables, closing a lost-wakeup window under concurrent readers and writers.
The global heap metadata lock is now rank-50 IRQ-safe; page mapping happens
before that guard is taken during boot. Its earlier full release and 2/4-CPU
hotplug gates passed. The follow-up makes boot capacity RAM-scaled from
256 KiB to 8 MiB, raises live-allocation metadata to 2,048 entries, recovers
alignment padding, and coalesces freed intervals. A partial kernel map now
rolls back. Runtime growth beyond the eager mapping remains open.
See the [Stage 6 heap audit](docs/audit-stages-consolidated.md).
The final heap change passed the full release matrix and twice-cycled 2/4-CPU
hotplug gates.
The heap metadata follow-up replaces the fixed live-object and free-interval
tables with per-span headers and an in-place coalescing free list. Host tests
and a 4,096-object QEMU probe cover the former 2,048-object ceiling; mapped
runtime growth and broad allocator throughput remain open. See the
[heap metadata audit](docs/audit-stages-consolidated.md).
The final metadata source passed the full 33-check release matrix and the
twice-cycled 2/4-CPU hotplug gates.
Runtime heap growth now maps and publishes 256 KiB chunks outside the heap
guard. Rank-free, IRQ-enabled allocations can grow on failure; BSP idle maps
ahead for guarded callers when free space is low. QEMU exercises growth after
SMP startup and reserve maintenance. Sudden large guarded allocations,
low-memory behavior, and multicore throughput still need qualification; see
the [heap growth audit](docs/audit-stages-consolidated.md).
The final growth source passed the complete 33-check release matrix and
twice-cycled 2/4-CPU hotplug gates.
The frame allocator now reserves a bitmap in each usable physical range and
uses it to track allocations, reject double frees, and recover returned
frames beyond its 4,096-entry hot cache. See the
[frame reclamation audit](docs/audit-stages-consolidated.md).
The QEMU memory gate exercises 4,352 returned frames; physical high-RAM and
NUMA latency qualification remains open.
The final frame-reclamation source passed the full 33-check release matrix
and dedicated twice-cycled 2/4-CPU hotplug gates.
The bounded device, keyboard decoder, journal, swap-state, mount-record, and
key-vault tables now use IRQ-safe locks. Keyboard input preserves the caller's
pre-lock interrupt state for early-boot polling; Stage 1-5 journal, Stage 12.3
key-vault, Stage 12.5 mount-record, and normal PS/2 shell gates passed.
The full release matrix and dedicated twice-cycled 2/4-CPU hotplug gates also
passed after the keyboard interrupt-state correction.
The FAT32 clean-page cache now uses a rank-20 IRQ mutex only for short metadata
and page-copy sections; physical page loads run after releasing it, with an
invalidation epoch preventing stale publication. Host, full release, and
2/4-CPU hotplug gates passed. Cross-layer FAT32 mutation transactions remain
open for unrestricted concurrent filesystem service.
The shell current-directory state now has a short rank-10 IRQ guard, with a
fixed-buffer copy released before console output or allocation. The terminal
has a rank-10 preemption guard that keeps device IRQs live during rendering,
and syscall writes restore live IRQs around full-frame operations. Lock-order
tracker updates remain interrupt-atomic. This closes the audited terminal and
shell compatibility-lock gap, but not the other Stage 6 production gaps above.
The final source passed the complete release matrix and twice-cycled 2/4-CPU
hotplug gates; details are in the lock-order audit.
The network runtime and VirtIO transport now declare ranks 20 and 30 for
their existing runtime-to-transport nesting. No kernel `IrqMutex::new` call
sites remain; this is rank coverage, not proof of every cross-subsystem path.
The ranked network change passed the full release matrix and twice-cycled
2/4-CPU hotplug gates.

## Stage 10.7 — accepted on 2026-09-16

The imported recovery source is preserved in Git commit `49e789d`. That commit
records the baseline only and does not certify acceptance.

The audit corrected worker starvation, local-preemption lock safety, readiness
observation, host-exchange validation and evidence retention. The complete gate
ended with `=== STAGE 10.7 ACCEPTANCE: PASS ===` and host exit status 0.

| Gate | Result |
| --- | --- |
| `cargo build` | Passed |
| Host and freestanding-kernel Clippy with `-D warnings` | Passed |
| Rust host regressions | 13 passed |
| Python host-harness regressions | 4 passed |
| Memory/scheduler/pager/IPC/security preservation | 1 CPU: 10/10; 2 CPUs: 30/30; 4 CPUs: 20/20 |
| Live DHCP/DNS/ICMP/UDP/TCP | 1/2/4 CPUs passed |
| Stage 10.4 asynchronous block I/O | 1/2/4 CPUs passed |
| Stage 10.5 asynchronous file I/O | 1/2/4 CPUs passed |
| Stage 10.6 asynchronous UDP | 1/2/4 CPUs passed |
| Strengthened Stage 10.7 asynchronous TCP | 1/2/4 CPUs passed |

Total: 75 QEMU boots passed in the final full chain. Each boot required debug-exit
status 33 and its required markers; network harnesses also verified host traffic.
Final build/lint/host checks also passed and are recorded under
`audit-artifacts/final-static/`. The accepted-stage commit follows the imported
baseline as a separate Git commit; use `git log --oneline` to identify it.

## Stage 10.8 — accepted on 2026-09-16

Completion ports, operation association, batch poll/wait, bounded reservations,
cancellation events, generation retirement and timeout-aware scheduler blocking
are implemented. The full gate passed with exit code 0: all 75 Stage 10.7 boots
plus completion-port boots on 1/2/4 CPUs (78 total). The latter include Ring-3
copyout retry, measured timeout, teardown, SMP producers and consumer wakeup.
Build, warning-denying host/kernel/probe Clippy, 18 Rust tests and four Python
harness tests passed. Final host tests include waiter notification and port quota.

Full transcript: `audit-artifacts/acceptance-20260916-085314-575/acceptance-output.txt`.
Focused serial evidence is retained in timestamped `audit-artifacts/stage10.8-*`
directories. See [ABI contract](completion-port-abi.md) and
[audit](docs/audit-stages-consolidated.md).

## Stage 10.9 — accepted on 2026-09-16

Timers, deadlines, asynchronous event objects, cancellation and sleep-until
are implemented under bounded process ownership. The 1/2/4-CPU QEMU gate,
build, warning-denying Clippy and Rust regressions passed. See
[audit](docs/audit-stages-consolidated.md).

## Stage 10.10 — accepted on 2026-09-16

The aggregate gate validates the Stage 10.8 completion architecture together
with Stage 10.9 timers/events on 1, 2, and 4 CPUs. All six boots passed with
exit 33 and retained serial evidence. Stage 10 is closed; Stage 11 follows.

## Stage 11.1 — accepted on 2026-09-16

PID lifecycle, parent/child ownership, exit status, wait semantics, process
groups, isolated resources and teardown passed build, Clippy, host tests, and
1/2/4-CPU QEMU validation. See [audit](docs/audit-stages-consolidated.md).

Next: Stage 11.2 per-process threads and join/TLS semantics.

## Stage 11.2 — accepted on 2026-09-16

Generation-safe thread IDs, owner-checked join, termination status, TLS and
thread-local errno state passed build, Clippy, host tests, and 1/2/4-CPU QEMU
validation. See [audit](docs/audit-stages-consolidated.md).

Next: Stage 11.3 structured notifications and lifecycle events.

### Stage 11.2 scheduler-backed thread follow-up — accepted increment on 2026-10-14

The bounded scheduler-backed user-thread path now has an end-to-end Ring-3
create/exit/join acceptance. It uses private NX stacks, copied initial-exec
TLS, stable process ownership, generation-safe IDs, and post-switch resource
reclamation. Build, warning-denying Clippy, host tests, and 1/2/4-CPU QEMU
passed; see [audit](docs/audit-stages-consolidated.md).

The 2026-10-15 cancellation follow-up adds ABI 136 for owned sibling-thread
cancellation. Ready/blocked and running targets both remain joinable, wake the
registered joiner, and reclaim stack/TLS mappings only through join. The
focused 1/2/4-CPU QEMU gate, host tests, and warning-denying Clippy passed;
see the scheduler-thread audit. Process-wide teardown, shared libraries,
symbol resolution, dynamic TLS, and hardware qualification remain open.

The 2026-10-15 teardown follow-up closes the process-wide scheduler lifecycle
for unjoined threads: process exit retires ready/blocked siblings, running
siblings take the thread-safe termination path, and the parent wait defers
address-space destruction until all sibling hand-offs complete. The exact
1/2/4-CPU gate now exercises create/exit/join, cancel/join, and process exit
with an unjoined sibling.

## Stages 11.3–11.5 — accepted on 2026-09-16

Structured notifications, the `libwoven` userspace API boundary, and hardened
ELF/W^X loader validation passed their individual 1/2/4-CPU QEMU gates,
freestanding Clippy, and host tests. See [audit](docs/audit-stages-consolidated.md).

The follow-up ASLR pass now randomizes production ELF, stack and mmap bases
with a deterministic `qemu-test` switch; see [audit](audit-aslr-2026-09-16.md).

The 2026-10-05 loader-boundary follow-up rejected `PT_INTERP`, `PT_DYNAMIC`,
`PT_TLS`, and `PT_GNU_RELRO` program headers instead of silently accepting
binaries whose dynamic-linking, TLS, or RELRO semantics were not implemented
at that point. This was a fail-closed production-safety boundary; later
follow-ups narrow the remaining unsupported surface.
See [audit](docs/audit-stages-consolidated.md).

The 2026-10-14 RELRO follow-up closes the static RELRO permission transition:
a single validated `PT_GNU_RELRO` range is protected read-only after loading,
ASLR relocation carries the range, and COW write-fault bookkeeping refuses to
reattach write authority to it. Valid/invalid parser cases and the complete
Stage 11.5 1/2/4-CPU gate passed. Shared libraries, symbol-based relocation,
and physical/hardware qualification remain open. See
the [RELRO audit](docs/audit-stages-consolidated.md).

The 2026-10-14 static TLS follow-up adds a bounded initial-exec `PT_TLS`
contract: the loader validates and maps one initial TLS image, task context
switches install its FS base, and fork/exec preserve the ownership and COW
rules. Shared-library TLS, TLS module IDs, multi-thread TLS semantics, and
physical/hardware qualification remain open. See the
[static TLS audit](docs/audit-stages-consolidated.md).

The 2026-10-14 dynamic-relocation follow-up accepts bounded self-contained
ET_DYN images with R_X86_64_RELATIVE plus locally defined symbol RELA entries.
SysV hash/symbol/string metadata is bounded and validated; relocations are
applied before W^X and RELRO finalization. The inherited 1/2/4-CPU Stage
11.3–11.5 matrix passed with an end-to-end mapped-image local-symbol
relocation self-test. Shared-library dependency mapping, cross-image symbol
resolution, versioning, lazy binding, and dynamic TLS remain fail-closed. See
the [dynamic relocation audit](docs/audit-stages-consolidated.md).

The 2026-10-15 scope groundwork adds an immutable, bounded exact-name
cross-image symbol-scope contract with duplicate/capacity/unknown-name
negative checks. It is not yet connected to dependency mapping or relocation;
the loader remains fail-closed for `DT_NEEDED`. Neighboring Stage 11.3/11.4
regression matrices pass, while the final 11.5 2/4-CPU rerun remains pending
after QEMU contention in the initial report; those final reruns subsequently
passed as recorded in the [symbol-scope audit](docs/audit-stages-consolidated.md).

The symbol-scope review found ASLR address-validation, export-visibility,
dependency-order and dependency-index defects that the zero-bias fixture had
missed. The production local-relocation path now shares the corrected address
validation with scope lookup. Parser-backed host regressions cover the failing
cases, private versus exported symbols, overflow and segment bounds. Full
shared-library mapping and teardown remain open.

The subsequent signed-library integration implements a bounded `/lib` graph,
cross-image eager relocation, per-library RELRO, transactional load rollback,
and complete private-page-table reclamation. The new gate executes a Ring-3
import call returning 42 and requires exact frame reclamation, along with
signature, namespace and rollback negatives. Final acceptance and inherited
regression results are tracked in the
[shared-library audit](docs/audit-stages-consolidated.md).
Versioning, legacy PLT/REL tables, constructors/destructors, dynamic library
TLS, runtime loading and broader qualification remain open.

## Stage 12.1 — accepted on 2026-09-16

The VFS 2.0 typed interface and SystemVfs adapter passed build, freestanding
Clippy, and QEMU validation on 1/2/4 CPUs. See
[audit](docs/audit-stages-consolidated.md).

Next: Stage 12.2 WovenFS metadata and crash-consistency design.

## Stage 13.1 — accepted on 2026-09-16

The WovenDriver manager passed device matching, binding, suspend/resume,
build, freestanding Clippy, and 1/2/4-CPU QEMU validation. See
[audit](docs/audit-stages-consolidated.md).

## Stage 13.2 — topology/ownership increment implemented; full acceptance open (2026-09-27)

The PCIe foundation now includes bounded generation-tagged function/bridge
topology, explicit single-owner claims, generation-tagged logical MMIO BAR
leases, teardown-before-reuse invalidation, and bridge bus-window parentage.
The dedicated preservation gate requires focused host tests, warning-denying
kernel Clippy, runtime-harness regressions, and 1/2/4-CPU QEMU evidence.

Full Stage 13.2 acceptance remains open for physical BAR qualification,
bridge interoperability beyond the QEMU fixture, MSI qualification beyond the
QEMU SATA path, MSI-X delivery, PCIe link/power hotplug, and device-specific
reset/recovery. The software BAR probe, bounded
allocation/rebalance, bridge routing transaction, interrupt ownership paths,
and hotplug reconciliation are covered by the follow-ups below. Raw legacy
MMIO mappings remain shared and non-revocable; owner-scoped mappings are
covered by the follow-up below.

The WovenDriver integration item now has a bounded acceptance probe. The
Stage 13.2 gate claims one discovered function for a synthetic feature-gated
owner, retains its programmed MMIO BAR leases, and releases subordinate
resources before function ownership. The 1/2/4-CPU preservation gate passed;
see [the lifecycle audit](docs/audit-stages-consolidated.md).
This does not close the remaining physical rebalance, hardware interrupt
qualification, hotplug, or MMIO mapping-revoke requirements.

The follow-up adds owner-scoped page mappings: raw legacy MMIO mappings remain
shared and non-revocable, while PCI driver mappings carry an owner token and
are revoked after interrupt quiescence and before BAR/function release. The
1/2/4-CPU gate passes and proves same-page remap after teardown. QEMU's only
candidate firmware BAR is outside the direct-map range and is rejected
fail-closed, so this is not physical device-BAR qualification. Remaining work
is MSI-X/physical interrupt qualification, PCIe link/power hotplug, and
valid-BAR hardware qualification beyond the QEMU fixture.

The acceptance probe now also runs the driver-aware no-removal rescan after
teardown and recomputes bridge routing before it finishes. The 1/2/4-CPU
matrix passed; this validates coordinator ordering but does not substitute for
physical PCIe insertion/removal qualification.

Initial PCI discovery now has the same admission boundary: `hal::init` uses
discover-and-route, bridge-bearing generations are not driver-admissible until
the rollback-capable bridge transaction succeeds, and routing/discovery failure
rejects later driver claims fail-closed. The 1/2/4-CPU preservation gate passed;
see the [admission-routing audit](docs/audit-stages-consolidated.md).
This validates ordering and failure policy, not physical bridge-window
interoperability on hardware absent from QEMU.

The ECAM configuration path now applies the mapped page offset for both reads
and writes. Before this repair, live ECAM discovery repeated the vendor/device
dword at every offset, masking real class, BAR, and capability behavior. The
corrected path is covered by the 1/2/4-CPU gate and the
[ECAM offset audit](docs/audit-stages-consolidated.md).

The reconciliation boundary also clears admission before publishing a
refreshed inventory, eliminating the interval where a concurrent bind could
observe new topology before bridge programming completed. The updated 1/2/4
CPU matrix passed with warning-denying Clippy.

The firmware BAR rebalance path now retains successful reservations in bounded
persistent PCI resource state, rejects duplicate function admission, clears
and verifies BAR registers before releasing reservations, and refreshes the
published device state. The focused freestanding check and warning-denying
Clippy pass; see the [persistent BAR audit](docs/audit-stages-consolidated.md).

Persistent BAR teardown now verifies PCI command-space decoding is disabled,
clears the assigned BARs, refreshes inventory, and reroutes bridge windows
before releasing address reservations; failed verification retains the
allocation for retry. This prevents allocator reuse while a device could
still decode its old BAR assignment.

Normal PCI driver admission now invokes the BAR allocator for the safe
all-unassigned case, refreshes the published BAR view before leasing, rejects
mixed layouts fail-closed, and checkpoints persistent ownership through
unbind. Successful rebalance also reroutes the bridge hierarchy before any
MMIO lease is published. See the [automatic BAR admission audit](docs/audit-stages-consolidated.md).
QEMU still lacks a valid physical unassigned-BAR fixture, so the positive
hardware allocation path remains unqualified.

PCI rescans now report newly inserted functions through a bounded
`hotplug_rescan_report` while preserving the existing summary-only API. Full
driver-relevant identity changes at a BDF are detected; if replacement occurs
before old-owner quiescence, the entire batch fails closed before any teardown
configuration write. Persistent BAR reservations are released before unbound
generations are invalidated, and new functions are verified after topology
publication and bridge routing before being returned to driver matching;
unknown hardware is never auto-bound. See the [hotplug insertion audit](docs/audit-stages-consolidated.md).
Physical PCIe link/power hotplug remains hardware-dependent.

The driver manager now provides bounded PCI identity matching and an explicit
match-then-bind API for insertion reports. Empty matchers are rejected and
unknown hardware remains unclaimed. See the [driver-matching audit](docs/audit-stages-consolidated.md).
Device-specific drivers, initialization policies, and physical PCIe
interoperability remain later qualification work.

The driver layer now exposes generic interrupt selection: MSI-X is attempted
when capability and retained BAR authority permit it, clean failures fall back
to MSI, and activation-retained leases never fall back or lose ownership. See
the [interrupt-selection audit](docs/audit-stages-consolidated.md).
Physical vector delivery remains unqualified on QEMU.

PCI driver suspend/resume now owner-checks the function, disables and verifies
command decoding, and, when the PCI Power Management capability is present,
checkpoints PMCSR, enters D3hot, restores D0, and restores the exact command
checkpoint before resuming. Missing checkpoints and failed transitions fail
closed. See the [PCI suspend/resume audit](docs/audit-stages-consolidated.md).
Device-specific reset, D3cold, wake signaling, and physical power-management
qualification remain open.

The generic MSI programming path now snapshots and restores the original
capability/message state on every configuration failure, including a failed
initial disabled-header write. Every header, address, and data write is
read-back verified; `RestoreFailed` is returned when repair itself cannot be
verified. Vector and interrupt-mode ownership remain retained until successful
quiescence. Corrected ECAM offset handling now decodes live PCI classes and
capabilities correctly; the QEMU SATA fixture exercised generic MSI activation
and ordered teardown on the 1/2/4-CPU preservation matrix. See the
[MSI rollback audit](docs/audit-stages-consolidated.md). Physical
MSI qualification beyond this QEMU fixture and MSI-X table/vector delivery
remain hardware-dependent.

MSI-X activation now asserts Function Mask and keeps MSI-X disabled before any
table MMIO. Table reads and programming use owner-scoped MMIO mappings and
release those mappings after each bounded volatile operation. If programming
has made a table entry device-visible but cleanup fails, the `MsixLease` is
retained for ordered teardown rather than losing the vector obligation. The
Stage 13.2 probe now also scans disposable QEMU PCI fixtures, activates a real
MSI-X lease when its table BAR is admissible, and verifies the bounded
high-MMIO fallback mapping; the 1/2/4-CPU preservation matrix passed with
those fixtures present. See the
[MSI-X mapping audit](docs/audit-stages-consolidated.md). Physical
MSI-X delivery, PCIe hotplug, and physical device lifecycle remain
unqualified because QEMU does not provide those physical conditions.

High 64-bit BAR pages no longer depend on the low fixed virtual alias. The
bounded high-MMIO window retains each mapping's virtual address and uses that
address for revocation/release; see the
[high-MMIO audit](docs/audit-stages-consolidated.md).

Stage 13.3 NVMe now passes its ordered 1/2/4-CPU QEMU acceptance gate. The
driver identifies a namespace, creates admin and I/O queues, performs
write/read/flush against a disposable image, returns owned DMA pages on failed
initialization, and restores the original PCI command after bounded controller
quiescence. See the [Stage 13.3 NVMe audit](docs/audit-stages-consolidated.md).
This closes the emulated synchronous foundation only; interrupt-backed
completion, reset/recovery, hotplug removal, multi-namespace policy, and
physical NVMe qualification remain open.

Stage 13.8 HDA/WovenAudio now passes its ordered 1/2/4-CPU QEMU gate after
correcting register page-offset handling in all HDA MMIO helpers. The gate
covers codec discovery, topology, playback/capture streams, mixer state, and
the WovenAudio API boundary. See the [HDA MMIO audit](docs/audit-stages-consolidated.md).
Physical codec interoperability and interrupt/DMA fault recovery remain open.

The local Stage 14.1–14.5D feature gates now pass build/lint and live network
acceptance on 1/2/4 CPUs. A shared PCI `expect(dead_code)` guard was extended
through the later feature family after Stage 14.1 exposed that it could not
compile cleanly under warning denial. See the [Stage 14 feature-gate audit](docs/audit-stages-consolidated.md).

## Stages 12.2–12.5 — accepted on 2026-09-16

WovenFS metadata/integrity, volume-integrity envelope boundaries, snapshots,
and storage-management inventory each passed their 1/2/4-CPU gates. See
[audit](docs/audit-stages-consolidated.md). Production AEAD now passes the
isolated 1/2/4-CPU gate with a bounded revocable key vault; see
[audit](audit-volume-crypto-2026-09-16.md). The follow-up integration now
covers measured provisioning, authenticated persistent wrapped keys,
successor-generation rotation, and encrypted mount-record I/O. A TPM/firmware
measured-root backend, physical power-loss evidence, and deployment key-slot
policy remain open.

## Stage 7.1.5 - accepted on 2026-09-16

The scheduler/pager watchdog and termination lifecycle gate passed in full:
50 one-CPU memory runs, 100 two-CPU runs, 50 four-CPU runs, and live
DHCP/DNS/ICMP plus host-verified UDP/TCP at 1/2/4 CPUs. See
docs/audit-stages-consolidated.md for the bounded lifecycle handoff
correction and retained serial evidence.

## Roadmap Stage 13.10A — Bluetooth HCI core accepted on 2026-09-30

The hardware-independent Bluetooth HCI foundation is accepted. Bounded command encoding, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests passed the dedicated 1/2/4-CPU QEMU gate with exit 33 in GitHub Actions run 36718370038. See [acceptance audit](docs/audit-stages-consolidated.md).

Historical Wi-Fi labels 13.10A–AC are Stage 13.9 AX200/WovenWiFi extensions; roadmap Stage 13.10 is Bluetooth. Stage 13.10A does not claim USB transport or physical Bluetooth hardware support.

Next: Stage 13.10B Bluetooth USB HCI transport over xHCI.


## Roadmap Stage 13.10B — Bluetooth USB HCI transport contract accepted on 2026-09-30

The xHCI-owned Bluetooth USB transport contract is accepted at the descriptor/protocol boundary. Stage 13.10B recognizes the USB Wireless Controller / RF Controller / Bluetooth Primary Controller interface (E0/01/01), validates the interrupt-IN HCI event endpoint plus bulk-IN/bulk-OUT ACL endpoints, and constructs the class control-request setup packet used to carry HCI commands. The hardware-independent `bluetooth_hci.rs` protocol layer remains separate from USB transport semantics.

GitHub Actions run 36722602921 passed at source SHA `0ba3ce257f93775afe67ed975f8549fcdbb78506`, including lint/host regressions, the 1/2/4-core boot matrix, Stage 13.3–13.9 regression gates, and the Stage 13.10 runtime gate. The runtime harness requires `[S13.10B] Bluetooth USB HCI transport contract: PASSED`; the workflow step label still says Stage 13.10A and should be renamed in the next acceptance-maintenance change.

This acceptance is a transport-contract milestone, not a physical-controller claim. Live xHCI endpoint/ring ownership, HCI command/event transfer execution, ACL transfer execution, controller initialization, discovery, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical hardware qualification remain open.

Next: Stage 13.10C live USB HCI transfer execution and controller initialization boundary.


## Roadmap Stage 13.10C — bounded HCI transfer execution contract accepted on 2026-09-30

The Bluetooth command/event transaction boundary is accepted. The HCI core now serializes one outstanding command against controller credits, rejects a second command while one is in flight, requires the matching Command Complete opcode, and restores readiness only after successful completion. The xHCI Bluetooth layer also exposes bounded command, interrupt-event, ACL-IN, and ACL-OUT transfer plans derived from the accepted Stage 13.10B interface/endpoint contract.

GitHub Actions run 36729724615 passed at source SHA `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`. The dedicated “Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance” gate and the complete Stage 13.3–13.9 regression chain passed. Preserved release-validation artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`.

This milestone does not yet claim a physical Bluetooth controller or live USB transfer completion. Dedicated xHCI Bluetooth endpoint contexts/rings and DMA buffers, live control-OUT HCI command submission, interrupt-IN event reception, ACL execution, Reset/Read Local Version against hardware, teardown/recovery, and physical qualification remain open.

Next: bind the accepted transfer plans to dedicated xHCI rings/DMA ownership and execute the controller-initialization sequence.


## Roadmap Stage 13.10D — live USB HCI mechanics and controller initialization accepted on 2026-09-30

Stage 13.10D materializes the Bluetooth USB transport contract as xHCI-owned state: endpoint-zero control-OUT-with-data for HCI commands, dedicated interrupt-IN event and bulk-IN/bulk-OUT ACL endpoint contexts, separate transfer rings and DMA buffers, direction-aware DCI mapping, and bounded event/ACL execution primitives. The hardware-independent HCI layer adds a fail-closed controller initializer that requires Reset Command Complete before Read Local Version and reaches Ready only after the matching second completion.

GitHub Actions run 36747805657 passed at exact source SHA `41e009543e01afc6a348122ff3a2c4f0be16e351`. The dedicated “Stage 13.10D Bluetooth controller init 1/2/4-core acceptance” step passed together with the complete regression chain. Preserved release-validation artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`.

This acceptance proves the bounded transport/initialization architecture under deterministic CI; it does not claim physical Bluetooth-radio qualification because the QEMU acceptance device is not a Bluetooth controller. Physical USB-controller execution, teardown/recovery, hotplug, and radio qualification remain explicit hardware work.

Next: Stage 13.10E Bluetooth discovery/scanning and controller capability discovery above the accepted HCI initialization boundary.


## Roadmap Stage 13.10E — Bluetooth discovery and controller capability contract accepted on 2026-09-30

Stage 13.10E adds bounded BR/EDR GIAC Inquiry discovery above the accepted HCI initialization boundary. It constructs the Inquiry command, parses Inquiry Result and Inquiry Complete events, retains up to 16 unique devices by Bluetooth address, and records page-scan repetition mode, class-of-device, and clock offset. Malformed/truncated events fail closed and duplicate addresses do not consume additional discovery slots.

The HCI layer also parses controller identity/capability responses for Read Local Version, Read BD_ADDR, and Read Local Supported Commands. Opcode, controller status, and response length are validated before exposing HCI/LMP version data, manufacturer/subversion, local Bluetooth address, or the 64-byte supported-command bitmap.

GitHub Actions run 36753355665 passed at exact source SHA `98020bfbfa71dd68f0fc54b234761490947fe346`, including the Stage 13.10E 1/2/4-core acceptance gate and evidence preservation. Release-validation artifact digest: `sha256:dd94f81eed0b3916135051b78c0c27944a33084fba8405c1045dcf49e8196fe2`.

This remains deterministic software validation; physical Bluetooth-radio scanning is not claimed. BLE advertising reports, remote-name discovery, connection establishment, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical interoperability remain later work.

Next: Stage 13.10F Bluetooth connection establishment and bounded link lifecycle above the accepted discovery state.


## Roadmap Stage 13.10F — Bluetooth ACL connection and link lifecycle accepted on 2026-09-30

Stage 13.10F consumes bounded discovery records to construct classic BR/EDR Create Connection commands, validates Connection Complete and Disconnection Complete events, and owns up to eight live ACL links by 12-bit controller handle and Bluetooth address. Duplicate identical completion is idempotent; conflicting handle/address reuse fails closed.

The HCI layer now frames and parses bounded ACL packets (1024-byte software ceiling), validates handle/packet-boundary/broadcast fields, and permits inbound/outbound ACL data only for currently owned live handles. Disconnect immediately revokes ACL authority, so stale-handle traffic is rejected.

GitHub Actions run 36761870548 passed at exact source SHA `79cecd7b0e3bab5db4426e2f6fed3eaa451c0a2b`, including Stage 13.10F 1/2/4-core acceptance and evidence preservation. Artifact digest: `sha256:878f9d8866f09fd6e202062dbfa0784073ae7dbed5e196eb223387c26b40410c`.

This remains deterministic software validation. Physical controller timing, ACL flow-control credits, asynchronous transport recovery and real peripheral interoperability remain hardware/production qualification work. Next: Stage 13.10G L2CAP framing and bounded channel lifecycle above owned ACL links.


## Roadmap Stage 13.10G — Bluetooth L2CAP accepted on 2026-09-30

Stage 13.10G adds bounded L2CAP Basic Mode framing above the Stage 13.10F owned ACL link. L2CAP length/CID parsing is exact, CID zero is rejected, payload storage is fixed-capacity, and every frame remains anchored to a live ACL handle.

The signaling layer implements bounded Connection Request/Response and Disconnection Request/Response state with dynamic local CIDs beginning at 0x0040 and a fixed eight-channel table. Established channels own the tuple of ACL handle, PSM, local CID and peer CID. Outbound data targets only the peer CID; inbound data is accepted only for the owned local CID on the same live ACL handle. Teardown revokes channel authority immediately.

GitHub Actions run 36785295506 passed at exact source SHA `c2dff3c12c9c9387a10493900b3846489e8539a6`, including the Stage 13.10G 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:6ac9ff255444ffa36128cd03d75f3d2c89eff086e308618ed21fed463c4a8eab`.

This is deterministic software acceptance. Configuration negotiation, fragmentation/reassembly, enhanced modes, controller flow control and physical interoperability remain outside this boundary. Next: Stage 13.10H pairing, authentication and encryption/security.


## Roadmap Stage 13.10H — Bluetooth pairing/authentication/encryption accepted on 2026-10-01

Stage 13.10H adds a fail-closed BR/EDR security boundary above the accepted ACL/L2CAP ownership layers. A live ACL link progresses explicitly through unauthenticated, authenticating, authenticated, encryption-enabling and secured controller states. Authentication or encryption failure does not grant secured authority.

The stage also adds bounded link-key ownership, Link Key Request positive/negative replies, Link Key Notification ingestion, explicit key revocation, and policy-mediated Secure Simple Pairing interactions for IO capability, numeric confirmation and passkey requests. Pairing confirmation is never silently auto-approved.

Trusted secured authority requires all of: a live owned ACL handle, the same peer address owning a stored link key, successful controller authentication, and controller-confirmed encryption. Removing the key or disconnecting the ACL link makes the trust predicate fail closed.

GitHub Actions run 36791065311 passed at exact source SHA `dda2933eef36026ab70cefbe85898bcd62256f06`, including the Stage 13.10H Bluetooth security 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f731dbd2f52dc7c2743aa052eaa7f801eef99dc0fc6050f13e6fa71518a07405`.

This is deterministic software acceptance. Persistent secure key storage, full controller event orchestration, physical-radio pairing/interoperability and BLE security remain outside this boundary.


## Roadmap Stage 13.10I — Bluetooth LE foundation accepted on 2026-10-01

Stage 13.10I establishes a bounded Bluetooth Low Energy controller/discovery and connection-lifecycle foundation without conflating LE identity or security with the accepted BR/EDR link-key model. The HCI layer can configure scanning, enable or disable scanning, parse LE Meta Advertising Reports, retain public/random peer address type, capture bounded legacy advertising data and RSSI, and deduplicate repeated reports by address type plus address.

The connection layer consumes an LE discovery record to build LE Create Connection, accepts exact LE Connection Complete events into a fixed eight-link ownership table, rejects malformed, conflicting or out-of-range handle/address state, and removes ownership on Disconnection Complete. A disconnected LE handle is immediately stale and cannot retain authority.

GitHub Actions run 36810460362 passed at exact source SHA `edcfe31b5cd1406c4feb2d001dbc0e2d2f6d7a9f`, including the Stage 13.10I Bluetooth LE foundation 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:f14abafba66e1a599c2410e7e458c3cd2447ca783780deb688afffbfb26b5251`.

This is deterministic software acceptance. Physical LE controller/radio interoperability, extended advertising, LE privacy/resolving lists, connection-update orchestration, ATT/GATT and BLE SMP/bonding remain outside this boundary. Next: Stage 13.10J ATT/GATT foundation above owned LE ACL links.


## Roadmap Stage 13.10J — Bluetooth ATT/GATT foundation accepted on 2026-10-01

Stage 13.10J builds a bounded ATT/GATT software contract above live Stage 13.10I LE link ownership. ATT read/write transactions reject stale LE handles, malformed PDUs, invalid attribute handles and permission violations. The attribute database is fixed-capacity and uses nonzero handles with bounded values.

The GATT layer represents 16-bit Primary Service and Characteristic Declaration/value attributes, preserves ATT permission enforcement, supports bounded service/characteristic discovery by handle range, and models per-LE-link notification/indication subscriptions. Subscription emission rechecks the live LE handle and exact characteristic value handle; unsubscribe or disconnect revokes authority.

GitHub Actions run 36815325948 passed at exact implementation SHA `48bdd085e8d8918bb058a59de06d42ce8ad7108a`, including the Stage 13.10J 1/2/4-core acceptance and evidence-preservation steps. Artifact digest: `sha256:9247004a999b8470e5431a2396d1d1237bd1cd278f43abdf9ebf9460ed0a3dca`.

This is deterministic software acceptance. Physical BLE ATT/GATT interoperability, ATT MTU negotiation, 128-bit UUIDs, full wire-level discovery procedures, indication confirmations, BLE SMP/bonding and protected persistent keys remain outside this boundary. Next: Stage 13.10K BLE profiles and services above the accepted ATT/GATT authority model.


## Roadmap Stage 13.10K — BLE profiles and services acceptance candidate on 2026-10-01

Stage 13.10K builds bounded BLE services above the accepted Stage 13.10J ATT/GATT authority model. Standard profile support includes Device Information with read-only Manufacturer Name and Model Number characteristics plus Battery Service with a range-checked read-only Battery Level.

Battery updates can emit Handle Value Notifications only through an existing per-link, per-value-handle GATT subscription. Failed authorization, invalid levels, wrong handles or stale LE links leave the prior battery state unchanged. The stage also adds a bounded WovenHat OS service with a read-only status characteristic and write-only command characteristic, preserving ATT permission checks and live-link authority.

The accepted profile checkpoint run 36819330823 passed at exact source SHA `c5f3c6194c58ad16e75c0f44cf90c6d336393c40`. Preserved evidence digest: `sha256:677f88e58b878de73553225ec533f76c4d1b0e74864b1a69aaa30367caef50db`.

Closure hardening makes characteristic creation transactional with respect to ATT table capacity: two slots are preflighted before declaration/value insertion, preventing a capacity failure from leaving an orphan declaration.

This remains deterministic software validation. Physical BLE profile interoperability, standardized conformance testing, BLE SMP/bonding, persistent protected keys and production controller recovery remain outside this boundary. Next after final exact-head acceptance: Stage 13.10L Bluetooth lifecycle and hardening.


## Roadmap Stage 13.10L — Bluetooth lifecycle and hardening acceptance candidate on 2026-10-01

Stage 13.10L hardens the accepted BLE software authority model around teardown, reset and reconnect. `BluetoothLeLifecycle` coordinates live `LeLinkState` with `GattSubscriptions`: a valid disconnect removes the link and revokes every subscription owned by that handle, while controller reset clears all LE links and subscriptions. A monotonic wrapping generation records successful lifecycle invalidation.

Malformed disconnect events and unknown/stale handles fail closed without changing links, subscriptions or generation. Reconnecting a peer does not inherit its previous subscription authority; explicit re-subscription is required before notification emission is authorized.

The bounded stress gate executes 32 connect/subscribe/notify/teardown cycles, alternating normal disconnect with controller reset. Every cycle must end with zero live links, zero subscriptions and stale notification rejection. The accepted stress checkpoint run 36825376710 passed at exact SHA `4c0b0e5e0c8d3b9e00b47dc58868a154c9c970ef`; preserved evidence digest `sha256:67cc6f35203061ef8a79dfba3a8fef739db667bf7fb907b1eb989659c104f9e0`.

This acceptance remains deterministic software validation. Physical Bluetooth controller/radio recovery, transport fault injection, BLE SMP/bonding/LTK security, persistent protected keys, radio interoperability and conformance certification remain outside this boundary.


## Roadmap Stage 13.10R — BLE secure-session identity binding accepted on 2026-10-01

Protected LE ATT/GATT operations now require the transient secure session to
match the live link's peer address type and address as well as its connection
handle. This rejects stale authorization when a controller reuses a handle for
a different peer, including outbound notifications and indications. The exact
tip passed host tests, warning-denying kernel Clippy, and the Stage 13.10
1/2/4-CPU QEMU gate with the identity-binding marker.

This remains deterministic software validation. Physical BLE controller/radio
interoperability, transport fault injection, BLE SMP/bonding/LTK security,
persistent protected keys, radio interoperability and conformance
certification remain outside this boundary.


## Stage 13.11A — BLE SMP pairing/fixed-channel foundation — 2026-10-05

The BLE Security Manager Protocol foundation validates bounded Pairing Request
and Response parameters, enforces supported key-size and distribution flags,
and accepts SMP frames only on fixed L2CAP CID `0x0006` for a live LE link.
Pairing negotiation is handle-bound and fail-closed on malformed or stale
traffic. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub acceptance
for the pushed tip remains required.

Cryptographic key derivation, user interaction, passkey/numeric comparison,
bonding persistence, controller transport and physical BLE qualification remain
outside this foundation.


## Stage 13.11B — BLE SMP LTK/key distribution — 2026-10-05

Legacy Encryption Information and Master Identification PDUs are now validated
and integrated with the canonical LE bond store. Distribution is bound to the
live pairing handle, negotiated key size and peer identity; malformed, stale,
duplicate and mismatched inputs fail closed. The dedicated 1/2/4-CPU QEMU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Cryptographic derivation, user-confirmed pairing, controller transport and
physical BLE qualification remain outside this bounded distribution stage.

The distribution closure additionally binds pending LTK material to peer
identity, rejects duplicate pending material, and rejects completion after
connection-handle reuse.


## Stage 13.11C — BLE SMP confirm/random verification — 2026-10-05

Pairing Confirm and Pairing Random verification is now handle-bound and tied to
the negotiated pairing parameters and peer address context. Confirm mismatches
transition the bounded state to failure; malformed or out-of-order messages
fail closed. The dedicated 1/2/4-CPU QEMU gate passes locally; GitHub
acceptance for the pushed tip remains required.

This is a protocol/crypto boundary, not complete production pairing. User
confirmation UX, authenticated key-derivation integration, bonding
persistence, controller transport and physical BLE qualification remain open.


## Stage 13.11F — BLE controller encryption/session authority — 2026-10-05

The SMP flow now owns the LE Start Encryption command boundary and binds
Encryption Change completion to the pending handle, peer identity, key size and
authentication state. Failed, disabled, stale or mismatched events fail closed
and revoke authority. The dedicated 1/2/4-CPU gate passes locally; GitHub
acceptance for the pushed tip remains required.

Physical controller transport, SMP interoperability, bond persistence and
hardware qualification remain outside this deterministic software boundary.


## Stage 13.11G — BLE identity/privacy foundation — 2026-10-05

The SMP layer now validates identity distribution, stores bounded IRK-backed
peer identities, and resolves private addresses only against retained identity
material. Duplicate, malformed and stale inputs fail closed, while pending LTK
distribution remains bound to the live peer identity. The dedicated 1/2/4-CPU
gate passes locally; GitHub acceptance for the pushed tip remains required.

Physical privacy-list behavior, SMP interoperability, persistent protected keys
and hardware qualification remain outside this deterministic foundation.


## Stage 13.10S closure hardening — 2026-10-05

Minimum BLE encryption-key-size policy is enforced for both protected ATT/GATT
requests and outbound notifications/indications. Weak authenticated sessions
are rejected before either inbound data access or outbound value encoding;
sessions meeting the configured threshold remain bound to the live peer
identity. The exact 1/2/4-CPU Stage 13.10 gate, host tests and warning-denying
kernel Clippy passed locally. GitHub acceptance for the source tip is required
before this closure is considered complete.


## Stage 13.10T — outbound BLE key-size acceptance contract — 2026-10-05

The Stage 13.10 runtime harness now requires a dedicated marker for outbound
notification authorization by minimum encryption key size. A weak authenticated
session is rejected before notification encoding, while a session meeting the
16-byte policy is accepted. The local 1/2/4-CPU gate, host tests and
warning-denying kernel Clippy pass; GitHub acceptance for the pushed tip is
still required.

## Stage 1–5 WDJ2 capacity extension — 2026-10-14

The compatible WDJ2 batch journal now accepts the full 255-entry range
representable by its existing one-byte count field, with a host round-trip
self-test at the maximum. This removes the former sixteen-file implementation
ceiling for package/directory transactions without changing the on-disk
version or production block-device ABI. Recovery allocation remains explicitly
bounded; arbitrary-size/full-volume rollback, physical storage/DMA, torn-write
and reboot qualification remain open.

The public `persist_paths_atomic` boundary now returns `TooLarge` for a
256-entry request before mount or device access, rather than collapsing the
bounded-capacity refusal into a generic failure.



# Stage 27.1 bounded update A/B state (2026-10-20)

`kernel/src/update.rs` now provides the bounded two-slot activation state
machine and verifies bounded version-1 `WUPD` envelopes: it rejects generation
rollback and malformed or unsigned/tampered candidates, keeps the known-good
slot active until a health check succeeds, and rolls back after three failed
boot attempts. Host tests, host/freestanding warning-denying Clippy, and
dedicated QEMU acceptance passed on 1/2/4 CPUs. Evidence is in
`docs/audit-stages-consolidated.md`.

This does not yet implement privileged service ownership, durable A/B records,
power-loss recovery, or installation.
The fixed-size `WUAB` snapshot codec and checksum validation are now present,
but the service-owned dual-copy WovenFS commit and power-loss acceptance are
still open.

## Stage 27.2 durable update records (2026-10-20)

The update record boundary now commits exact-length dual-copy `WUAB` snapshots
through native WovenFS on a disposable ATA-backed scratch image. It remounts,
reads both copies, selects the newest valid sequence, and falls back to the
older copy after checksum corruption. The dedicated QEMU gate passed on 1/2/4
CPUs; evidence is in
`docs/audit-stages-consolidated.md`.

Long-lived service lifecycle, power-loss injection/recovery, installer and
boot-slot integration, and physical qualification remain open.

The record adapter now has a capability-gated service boundary: only a task
with `Capability::StorageIo` can obtain the private WovenFS writer token.
This does not yet constitute the long-lived userspace update daemon or its
IPC/syscall protocol.

## Stage 21.1 bounded WovenPackages envelope (2026-10-20)

The new `libwoven::package` parser accepts only bounded version-1 `WPKG`
envelopes with a structurally valid manifest, application, resource payload,
and fixed signature field. It rejects truncation, bad magic/version, overflow,
length mismatch, invalid names, and excessive dependencies before returning
borrowed payload slices. Host workspace tests, host `-D warnings` Clippy,
freestanding kernel `stage21-1-test` `-D warnings` Clippy, and the dedicated
QEMU gate passed on 1/2/4 CPUs. Evidence is recorded in
`docs/audit-stages-consolidated.md` and the corresponding
`audit-artifacts/stage21.1-*` directories.

This is a parser/format foundation only. Signature authentication, package
production, persistent WovenStore installation, updates, removal, rollback,
sandbox declaration enforcement, and a service/IPC boundary remain open.

## Stage 21.2 WovenPackages producer and authentication (2026-10-20)

`tools/wpkg-packager` now produces bounded WPKG v1 envelopes from ELF input,
manifest metadata, dependencies, capabilities, and optional resources. The
signature covers a domain-separated SHA-256 digest of the immutable envelope;
the kernel verifier checks it against the provisioned application trust root
and rejects payload tampering. Tool tests/Clippy, kernel warning-denying
Clippy, and the dedicated QEMU gate passed on 1/2/4 CPUs. Evidence is recorded
in `docs/audit-stages-consolidated.md`.

Persistent WovenStore installation, update/removal transactions, rollback,
sandbox declaration enforcement, service/IPC ownership, and physical USB
qualification remain open.

# Stage 15 native WovenFS VFS lifecycle (2026-10-20)

The native WovenFS adapter now owns the `/wfs` publication lifecycle. Mount
and unmount are serialized with journal mutations; remount invalidates cached
descendants and open descriptions before importing fresh contents, and generic
syscall access fails closed while unpublished. The stale-handle/remount
acceptance passed on scratch ATA images under QEMU with 1/2/4 CPUs. Host tests,
host Clippy, and freestanding `stage15-2-test` Clippy passed. This is a
bounded software increment: physical flash, power-loss, and replacement of
the FAT32 system volume remain open.

## Stage 25.14 - suspend admission boundary (2026-10-21)

The power ABI now recognizes action `2` as suspend for callers holding
`PowerControl`, and `libwoven::suspend` exposes the same operation. It fails
closed with a deterministic error because the current Stage 25.13 wake path
returns to a kernel rollback continuation, not the interrupted userspace
syscall frame. The rejection is audited and performs no preparation, FACS,
CPU, or PM1 mutation. The Stage 25.14 QEMU gate verifies that admission is
idle and the userspace resume target is explicitly unavailable.

This is not physical S3 qualification and does not claim that suspend is
available to production users. The next prerequisite is a task-owned resume
record and a verified return path before enabling the transition.

## Stage 31 controller-owned driver restart slice (2026-10-23)

NVMe, AHCI, xHCI, and HDA now stop controller DMA/rings before releasing PCI
ownership and rebind under a fresh generation owner. Failed quiescence restores
the controller object instead of dropping potentially referenced DMA pages.
xHCI stops its deferred enumerator and HID worker, removes the vector waiter
after task death, and halts the controller before release. The focused
`stage31-driver-restart-test` QEMU gate passed at 1/2/4 CPUs with 4 GiB guests;
evidence is in `audit-artifacts/stage13.12-restart-*`.

This slice does not claim generic automatic driver crash supervision, physical
hardware certification, or production readiness.

The production Stage 13.12 boot matrix additionally passed at 2 and 4 CPUs
(`audit-artifacts/stage13.12-production-2cpu-*` and
`audit-artifacts/stage13.12-production-4cpu-*`); the earlier 1-CPU evidence is
retained separately.

The seeded Stage 31 randomized matrix now schedules seven scenarios, including
the controller-owned driver restart gate. Seed `0x51A7E` passed at 1/2/4 CPUs;
evidence is retained under `audit-artifacts/stage31-randomized-51a7e-{1,2,4}cpu`.
This remains bounded child-gate orchestration, not arbitrary in-guest fault
injection or native hardware qualification.

The release validator now invokes the same controller-restart gate at 1/2/4
CPUs, keeping the release chain aligned with the accepted Stage 31 matrix.

## Current-head release validation and firmware preflight (2026-10-24)

The release validator had a real local-run defect: storage read/write-fault
children require QEMU and OVMF arguments, but an invocation without explicit
paths reached those gates with neither argument. The validator now resolves
QEMU from `PATH` and the conventional adjacent OVMF CODE location, or fails
closed before the acceptance chain with an actionable diagnostic. Explicit
CI-provided paths remain supported. Three focused preflight tests cover
explicit paths, missing OVMF, and missing QEMU.

The current-head release chain then completed with `release_exit=0`: all 158
recorded checks passed, including host warning-denying lint/tests, the 1/2/4
CPU debug and release boot/storage/network matrix, storage read/write-fault
containment, service and driver restart, signed system-app catalog and boot,
package transactions, AI provider/security/search/action gates, and app
qualification. Evidence is in `target/release-validation/results.json` and
the preserved serial/evidence directories under `target/` and
`audit-artifacts/`.

This is a current software/QEMU acceptance result. It does not convert the
read-only laptop preflight into native certification: no reboot, physical
disk write, firmware change, TPM provisioning, or Secure Boot qualification
was performed. Physical power-loss/torn-sector testing, production signing
key custody, live TPM/Secure Boot evidence, native device qualification, and
independent Claude review remain outside this run's evidence boundary.

## Software architecture mandate S01 native ABI (2026-10-24)

S01 is closed for bounded software scope at commit `f9091b6a`. `libwoven::abi`
and kernel syscall 243 negotiate fixed native ABI descriptors and reject
unsupported major versions or malformed reserved bytes before IPC/syscall
continuation. The independently built Ring-3 fixture passed at 1/2/4 CPUs;
the inherited current-head release chain passed 158/158. This does not claim
POSIX/foreign-OS compatibility, physical hardware support, production trust,
or final release readiness. S02 is the next sequential prerequisite.

## Software architecture mandate S02 runtime contract (2026-10-24)

S02 implementation has started with a bounded freestanding C ABI contract in
`tools/runtime-fixtures/c-abi.c` and the conservative checker
`scripts/check-runtime-contract.py`. Source-shape validation passed, while the
host reported no target-qualified ELF C compiler; therefore C ELF execution,
loader qualification, libc coverage, destructors, and shared-library
compatibility remain open. The checkpoint is `IMPLEMENTING`, not accepted.

## Production desktop fallback cleanup (2026-10-25)

Interactive QEMU reproduced and then removed the obsolete diagnostic fallback
desktop. Normal `system-apps` boots now start WovenShell directly; the kernel
fallback no longer exposes an F1 desktop switch or placeholder controls. The
WovenShell production default is 3D, with its 2D painters retained as the
texture/composition source for the depth-tested renderer. Plain and
`system-apps` kernel warning-denying Clippy passed. A 1-vCPU interactive
`system-apps` boot reached the real desktop and its F1 launcher.

## Branded boot handoff and desktop visual pass (2026-10-25)

The production `system-apps` console now shows a quiet WOVENHAT / Starting
desktop splash while boot diagnostics remain on serial. The desktop wallpaper
and taskbar palette were softened to a restrained blue/teal desktop with a
subtle weave, while the production 3D renderer remains the default. Interactive
QEMU evidence reached the desktop and confirmed F1 opens the WovenShell
launcher; screenshots are `target/updated-desktop.png` and
`target/f1-launcher.png` (generated, ignored artifacts). This is a
visual/QEMU pass, not native hardware or production-release qualification.

## Branded circular boot loader (2026-10-25)

Refined the production splash with a woven-hat emblem, decorated WOVENHAT
wordmark, and an integer-rendered segmented circular spinner. The spinner
advances at each boot-loader milestone while verbose diagnostics remain on
serial. Plain and `system-apps` warning-denying kernel Clippy passed; this is
still a QEMU/UI qualification change, not native graphics certification.

## Repository merge and cleanup audit (2026-10-25)

`master` was clean and matched `origin/master` before cleanup. Local branches
that were already merged into `master` were pruned. Historical or WIP branches
with unmerged code were retained rather than blindly merging stale changes.
Exact duplicate hashes found in per-application `.cargo/config.toml`, `build.rs`,
and linker templates are intentional shared build inputs; audit and archived
documents remain because they preserve acceptance evidence. Ignored build,
QEMU, firmware, and audit artifacts were not deleted while interactive QEMU
was active.

## Login and taskbar readability pass (2026-10-25)

The hardware-demo QEMU path was verified with the real sign-in screen and
`ada` PIN authentication. The production system-apps path remains account-file
driven: it cannot show a usable login until `/etc/wovenhat/accounts` is
provisioned. The taskbar now keeps the desktop 3D background but renders its
foreground bar flat and crisp, restoring readable Woven, account, workspace,
and clock labels. Evidence was captured in
`target/hardware-demo-login-final.png` and
`target/hardware-demo-desktop-final2.png`.

## Consolidated audit and hash records (2026-10-25)

The 341 standalone stage-audit records were merged into
`docs/audit-stages-consolidated.md` with source paths retained. The 28 legacy
SHA256 manifests were merged into
`docs/sha256-stage-manifests-consolidated.txt`. All 74 stage run scripts remain
separate and executable. A fresh repository-wide SHA-256 inventory is recorded
in `docs/repository-sha256-2026-10-25.txt`.

## Stage-run and README cleanup (2026-10-25)

Removed obsolete stage-only README files and legacy Stage 6–9, Stage 10.8–10.9,
and Stage 13 runner wrappers. The current Stage 10.1–10.7 acceptance chain,
`RUN-STAGE10.7.ps1`, `run-release.ps1`, the root README, and app/tool/vendor
READMEs remain because they are active instructions or package documentation.

## Stage 32 production compilation repair - 2026-10-27

Default production instrumentation compiles with warnings denied after API
visibility and optional-state/self-test cfg repairs. Kernel Clippy passes for
default, tracing, profiler, and Stage 32.1-32.4; host all-target Clippy and
tests pass. All four Stage 32 QEMU gates pass on 1/2/4 CPUs, exit 33.
Evidence: target/stage32-{1-diagnostics,2-panic-dump,3-tracing,4-profiler}-*cpu;
build/host logs and archived prior evidence: target/stage32-production-fix.
This is a focused build/regression repair, not full Stage 32 acceptance.
Release hardware-demo UEFI build and hardware-demo target Clippy also pass. Visible 2-CPU QEMU reaches WovenShell desktop-ready and catalog entries=11; interactive boot logs are under C:\Users\banda\WovenHat-OS\target\desktop-interactive-20261027-145808-379. Full historical Windows acceptance was not rerun.

## Desktop experience repair - 2026-10-27 (focused runtime path validated)

The improved interactive desktop was verified with WHPX CPU acceleration,
5 GiB configured RAM and a USB tablet. Demo startup selects flat composition
and reduced motion. Shell work/presentation is bounded, tablet coordinates
are routed, cursor/close damage is kept separate, desktop shortcuts survive
taskbar painting, dead-window capture is cleared, and Calculator starts with
an empty field and avoids redundant hover uploads. The shell release build
uses speed optimization; credential work factor and authority are unchanged.

Validation:
- WHPX desktop keyboard/mouse/framebuffer/performance qualification passes on
  1/2/4 CPUs with unchanged assertions and timing limits.
- TCG Stage 20.1, 20.5, 20.7, 20.10, 20.11, 20.12, 20.15, 20.17 and 20.18
  pass on 1/2/4 CPUs. Stage 20.15's initial build-environment failure was
  preserved and rerun with the required project-local Cargo settings.
- Stage 20.2, 20.3, 20.4, 20.6, 20.8, 20.9 and 20.13 fail their pixel gates
  on 1/2/4 CPUs. The pre-change shell from f7273ba9 independently reproduces
  20.2 and 20.6 failures at the same assertions on 1 CPU; the other failed
  gates remain open and are not represented as accepted.
- App/host warning-denying Clippy, host tests, kernel default/system-apps/
  hardware-demo/stage20-15-test Clippy and seven qualification-helper tests pass.
- Real interactive USB input: typed 12-3 and clicked Evaluate, captured result
  9. Sign-in key sequence measured 766 ms including injection delays; absolute
  tablet click delivery measured 186 ms including the QMP button delay.

Evidence: audit-artifacts/desktop-ux-final-20261027-152146 (full legacy sweep),
audit-artifacts/desktop-ux-final-validation-20261027-155614 (WHPX matrix,
baseline comparisons, session rerun), audit-artifacts/desktop-ux-before-legacy,
and audit-artifacts/desktop-ux-static-20261027. Earlier TCG size-optimized
1-CPU sign-in timing failures remain preserved; no thresholds were increased.
Interactive logs/screenshots: target/desktop-interactive-20261027-154331-443.
At final inspection QEMU PID 17040 had exited; RUN-DESKTOP.ps1 relaunches it.
RUN-DESKTOP.ps1 defaults to WHPX, accepts -Accelerator tcg and boots a private
read-only image snapshot. This focused repair is not full Stage 20/32 or full
historical Windows acceptance; the seven failed graphics gates need follow-up.
