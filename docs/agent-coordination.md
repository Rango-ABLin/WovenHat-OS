# Agent coordination (Claude Code and Codex)

**Claude Code: diagnostic shell no longer overwrites a slow-starting
WovenShell session; waits and reports instead (2026-10-20):** the previous
screen-visibility fix below was flashed and re-tested, and nothing changed:
still the plain diagnostic-shell banner with no `SESSION:` line, F1 still
only reaching the kernel's own placeholder window. Confirmed (byte-search)
the new `SESSION:` strings WERE in the exact image flashed, which rules out
a stale build and instead proves `session::start()` is only reporting that
the process was *spawned*, not whether it ever calls
`compositor::activate_display()` -- the kernel's diagnostic-shell fallback
ran and cleared the screen (`kernel/src/main.rs`) before WovenShell got that
far, meaning WovenShell is failing or hanging early on this real machine,
well before sign-in, with nothing in serial or (still-empty `WOVENLOG`)
reaching the stick either. Fixed: `task::process_alive(id: u64)` (new,
`kernel/src/task.rs`, cfg-gated to `system-apps`) plus a bounded ~5s wait in
`kernel/src/main.rs` after `session::start` for `session::active()` to flip
true or the process to die trying, giving three distinguishable screen
outcomes (`started as pid N` / `pid N exited before the desktop appeared` /
`pid N has not shown the desktop yet`) instead of one silent fallback. The
diagnostic shell's placeholder `ACTIVATE`/`SECOND` window and banner are now
shown only when the session did not become ready. `cargo clippy` (kernel
default features, `--features hardware-demo` with images wired up, host
`--all-targets`), `python scripts/lint-all-features.py` (one unrelated
pre-existing `physical-wifi` FAIL, missing firmware env var, untouched by
this change), and `scripts/test-shell-qemu.py --hardware-demo` on 1/2/4 CPUs
all passed, with the serial log confirming `session_ready` still flips true
quickly under QEMU (no behavior change there). A fresh image (SHA-256
`6d887f0f0b94ebf213ad3f02d4b3a50378445ba24f28df677711504e91bcdd59`) still
needs writing to the user's confirmed stick (serial `2007072809430484`) via
`scripts/write-live-usb.ps1`, needs an elevated shell. See
[docs/stage-status.md](stage-status.md#diagnostic-shell-no-longer-overwrites-a-slow-starting-wovenshell-session-waits-and-reports-instead-2026-10-20).

**Claude Code: WovenShell session status now shown on screen (2026-10-20):**
with the MAX_CPUS/IOAPIC fix below flashed, the user's real hardware no
longer stalls, but the real WovenShell desktop/sign-in never appeared --
only the kernel's own `WOVENHAT DIAGNOSTIC SHELL` fallback and, on F1, its
unrelated placeholder demo window (`ACTIVATE`/`SECOND` buttons). The stick's
`WOVENLOG` had no `LOGS\BOOTnnnn.TXT` at all for that boot either, so there
was no log evidence and (no serial cable) no other evidence of why. Root
cause of the *visibility* gap: `session::start()`'s `Ok`/`Err` result
(`kernel/src/main.rs:1664`) was only ever logged to serial; its on-screen
`BOOT LOADER [6/7]/[7/7]` lines are wiped a few lines later by the
diagnostic-shell fallback's unconditional `console.clear()`
(`kernel/src/main.rs:1716`), so success and failure looked identical on a
real screen. Fixed: the outcome is now kept in a `session_status` string and
reprinted, persistently, right after the `WOVENHAT DIAGNOSTIC SHELL` banner.
This does not explain *why* WovenShell or livelog might fail on this
hardware -- it only makes the first fact (did the session even start)
visible for the user's next boot. `cargo clippy` (kernel `--features
hardware-demo` with packaged system-app images wired up, and host
`--all-targets`) and `scripts/test-shell-qemu.py --hardware-demo` on 1/2/4
CPUs all passed (`Hardware demo: ... PASS`, serial markers unchanged -- the
new line is screen-only). A fresh image (SHA-256
`59d7d5f223bc7cbc2171ff5d0cb92fab5768757f65ab9d511840308f239734ff`) still
needs writing to the user's confirmed stick (serial `2007072809430484`) via
`scripts/write-live-usb.ps1`, which needs an elevated shell. See
[docs/stage-status.md](stage-status.md#wovenshell-session-status-now-shown-on-the-diagnostic-shell-screen-2026-10-20).

**Claude Code: boot-stall real root cause found after the PIT/AP-timeout fix
below still didn't resolve it (2026-10-20):** the user re-tested on real
hardware with that fix flashed and got the identical stall. Pulled the
stick's `WOVENLOG` -- empty, no `LOGS\BOOTnnnn.TXT` at all, confirming the
failure is still before `livelog::start_deferred()` runs, i.e. inside or
immediately around `smp::start()`. Found the real cause: `smp::start()` had
`assert!(!topology.truncated && topology.processor_count <= MAX_CPUS, ...)`,
and `MAX_CPUS` is a compile-time 4. Any real machine with more than 4
logical processors -- most modern hardware, since even a plain hyperthreaded
quad-core reports 8 -- hits this fatal, serial-only (so invisible on a real
screen) panic immediately, before the PIT-timing/AP-SIPI code the earlier
fix touched is ever reached. None of the QEMU gates catch it because they
only run at exactly 1/2/4 CPUs. Also found and relaxed a second,
narrower one: `assert_eq!(topology.io_apics, 1, ...)`, which some real
boards fail despite `hal::acpi` already only ever using the first IOAPIC
regardless of how many exist. Fixed in `kernel/src/smp.rs`: boot with the
first `MAX_CPUS` processors (BSP + whatever APs fit) instead of refusing to
boot, and accept `io_apics >= 1`. Also added `kernel::terminal::panic_screen`
(called from `panic::kernel_panic`) so panics show on the framebuffer too,
not only serial, without ever taking `TERMINAL`'s lock -- this whole bug
class was undiagnosable blind on hardware with no serial cable, and now
isn't. `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none`, host
`cargo clippy --all-targets -- -D warnings`, and `scripts/test-shell-qemu.py
--hardware-demo` on 1/2/4 CPUs all passed with `online=4 expected=4` and no
new warning lines -- zero behavior change under QEMU, since it's always
configured at exactly 4 CPUs and 1 IOAPIC. A fresh image (SHA-256
`f8bd474e3658d8a81b38d6e02c8ddff2d9270fb541d377638f360574d4e595ae`) is
written to the user's confirmed stick (serial `2007072809430484`), read back
byte-for-byte matching. See
[docs/stage-status.md](stage-status.md#boot-stall-real-root-cause-max_cpus4-rejected-real-topologies-ioapic-count-relaxed-panics-now-shown-on-screen-2026-10-20)
for the full writeup. Not yet re-tested on the user's hardware -- if it
fails again, the screen itself should now say why, or the stick's
`WOVENLOG` will have a boot log this time if it got far enough to matter.

**Claude Code: boot-stall still reported after the flash below -- found and
fixed the real cause (2026-10-20):** the user reported the hang at "BOOT:
starting CPU and interrupt setup" persisted on their physical hardware after
the Codex boot-stall image two entries below was already flashed. Root cause:
`smp::delay_10ms`'s port-0x80 fallback bounded one call to ~5 s, but re-ran
the full "is the PIT there" probe on *every* call rather than remembering the
answer -- AP bring-up calls it roughly a dozen times, so a dead PIT gate
turned into tens of seconds of a static screen, not a true hang but
indistinguishable from one. Also found: an AP that never answers SIPI hits
`assert!(..., "AP startup timeout")`, a fatal panic that this kernel only
ever writes to serial (never the on-screen console), and which fires before
`livelog::start_deferred()` has even queued -- so on real hardware with no
serial cable attached, that failure mode is a genuinely silent, undiagnosable
freeze with nothing on screen and nothing reaching the stick's `WOVENLOG`.
Fixed both in `kernel/src/smp.rs`: a cached `PIT_UNAVAILABLE` flag so only the
first probe pays the ~5 s cost, and AP timeout now degrades to "continue
without this CPU" (logged to serial) instead of halting the whole boot.
`cargo clippy -p wovenhat-kernel --target x86_64-unknown-none` (default
features), host `cargo clippy --all-targets -- -D warnings`, and
`scripts/test-shell-qemu.py --hardware-demo` on 1/2/4 CPUs all passed, with
`online=4 expected=4` and no AP-timeout/PIT-fallback lines on the 4-CPU
serial log -- confirming zero behavior change under QEMU (its PIT always
answers), so this only changes anything on hardware where the PIT or an AP
actually misbehaves. See
[docs/stage-status.md](stage-status.md#boot-stall-follow-up-cached-pit-unavailable-result-non-fatal-ap-timeout-2026-10-20)
for the full writeup. Not yet re-validated on the reporting user's physical
machine -- that's the next step, and it would help a lot to get the
`WOVENLOG` `LOGS/BOOTnnnn.TXT` from a prior failed boot off that stick if
still available, to confirm against ground truth rather than static analysis
alone.

**Codex boot-stall image refreshed (2026-10-20):** The guarded writer flashed
the rebuilt image containing the bounded SMP/PIT fallback and boot sub-markers
to the user-confirmed USB `USB2.0 Flash Disk USB Device`, serial
`2007072809430484`, as PhysicalDrive1. The 13,697,024 image bytes read back
with SHA-256
`ac9f8058a0fdc8f64ea6f3547f1f4dbce4e963e75b1a457b22d0591f9151bfc3`; the
remaining 488 MiB FAT32 `WOVENLOG` partition was recreated. Evidence:
`audit-artifacts/hardware-demo-usb-write-20261020-boot-stall-fix.txt`.

**Codex boot stall hardening (2026-10-20):** Normal boots now expose
post-graphics sub-progress for CPU/AP startup, PS/2 probing, interrupt enable,
and service-worker/async-I/O startup. SMP's PIT channel-2 settle delay now has
a bounded port-0x80 fallback when firmware leaves the speaker gate unavailable,
so a physical machine cannot spend the delay timeout on every SIPI. Target
kernel check, host all-targets warning-denying Clippy, and hardware-demo QEMU
acceptance passed on 1/2/4 CPUs. The direct feature Clippy command remains
dependent on generated packaged-app environment variables and was run through
the acceptance launcher instead.

**Codex hardware-demo USB refresh (2026-10-20):** The updated 13,697,024-byte
hardware-demo image, including the visible boot-loader progress markers, was
written to the user-confirmed USB `USB2.0 Flash Disk USB Device`, serial
`2007072809430484`. The guarded writer matched the raw device identity, wrote
the image to PhysicalDrive1, read it back with matching SHA-256
`da97e3399aa31f33dec19979d280c49bf224b792ac053e5f4a771f4e9a88a674`, and
recreated the remaining 488 MiB FAT32 `WOVENLOG` partition. Evidence:
`audit-artifacts/hardware-demo-usb-write-20261020-loader.txt`.

**Codex boot-loader feedback (2026-10-20):** Normal boots now show visible
`BOOT LOADER [n/7]` framebuffer progress after heap initialization, through
graphics setup, workers, storage, system apps, devices, WovenShell startup,
and desktop readiness. This addresses the apparent freeze after
`KERNEL HEAP: CAPACITY/FRAGMENTATION OK`; the boot path and acceptance
behavior are unchanged. Host `cargo clippy --all-targets -- -D warnings` and
the hardware-demo QEMU shell gate passed on 1 CPU.

**Claude Code: found and fixed the heap-crash known issue (2026-10-19
entry below) -- and found a separate, pre-existing [S19.9] regression while
verifying it (2026-10-20):**

Root cause of the `KERNEL ALLOC ERROR: layout size=4096000 align=4` crash:
`compositor::rescale_to` (called on every display-scale change, e.g. Quick
Settings' scale toggle) used the panicking `Surface::new` instead of the
fallible `Surface::try_new` -- at scale 1 on a 1280x800 framebuffer that is
exactly a 4,096,000-byte allocation, and the function's own doc comment
already flagged "a surface is up to 9 MB" as a real risk. Same bug, same
fix, in four `user_graphics.rs` snapshot/copy helpers (`snapshot`,
`snapshot_region`, `snapshot_shared`, `take_damage_shared`) that run on
every blit -- `user_graphics.rs:247`'s `create` already used the correct
`try_new` pattern, so these four were just inconsistent with their own
file. Fixed in commit `6bf6e55c` (landed under your authorship via the
shared tree, but authored by me this session -- see the identical wording).

While verifying with `scripts/test-hello-gui-compositor-qemu.py`, Stage
19.14 (scaling) and 19.11 (window stacking) pass clean on 1/2/4 CPUs, but
**Stage 19.9 (damage compositing) fails at HEAD, before my change too** --
confirmed by `git stash`-ing just my `user_graphics.rs` edit and
re-running: `[S19.9] damage rules FAIL` persists identically. So this is a
pre-existing regression, not something I introduced, and not something I
investigated further (out of scope for the heap-crash task). It's
`user_graphics::damage_self_test()` specifically, which exercises
`take_damage_shared`'s 9x9 declared-region round trip; given the volume of
recent `Graphics: ...` commits touching compositing/damage, this is
presumably a recent regression somewhere in that area and should be easy
for whoever's been in that code to spot. Repro: `python
scripts/test-hello-gui-compositor-qemu.py --stage 19.9 --cpus 1`.

**Codex persistent session account import accepted (2026-10-20):** The
hardware-demo boot now imports a bounded, valid salted-hash account file from
`/mnt/etc/wovenhat/accounts` into the read-only session path before WovenShell
starts, fails closed for malformed persistent credentials, and preserves the
disposable fallback only when the persistent file is absent. Host tests,
warning-denying kernel Clippy, and Stage 20.12 sign-in passed on QEMU 1/2/4
CPUs. Account creation, rotation, and a stronger password KDF remain open.

**Codex packaged app launch regression revalidated (2026-10-20):** The
production WovenCalculator package launched through the logged-in WovenShell,
changed the presented frame, and activated its bounded control on QEMU 1/2/4
CPUs after the SMP repaint fix. The other nine client gates remain accepted;
persistent release account provisioning is next.

**Codex session/SMP sign-in repaint fix accepted (2026-10-20):** WovenShell
now repaints only the changed account/PIN region for lock-screen keys instead
of its full wallpaper. Stage 20.12 sign-in and Stage 20.15 session gates pass
on QEMU 1/2/4 CPUs. The hardware-demo login accounts are still volatile;
persistent release account installation is the next slice.

**Codex graphics-track WovenGraphics compositor visual acceptance revalidated
(2026-10-19):** Stage 19.2 passed on QEMU 1/2/4 CPUs. The compositor accepted
the WovenGraphics visual, 2D, and software 3D frames, and retained-window,
clean-exit, and owner-release checks also passed. This closes the software
renderer/compositor visual slice; accelerated GPU scanout remains a separate
device-dependent boundary.

**Codex graphics-track standard virtio-gpu 2D scanout revalidated
(2026-10-19):** Fresh QEMU 1/2/4-CPU runs captured both 640x400 heads and
verified every pixel against the gradient/checkerboard contract. This is the
authoritative visible software/2D presentation path for the release. It is
separate from the GL/VirGL scanout probe, which remains rejected by QEMU with
`Device(4611)` (`0x1203`).

**Codex graphics-track VirGL 2D-on-GL scanout boundary (2026-10-19):** The
stage 20.14 lifecycle gate passes on 1/2/4 CPUs. A capability-gated follow-on
probe attempts a conventional 2D resource, known-gradient fill, and present
on the GL-capable QEMU device; QEMU rejects `SET_SCANOUT` with
`Device(4611)` (`0x1203`). This is boundary evidence, not visible GPU-frame
acceptance. Software/2D presentation remains the accepted fallback; physical
GPU and USB HID qualification remain open.

**Codex graphics-track functional-control completion and VirGL boundary
revalidation (2026-10-19):** All ten packaged desktop clients now have a
bounded functional pointer control accepted on QEMU 1/2/4-CPU runs, with
serial/frame evidence recorded in the per-client audit documents. Fresh
VirGL 1/2/4-CPU runs passed capability/context/resource/command lifecycle only;
QEMU still rejects the tested visible 3D/blob scanout path. Software/2D
presentation is the accepted fallback, while physical GPU and USB HID remain
hardware-dependent boundaries.

**Codex graphics-track WovenUpdate/WovenStore pointer controls accepted
(2026-10-19):** WovenUpdate's bounded local Refresh control at x=250..329,
y=145..164 and WovenStore's control at x=250..329, y=170..189 now re-read
their bounded read-only views on forwarded `WPBT` release and repaint a
visible counter. Both production QEMU gates passed on 1/2/4 CPUs with
`--exercise-client-click`; evidence is under the corresponding update/store
desktop audit directories. All ten packaged clients now have a functional
pointer control. USB HID and hardware GPU presentation remain separate.

**Codex graphics-track WovenImages pointer control acceptance (2026-10-19):**
WovenImages now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=250..329, y=170..189 re-reads the read-only bounded `/mnt`
image summary, increments a visible counter, and repaints. The production
QEMU gate passed on 1/2/4 CPUs with `--app images --exercise-client-click`;
evidence is under `audit-artifacts/images-desktop-{1,2,4}cpu-*`. Image
decoding/mutation, USB HID, and hardware GPU presentation remain separate.

**Codex graphics-track WovenEditor pointer control acceptance (2026-10-19):**
WovenEditor now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=250..329, y=170..189 re-reads the read-only bounded `/mnt`
workspace summary, increments a visible counter, and repaints. The production
QEMU gate passed on 1/2/4 CPUs with `--app editor --exercise-client-click`;
evidence is under `audit-artifacts/editor-desktop-{1,2,4}cpu-*`. The CLI write
path is unchanged; USB HID and hardware GPU presentation remain separate.

**Codex graphics-track WovenSecurity pointer control acceptance (2026-10-19):**
WovenSecurity now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=250..329, y=145..164 re-reads the read-only bounded audit
summary, increments a visible counter, and repaints. The production QEMU gate
passed on 1/2/4 CPUs with `--app security --exercise-client-click`; evidence
is under `audit-artifacts/security-desktop-{1,2,4}cpu-*`. USB HID and hardware
GPU presentation remain separate.

**Codex graphics-track WovenMonitor pointer control acceptance (2026-10-19):**
WovenMonitor now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=250..329, y=170..189 re-reads the read-only
clock/display/storage snapshot, increments a visible counter, and repaints.
The production QEMU gate passed on 1/2/4 CPUs with `--app monitor
--exercise-client-click`; evidence is under
`audit-artifacts/monitor-desktop-{1,2,4}cpu-*`. USB HID and hardware GPU
presentation remain separate.

**Codex graphics-track WovenTerminal pointer control acceptance (2026-10-19):**
WovenTerminal now has a bounded local `Clear` button. A forwarded `WPBT`
release inside x=250..329, y=170..189 clears only the preview input and
updates its status. The production QEMU gate passed on 1/2/4 CPUs with
`--app terminal --exercise-client-click`; evidence is under
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`. The CLI path is unchanged;
USB HID and hardware GPU presentation remain separate.

**Codex graphics-track WovenFiles pointer control acceptance (2026-10-19):**
WovenFiles now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=220..299, y=206..225 re-reads the bounded read-only `/mnt`
listing, increments a visible counter, and repaints. The production QEMU gate
passed on 1/2/4 CPUs with `--app files --exercise-client-click`; evidence is
under `audit-artifacts/files-desktop-{1,2,4}cpu-*`. No file mutation is
introduced; USB HID and hardware GPU presentation remain separate.

**Codex graphics-track Settings pointer control acceptance (2026-10-19):**
WovenSettings now has a bounded local `Refresh` button. A forwarded `WPBT`
release inside x=250..339, y=190..213 re-reads the live read-only settings
values, increments a visible refresh counter, and repaints. The production
QEMU gate passed on 1/2/4 CPUs with `--app settings
--exercise-client-click`; evidence is under
`audit-artifacts/settings-desktop-{1,2,4}cpu-*`. USB HID and hardware GPU
presentation remain separate boundaries.

**Codex graphics-track Calculator pointer control acceptance (2026-10-19):**
WovenCalculator now has a bounded local `Evaluate` button. A forwarded
`WPBT` release inside x=150..209, y=110..127 evaluates the current expression
and repaints the result. The production QEMU gate passed on 1/2/4 CPUs with
`--app calculator --exercise-client-click`; evidence is under
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`. This accepts one functional
client control; other clients, USB HID, and hardware GPU presentation remain
separate boundaries.

**Codex graphics-track remaining client pointer acceptance (2026-10-19):**
Settings, Monitor, Security, Editor, Images, Update, and Store now consume
bounded `WPMV`/`WPBT` messages and paint distinct markers. The production QEMU
gate passed for every client on 1/2/4 CPUs; evidence is under each app's
`audit-artifacts/{settings,monitor,security,editor,images,update,store}-desktop-*`
directory. All ten packaged clients now accept the pointer protocol; their
functional controls remain bounded/read-only as previously documented. USB HID
and hardware GPU presentation remain separate boundaries.

**Codex graphics-track WovenFiles client pointer acceptance (2026-10-19):**
WovenFiles now consumes bounded `WPMV`/`WPBT` messages and paints a distinct
marker. The production QEMU gate passed on 1/2/4 CPUs with
`--app files --exercise-client-pointer`; evidence is under
`audit-artifacts/files-desktop-{1,2,4}cpu-*`. Calculator, Terminal, and Files
are now accepted pointer clients. The Files UI remains read-only; USB HID and
hardware GPU presentation remain separate.

**Codex graphics-track Terminal client pointer acceptance (2026-10-19):**
WovenTerminal now consumes the bounded `WPMV`/`WPBT` protocol and paints a
distinct marker. `scripts/test-files-desktop-qemu.py --app terminal
--exercise-client-pointer` passed on 1/2/4 CPUs with evidence under
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`. Calculator and Terminal are
accepted clients; the remaining packaged clients, USB HID, and hardware GPU
presentation remain separate.

**Codex graphics-track client pointer forwarding (2026-10-19):**
WovenShell now forwards bounded client-local `WPMV` motion and `WPBT` primary
button edges after its own chrome/title-bar hit testing. WovenCalculator draws
a visible pointer marker and reports delivery. The production QEMU gate passed
on 1/2/4 CPUs with `scripts/test-files-desktop-qemu.py --app calculator
--exercise-client-pointer`; evidence is under
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`. This accepts the PS/2 to
shell to Calculator path only; other clients, USB HID, and hardware GPU
presentation remain separate boundaries.

**Codex graphics-track Stage 20.4 window interaction acceptance (2026-10-19):**
The existing Stage 20.4 QEMU gate passed on 1/2/4 CPUs. It drives real i8042
input through WovenShell: a title-bar drag moves Alpha and repaints both the
old and new regions; the Wide close control exits its owner; focus and key
routing then let Alpha exit; the shell releases the display. Evidence is in
`tar\target\woven-shell-windows-{1,2,4}cpu`. This accepts the production
window-manager move/close path; client pointer forwarding is covered by the
newer Calculator slice above. USB HID, hardware GPU presentation, and the
still-experimental direct QMP drag of the WovenFiles client remain separate.

**Codex graphics-track desktop pointer visual acceptance (2026-10-19):**
`scripts/test-files-desktop-qemu.py --app files --exercise-pointer` now
injects bounded relative motion and a left click through QEMU's input path and
checks before/after screendumps. The gate passed on 1/2/4 CPUs with evidence
in `audit-artifacts/files-desktop-{1,2,4}cpu-*`. This accepts the PS/2-backed
software cursor overlay; USB HID pointer qualification and hardware GPU
presentation remain separate boundaries.

**Codex graphics-track WovenFiles resize (2026-10-19):**
WovenFiles now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/files-desktop-{1,2,4}cpu-*`. The `/mnt` listing
remains bounded and read-only; file-operation input, pointer widgets, and
hardware GPU presentation remain out of scope.

**Codex graphics-track WovenStore resize (2026-10-19):**
WovenStore now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/store-desktop-{1,2,4}cpu-*`. The client remains a
read-only catalog with no package mutation; pointer widgets and hardware GPU
presentation remain out of scope.

**Codex graphics-track WovenUpdate resize (2026-10-19):**
WovenUpdate now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/update-desktop-{1,2,4}cpu-*`. The client remains
preflight-only and non-mutating; pointer widgets and hardware GPU presentation
remain out of scope.

**Codex graphics-track WovenImages resize (2026-10-19):**
WovenImages now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/images-desktop-{1,2,4}cpu-*`. Decoding and editing
remain outside this preview; pointer widgets and hardware GPU presentation
remain out of scope.

**Codex graphics-track WovenEditor resize (2026-10-19):**
WovenEditor now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/editor-desktop-{1,2,4}cpu-*`. The client remains
read-only; editing controls, pointer widgets, and hardware GPU presentation
remain out of scope.

**Codex graphics-track WovenSecurity resize (2026-10-19):**
WovenSecurity now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/security-desktop-{1,2,4}cpu-*`. The app remains
read-only with audit display capped at 16 events; security controls, pointer
widgets, and hardware GPU presentation remain out of scope.

**Codex graphics-track WovenMonitor resize (2026-10-19):**
WovenMonitor now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/monitor-desktop-{1,2,4}cpu-*`. The app remains
read-only; pointer controls and hardware GPU presentation remain out of
scope.

**Codex graphics-track WovenSettings resize (2026-10-19):**
WovenSettings now negotiates bounded `WSIZ/WRSZ` replacement surfaces with a
480×320 loader-safe Canvas cap. Its QMP resize gate passed on 1/2/4 CPUs with
evidence in `audit-artifacts/settings-desktop-{1,2,4}cpu-*`. The app remains
read-only; pointer controls and hardware GPU presentation remain out of scope.

**Codex graphics-track bounded WSIZ/WRSZ resize (2026-10-19):**
WovenShell now allows a five-second replacement grace period. Calculator and
Terminal negotiate replacement shared surfaces, clamp requests to 480×320 to
remain within the current Ring-3 image/stack geometry, and passed the QMP
resize gate on 1/2/4 CPUs. Evidence is under
`audit-artifacts/{calculator,terminal}-desktop-*`. Other clients still need
resize paths; pointer widgets and hardware GPU presentation remain out of
scope.

**Codex graphics-track Canvas damage and Calculator input (2026-10-19):**
Userspace graphics mutations now register client damage in
`kernel/src/user_graphics.rs`, so shared WovenShell clients receive repaint
wakeups. WovenCalculator accepts bounded ASCII/WKEY expression input and its
QMP interaction gate passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`. Pointer widgets, WSIZ, and
hardware GPU presentation remain out of scope.

**Codex graphics-track bounded key protocol (2026-10-19):**
WovenShell now forwards Enter and Backspace as `WKEY` plus bounded key codes;
WovenTerminal redraws its shared Canvas after ASCII edits, Backspace, and
Enter. The extended QMP gate passed on 1/2/4 CPUs with before/after input
evidence under `audit-artifacts/terminal-desktop-{1,2,4}cpu-*`. Desktop
command execution, pointer widgets, WSIZ, and hardware GPU presentation are
still out of scope; `--interactive` remains the full CLI path.

**Codex graphics-track WovenCalculator acceptance (2026-10-19):**
The existing bounded Canvas/WOPN calculator client is now covered by the
shared QMP desktop gate on 1/2/4 CPUs, with evidence in
`audit-artifacts/calculator-desktop-{1,2,4}cpu-*`. Its preview uses fixed
`2 + 2` data; CLI expression evaluation remains authoritative. Desktop input,
WSIZ, and hardware GPU presentation remain out of scope.

**Codex graphics-track WovenTerminal preview (2026-10-19):**
`apps/woven-terminal/src/gui.rs` adds a bounded read-only Canvas/WOPN preview,
and the production catalog now launches it with `--gui %p`. The shared QMP
desktop gate passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/terminal-desktop-{1,2,4}cpu-*`. `--interactive` remains the
CLI input path; desktop key forwarding, command execution, WSIZ, and hardware
GPU presentation remain out of scope.

**Codex graphics-track WovenUpdate/WovenStore clients (2026-10-19):**
Update and Store now have bounded read-only Canvas/WOPN clients, with
production catalog `--gui %p` entries. Their shared QMP launcher gates passed
on 1/2/4 CPUs; evidence is in
`audit-artifacts/{update,store}-desktop-{1,2,4}cpu-*`. WovenShell's catalog
capacity was raised from 8 to 10 so all signed production apps remain
launchable. Update is preflight-only, Store is non-mutating, and controls,
pointer widgets, and hardware GPU presentation remain out of scope; bounded
WSIZ/WRSZ for both clients is covered by their newer resize slices.

**Codex graphics-track WovenImages client (2026-10-19):**
`apps/woven-images/src/gui.rs` adds a bounded read-only image-workspace
preview and the catalog now passes `--gui %p`. The shared QMP desktop gate
launches it from F1 and passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/images-desktop-{1,2,4}cpu-*`. Decoding, selection, and editing
remain out of scope; pointer widgets and hardware GPU presentation remain out
of scope; bounded WSIZ/WRSZ is covered by the newer Images resize slice.

**Codex graphics-track WovenEditor client (2026-10-19):**
`apps/woven-editor/src/gui.rs` adds a read-only bounded `/mnt` workspace
preview and the catalog now passes `--gui %p`. The shared QMP desktop gate
launches it from F1 and passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/editor-desktop-{1,2,4}cpu-*`. CLI editing remains separate;
desktop text input, pointer widgets, and hardware GPU presentation remain out
of scope; bounded WSIZ/WRSZ is covered by the newer Editor resize slice.

**Codex graphics-track WovenSecurity client (2026-10-19):**
`apps/woven-security/src/gui.rs` adds a read-only bounded identity/audit
summary window. The catalog now passes `--gui %p`; the shared QMP desktop gate
launches it from F1 and passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/security-desktop-{1,2,4}cpu-*`. Audit display is capped at 16
events; controls, pointer widgets, and hardware GPU presentation remain out of
scope; bounded WSIZ/WRSZ is covered by the newer Security resize slice.

**Codex graphics-track WovenMonitor client (2026-10-19):**
`apps/woven-monitor/src/gui.rs` adds a read-only bounded telemetry window for
clock, display geometry, and `/mnt` storage count. The catalog now passes
`--gui %p`; the shared QMP desktop gate launches it from F1 and passed on
1/2/4 CPUs, with evidence in
`audit-artifacts/monitor-desktop-{1,2,4}cpu-*`. Storage remains capped at 128
entries; controls, pointer widgets, and hardware GPU presentation remain out
of scope; bounded WSIZ/WRSZ is covered by the newer Monitor resize slice.

**Codex graphics-track WovenSettings client (2026-10-19):**
`apps/woven-settings/src/gui.rs` adds a read-only Canvas/WOPN client for
identity, UTC clock, and compositor geometry. The production catalog now
passes `--gui %p`; the shared desktop QEMU harness launches it from F1 and
passed on 1/2/4 CPUs, with evidence in
`audit-artifacts/settings-desktop-{1,2,4}cpu-*`. Controls, pointer widgets,
and hardware GPU presentation remain outside this slice; bounded WSIZ/WRSZ is
covered by the newer Settings resize slice.

**Codex graphics-track WovenFiles visual acceptance (2026-10-19):**
`scripts/test-files-desktop-qemu.py` now covers the packaged client through
the real WovenShell launcher: QMP sign-in, F1 search/launch, WOPN window
registration, serial lifecycle markers, and before/after screendump comparison.
It passed on 1/2/4 CPUs with evidence in
`audit-artifacts/files-desktop-{1,2,4}cpu-*`. This closes the live desktop
launch gate for Files; file-operation input, pointer widgets, and hardware GPU
presentation remain explicitly out of scope; bounded WSIZ/WRSZ is covered by
the newer Files resize slice, and pointer visual acceptance is covered by the
newer desktop pointer slice.

**Claude Code: `GraphicsSurface::write_canvas` placement gotcha (2026-10-19),
relevant to anyone wiring more app windows:** it always uploads to the
surface's `(0, 0)` (`self.write(0, 0, width, height, canvas.pixels())`) --
the `(x, y)` passed to `Canvas::new` is only the canvas's own internal
coordinate-addressing origin (what `fill_rect`/`draw_text`/`set` measure
from), not a surface placement offset. woven-calculator and
woven-files (commit `46bb21e`) both initially drew a background/border
directly on the window surface at an inset `(12, 12)`, then wrote a
same-sized canvas assuming it would land at that same offset -- it always
landed at `(0, 0)` instead, misaligning the border from the content and
(since write_canvas overwrites whatever is under it) erasing the border's
top/left edges while leaving right/bottom intact by a boundary-convention
coincidence that looks exactly like a shell resize-grip affordance if you
are not looking closely (I mistook it for one, twice). Fix: draw
everything -- background, panel, a hand-rolled `stroke_rect` (`Canvas` has
no stroke primitive), and text -- into one canvas sized to the whole
window, one `write_canvas` call. See woven-calculator's `run_gui` for the
pattern. Caught by actually looking at a QMP screendump rather than trusting
"a window opened" log lines; worth doing before calling any new app window
done.

**Graphics WovenFiles client foundation (2026-10-19):** commit `39dc9c09`
adds the Canvas-backed `--gui <shell-pid>` Files client and catalog wiring;
`98c92061` normalizes its catalog string. Standalone app Clippy and production
system-app/normal-boot QEMU gates passed on 1/2/4 CPUs. Interactive Files
launch/screenshot evidence is still pending, so this remains a client/package
checkpoint rather than full desktop visual acceptance.

**Codex graphics-track GSUB multiple-substitution extension acceptance
(2026-10-19):** direct and type-7 extension-wrapped type-2 substitutions now
have end-to-end regressions, including oversized-sequence rejection. The 57
`libwoven` tests, warning-denying Clippy, formatting, and Stage 19.2 visual
acceptance passed on 1/2/4 CPUs. Full feature ordering and hardware GPU output
remain open.

**Codex graphics-track bounded GSUB multiple substitutions (2026-10-19):**
GSUB lookup type 2 now expands a glyph through checked sequences capped at 16
replacement glyphs, including direct and type-7 extension dispatch, while
preserving bounded output/source metadata. The 57-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
visual acceptance passed on 1/2/4 CPUs. Complete feature ordering and hardware
GPU acceleration remain open.

**Claude Code: known issue -- heap allocation failure under sustained
interactive desktop use (2026-10-19), not yet root-caused:** live interactive
testing (`scripts/run-interactive-qemu.py --mode hardware-demo`, `-m 512M`,
real keyboard/mouse, not a scripted gate) hit
`KERNEL ALLOC ERROR: layout size=4096000 align=4` (the kernel's
`alloc_error_handler`, a fatal, unrecoverable divergence) after a longer
session of real use: tiling toggled on/off repeatedly, lock/unlock, all four
workspaces visited, notifications cleared. 4,096,000 bytes is exactly a
1280x800x4 full-screen-sized buffer. `heap::MAX_SIZE` is 64 GiB (bounded by
physical frames, not a fixed cap) growing in 256 KiB chunks, so a single 4 MB
allocation failing is not obviously "out of RAM" -- it is at least as
plausible that a long session's many smaller alloc/free cycles (toasts,
redraws, tiling relayout) left no single contiguous 4 MB free block even with
enough total free memory, i.e. allocator fragmentation, as it is a genuine
leak or just an undersized `-m`. Not investigated further this session (the
user chose to keep testing rather than block on root-causing it); whoever
picks this up next should start by reproducing with `-m 1024M` to rule
memory limit in or out, then look at whether anything in the compositor/
desktop path (workspace switching, tiling relayout, notification toasts)
allocates and frees screen- or near-screen-sized buffers repeatedly rather
than reusing one.

**Codex graphics backend boundary rerun (2026-10-19):** the capability-gated
`virtio-gpu-gl-pci` VIRGL discovery/context/resource/command lifecycle passed
on 1/2/4 CPUs with fresh audit artifacts. This does not claim visible GPU
scanout: QEMU still rejects the tested 3D resource/blob presentation path, so
software/2D scanout remains the accepted fallback.

**Codex graphics-track bounded GSUB reverse-chaining substitutions
(2026-10-19):** GSUB lookup type 8 now validates current, backtrack, and
lookahead coverages before applying a bounded reverse-chaining replacement;
direct and type-7 extension paths are covered. The 57-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
visual acceptance passed on 1/2/4 CPUs. Hardware GPU acceleration remains
open.

**Claude Code to Codex: please commit your pending working-tree changes
(2026-10-19):** at the time of writing, `git status` on this shared tree
shows ~50 modified files outside anything Claude Code touched this
session -- `kernel/src/*` (block_io, compositor, device, display, drivers,
elf, hal/acpi, hal/pci*, hda, hid, interrupts, livelog, nvme, paging,
physical_probe, power, serial, session, shell, smp, syscall, task, usb_msc,
userspace*, virtio_gpu, virtio_pci, wfs, wifi*, xhci), `libwoven/src/lib.rs`,
several `tests/*.rs`, a few `scripts/*` and `write-live-usb.ps1`, plus
`Cargo.toml`/`kernel/Cargo.toml`/`.gitignore` -- all presumably your
graphics-track (GPOS/font-shaping) work in progress, going by the commit
history (`Graphics: ...`). You're clearly already committing incrementally
(several `Graphics: ...` commits landed during this session), so this may
just be normal mid-task state -- no urgency implied, just flagging per the
user's request. Two things worth noting from working in this tree today:
`cargo build --release -p libwoven` failed transitively once while a
`GposContextNeighbors`/`previous2`/`next2` edit was mid-flight (resolved on
retry a minute later), and a direct push to `phase2/userspace-platform` can
race a concurrent one -- worth a `git pull --rebase` (or equivalent) before
pushing if you haven't already. Whenever you reach your next safe/accepted
checkpoint, committing (and pushing) this batch would keep the shared tree
in a state other sessions can build against. Thanks!

**Claude Code: woven-calculator wired to a real desktop window (2026-10-19):**
**Claude Code: woven-calculator wired to a real desktop window (2026-10-19):**
touched `kernel/src/main.rs` (only the `install_production_system_apps`
catalog entry for woven-calculator, not the separate `stage19-app-catalog-test`
self-test's copy) and `apps/woven-calculator/src/main.rs`. See
[docs/stage-status.md](stage-status.md#wovencalculator-desktop-window-2026-10-19)
for the full writeup. Short version: `--gui %p` + the `share_with()`/`WOPN`
protocol now opens a real titled window on the WovenShell desktop, verified
with a live QMP screendump. Also fixed a latent stack-overflow bug (a 108 KB
buffer that was a stack local) while doing so. `cargo clippy --all-targets`,
the kernel `hardware-demo` feature Clippy, and
`scripts/test-shell-qemu.py --hardware-demo` all pass on 1/2/4 CPUs. The
other nine Stage 19 catalog apps are still CLI-only and are open follow-up
work, not started.

**Codex graphics-track bounded GSUB alternate substitutions (2026-10-19):**
GSUB lookup type 3 now selects the first bounded alternate from checked
coverage/set records, including extension wrappers. The 56-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU
acceleration remains open.

**Codex graphics-track saturating shared font metrics (2026-10-19):** shared
font width and line-count accumulation now saturate for arbitrary UTF-8 input.
The 55-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded surface dimension casts (2026-10-19):** Canvas
and DepthBuffer cap each accepted non-empty dimension at `i32::MAX`, matching
the internal offset conversions. The 55-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded Canvas text origins (2026-10-19):** the shared
`Canvas::draw_text` path now saturates glyph and newline cursor arithmetic
before clipped writes. The 55-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded surface coordinate extents (2026-10-19):**
Canvas and DepthBuffer constructors reject non-empty rectangles whose logical
right or bottom edge cannot be represented by their `i32` coordinate APIs.
The 54-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded affine transform arithmetic (2026-10-19):**
fixed-point scale and translation constructors now saturate large inputs, and
affine mapping uses `i128` intermediates before clamping to `i32`. The 53-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware
GPU acceleration remains open.

**Codex graphics-track widened path-fill intersections (2026-10-19):**
ordinary, compound, and supersampled path containment now use `i128`
edge-intersection arithmetic, preventing overflow for valid extreme `i32`
coordinates. The 52-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded path stroke rasterization (2026-10-19):** path
segments now use widened integer Cohen-Sutherland clipping before Bresenham,
so extreme endpoints cannot cause unbounded walks. The 51-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU
acceleration remains open.

**Codex graphics-track shared bounded path geometry (2026-10-19):** rounded
rectangles and path scan conversion now use a shared saturating Canvas-end
helper; rounded radii use checked conversion. The 50-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded text layout (2026-10-19):** centered and
wrapped text now derive rectangle edges with checked conversion, while scaled
glyph placement saturates cursor arithmetic before clipped fills. The 49-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware
GPU acceleration remains open.

**Codex graphics-track bounded transformed-rectangle corners (2026-10-19):**
transformed rectangles now construct far corners with checked
`usize`-to-`i32` conversion and saturation before affine mapping. The 48-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware
GPU acceleration remains open.

**Codex graphics-track bounded image blits (2026-10-19):** opaque and
blended WovenGraphics image paths now iterate only over the Canvas overlap and
map clipped coordinates back to source rows and columns. The 47-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware
GPU acceleration remains open.

**Codex graphics-track bounded stroke geometry (2026-10-19):** rectangle
outlines now convert far-edge dimensions with checked `usize`-to-`i32`
conversion and saturation before clipped fills, preventing extreme geometry
from wrapping. The 46-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded gradient rasterization (2026-10-19):**
WovenGraphics vertical and horizontal gradients now clip iteration to the
Canvas before calculating pixels while preserving full logical-rectangle
interpolation. Shared Canvas clipping uses widened arithmetic for extreme
`usize` dimensions. The 45-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded transformed-triangle clipping (2026-10-19):**
the shared 2D triangle helper used by transformed rectangles now clips its
raster bounds to the Canvas before iterating. Extreme transformed vertices
cannot cause unbounded software loops; widened edge arithmetic and existing
pixel clipping remain in force. The 44-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded software-3D triangle clipping (2026-10-19):**
the WovenGraphics software triangle path now clips raster iteration to the
intersection of the Canvas and caller-owned depth buffer before walking pixels.
Edge and barycentric arithmetic uses widened intermediates, so extreme
off-screen coordinates fail safely without overflow or unbounded iteration.
The 43-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded chaining format-2 target sequence positioning (2026-10-19):**
nested single-positioning records under GPOS chaining format 2 now resolve
coverage against target sequence indices beyond zero, while the outer class
coverage remains anchored to the first input. The bounded class-rule, context,
nested lookup and value-record checks remain enforced. The 42-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU
acceleration remains open.

**Codex graphics-track bounded chaining format-1 nested pair extension wrapper (2026-10-19):**
GPOS chaining format 1 now regression-tests a type-9 extension wrapper around
nested lookup-2 format-1 pair positioning. Wrapper format, nested offsets,
target coverage, partner glyph, pair-set bounds and value records remain
checked; stateful attachment lookups fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded chaining format-1 nested pair positioning (2026-10-19):**
nested lookup-2 format-1 pair positioning is now available from GPOS chaining
format 1 through the shared checked nested-record path. Target glyph coverage,
partner selection, pair-set offsets, value-record sizes and table extents are
validated; malformed or unsupported stateful nested lookups fail closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded chaining format-1 target sequence positioning (2026-10-19):**
nested single-positioning records under GPOS chaining format 1 now resolve
coverage against the selected target glyph for sequence indices 0 through 4;
the outer chaining coverage remains anchored to the first input glyph. The
bounded glyph-rule, context and value-record checks remain unchanged. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded chained class-matrix pair extension wrapper (2026-10-19):**
the bounded GPOS chaining class-rule path now regression-tests a type-9
extension wrapper around nested lookup-2 format-2 class-matrix pair
positioning. Extension format, nested offsets, class definitions, matrix
dimensions and value-record extents remain checked; malformed or unsupported
forms fail closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS class-matrix pair positioning (2026-10-19):**
nested lookup-2 format-2 class-matrix pair positioning is now supported from
the bounded GPOS chaining format-2 class-rule path. Coverage, class
definitions, matrix dimensions, pair value-record extents and the selected
partner class are checked before applying the first-glyph adjustment; malformed
or unsupported nested forms fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded contextual GPOS class-rule pair positioning (2026-10-19):**
the bounded GPOS chaining format-2 class-rule path now applies nested lookup-2
format-1 pair positioning, including checked pair-set, value-record and
partner-glyph bounds. Direct nested pair lookups are supported; nested
extension-wrapped pair lookups use the common type-9 path. Other nested
pair/class forms remain fail-closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded contextual GPOS chaining format 2 (2026-10-19):**
GPOS lookup type 8 chaining-context format 2 now matches bounded backtrack,
input and lookahead class rules with up to five input glyphs and two context
classes on either side, then applies nested single-positioning lookup-1
records through direct or type-9-wrapped lookups. Class definitions, class-set
and record bounds are checked; unsupported nested pair/class forms fail closed.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS chaining format 1 (2026-10-19):**
GPOS lookup type 8 chaining-context format 1 now matches bounded glyph-rule
records with up to five input glyphs and two backtrack/lookahead glyphs, then
applies nested single-positioning lookup-1 records, including type-9 wrappers.
Malformed rules and unsupported nested pair/class forms fail closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS chaining format 3 (2026-10-19):**
GPOS lookup type 8 chaining-context format 3 now reuses the checked bounded
contextual dispatcher for one- to five-input records, up to two backtrack/
lookahead coverages, nested single/pair positioning, class-pair matrices and
type-9 wrappers. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS class-pair positioning (2026-10-19):**
nested type-2 format-2 pair positioning now supports checked class definitions
and bounded class matrices for contextual targets and partners. Class counts,
matrix extents and table bounds remain validated. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded contextual GPOS extension wrappers (2026-10-19):**
contextual dispatch now unwraps checked type-9 extension records containing
nested type-7 subtables, preserving the bounded input, context and nested
single/pair paths. Extension formats, offsets and nested bounds remain
validated. The 42-test `libwoven` suite, warning-denying library/kernel/
workspace Clippy, formatting, and Stage 19.2 compositor visual acceptance on
1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded two-coverage contextual GPOS guards (2026-10-19):**
format-3 matching now validates up to two backtrack and two lookahead
coverages from independently supplied adjacent shaped glyphs. Missing context
fails closed; longer context and broader contextual lookup semantics remain
open. The 42-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded multi-input contextual GPOS pair positioning (2026-10-19):**
nested type-2 format-1 pair positioning now works across the bounded input
sequences, selecting an in-sequence following glyph or the next shaped glyph
as partner and propagating advances from the target. Bounds and partner
availability remain checked. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed; the 4-CPU run needed one early-boot retry and
then passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS sequence-index-1 pair positioning (2026-10-19):**
two-input format-3 records targeting sequence index one can dispatch bounded
nested type-2 format-1 pair positioning against the next shaped glyph. Pair
coverage, pair-set and partner availability remain checked; later target
indices and broader contextual semantics remain open. The 42-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU
acceleration remains open.

**Codex graphics-track bounded five-input contextual GPOS positioning (2026-10-19):**
format-3 subtables with five input coverages now apply bounded nested single
adjustments to sequence index four and propagate advances to later glyphs.
One- through four-input paths remain supported; pair positioning stays on the
two-input sequence-index-zero path, while longer context and broader lookup
semantics remain open. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded four-input contextual GPOS positioning (2026-10-19):**
format-3 subtables with four input coverages now apply bounded nested single
adjustments to sequence index three and propagate advances to later glyphs.
One- through three-input paths remain supported; pair positioning stays on the
two-input sequence-index-zero path, while longer context and broader lookup
semantics remain open. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded three-input contextual GPOS positioning (2026-10-19):**
format-3 subtables with three input coverages now apply bounded nested single
adjustments to sequence index two and propagate advances to later glyphs.
One- and two-input paths remain supported; nested pair positioning stays on
the two-input sequence-index-zero path, while longer context and broader
lookup semantics remain open. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded single-input contextual GPOS positioning (2026-10-19):**
format-3 subtables with one input coverage now apply bounded nested single
adjustments through the shaped-glyph pass, including saturating advance
propagation and adjacent context guards. Two-input contextual positioning and
broader semantics remain separately bounded. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration
remains open.

**Codex graphics-track bounded contextual GPOS sequence-index-1 positioning (2026-10-19):**
contextual format-3 records targeting sequence index one now apply bounded
nested single adjustments to the second input glyph and propagate advances to
following glyphs. Nested pair positioning remains limited to sequence index
zero; indices beyond one and broader contextual dispatch remain open. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Codex graphics-track bounded contextual GPOS multi-record composition (2026-10-19):**
contextual format-3 records targeting sequence index zero now accumulate
multiple nested single/pair adjustments with saturating arithmetic. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Sequence
indices beyond zero and broader contextual dispatch remain open; hardware GPU
acceleration remains open.

**Codex graphics-track bounded contextual GPOS context guards (2026-10-19):**
added checked one-coverage backtrack/lookahead matching from adjacent shaped
glyphs to the format-3 contextual path, while retaining context-free matching.
Longer context and multi-record dispatch remain open. The 42-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Hardware GPU
acceleration remains open.

**Codex graphics-track nested contextual GPOS pair adjustment (2026-10-19):**
extended bounded format-3 context matching with nested type-2 format-1 pair
lookups, including checked pair sets and extension wrappers. Broader
backtrack/lookahead and multi-record contextual composition remain open. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Hardware GPU acceleration remains open.

**Claude Code interactive QEMU launcher (2026-10-19):** added
`scripts/run-interactive-qemu.py`, a hands-on dev convenience script, not an
acceptance harness. It reuses the same build steps as
`scripts/test-shell-qemu.py` (package-system-apps, build `woven-shell`,
`cargo run --print-image`) to produce a `hardware-demo` (default),
`system-apps`, or `plain` boot image from the current working tree, creates/
reuses the FAT32 `wovenhat-disk.img` data disk via `scripts/create-fat32.py`,
and launches `qemu-system-x86_64` with a normal visible display (no
`-display none`, no QMP automation) so a person can sign in and use the
desktop/apps directly. Serial + QEMU monitor share this terminal's stdio via
`-serial mon:stdio`. This produces no stage acceptance evidence and does not
touch USB/physical hardware.

Follow-up: the `hardware-demo`/`system-apps` catalog apps (woven-settings,
woven-monitor, etc.) are currently CLI diagnostic utilities with no window of
their own -- `display: unavailable (DisplayIo required)` in their output is
expected (only the compositor-authority holder gets that capability), not a
defect. The actual 2D/3D `libwoven::graphics` compositor work lives in
`hello-gui`, exercised only by the Stage 19.x self-test features, which all
end boot via `qemu_test_exit_success()` (isa-debug-exit) a moment after
presenting one frame -- too fast for a human to see. Added
`scripts/run-interactive-graphics-qemu.py`: builds the same `stage19-2-test`
image but omits the `isa-debug-exit` device, so that exit call's port write is
a no-op and the kernel parks in its `hlt` loop with the presented frame still
in the real framebuffer, visible in the QEMU window until closed.

**Codex graphics-track bounded contextual GPOS single adjustment (2026-10-19):**
added format-3 two-input context matching with checked coverage and lookup
records, applying a nested type-1 format-1 adjustment to sequence index 0;
type-9 wrappers are supported. Contextual pair and broader nested composition
remain open. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hardware GPU acceleration remains open.

**Codex graphics-track bounded GPOS mark-to-ligature attachment (2026-10-19):**
implemented format-1 mark-to-ligature positioning with class/component bounds,
type-9 support and script/LangSys filtering. Since the shaped-glyph ABI does
not expose a mark's ligature component index, the bounded implementation uses
the final component deterministically. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Contextual GPOS and hardware
GPU acceleration remain open.

**Codex graphics-track GPOS attachment extension wrappers (2026-10-19):**
added bounded type-9 unwrapping for mark-to-base, mark-to-mark and cursive
lookups, with checked extension formats, nested types and offsets. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Broader
GPOS lookup/type semantics and hardware GPU acceleration remain open.

**Codex graphics-track script/LangSys-scoped GPOS attachments (2026-10-19):**
mark-to-base, mark-to-mark and cursive attachment now share bounded selected
script/LangSys feature filtering, with missing selections failing closed. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Broader GPOS lookup/type semantics and hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS lookup-order composition (2026-10-19):**
selected `kern` GPOS single and pair adjustments now accumulate in lookup
order with saturating arithmetic and strict query-kind filtering. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Broader
GPOS feature/type coverage and hardware GPU acceleration remain open.

**Codex graphics-track outline-resolved GPOS format-2 anchors (2026-10-19):**
resolved format-2 contour-point anchors through the bounded glyph outline
decoder for mark-to-base, mark-to-mark and cursive attachment, with checked
point and outline bounds. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Broader lookup ordering and hardware GPU
acceleration remain open.

**Codex graphics-track format-3 attachment anchors across GPOS (2026-10-19):**
extended the checked format-3/device-delta anchor path to mark-to-mark and
cursive attachment in addition to mark-to-base. Format-2 contour-point anchors
are now resolved through glyph outlines. The 42-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Broader lookup ordering
and hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS format-3 anchors (2026-10-19):** added
checked mark-to-base format-3 anchors with optional ppem device deltas through
the existing device-table decoder. Format-2 contour-point anchors are now
resolved through glyph outlines. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Broader lookup ordering and
hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS mark-to-mark attachment (2026-10-19):**
added checked type-6 format-1 `mkmk` attachment with mark1/mark2 coverage,
class arrays and anchors; stacked marks move without advance changes. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Alternate anchors, broader lookup ordering and hardware GPU acceleration remain
open.

**Codex graphics-track bounded GPOS cursive attachment (2026-10-19):** added
checked type-3 format-1 `curs` entry/exit anchor matching over adjacent
covered glyphs, adjusting the right origin without changing advances. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Mark-to-mark, alternate anchors, broader lookup ordering and hardware GPU
acceleration remain open.

**Codex graphics-track bounded GPOS mark-to-base anchors (2026-10-19):**
added checked type-4 format-1 mark-to-base attachment for `mark`/`mkmk`
features, including coverage, class-array and anchor validation; mark origins
move without advance changes. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Mark-to-mark, cursive, alternate anchors,
broader lookup ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS single adjustments (2026-10-19):** added
checked GPOS type-1 single-adjustment formats 1/2 with placement, advance and
ppem device deltas; advances propagate through following glyph origins. The
42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Mark/cursive attachment, broader lookup ordering and hardware GPU acceleration
remain open.

**Codex graphics-track bounded GPOS pair extensions (2026-10-19):** added
checked GPOS type 9 extension unwrapping for nested type-2 pair positioning,
including class and pair-set formats with existing ppem device adjustments.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Broader GPOS lookup ordering, other positioning types and hardware GPU
acceleration remain open.

**Codex graphics-track bounded GSUB extension class lookups (2026-10-19):**
extended type 7 records to reuse the checked class-based contextual and
chaining format-2 matchers, including bounded class sets, rules and nested
single lookups. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Complete lookup ordering and hardware GPU
acceleration remain open.

**Codex graphics-track bounded GSUB extension contextual/chaining format 3
(2026-10-19):** extended type 7 records to reuse the checked format-3
contextual/chaining matcher, including bounded coverage sequences and nested
single lookup invocation. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Extension-wrapped class-based forms,
complete lookup ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded GSUB extension ligatures (2026-10-19):**
extended type 7 extension unwrapping to nested type-4 ligature substitutions,
preserving bounded component consumption and validating coverage, component,
offset and glyph ranges. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Extension-wrapped contextual/chaining
lookups, complete lookup ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded GSUB extension lookups (2026-10-19):** added
checked GSUB type 7 extension unwrapping for nested single substitutions in
formats 1/2, with bounded offsets, coverage and substitute arrays;
unsupported extension lookup types fail closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Extension-wrapped
contextual/chaining/ligature lookups, complete lookup ordering and hardware
GPU acceleration remain open.

**Codex graphics-track bounded GSUB feature-order composition (2026-10-19):**
added an eight-pass local GSUB pipeline so ordered single substitutions and
contextual/chaining results can compose. Ligature consumption and no-op/cycle
repetition remain bounded. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Complete lookup ordering, broader nested
composition and hardware GPU acceleration remain open.

**Codex graphics-track bounded class-based GSUB chaining format 2
(2026-10-19):** added checked backtrack, input and lookahead class-definition
matching for lookup type 6, with nested type-1 substitution at the current
glyph. Class-set, rule, sequence, lookup and glyph bounds remain fail-closed.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Broader nested lookup composition, complete feature ordering and hardware GPU
acceleration remain open.

**Codex graphics-track bounded GSUB chaining format 3 (2026-10-19):** added
checked lookup type 6 format 3 chaining-context matching across backtrack,
input and lookahead coverage sequences, with nested type-1 substitution at the
current glyph. Lookup, sequence, coverage and glyph bounds remain fail-closed.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Class-based chaining, broader nested composition, complete feature ordering and
hardware GPU acceleration remain open.

**Codex graphics-track bounded class-based contextual GSUB format 2
(2026-10-19):** added checked class definitions, rule sets, subsequent input
class matching and nested type-1 substitution at the current glyph. Class-set,
rule, lookup and glyph bounds remain fail-closed. The 42-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Chaining substitution,
full feature ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded contextual GSUB format 3 (2026-10-19):** added
checked lookup type 5 format 3 backtrack/input/lookahead coverage matching with
one nested type-1 substitution at the current glyph. Unsupported nested forms
and malformed bounds fail closed. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Class-based/contextual format 1/2, chaining,
full feature ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded GSUB single substitutions (2026-10-19):**
added checked lookup type 1 formats 1/2 for selected Script/LangSys features,
including `ccmp`, `locl`, `smcp`, `subs`, `sups`, `case` and `cpsp`. Coverage,
substitute-array and glyph-range bounds remain fail-closed. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Contextual/
chaining GSUB, full feature ordering and hardware GPU acceleration remain open.

**Codex graphics-track bounded named LangSys selection (2026-10-19):** added
checked named language-system selection within a requested script and exposed
`TextShapingOptions` for script, language and ppem inputs. GSUB `liga`/`rlig`
and GPOS `kern` use the selected LangSys; missing language tags do not fall
back to another feature set. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Contextual/complex GSUB/GPOS, broader script
data and hardware GPU acceleration remain open.

**Codex graphics-track bounded script-scoped GPOS (2026-10-19):** extended
script-aware shaping so the requested ScriptList/default LangSys scopes both
GSUB `liga`/`rlig` and GPOS `kern` pair positioning. The combined
`shape_text_ligatures_with_script_and_ppem` entry point makes script and ppem
requirements explicit. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Named LangSys selection, contextual/complex
GSUB/GPOS and hardware GPU acceleration remain open.

**Codex graphics-track bounded GSUB script selection (2026-10-19):** added
`shape_text_ligatures_with_script` for checked ScriptList/default-LangSys
selection and bounded `liga`/`rlig` feature lookup resolution. Missing scripts
fail closed to no substitution; the legacy feature-list path remains intact.
The 42-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
GPOS script scoping, language-specific systems, contextual/complex GSUB and
hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS device-table deltas (2026-10-18):**
added ppem-aware, bounds-checked decoding for GPOS device formats 1, 2 and 3.
Device deltas augment the corresponding x/y placement or advance field. The
legacy shaping entry point fails closed when ppem is required, while
`shape_text_ligatures_with_ppem` makes the dependency explicit. The 42-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Complete
script shaping, variation-index device semantics and hardware GPU acceleration
remain open.

**Codex graphics-track bounded GPOS y-value positioning (2026-10-18):**
extended pair adjustments with y-placement/y-advance fields alongside x
values. Scaled values update shaped origins and following origins under the
existing bounds checks. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Device-table deltas, complete script shaping
and hardware GPU acceleration remain open.

**Codex graphics-track bounded GPOS class pair positioning (2026-10-18):**
extended optional GPOS `kern` handling to pair-positioning format 2 with
bounded class-definition formats 1/2 and class-matrix lookup. Class counts,
matrix extents and table bounds are checked while scaled x-placement/x-advance
adjustments update shaped runs. The 42-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Full GPOS value/device semantics, script
shaping and hardware GPU acceleration remain open.

**Codex graphics-track bounded OpenType GPOS pair positioning (2026-10-18):**
added optional GPOS `kern` feature handling for type-2 pair-positioning format
1. Checked x-placement/x-advance adjustments are scaled from font units and
following origins are shifted; unsupported formats and malformed records fail
closed. The 42-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Full GPOS value/device formats, script shaping and hardware GPU
acceleration remain open.

**Codex graphics-track variation-aware shaped rasterization (2026-10-18):**
extended glyph-ID shaping/rasterization with normalized variation coordinates:
`advance_width_glyph_with_variation`, variation-aware outline/compound paths,
and `render_shaped_text_with_variation` carry bounded gvar metrics and outlines
through GSUB-selected glyphs. Base rendering remains separate so deltas apply
once. The 41-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Full script shaping and hardware GPU acceleration remain open.

**Codex graphics-track shaped glyph-ID raster integration (2026-10-18):**
added direct glyph-ID outline/raster entry points and
`TrueTypeFace::render_shaped_text`, preserving GSUB-selected glyph IDs through
the antialiased path. Glyph, outline-storage and renderer-capacity bounds are
checked. The 41-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Variation-aware shaped rasterization, full script shaping and hardware
GPU acceleration remain open.

**Codex graphics-track bounded OpenType GSUB ligatures (2026-10-18):** added
`ShapedGlyph` and `TrueTypeFace::shape_text_ligatures` for bounded GSUB type-1
ligatures linked by `liga`/`rlig`. Coverage, lookup, component, source-span,
metric and table bounds are checked; absent features fall back to one glyph per
character and malformed shaping data fails closed. The 41-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Full script shaping,
shaped-glyph raster integration and hardware GPU acceleration remain open.

**Codex graphics-track bounded directional text runs (2026-10-18):** added
`TextDirection::classify` and `TrueTypeFace::layout_text_bidi` for strong
RTL/LTR classification, neutral inheritance and visual run ordering in
caller-owned placements. This is explicitly not full UAX#9 or script shaping.
The 40-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
OpenType shaping, full bidi and hardware GPU acceleration remain open.

**Codex graphics-track bounded hint control flow (2026-10-18):** added
instruction-span-aware `IF`/`ELSE`/`EIF` selection and bounded relative
`JMPR`/`JROT`/`JROF` branches. PUSH payloads are skipped during branch scans;
nesting, targets and the existing 1024-step cap are checked, with malformed
or unsupported control flow failing closed. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Full hint interpolation,
function calls, shaping and bidi remain open.

**Codex graphics-track bounded hint storage/CVT resources (2026-10-18):**
added `execute_glyph_hints_with_resources` with checked caller-owned storage
for `WS`/`RS` and CVT-backed `MIAP`/`MDAP` execution. The no-storage wrapper
fails closed on storage access. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Full hint interpolation,
control flow, shaping and bidi remain open.

**Codex graphics-track bounded hint point/CVT execution (2026-10-18):**
extended `execute_glyph_hints_with_cvt` with checked `MDAP` and `MIAP` support
along the active freedom axis. A synthetic program moves point 0 to a caller
CVT value, while the no-CVT wrapper fails closed. The 40-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Full hint interpolation,
shaping and bidi remain open.

**Codex graphics-track bounded hint-state execution (2026-10-18):** added
`HintState` and `TrueTypeFace::execute_glyph_hints` for vector-axis,
rounding-state, stack and PUSHB/PUSHW primitives with 1024-step and
256-entry-stack limits. Point movement, storage, CVT and control flow fail
closed. The 40-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Full hinting, shaping and bidi remain open.

**Codex graphics-track bounded TrueType hint-program boundary (2026-10-18):**
added `TrueTypeFace::glyph_hint_program`, a checked caller-owned copy path for
simple-glyph instructions with a 256-byte bound. Composite programs and
execution remain fail-closed. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. The VirGL/QEMU boundary is unchanged.

**Codex graphics-track gvar phantom-point metrics (2026-10-18):** added
`TrueTypeFace::advance_width_with_variation`, applying simple-glyph phantom
point deltas and deriving the varied advance from the right/left phantom
pair. Full and half scalar synthetic metrics pass. The 40-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Legacy fonts without
trailing LSB records retain base advance behavior; hinting, shaping, bidi and
the VirGL/QEMU boundary remain explicit constraints.

**Codex graphics-track composite gvar component variation (2026-10-18):**
extended `outline_points_with_variation` to apply bounded component-index
X/Y placement deltas for composite glyphs, recursively varying nested
components and tracking each component’s output range. The 40-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Phantom metrics,
hinting, shaping and bidi remain separate constraints; the VirGL/QEMU boundary
is unchanged.

**Codex graphics-track shared-point gvar tuples (2026-10-18):** added
glyph-level packed shared-point handling, including the valid all-points form,
with checked point runs, bounds and reserved tuple flags. The 40-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Composite
variation, hinting, shaping, bidi and the VirGL/QEMU boundary remain explicit
follow-on constraints.

**Codex graphics-track intermediate gvar regions (2026-10-18):** extended
simple-glyph variation to embedded-peak intermediate regions with checked
start/peak/end ranges and piecewise scalar blending. The synthetic fixture
verifies full and half strength. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Shared points, composite variation, hinting,
shaping and bidi remain explicit follow-on constraints.

**Codex graphics-track simple gvar tuple application (2026-10-18):** added
`TrueTypeFace::outline_points_with_variation` for bounded embedded-peak
`gvar` tuples on simple glyphs, including normalized F2DOT14 scalar handling
and packed point/delta decoding. Composite glyphs and shared-point sets fail
closed. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Hinting, shaping and the VirGL/QEMU boundary
remain explicit follow-on constraints.

**Codex graphics-track bounded gvar payload boundary (2026-10-18):** added
`TrueTypeFace::glyph_variation_data`, a borrowed, fail-closed slice over
validated `gvar` glyph data with checked version, glyph count, offset width,
shared-tuple bounds and table extents. A synthetic fixture covers empty and
non-empty glyph payloads. The 40-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Delta application remains open; the
VirGL/QEMU boundary is unchanged.

**Codex graphics-track accepted fvar axis fixture (2026-10-18):** added a
positive synthetic-font test for the bounded `wght` axis metadata path,
including checked 16.16 minimum/default/maximum values. The 39-test
`libwoven` suite, warning-denying library/kernel/workspace Clippy, formatting,
and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Variation
rendering remains metadata-only; the VirGL/QEMU boundary is unchanged.

**Codex graphics-track bounded variable-font axis metadata (2026-10-18):**
added checked `fvar` axis metadata through `VariationAxis` and
`variation_axes`, exposing 16.16 min/default/max values without applying
`gvar` deltas. The 38-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Variation rendering remains open; the
VirGL/QEMU boundary is unchanged.

**Codex graphics-track explicit RTL placement (2026-10-18):** added
`TextDirection` and bounded RTL mirroring over validated glyph placements.
This is an explicit placement mode, not Unicode bidi or script shaping. The
38-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed;
the VirGL/QEMU boundary is unchanged.

**Codex graphics-track Unicode format-12 cmap (2026-10-18):** added bounded
format-12 group lookup for non-BMP code points while preserving format-4 BMP
mapping, with checked group extents and glyph IDs. A synthetic non-BMP fixture
passes mapping and advance lookup. The 38-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Full shaping, bidi and
variation tables remain open; the VirGL/QEMU boundary is unchanged.

**Codex graphics-track TrueType point-matched composites (2026-10-18):**
added support for composite component point arguments, anchoring transformed
components to parent points with strict index validation. The synthetic glyph
fixture now exercises XY and point-matched components. The 37-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Variation deltas,
hinting, modern shaping and scripts remain open; the VirGL/QEMU boundary is
unchanged.

**Codex graphics-track bounded TrueType composite glyphs (2026-10-18):**
added recursive, depth-limited composite component decoding with XY offsets,
uniform/non-uniform/two-by-two transforms, instruction skipping and bounded
output. The synthetic mapped glyph is now composite and passes raster tests.
Point-matching arguments, variation deltas and hint execution fail closed. The
37-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed;
the VirGL/QEMU boundary is unchanged.

**Codex graphics-track bounded TrueType word wrapping (2026-10-18):** added
`layout_text_word_box`, keeping whole words intact at spaces while preserving
newlines, checked advances and optional kerning in caller-owned placements. The
37-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Script shaping, bidi, hinting and compound components remain open; the
VirGL/QEMU boundary is unchanged.

**Codex graphics-track TrueType bitmap fallback (2026-10-18):** added
`render_text_with_bitmap_fallback`, using validated outlines for mapped glyphs
and the shared bounded bitmap font for missing/unsupported glyphs. Mixed output
is covered by the synthetic face fixture. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Modern shaping, bidi,
hinting and compound components remain open; the VirGL/QEMU boundary is
unchanged.

**Codex graphics-track bounded TrueType line breaking (2026-10-18):** added
`TrueTypeFace::layout_text_box` with explicit newlines, width wrapping, line
height, checked advances and optional kerning into caller-owned placements.
The 37-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Script shaping, bidi, word-level breaking, hinting and fallback remain open;
the VirGL/QEMU boundary is unchanged.

**Codex graphics-track optional TrueType kerning (2026-10-18):** added
fail-closed legacy `kern` format-0 pair lookup and applied adjustments in the
bounded single-line TrueType layout pass. The 37-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Modern shaping, scripts,
line breaking and fallback remain open; the VirGL/QEMU boundary is unchanged.

**Codex graphics-track bounded TrueType text layout (2026-10-18):** added
`GlyphPlacement`, `TrueTypeFace::layout_text`, and `render_text` using checked
horizontal advances and the antialiased glyph pipeline. The 37-test `libwoven`
suite, warning-denying library/kernel/workspace Clippy, formatting, and Stage
19.2 compositor visual acceptance on 1/2/4 CPUs passed. Kerning, shaping,
line breaking, hinting and fallback remain open; the VirGL/QEMU boundary is
unchanged.

**Codex graphics-track TrueType glyph raster entry point (2026-10-18):**
connected `TrueTypeFace::render_glyph` to the bounded antialiased compound
path renderer and verified a synthetic glyph reaches Canvas pixels. The
37-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
Shaping, kerning, hinting, fallback and compound components remain open; the
VirGL/QEMU hardware boundary is unchanged.

**Codex graphics-track bounded antialiased fills (2026-10-18):** added
integer 4x4 coverage rasterization for `Path` and `CompoundPath`, including
source-over opacity and even-odd holes, and routed the hello-gui badge through
the antialiased path. The 37-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. The VirGL/QEMU device capability boundary
remains explicit.

**Codex graphics-track bounded multi-contour paths (2026-10-18):** added the
eight-contour `CompoundPath`, even-odd hole filling, compound stroking, and
`TrueTypeFace::outline_compound` for simple glyphs. Overflow and compound
TrueType components fail closed. The 36-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. The VirGL/QEMU hardware capability boundary
remains unchanged and documented.

**Codex graphics-track TrueType contour assembly (2026-10-18):** added
`TrueTypeFace::outline_contour`, which maps one simple contour into a bounded
closed `Path`, handles implied on-curve midpoints and quadratic transitions,
and commits only after capacity checks succeed. The 35-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Multi-contour fill/hole
semantics, compound glyphs, shaping and antialiasing remain open.

**Codex graphics-track bounded quadratic paths (2026-10-18):** added
`Path::quadratic_to`, a four-segment deterministic quadratic Bézier flattening
primitive with atomic capacity failure, and exercised it in `hello-gui`. The
35-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
TrueType contour assembly and antialiased rasterization remain open.

**Codex graphics-track TrueType simple outlines (2026-10-18):** added
optional `glyf`/`loca` validation and bounded simple-glyph decoding into
caller-owned `OutlinePoint` buffers. Compound/oversized/malformed glyphs fail
closed. The 34-test `libwoven` suite, warning-denying Clippy, formatting, and
Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed. Curve expansion,
rasterization, antialiasing and shaping remain open.

**Codex graphics-track TrueType horizontal metrics (2026-10-18):** extended
`TrueTypeFace` with validated `hhea`/`hmtx` advance-width lookup, including the
final long-metric fallback rule. The 33-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. Outline extraction/rasterization remains
open.

**Codex graphics-track TrueType face boundary (2026-10-18):** added the safe,
borrowed `graphics::TrueTypeFace` parser for sfnt metadata and format-4 BMP
cmap lookup, with fail-closed validation and no allocation/unsafe code. The
33-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.
This is metadata/cmap support only; outline rasterization, antialiasing and
complex shaping remain open because no font asset is bundled.

**Codex graphics-track alpha image blits (2026-10-18):** added bounded
`Renderer::image_blended` uploads and routed the `hello-gui` icon through the
alpha-aware path. The 32-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed.

**Codex graphics-track rounded alpha panels (2026-10-18):** added clipped
`Renderer::rounded_rect` and `rounded_rect_blended` fills and exercised a
translucent rounded panel in `hello-gui`. The 32-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed.

**Codex graphics-track bounded text layout (2026-10-18):** added
`Renderer::text_box` for allocation-free UTF-8 character wrapping and clipping
with shared glyph fallback and integer scaling. `hello-gui` exercises it. The
31-test `libwoven` suite, warning-denying library/kernel/workspace Clippy,
formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs passed.

**Codex graphics-track alpha path fills (2026-10-18):** added uniform
source-over alpha for closed paths through `Renderer::fill_path_blended` and
exercised a translucent polygon in `hello-gui`. The 30-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed.

**Codex graphics-track horizontal gradients (2026-10-18):** added the clipped
`Renderer::horizontal_gradient` primitive and exercised it alongside the
vertical gradient in `hello-gui`. The 29-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed.

**Codex graphics-track bounded path filling (2026-10-18):** added clipped
even-odd filling for closed `graphics::Path` geometry through
`Renderer::fill_path`; work is restricted to Canvas bounds. `hello-gui`
exercises a filled polygon. The 28-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed.

**Codex graphics-track bounded paths (2026-10-18):** added allocation-free
`graphics::Path` move/line/close geometry and clipped `Renderer::stroke_path`.
`hello-gui` exercises an open chevron path. The 27-test `libwoven` suite,
warning-denying library/kernel/workspace Clippy, formatting, and Stage 19.2
compositor visual acceptance on 1/2/4 CPUs passed. Path filling and richer
outline/font work remain open.

**Codex graphics-track integer font scaling (2026-10-18):** extended the
shared-glyph `graphics::Font` with bounded 1×–16× metrics and added
`Renderer::text_scaled`/`text_centered_scaled`. `hello-gui` exercises a scaled
glyph. The 26-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. Outline fonts, hinting and shaping remain open.

**Codex graphics-track shared font metrics (2026-10-18):** added the
`graphics::Font`/`TextMetrics` abstraction over the shared 5x7 glyph table and
`Renderer::text_centered`. `hello-gui` exercises centered text in the visible
frame. The 25-test `libwoven` suite, warning-denying library/kernel/workspace
Clippy, formatting, and Stage 19.2 compositor visual acceptance on 1/2/4 CPUs
passed. TrueType/OpenType shaping and richer fallback remain open; the current
provider deliberately preserves compositor pixel identity.

**Codex graphics-track fixed-point transforms (2026-10-18):** added the safe,
deterministic signed 16.16 `graphics::Transform` and clipped
`Renderer::transformed_rect` primitive. `hello-gui` now exercises transformed
geometry in its visible frame. The 24-test `libwoven` suite, warning-denying
library/kernel/workspace Clippy, formatting, and Stage 19.2 compositor visual
acceptance on 1/2/4 CPUs passed. The surface ABI and software fallback are
unchanged; the QEMU VirGL blob presentation boundary remains open as an
environment/device capability issue.

**Codex graphics-track 2D increment (2026-10-18):** extended the existing
`libwoven::graphics::Renderer` with clipped vertical gradients, one-pixel
strokes, and bounded image blits. `hello-gui` now exercises those primitives
and emits `[S16G2] WovenGraphics 2D frame: PASSED`; the Stage 19.2 QEMU
harness requires both graphics markers. The 21-test `libwoven` suite,
warning-denying workspace/application Clippy, formatting, harness syntax, and
QEMU visual acceptance on 1/2/4 CPUs all passed. This remains a safe software
renderer over `Canvas`; no ABI or unsafe code changed. Paths, font shaping,
GPU backends, window policy, and physical display acceptance remain open.

**Codex graphics-track software 3D foundation (2026-10-18):** added a safe,
caller-owned `DepthBuffer`, projected `Vertex3`, and depth-tested triangle
rasterization with interpolated vertex colour to `libwoven::graphics`.
`hello-gui` exercises a small two-face 3D frame and emits `[S16G3D]
WovenGraphics software 3D frame: PASSED`; the Stage 19.2 harness requires it
with the existing 2D markers. The 23-test
`libwoven` suite, warning-denying Clippy, formatting, and QEMU visual
acceptance on 1/2/4 CPUs passed. This is still a deterministic software
backend; VirtIO-GPU capability negotiation, GPU buffers, command submission,
and shaders remain the next hardware-facing work.

**Codex graphics-track VirtIO-GPU capability boundary (2026-10-18):** the
modern VirtIO transport now retains negotiated features, and the VirtIO-GPU
driver requests/exposes a typed capability record for scanouts and VIRGL/3D
support. No shader or VIRGL command submission is claimed yet; software
rendering remains the fallback. Kernel target Clippy, workspace all-target
Clippy, and Stage 20.14 two-monitor pixel acceptance passed on 1/2/4 CPUs.
Evidence is under `audit-artifacts/stage20.14-{1,2,4}cpu-*`.

The next VIRGL slice adds capability-gated, bounded `CTX_CREATE` and
`CTX_DESTROY` packets using the VirtIO-defined context header. The current
QEMU acceptance device continues through the software/2D fallback, so this
does not claim shader or GPU rendering yet; the 1/2/4 CPU Stage 20.14 display
acceptance still passes.

The follow-up adds the bounded `GET_CAPSET_INFO` discovery query and records
the advertised capset count, version, and maximum payload size before a VIRGL
context is selected. The current standard QEMU device still uses the 2D
fallback; Stage 20.14 remains pixel-verified on 1/2/4 CPUs.

The harness now supports `--virgl`, selecting QEMU `virtio-gpu-gl-pci` with
`egl-headless`; VIRGL capset discovery and context lifecycle pass on 1/2/4
CPUs. The GL device rejects the legacy 2D resource mode, so this mode is
discovery-only until 3D resource creation and command submission are wired.

The resource transport slice is now wired and exercised on GL QEMU: create a
64x64 3D resource, attach it to the context, detach it, and release it. The
remaining unaccepted step is a valid VIRGL command stream with a visual result;
the software renderer remains the fallback until that gate passes.

The next probe now submits the protocol-defined VIRGL NOP command through
`SUBMIT_3D` after resource attachment. GL QEMU passes the complete
capability/context/resource/command transport lifecycle; no visible GPU frame
or compositor handoff is claimed yet.

The command packet is now assembled by a bounded `VirglStream` encoder with
checked payload/header capacity. The encoder-backed NOP transport acceptance
passes on 1/2/4 CPUs; the next graphics increment must add a real VIRGL clear
or draw command and prove a visible GPU-backed frame before claiming backend
completion.

The stream now also encodes surface creation, framebuffer binding, an RGBA
clear, and surface destruction; the GL-QEMU transport gate passes on 1/2/4
CPUs. A direct 3D-resource-to-scanout probe was attempted and rejected by QEMU
with device status `0x1203`; that failed evidence is preserved, and the
accepted gate was restored. The next increment must resolve the resource
scanout compatibility boundary before visual GPU acceptance.

The presentation probe now has a bounded, capability-gated resource-blob path
and an opt-in harness switch (`--blob`). This QEMU build can create a guest
blob but rejects it in the VirGL context; the host-3D-plus-guest blob class is
also rejected during creation. The default blob-disabled VIRGL acceptance
passes on 1/2/4 CPUs, and the blob path remains an explicit failed probe rather
than being folded into the accepted gate. Repeating the host-3D probe with
QEMU `hostmem=64M` produced the same `0x1200` creation failure.

**Codex graphics-track increment (2026-10-18):** `libwoven::graphics` now
provides typed `Color` and clipped `Rect` geometry plus a safe `Renderer`
over the existing software `Canvas`. `hello-gui` routes a visible frame
through that boundary and emits `[S16G] WovenGraphics visual frame: PASSED`.
The Stage 19.2 compositor harness now requires that marker in addition to its
kernel screen-pixel and retained-window checks. QEMU visual acceptance passed
on 1/2/4 CPUs; `libwoven` host tests (20), application Clippy, formatting and
Python validation passed. This is the first graphics abstraction checkpoint;
paths/gradients, font shaping, GPU backends, window policy and app launching
remain subsequent work.

**Codex Stage 10.7 release gate closure (2026-10-18):** after reclaiming
regenerable build output, `powershell.exe -NoProfile -ExecutionPolicy Bypass
-File .\RUN-STAGE10.7.ps1` exited 0 on the current shared working tree.
Host acceptance, host lint, build, Clippy, host tests, the repeated Stage 7.5
1-CPU memory matrix, Stage 7.6 2/4-CPU closure, 1/2/4-CPU network round
trips, and isolated Stage 10.1–10.7 production boots all passed. Preserved
transcript and serial evidence is under
`audit-artifacts/acceptance-20261018-091013-150/`. This closes the local
Stage 10.7 acceptance gate for this exact working tree; physical USB boot,
production release packaging, and later graphics/app-window work remain
separate.

**Claude Code live hands-on test, session 2 (2026-10-18):** at the user's
request, rebuilt the `hardware-demo` feature from the current working tree
(which carries the large set of uncommitted modifications already reflected
in `git status`, not the committed `phase2` tip). `python
scripts/test-shell-qemu.py --hardware-demo --cpus 1` passed: `Hardware demo:
sign-in + 10 applications + desktop + PS/2 IOAPIC + smptest: PASS`. The
resulting image is 13,697,024 bytes, SHA-256
`57922cab26a2df3a44f6228f648dffd714eae5653461b09f6eedce30f8dc0eec` — a
different candidate from the previously recorded `67528fa7c4...` write below,
since the working tree has changed. The same authorized Disk 1 / `VendorCo
ProductCode USB Device` / serial `4045141272666606378` (15,726,735,360 bytes)
was rewritten via an elevated `scripts/write-live-usb.ps1` run; raw read-back
matched, and a 14,985 MiB FAT32 `WOVENLOG` partition was recreated. Transcript:
`audit-artifacts/hardware-demo-usb-write-20261018-session2.txt`. This is only
a 1-CPU synthetic/QEMU acceptance plus a verified write; the user is about to
boot the real target hardware, log in, and exercise the 10 apps by hand.
Physical boot/login/app evidence is pending. Separately, an interactive
(non-headless) QEMU GTK window was also launched locally against the prior
`67528fa7c4...` build artifact for hands-on desktop testing on this machine;
that is informal, not acceptance evidence.

**Codex physical-login follow-up (2026-10-18):** a fresh
`python scripts/test-shell-qemu.py --hardware-demo --cpus 1` reproduction
passed. Its serial evidence contains `[DEMO] sample accounts installed in
volatile VFS (ada, lin)`, `[SESSION] WovenShell desktop started`, and the
script's `Hardware demo: sign-in + 10 applications + desktop ...: PASS`.
Windows currently reports no attached physical disk, so the USB contents and
firmware boot selection cannot be rechecked or rewritten from this session.
The previously recorded USB write still identifies the intended
`hardware-demo` image (`6eb71be0...`) and matching raw read-back; a physical
boot failure therefore still needs the exact on-screen symptom or serial
evidence before another image write is justified.

The USB was subsequently reinserted and the user confirmed that it still
boots the old diagnostic image. Windows identified the authorized target as
Disk 1 / `VendorCo ProductCode` / serial `4045141272666606378`; the guarded
rewrite was attempted with the current `hardware-demo` image but refused
before opening the disk because this shell was not Administrator. No USB
bytes were changed by that attempt.

The same authorized disk was then rewritten from an elevated PowerShell
handoff with the correctly quoted model argument. The guard matched
`PhysicalDrive1`, wrote the current 13,697,024-byte `hardware-demo` image
(SHA-256 `67528fa7c472efc9d7ab82cc82198002b577056a814fc250ad6ce455258208b4`),
read it back successfully, and recreated the FAT32 `WOVENLOG` partition.

**Codex desktop-on-hardware correction (2026-10-17):** the USB image recorded
in `audit-artifacts/ax201-physical-write-20261017.txt` was built with the
`physical-wifi` feature. `run_ax201_activation` is intentionally non-returning
and halts after its diagnostic, so that image cannot reach `system-apps`,
WovenShell, or the desktop session. Do not treat it as a production OS boot.
There is no current `target/release-validation/validated-image.json`; the
normal `system-apps` release gate must be regenerated before writing a desktop
image. Keep AX201 hardware diagnostics separate from the desktop boot path.
The direct `python scripts/test-shell-qemu.py --system-apps --cpus 4` test
passed: all 10 signed apps installed, WovenShell started, F12 switched to the
diagnostic console, and PS/2/SMP shell input passed. Candidate image SHA-256:
`2c26fb499a8bb0e3f8f626de9f66c672a0cc1d74af7b227fa9f1e8be0c0ad864`.
Evidence is in `audit-artifacts/system-apps-desktop-qemu-20261018/`. This
candidate is not a complete release: it uses the acceptance test app-trust
key and has no provisioned accounts file, so normal boot skips the sign-in
screen. The full release chain also remains incomplete: after the generated
build tree was reclaimed, a fresh run timed out at the fixed 180-second
`memory-4-release` gate after `[FILE MMAP] regression tests: PASSED`; its log
is preserved under `audit-artifacts/release-gate-memory4-timeout-20261018/`.
The earlier run passed that matrix row but stopped at `shell-smoke` when C:
ran out of space; both logs/results are preserved. Do not label or write this
candidate as a validated, login-ready production image.

**Codex hardware-demo USB profile (2026-10-18):** user explicitly
requested a full OS demo USB. Build a separate `hardware-demo` profile that
boots the normal system-apps desktop (never `physical-wifi`), seeds only the
documented `ada`/`1234` and `lin`/`9876` test accounts when no accounts file
already exists, and tests sign-in plus apps on 1/2/4 CPUs. Use the acceptance
app-signing key only for this disposable demo image; it is not a production
trust key. The QEMU demo passed 1, 2 and 4 CPU counts, each verifying sign-in,
the desktop, installation of 10 signed apps, keyboard/IOAPIC and SMP smoke.
Warning-denying bare-metal `hardware-demo` Clippy, `cargo clippy --all-targets
-- -D warnings`, `cargo fmt --all -- --check`, and Python syntax validation
passed. Serial/QEMU evidence is in
`audit-artifacts/hardware-demo-{1,2,4}cpu-20261018/`. Current image is
`C:\WovenHat-OS\.cargo-build\release\build\wovenhat-os-a76e5ab77178b0d9\out\wovenhat-os-uefi.img`
(13,697,024 bytes; SHA-256
`6eb71be03ce1573cfca5b7bf5096f951da6c44511dd4ac74c2c35486dc72354c`). The USB write is
complete: `audit-artifacts/hardware-demo-usb-write-20261018.txt` records the
exact USB identity, raw write, and matching read-back SHA-256. The previous
physical-Wi-Fi image was therefore what produced the hardware-test-only screen
reported by the user; that image has been replaced. Still awaiting a new
physical boot to confirm graphics, sign-in and apps on the target hardware.
The full production release gate remains incomplete as noted above, so this
is a disposable hardware-demo image, not a production release.

**Codex AX201 provider and queue-owner increment (2026-10-17):** AX201 now
has an explicit firmware-provider filename boundary for
`iwlwifi-QuZ-a0-hr-b0-77.ucode`; the provider bytes are still copied into
owned DMA and no firmware blob is committed. `Ax201QueueOwner` allocates the
22000 MQ RX free/used/status rings, command ring and one RX buffer per
descriptor, and
`Ax201Driver` refuses startup without attaching that owner. Queue storage is
retained until the caller marks hardware quiesced. Stage 13.9 passed on
1/2/4 CPUs and warning-denying kernel/host Clippy passed. Physical BAR/MSI
attachment, device-produced ALIVE and radio qualification
remain open.

The dedicated `physical-wifi` image now includes the external API-77 firmware,
Intel reset/readiness and MAC-access sequencing, exact AX201 binding, owned
512-entry MQ RX storage, context publication, CPU_INIT_RUN and bounded RX ALIVE
polling. The rebuilt 42,008,576-byte image has SHA-256
`5b5fd671fc67b48cc3e55cb778c5cab3cdd56cc3e92512d67657857f8eacfaf9`; the
guarded USB write and raw read-back passed to the authorized USB. Physical boot
evidence is pending.

The context-info command-queue size now uses the upstream 22000
`TFD_QUEUE_CB_SIZE` encoding (`2` for 32 entries); the ring itself remains a
32-entry, 256-byte-TFD allocation.
The physical startup now also programs the 22000 RFH free/used/status queue
bases and resets their device cursors before ringing the 512-entry RX doorbell.
The physical poller now consumes completions and recycles buffers to that free
ring, ringing on the eight-descriptor cadence.
The physical activation path now enters after IDT/APIC setup, enables PCI
memory/bus mastering, programs AX201 MSI and CSR RX causes, services deferred
IRQ work, and disables those paths before PCI command restoration.
The latest scan-session image was written and raw-read back through
`audit-artifacts/ax201-physical-activation-usb-write-scan-session-20261017.txt`.
The PHY-context candidate image is `8774dc764c9042acbf7f9b94258014707ee91b63a8e3addb5d35a26f89bcd180`;
the subsequent guarded write was refused because this shell was not elevated.
The failed transcript is retained in
`audit-artifacts/ax201-physical-activation-usb-write-phy-context-20261017.txt`.
The latest code also adds exact managed-station MAC and API-2 binding
encoders, generic zero-status response validation, and a physical post-ALIVE
PHY → MAC → binding sequence. Its 42,008,576-byte candidate image SHA-256 is
`bde089b3b692fc585ff6d9cda73a81a765f69aca321f9f0aed5cf8722665c312`; it has
not replaced the USB image because the writer requires Administrator access.
The current RX MPDU/scan-table image is 42,008,576 bytes with SHA-256
`f0c1d430652071c8f917169da37acc281d2493218db75390ee9253a14f286073`.
Its Stage 13.9 1/2/4-CPU evidence is
`stage13.9-1cpu-1792259384946552100`,
`stage13.9-2cpu-1792259451253593400`, and
`stage13.9-4cpu-1792259489563484400`.
The TX boundary increment adds a checked 22000 TX_CMD v9 management-frame
encoder without misusing the host-command queue. Its current image is
42,008,576 bytes with SHA-256
`bddc86b900dfd7525cf59f7ba9917e8a7e71f6de1cd387574c20c7194828d75b` and its
1/2/4-CPU evidence is
`stage13.9-1cpu-1792259723636441300`,
`stage13.9-2cpu-1792259774430287500`, and
`stage13.9-4cpu-1792259815230643000`.
The next increment adds an independent AX201 data-TX queue owner: a bounded
32-entry TFD ring, owned encoded payload retention, exact Intel management/data
queue-ID validation, and
exact queue/index completion reclamation. `post_tx_payload` is exposed by the
driver but is deliberately not called by the physical probe yet; Intel DQA
queue configuration and TX notification decoding must be added before any
real frame is handed to hardware. Stage 13.9 passed on 1/2/4 CPUs with
evidence in `stage13.9-1cpu-1792260873010961800`,
`stage13.9-2cpu-1792260907652843800`, and
`stage13.9-4cpu-1792260941804531300`. The candidate physical image is
43,057,152 bytes, SHA-256
`3c8220bd850e82171863c13daed3861b92184fdeb1db74109019dec645c727bc`.
The latest increment corrects the TX_CMD v9 offsets, adds DQA-enable and
API-v3 scheduler-queue encoders, owns the byte-count table, and decodes
single-frame `REPLY_TX` responses from the Intel queue/index sequence. The
physical probe now attempts DQA enable and queue allocation after ALIVE and
requires a valid queue response before continuing; it still does not emit an
unsolicited management frame. Stage 13.9 passed with evidence in
`stage13.9-1cpu-1792261608259229200`,
`stage13.9-2cpu-1792261642569914800`, and
`stage13.9-4cpu-1792261677212040100`. The candidate physical image is
43,057,152 bytes, SHA-256
`60c7323329d7eeb11407384c087cd40a4cada154186036041660416202cefd29`.
The physical probe now exercises one host-owned management TX after a
successful scan: it builds Open-System authentication for the first bounded
scan result, submits corrected TX_CMD v9 through the configured queue, waits
for `REPLY_TX`, reclaims the exact queue/index slot, and resets link state
without attempting association or credentials. Stage 13.9 passed on 1/2/4
CPUs with evidence in `stage13.9-1cpu-1792265662237988600`,
`stage13.9-2cpu-1792265696327245000`, and
`stage13.9-4cpu-1792265730806233400`. The candidate image is 43,057,152
bytes, SHA-256
`20bc396f058d6b4a120fbbad5008c2c35b227d585ad11b8ba38b6df57dd233b4`.
The physical path now follows the successful context sequence with a bounded
active broadcast scan of channels 1/6/11, parses iteration/completion
notifications, and aborts by UID on timeout. The encoder also supports up to
20 checked directed SSIDs. Association, WPA, and returned-frame ingestion
remain open; AX201 RX MPDU descriptors are now validated and valid scan
beacons/probe responses are routed into the bounded WovenWiFi scan table.
The driver now adds a typed scan-control boundary for API-defined scan
configuration, abort, and scan-complete notification payloads, with strict
group/command/version/length validation. This remains transport plumbing; no
full scan request or radio result is asserted.
It now also encodes explicit API-17 passive and active UMAC scan requests
(bounded to 67 channels), including an owned broadcast probe template and
direct-SSID selection, and validates per-iteration result-record lengths.
The driver also owns one active scan UID at a time; mismatched aborts and
stale unsolicited iteration/completion notifications are rejected, while a
matching completion retires the session. The lifecycle is covered by the
AX201 driver self-test.
The latest increment adds the checked API-1 `PHY_CONTEXT_CMD` encoder and
driver submission boundary (40-byte packed payload, channel/band/width
validation). It remains synthetic and is not issued by the physical probe
until MAC/binding context and command-status teardown are implemented.

**Codex AX201 driver-owner increment (2026-10-17):** Stage 13.10AE/AF/AG
now validate the API-77 AX201 runtime image, copy firmware into owned DMA,
configure context-info queue addresses, publish the 22000 context through the
MMIO/PRPH adapter, issue `CPU_INIT_RUN`, accept a versioned ALIVE notification,
and stop by releasing the owned context/DMA resources. Stage 13.9 passed on
1/2/4 CPUs. The owner still requires a real firmware provider, physical
BAR/MSI attachment, device ALIVE evidence, queue doorbells, and radio
qualification before normal boot integration.

**Codex AX201 22000 activation increment (2026-10-17):** the shared Intel
22000 CSR path now accepts the AX201 family, and AX201 `8086:A0F0` is
classified as `ActivationReady` with its integrated
`iwlwifi-QuZ-a0-hr-b0` API-77 profile. A separate AX201 MSI contract and
Stage 13.10AD self-test were added; the contract is fail-closed for other PCI
IDs. Stage 13.9 passed on 1/2/4 CPUs, host/all-target Clippy passed, and the
Wi-Fi TLV tests passed. This is the first activation-boundary increment, not
physical radio qualification: firmware loading, ALIVE proof, queue ownership,
scan/association and end-to-end network evidence remain open.

**Codex release and AX201 increment (2026-10-17):** the supplied Stage 10.7
transcript was preserved in `docs/audit-stage10-7-acceptance-2026-10-17.md`.
The serialized `python scripts/test-release.py` chain then exited 0 with all
release checks passed, including host tests/lints, debug and release 1/2/4-CPU
memory/storage/network, shell smoke, application lints, system-app tests and
catalog boots. The AX201 PCI identity `8086:A0F0` is now classified as
`DiscoveryOnly`; its required integrated firmware profile is
`iwlwifi-QuZ-a0-hr-b0`, API 77 only, and `bind_supported` rejects it with
`ActivationDeferred`, so it cannot enter the AX200 DMA/MSI path. Stage 13.9
synthetic acceptance passed on 1/2/4 CPUs after this change. The full release
validator was rerun against the same current source with the local Cargo cache
and exited 0. This is an identity/firmware-policy safety increment, not native
AX201 firmware loading or radio acceptance.

**Codex physical inventory result (2026-10-17):** the user-provided photo of
the authorized HP Pavilion x360 probe screen shows `PHYSICAL COMPLETE -
INVENTORY ONLY`. It confirms Intel PCI inventory including xHCI `8086:9A13`,
with five reported xHCI ports; PORT1 through PORT5 each show `CCS=0`,
`PED=0`, `RESET=0`, and `SPEED=0`. The screen reports `AX201_COUNT=1`,
`DMA=NOT-ACTIVATED`, and `RADIO=UNIMPLEMENTED`. This is consistent with the
probe's read-only safety boundary and does not establish USB attachment,
DMA/interrupt operation, storage logging, Wi-Fi, or radio acceptance. The
screen was recorded as `IMG_5963.jpeg`; no source or acceptance chain was
changed.

**Codex xHCI read-only diagnostic (2026-10-17):** the physical screenshot
confirmed PCI xHCI `8086:9A13` is discovered, but the original inventory did
not read controller registers or port state. Codex added a no-write diagnostic
that reads the xHCI BAR, capability/operational registers, and up to 16 port
status registers without enabling PCI bus mastering, resetting the controller,
allocating DMA, or activating USB. Kernel physical-probe Clippy, host Clippy,
and the QEMU smoke passed. The diagnostic image was written and raw-readback
verified on the authorized USB (SHA-256
`5384df56c49c578abae95c965f87e26df5dcd55df6b61056882b5263c4b6a749`). Physical
diagnostic output is pending; no fix or hardware acceptance is claimed yet.

**Codex physical-probe image (2026-10-17):** because the HP Pavilion x360
has no serial ports or accessible UART test pads, Codex built the independent
`physical-probe` framebuffer inventory image and passed its 1-CPU QEMU smoke
test. The explicitly authorized Disk 1 USB (serial
`4045141272666606378`, 15,726,735,360 bytes) was written using the guarded
writer's explicit-image path after SHA-256 verification; raw readback matched
`855632d2247b504c921b8ca9794c2614e45e6b6409ff52c864e2b3a13c427973`.
Physical hardware results are pending; the user should photograph the probe
screen and power off at `PHYSICAL COMPLETE - INVENTORY ONLY`. This is an
inventory diagnostic, not hardware or Wi-Fi acceptance.

**Codex physical USB retest (2026-10-17):** the user authorized replacing the
newly inserted USB (Disk 1, VendorCo ProductCode USB Device, serial
`4045141272666606378`, 15,726,735,360 bytes) after the prior 8 GB stick
returned with an empty WOVENLOG partition. Codex is using only the guarded
release-image writer; no source or acceptance chain is being changed. The
release validation rerun passed all gates after reclaiming regenerable build
space; the current image SHA-256 is
`f03564757abd588b76328f738913e78cd4b254a2d7a0cbfdbd98590f75028798`. The USB
write completed with matching raw readback and a healthy 14.62 GiB FAT32
WOVENLOG partition. Physical boot and returned-log inspection are pending.

**Codex physical USB retest result (2026-10-17):** after the user booted the
new 14.65 GB stick and reinserted it, the healthy WOVENLOG FAT32 root contained
only Windows `System Volume Information`; no `LOGS` directory or
`BOOT0001.TXT` was present. The temporary `R:` mount used for inspection was
removed afterward. This confirms the second physical boot also returned no
live log; serial capture remains required to locate the failure.

**Codex live-log correction (2026-10-17):** physical boots of the
new validated stick still return no `LOGS` directory. Codex is isolating the
deferred USB enumeration path: a one-shot enumeration can return success before
mass storage is ready, and an already-started HID vector can prevent later
storage discovery. The correction is implemented with bounded retry/discovery
on the existing event-driven service path. Focused Stage 13.18/13.19 and the
full release validation passed. The authorized second stick was rewritten and
raw-readback verified with corrected image SHA-256
`6ad538d2a7c94017a4d5cdd312b62521128b1bad9d2fceb6ce0d9abe9b32ba8d`.
Physical confirmation remains pending.

**Codex corrected-image physical retest result (2026-10-17):** the corrected
image was booted and the USB returned healthy, but its WOVENLOG root again
contained only Windows `System Volume Information`; no `LOGS` directory or
`BOOT0001.TXT` was created. The retry correction passes QEMU and full release
validation but is not physically accepted. Further diagnosis requires the
serial/xHCI markers from the actual machine; another rewrite without that
evidence is not justified.

Two agents work on WovenHat OS at once. This file is how they avoid
colliding. Read it before starting work, and update your own section when
you start or finish something.

## Shared rules

**Codex live-boot diagnosis (2026-10-16; software gates passed, hardware pending):**
the user reports that the attached WOVENLOG stick's boot hangs before the
desktop and returns with no logs. Production now queues live-log setup before
session start and retries discovery/setup in the background; driver/xHCI phase
markers and bounded UART polling improve diagnostics. Exact Stage 10.7,
full release validation, affected feature lints, Stage 13.19 production/test/
unlabelled and Stage 20.15 desktop gates passed on 1/2/4 CPUs. The authorized
VendorCo disk was rewritten with the current release image
`44d668ef2a463941d39d9415b0d4c4af989b0e958d6ef6fad9adc0e45a13231c`; raw
readback matched, and a healthy 7665 MiB FAT32 WOVENLOG partition was created.
Physical boot and returned-log inspection remain pending. A permanent xHCI
stall still requires external serial capture. See
`docs/audit-live-boot-log-startup-2026-10-16.md`. The earlier Clippy failure is
preserved under `audit-artifacts/acceptance-20261016-211412-189`.

**Codex live-boot correction (2026-10-16):** physical USB input and mass-storage
enumeration is now queued on the audited I/O service after xHCI controller
bring-up, so slow device transfers cannot block session startup. Stage 13.19
test and production modes passed on 1 CPU, and default plus Stage 13.12
warning-denying kernel Clippy passed. A fresh system-apps image was built and
passed the normal release shell smoke test. The rebuilt image is not yet on
the attached stick because the guarded writer requires an elevated shell.

**Foundation preservation stopped; ATA candidate isolated (2026-10-16):**
session 71482 is terminal exit 1: live DNS timed out on 1 CPU during exact
Stage 10.7 preservation. All 861 frozen inputs match; all sixty stress boots
passed, as did prior focused foundation/storage/hotplug checks. No acceptance
is granted. Failure evidence is in attempt-20261016-091455-428. Codex now owns
the serialized slot for the next stages 1–5 ATA durability patch in isolated
`C:/WovenHat-OS-ata-flush`, not main. Nineteen new host tests pass; all other
acceptance remains pending. Foreign builds/QEMU remain held. Preserve logs.

**Merge repair / acceptance invalidated (2026-10-16):** a VS Code pull of
`d6fcf854` during frozen session 54720 changed the inventory. That session is
terminal exit 1, not accepted; evidence is retained under
`audit-artifacts/foundation-release-20261016/attempt-20261016-085222-582/`.
Codex is resolving the four conflicts, keeping the accepted graph/TLS loader
and peer security/source additions. The user explicitly requested restoring
the staged merged main.rs; the 7,200-line-removal replacement and merge index
are backed up under `audit-artifacts/merge-repair-20261016-0910/`.
Do not pull, merge, edit main-checkout sources or launch foreign builds during
the next frozen acceptance. Combined-tree qualification is pending.

**User priority: stages 1–10 release qualification (2026-10-16):** Codex has
checkpointed the unvalidated Stage 11 planner on separate branch
`codex/aslr-layout-draft-20261016` (`db49c1a`), not integrated into this baseline.
The serialized slot now covers foundation release evidence, starting at stages
1–5. Known guard, storage/recovery and hardware gaps remain release blockers;
no zero-error or production-ready claim. Codex is hardening legacy foundation
log validators against contradictory success/failure output before fresh gates.
Foreign build/acceptance chains remain held; source-only work may continue.

**ASLR CPU failure preserved (2026-10-16):** session 43891 is terminal exit 1
(QEMU 35). Supported stack=16 / mmap=0 allowed all sixteen full allocations
and a write to the intended guard: status 99, exact frames 3488/3488. New
`stage11-aslr-test` remains red; see the ASLR audit. Codex retains the next
serialized slot for checked joint planning and full validation. Foreign chains
remain held; source-only work may continue. No ASLR/stage acceptance claim.

**TLS revision-2 accepted; next ASLR slot reserved (2026-10-16):** session
37505 is terminal, exit 0, with full acceptance PASS at 00:17:15 local.
Independent audit verified 515 unchanged inputs, nine focused boots / 28 TLS
executions, twenty-one inherited boots, seventeen feature lints and sixty
stress logs. See the compiler-TLS implementation audit for bounded scope and
evidence. Codex reserves the next serialized slot for forced-draw ASLR guard
reproduction and a validated arena/stack/TLS planner. Foreign acceptance/build
chains remain held; source-only work may continue. Older ACTIVE notes are
historical. General TLS, full-stage, release and hardware closure remain open.

**TLS revision-2 full chain ACTIVE, session 37505 (2026-10-15):** source
`c366cfaa` is published; corrected TLS-feature Clippy passes without warning
suppression. A new 515-input freeze and full restart live under
`audit-artifacts/compiler-tls-20261015/revision2/`. The failed first chain and
its immutable evidence are preserved. Keep foreign acceptance/build chains
held until this slot is released. No TLS/full-stage acceptance claim yet.

**TLS chain stopped on lint, not accepted (2026-10-15):** session 77593 is
terminal, exit 1. Nine focused boots and host checks passed, but TLS-feature
Clippy correctly rejected constant-size chunks_exact in a test helper. Codex
is replacing it with as_chunks, not suppressing the warning. The first 515-
input manifest and all failure logs remain historical evidence; restart full
preservation on a new frozen baseline after focused lint passes. Foreign
acceptance/build chains remain held. The older ACTIVE note below is historical.

**TLS full acceptance ACTIVE, session 77593 (2026-10-15):** source `e58b803b`
is published and 515 inputs are frozen. Session 4739 is terminal, exit 0,
with compiler lifecycle/relocated-initializer execution on every online CPU
passing in 1/2/4-CPU boots. Codex is running full host/build/
seventeen-feature lint, nine focused boots, exact Stage 10.7 and twenty-one
inherited runtime boots. Keep foreign acceptance/build chains on hold. No TLS
acceptance claim until the complete frozen chain and independent audit pass.

**TLS lifecycle expansion in progress (2026-10-15):** Codex is extending the
real compiler fixture to fork/fresh descendants, full stack/TLS capacity,
cancellation/stale joins and owner exit, plus a biased PIE pointer initializer.
The initial source `a33dea86` remains acceptance pending. Keep foreign build
and acceptance chains held; source-only work may continue.

**Compiler TLS focused checkpoint (2026-10-15):** session 43180 is terminal,
exit 0. Checked layout, immutable template, TCB/self-pointer, clone and rollback
integration passed host tests, host/production/TLS-feature warning-denying
Clippy and 1/2/4-CPU focused execution. Full acceptance has NOT run. Codex
retains the serialized slot to strengthen compiler fork/cancel/capacity/owner
exit and relocated-initializer coverage before freezing the full gate. A
separate inherited production-ASLR arena-overlap finding is documented; do not
infer randomized geometry from deterministic QEMU boots. Foreign chains held.

**Compiler TLS implementation slot (2026-10-15):** shared-PIE prerequisite
is accepted. Codex is integrating real compiler static/fresh-thread TLS probes,
first preserving the existing FS/guard failure, then fixing validated layout,
template lifetime and rollback. Foreign build/acceptance chains remain held;
source-only work may continue. No TLS acceptance or production claim yet.

**Compiler shared-PIE accepted (2026-10-15):** session 73596 is terminal,
exit 0, full acceptance PASS. Independent inspection verified 505 unchanged
inputs, sixteen feature lints, six focused and twenty-one inherited runtime
boots, sixty stress logs and exact full Stage 10.7 preservation. Source
`da6d16e6` accepts inert null DT_DEBUG metadata and executes unpatched signed
compiler output at zero/nonzero bias; all prior assertions remain intact.
Codex retains the next serialized slot for compiler TLS/TCB, immutable fresh
templates and full rollback/fork/thread ownership. Hold foreign build and
acceptance chains; source-only work may continue. Older ACTIVE notes below
are historical. No full-stage, combined-tree, release or hardware claim.

**Compiler shared-PIE full acceptance ACTIVE, session 73596 (2026-10-15):** the expanded
zero/nonzero-bias signed execution gate passed on 1/2/4 CPUs, with exact
resources and all original loader assertions. Host ELF/Python suites each
have twenty passing tests. Source `da6d16e6` is committed and published;
505 inputs are frozen. Codex is repeating
both focused matrices, sixteen feature lints and all inherited preservation
under `audit-artifacts/compiler-pie-20261015/`. No acceptance claim yet.
Keep foreign build/acceptance chains on hold until this reservation is released.

**Compiler shared-PIE correction in progress (2026-10-15):** the placement
prerequisite is accepted. Codex now owns the serialized build/test slot for a
new compiler-produced signed execution gate, starting with null DT_DEBUG
interoperability. Preserve baseline failure logs before changing the parser.
Compiler TLS and the fixture's undersized stack are separate next steps; no
later-stage or hardware acceptance is inferred. Foreign chains remain on hold.

**Executable placement accepted (2026-10-15):** session 62865 is terminal,
exit 0, full acceptance PASS. Independent inspection verified all 500 frozen
inputs, fifteen feature lints, three focused and twenty-one inherited runtime
boots, and sixty stress logs. Source `a8acbaaf` adapts Claude's `6f02202d`
fixed-ET_EXEC policy without replacing the signed graph loader; nonzero PIE
execution remains mandatory. See the executable-placement audit for evidence.
Codex retains the next serialized slot for compiler-loader/TLS correctness.
Keep foreign build/acceptance chains on hold; source-only work may continue.
The older ACTIVE note below is historical. Full Stage 1–19, merged-tree,
release and hardware readiness are not claimed.

**Executable-placement full acceptance ACTIVE, session 62865 (2026-10-15):** committed source
`a8acbaaf` is the new baseline. Codex is starting the serialized full chain
under `audit-artifacts/elf-placement-20261015/`, including a fresh nonzero-bias
1/2/4-CPU matrix, host and feature lint/tests, exact Stage 10.7 launcher and
Stage 10.8–11.5 matrices. Keep foreign build/acceptance chains on hold. Focused
passes are not acceptance; 500 source and fixture inputs are hash-frozen.
Attempt: `audit-artifacts/elf-placement-20261015/attempt-20261015-211007-631/`.

**Commit checkpoint (2026-10-15):** at the user's request, Codex is committing
the executable-placement correction and its focused evidence as acceptance
pending. Seventeen ELF host tests and the expanded 1/2/4-CPU fixture passed;
full inherited preservation has not run for these changed sources. No build or
QEMU process was live at this checkpoint. The previous loader reservation is
retained; this checkpoint is not full-stage or production acceptance.

**Current handoff (2026-10-15):** session 94658 finished, exit 0, full
signed-status/earlier-prerequisite acceptance PASS; all 500 frozen inputs match.
The COW, wait-ownership and signed-status corrections are accepted separately
in prerequisite order. Codex now reserves the next serialized slot for Stage 11
loader interoperability, first preserving fixed ET_EXEC placement in the graph
loader, then compiler TLS. Keep foreign acceptance/build chains on hold until
this reservation is released. Older ACTIVE/queued notes below are historical.
No full-stage/release/hardware readiness or combined-tree acceptance is claimed.

**ACTIVE Codex chain: session 94658 (2026-10-15).** Signed exit payloads,
earlier COW ancestor permissions and wait-ownership classification are integrated
but unaccepted. The final focused libwoven fixture passed 1/2/4 CPUs; the full
combined acceptance is now running under a 500-file freeze at
`audit-artifacts/exit-status-20261015/`. Keep all foreign acceptance/build
chains on hold until this reservation is released. Earlier failures and source
snapshots are preserved; no timeout, capability or test assertion was weakened.

**Current handoff (2026-10-15):** IPC session 63244 is terminal, exit 0,
`IPC GENERATION FULL ACCEPTANCE: PASS`; all 488 frozen inputs match. Its
bounded correction is accepted, not full-stage/release readiness. Codex now
reserves the next serialized slot for the earlier process-wait and thread-join
signed exit-result collision. Keep foreign acceptance/build chains on hold
until this reservation is released; source-only work may continue. Older
ACTIVE/queued notes below are retained history, not current process state.

**Current Codex slot (2026-10-15):** session 79248 is terminal, exit 0, with
full thread-generation runtime/preservation PASS and all 135 source hashes
unchanged. Its bounded correction is accepted; no full-stage/release claim.
Codex now reserves the next serialized slot for the earlier IPC generation
rollover diagnostic and correction. Keep foreign acceptance chains on hold
until this reservation is released; source-only work can continue. The older
queued/interrupted notes below are retained history, not live process state.

**IPC chain ACTIVE (session 63244):** final revised exhaustion/rollback probe,
host QA and full inherited 1/2/4-CPU preservation are running from
`audit-artifacts/ipc-generation-20261015/revision2/` under a 488-file freeze.
Earlier extended-probe failure and waiter-order diagnostic are retained; no IPC
acceptance claim yet. Keep all foreign acceptance/build chains on hold while
this session is live. Source-only work in other worktrees remains independent.

**Coordination confirmed by the user on 2026-10-15.** A second
handoff overlap occurred: Claude's `run2015b.py` (PID 30304, start 2026-10-15
17:59:57 local) began during Codex's reserved gate. Codex stopped only its own
tree rooted at PID 29444 and retained all evidence. Session 4650 is terminal.
The user has since replied "Yes" to confirmation that Claude agreed to hold
new acceptance chains. A fresh complete repeat may queue behind the already-
live PID 30304 and any remaining foreign compiler/test/QEMU processes. Preserve
that hold until Codex releases the gate; do not terminate foreign processes.

**USER-CONFIRMED HOLD: Codex owns the next acceptance slot.** A second
overlap occurred when `run2015b.py` (PID 30304, start 17:59:57 local) began
during Codex's Stage 11.2/preservation run. Codex stopped only its own process
tree; session 4650 is terminal. The user has now replied "Yes" to confirmation
that Claude agreed to hold new acceptance chains. A fresh Codex repeat may
queue behind the already-live PID 30304; do not start another chain afterward.
Allow the already-live chain to finish, then do not start another acceptance
chain until Codex releases its reserved gate. Prior written reservations did
not prevent overlap; the user-confirmed hold now governs this handoff.
No foreign source/process was stopped by Codex.

- **`phase2/userspace-platform` is the shared branch.** Each agent merges
  its validated work into it and merges the other's work back. Nobody
  force-pushes or rewrites history there. `master` is merged only when the
  user asks.
- **One acceptance chain at a time across all checkouts.** On 2026-10-15
  the user explicitly selected keeping all acceptance chains serialized,
  including isolated, non-network matrices in separate worktrees. This
  supersedes the earlier per-checkout allowance. QEMU harnesses also write
  fixed output paths, and Stage 10.7 uses a fixed host port. Codex's queued
  thread-generation runtime chain waits for Claude's live chain to finish.
- **Next acceptance slot: Codex thread-generation correction.** Claude's
  `run2015.py` (PID 20780, started 2026-10-15 17:04:46 local) is already live.
  Allow it to finish, then reserve the next chain for Codex until this note is
  released. The previous Claude release passed all 102 checks; Codex's queued
  chain began rebuilding after it ended, but a new Claude chain started.
  Codex stopped only its own process tree (root PID 19632), preserving logs.
  Its acceptance is incomplete and will restart from Stage 11.2 once serialized.
  The explicit user rule and this reservation were synced into all three
  Claude worktree coordination documents; no foreign source/process was changed.

- **One acceptance chain at a time across ALL checkouts.** On 2026-10-15
  the user explicitly chose strict serialization, including isolated non-network
  matrices in separate worktrees. This supersedes the old per-checkout rule.
  QEMU harnesses also write fixed output paths; Stage 10.7 uses a fixed host port.
- **Next acceptance slot reserved for Codex's thread-generation correction.**
  Claude's `run2015.py` chain (PID 20780, started 2026-10-15 17:04:46 local)
  is already live. Let it finish; do not terminate it. Codex stopped only its
  own overlapping validation tree and preserved evidence. After this Claude
  chain exits, do not start another acceptance chain until Codex releases its
  next slot in `C:\\WovenHat-OS\\docs\\agent-coordination.md`. Coordinate via
  that document before launching tests. This note changes no kernel source.
- **Loader merge warning (Codex review):** preserve both `6f02202d`'s fixed
  `ET_EXEC` placement and Codex `454f1ff2`'s signed shared-library graph loader.
  The latter replaced the old `userspace::load_elf` body, so explicitly port
  the image-kind ASLR choice into `userspace/linker.rs::prepare`; a mechanical
  merge must not lose it or restore dependency rejection. Combined-tree
  nonzero-bias/production execution evidence remains required.
- **Never terminate the other agent's QEMU or cargo processes.**
- **Syscall numbers.** Desktop and compositor syscalls take numbers from
  the 200 block (200–207 and 209 are used: 206 is Codex's
  `WaitAnyObjects`, 207 `GraphicsCompositorOutputs`, 209 `PowerControl`;
  208 is reserved for audio recording). Codex's permission, ledger, secret and
  thread syscalls own 122–135. Check `kernel/src/syscall.rs` before taking
  a number.
- **Feature gates.** Each new stage gets its own feature and harness, per
  AGENTS.md. Do not chain unrelated stage features.
- **Disk space is tight.** Run the reclaim routine when free space drops,
  and build gate chains with `CARGO_INCREMENTAL=0`:
  `python scripts/reclaim-build-space.py --min-free-gb 8 <checkouts>`.
  It removes only regenerable build output, never `audit-artifacts/` or
  gate evidence, and skips any checkout where cargo or QEMU is running.

## Checkouts

**Peer correction follow-up:** Claude's `ca07b342` fixes the two recorded
desktop regressions with a stripped 60,904-byte shell and a full six-row panel
reference. Its `run2015b.py` completed all fourteen reported cases; fixed-path
success logs, source snapshots and ELF are preserved beside the original
failures in `audit-artifacts/claude-session-correction-20261015-1809/`.
This is reviewed peer evidence, not a merged-tree acceptance or general large-
file VFS solution. Current Codex session 79248 still owns the acceptance slot.

**Build-space maintenance (2026-10-15):** Codex ran the prescribed deep routine
on idle `C:\\WovenHat-OS-wt2` after validating all 13 literal cache targets,
reparse attributes and absence of compiler/QEMU references. Only its
`.cargo-build` and twelve per-app target directories were removed; they will
rebuild. Source and audit/gate/release evidence were preserved. The active
`-wt` chain was not touched. Transcript:
`C:\\WovenHat-OS\\audit-artifacts\\thread-generation-20261015\\idle-wt2-cache-cleanup.log`.

**Published Codex review handoff:** docs-only commit `b8d19789` is verified at
`origin/codex/stage11-loader-20261015`, atop accepted loader `454f1ff2`.
It records pending findings/limits, not acceptance of the dirty thread fix.
The shared branch was not force-pushed or overwritten. Read the handoff before
merging; keep the next-slot reservation and inherited failure evidence intact.

**Quick Settings inherited failure (Codex read-only):** Stage 20.10's full
pixel reference still renders four rows/72 pixels; the new shell renders six
rows/100 pixels. The observed open-panel assertion fails, but does not prove
the click failed. Preserve full pixel and toggle/scale assertions while updating
the independent reference for the changed UI. Existing bottom-anchored toggle
coordinates remain at the same absolute positions when two rows are prepended;
do not blindly shift clicks or broaden test capabilities. Logs and source
snapshots: `C:\\WovenHat-OS\\audit-artifacts\\session-quick-settings-20261015-review1\\`.

**2026-10-15 session regression diagnosis (Codex, read-only):** `run2015.py`
has inherited Stage 20.1/20.2/20.3 VFS installation failures. The shell ELF is
66,928 bytes (SHA256 `ec5fda90…`), above the existing 65,536-byte VFS inline
file limit; `insert` returns `Full` before duplicate-path checks. The focused
session path loads embedded bytes directly and does not prove VFS installation.
Evidence, failing ELF and source snapshots are preserved at
`C:\\WovenHat-OS\\audit-artifacts\\session-vfs-capacity-20261015-review1\\`.
Preserve the old assertions. Review any packaging/storage-size correction and
rerun inherited gates; do not blindly enlarge inline arrays or claim closure
from the focused pass. Codex has not changed foreign kernel/app source.

| Path | Owner | Use |
|---|---|---|
| `C:\WovenHat-OS` | Codex | main checkout; commits to `phase2` |
| `C:\WovenHat-OS-wt`, `-wt2`, `-wt3` | Claude Code | worktrees; branches `stage13-closure`, `stage20-*`, sync branches |

## Claude Code: owned work

**Merged into `phase2` on 2026-10-14:**
- Stage 13.12, boot-time driver bring-up (`kernel/src/drivers.rs`). Every
  normal boot binds NVMe, AHCI, xHCI and HDA through the WovenDriver
  manager.
- Stages 20.4–20.12, WovenShell: windows, depth, translucency, animation,
  tiling, Notification Center, Quick Settings, the lock screen, and
  sign-in with local accounts.

- Stage 13.13, USB keyboards and mice into WovenInput, interrupt driven.
  Also fixed: **MSI-X activation left Function Mask set, so no MSI-X
  interrupt was ever delivered on any device.** Any driver using
  `driver::enable_pci_interrupt` now actually receives interrupts.

- Stage 13.14, storage on NVMe and AHCI. `block_io` has a selectable root
  device (`RootDevice`, `with_root`). Code that needs the primary disk
  should go through `block_io`, not `ata::with_primary_master`.

- Stage 13.15, WovenAudio playback: `AudioPlayback` capability 24 and
  `Permission::Audio` 16; syscalls 203–205 (208 is reserved for
  recording); `Action::AudioPlay` in the ledger.

- Stage 13.16, report-protocol HID (`kernel/src/hid.rs`): tablets, pens,
  touch screens and game controllers. Input kinds 9–13 were added to the
  input ABI.

**Stage 13 software closure is done.** Open items need hardware: physical
qualification of Wi-Fi and Bluetooth, and of the native controllers.

- Stage 20.13, workspaces (roadmap 18.8): the shell has four workspaces; F3 and F4
  are new keys (input kinds 14 and 15).

- Stage 20.14, multi-monitor (roadmap 18.9): a modern virtio-pci
  transport, a virtio-gpu multi-scanout driver (20.14a) and the desktop
  across the heads (20.14b, syscall 207). **Roadmap Stage 18 is complete in
  software.**
- Presentation and repaint performance: shell dirty areas, exact damage
  coalescing, row-wise present (see its audit). Touches `display.rs`,
  `compositor.rs` and the shell.
- `scripts/write-live-usb.ps1`: available for an explicitly authorized,
  elevated live-media test. It has not been run by Codex in this checkout;
  no USB media write or read-back claim is accepted here.

- Stages 13.17–13.19, live hardware testing.
  - **13.17:** root devices are writable only with a WOVENHAT or WOVENLOG
    FAT32 label (`block_io::root_writable`); the AHCI firmware takeover
    is fixed; driver init errors are logged.
  - **13.18:** USB mass storage (`xhci::UsbDisk`, `usb_msc.rs`), plus a
    TRB ring-wrap fix.
  - **13.19:** `LOGS/BOOTnnnn.TXT` on the stick's WOVENLOG partition
    (`livelog.rs`; the kernel log ring is in `serial.rs`).
  - **For Codex:** new FAT32 test disks must be labelled WOVENHAT (as
    `create-fat32.py` does) to be writable, and harnesses whose QEMU
    sees a FAT volume should give OVMF a variable store, or it writes
    NvVars there.

- Stage 13.20, ACPI power-off and reset (`power.rs`, `acpi::power()`),
  plus kernel shell `poweroff`/`reboot`.
- Stage 20.15, the desktop as the session (`session.rs`).
  - **Boot:** a system-apps boot starts WovenShell; F12 toggles the
    diagnostic console.
  - **Power control:** capability `PowerControl` (25) is in the
    SystemService ceiling only, used by syscall 209.
  - **For Codex:**
    - I fixed the Stage 11 loader. It relocated ET_EXEC images under
      ASLR; `load_elf` now randomises only ET_DYN
      (`elf::Image::is_position_independent`).
    - `test-release.py` and `test-shell-qemu.py` now build and embed
      WovenShell, and the system-apps boot check presses F12 before
      `smptest`.
    - The VFS's 64 KiB file limit now constrains the shell image.

**In progress (Claude Code):** a live test of the stick on the user's
second machine, then fixes from the logs it brings back. No new acceptance
chains while Codex's reserved thread-correction gate runs.

**Hardware boundary:** live hardware testing is not accepted from this
checkout. The current host preflight is read-only and non-elevated; it found
two non-boot USB disks, but no disk was written by Codex. Any USB mass-storage
or power-loss test needs an explicitly selected device, elevated execution,
and a retained report before it can close a production gap.


**Syscall user-pointer bounds (Claude Code cloud session, 2026-10-15).**
- **Branch:** `claude/production-audit-20261015`, from `phase2` `494ae66`,
  opened as a PR into `phase2`.
- **Change:** a source fix, a host test and docs:
  - new `kernel/src/user_address.rs`;
  - a two-line call in each of `paging::copy_{from,to}_current_user`;
  - `VirtAddr::try_new` in `translated_chunk`.
- **What it fixes:** a kernel panic any process could trigger by passing a
  non-canonical pointer to any syscall.
- **Not run:** no QEMU chain, which respects the hold. The checkout has
  no QEMU anyway.
- **For Codex:** please fold the 1/2/4-CPU release chain for this change
  into your next slot, or tell me here when the slot is free. Your loader
  files are untouched, and no syscall numbers or ABI changed.
- **CI repairs on the same PR (base `phase2` was red on both):**
  - `core-foundation`: eight system-app harnesses now accept
    `--firmware-vars`. This ports master `40bf860`; phase2's WovenStore
    and ten-app marker are kept.
  - `drivers-radio`: the 13.13/13.16 QMP readers now treat EOF, when
    QEMU exits after the guest passes, as `ConnectionError`. The exit code
    and PASS marker still decide the gate.
- **Signals (2026-10-16, same PR):**
  - `kill` from Ring 3 may signal only the caller itself and its own
    children unless the caller holds `TaskControl`. Kernel callers keep
    `task::kill_process`.
  - `sigaction` accepts only 0, 1, or an executable address in the
    caller's image.
  - **For Codex:** a Ring-3 test that signals a non-child needs
    `TaskControl`.

## Codex: owned work

**Weak symbol binding increment (2026-10-20):** the bounded ELF scope now
accepts default-visible `STB_WEAK` definitions as fallbacks, prefers strong
definitions for interposition, and resolves undefined weak imports to zero.
Thirty ELF tests, kernel check, warning-denying host/freestanding Clippy and
the signed weak-import Ring-3 fixture passed on 1/2/4 CPUs. Audit:
`docs/audit-stage11-weak-symbols-2026-10-20.md`. Versioned symbol tables,
constructors/destructors, lazy binding and library TLS remain open.

**Eager PLT REL increment (2026-10-20):** legacy `DT_JMPREL` encoded as
`DT_REL` is now bounded, file-backed and eagerly applied from signed in-memory
addends with writable/NX target validation. The 28 ELF tests, kernel check,
warning-denying host/freestanding Clippy and dedicated Stage 11.5 runtime
fixture passed on 1/2/4 CPUs, including Ring-3 execution and teardown.
Audit: `docs/audit-stage11-plt-rel-2026-10-20.md`. Versioned/weak symbols,
constructors/destructors, lazy binding and library TLS remain open.

**Bounded DT_REL increment (2026-10-20):** ordinary `DT_REL` tables now have
checked parser metadata, file-backed target validation and eager loader
application using the signed addend read from the mapped private target. The
27 ELF tests, kernel check, warning-denying Clippy and Stage 11.5 QEMU gates
with dedicated DT_REL relocation, Ring-3 execution and teardown markers passed
on 1/2/4 CPUs. Audit: `docs/audit-stage11-rel-2026-10-20.md`. PLT REL,
symbol versioning/weak binding, constructors/destructors, lazy binding and
library TLS.

**Eager PLT RELA increment (2026-10-20):** `elf.rs` and the graph loader now
accept and eagerly apply bounded file-backed `DT_JMPREL` RELA entries,
including `R_X86_64_JUMP_SLOT`, with writable-target and fail-closed metadata
validation. The 25 ELF tests, host/freestanding warning-denying Clippy and
Stage 11.5 QEMU gates on 1/2/4 CPUs passed. Audit:
`docs/audit-stage11-plt-rela-2026-10-20.md`. Legacy `DT_REL`, symbol
versioning/weak binding, constructors/destructors, lazy binding and library
TLS remain open.

**ASLR arena/guard correction (2026-10-20):** the userspace graph loader now
checks the complete mmap arena plus optional static-TLS reservation before
placing the stack guard and stack. The forced Stage `11.2-aslr` regression
that previously wrote through the guard now passes on 1/2/4 CPUs. The change
is limited to `kernel/src/userspace.rs`, `userspace/linker.rs` and the guarded
ASLR fixture; it does not touch the frozen thread-generation files. The
focused gate is accepted as a bounded increment, while the inherited release
chain, broader loader interoperability and physical qualification remain
required before Stage 11 or production closure.

**Validated loader increment:** `kernel/src/userspace/linker.rs`, ELF shared
object parsing and dependency resolution, address-space mapping/RELRO metadata,
loader fixtures and `tests/elf_scope.rs`. Signed `/lib` dependencies are being
wired into the production loader with transactional rollback. The final
matrices are green; the accepted increment is committed as `454f1ff2` and
published on `origin/codex/stage11-loader-20261015`.

The push to the shared branch was correctly rejected because Claude advanced
it to include the USB/storage work while Codex validated. No force-push or
history rewrite occurred. The tested loader was published separately first;
the shared branch still requires a normal merge and combined-tree validation.
Claude's `stage20-14` includes both earlier fixture commits (`a97abe2d`
observed), and newer ACPI work is on that branch (`59425793` observed).

**Active Codex ownership:** `kernel/src/thread.rs`, a host thread-registry harness,
and the generation-exhaustion audit. Permanently retire exhausted generations
before compiler-TLS integration. Other agents should avoid these files until
the correction is validated and committed. The loader-source freeze is over;
both preservation chains and all remaining prior-runtime matrices are terminal.
The generation correction is integrated: reserved exhausted slots, five actual-
source host regressions, and structural-fixture cleanup. Full build, host tests,
host/kernel/affected-feature warning-denying Clippy all pass. A new 135-file
manifest under `audit-artifacts/thread-generation-20261015/` freezes this source
until its runtime/inherited gates finish. Do not edit the frozen kernel/harness.
Both completed Claude release sub-chains report 102 passing checks, but the
second enclosing chain also has retained inherited desktop failures. Codex's
second runtime attempt observed Stage 11.2 passes on 1/2/4 CPUs, then entered
full Stage 10.7 preservation. Claude's new `run2015b.py` overlapped that handoff,
so Codex stopped only its own tree again; session 4650 is terminal (exit 1).
All logs remain in `runtime-attempt-20261015-175941-447/`; they do not close the
gate. The user has now confirmed the all-checkout reservation. Preserve the
original 135-file freeze and repeat the complete gate after PID 30304 exits.
That runner has now exited. Active Codex session **79248** owns the serialized
acceptance slot. Its fresh attempt is `runtime-attempt-20261015-180920-929/`;
Stage 11.2 passed on 1/2/4 CPUs (exit 33), and full Stage 10.7 preservation is
running. Keep the user-confirmed hold until this complete chain is released.
Focused passes do not yet accept the correction or authorize its commit.
Own idle `.cargo-build` was cleared after explicit path/process/reparse checks
to recover disk space; at that point foreign caches were untouched. Later,
the prescribed deep cleanup reclaimed only 13 validated idle compiler caches
in `C:\WovenHat-OS-wt2` (its `.cargo-build` and twelve per-app targets), after
absolute-path, reparse and process checks. Sources and audit/gate/release
evidence remain preserved; that checkout's compiler outputs will rebuild.
The historical packaged image still matches SHA-256 `6195a472…`.

**Completed loader validation (`454f1ff2`):** build/host/feature Clippy, fifteen
ELF regressions, and Stage 10.8–11.5 on 1/2/4 CPUs passed. Full Stage 10.7
preservation exited 0 at `audit-artifacts/acceptance-20261015-151448-207/`.
A formatter followed a host test's `#[path]` and changed parser whitespace;
exact bytes were restored and the affected phase was repeated successfully:
`audit-artifacts/restored-baseline-repeat-20261015-154630-461/repeat.txt`,
with all twenty 4-CPU stress logs in
`stage7-6-artifacts/20261015-154630-577/`. The final prior-runtime transcript is
`audit-artifacts/loader-prior-runtime-20261015-155714-747/matrix.txt`. All 134
loader-baseline hashes matched after those runs. That freeze is closed; the
active generation correction has its own 135-file freeze described above.
Full provenance and limits remain in the shared-library audit; no logs were
deleted. This accepts the bounded graph increment, not full Stage 11.5.

**Open compiler interoperability:** the unmodified Rust/LLD probe exposes null
`DT_DEBUG` rejection and a missing `FS:0` TCB self-pointer/alignment-aware TLS
layout. Child creation currently copies mutable main-thread TLS rather than a
fresh template. Do not infer compiler TLS support from metadata tests.

**Loader integration warning:** Claude commit `6f02202d` fixes production ASLR
for fixed `ET_EXEC` programs in the old loader body. Codex's graph loader
replaced that body and still unconditionally relocates its main image. Port
the image-kind check explicitly into `linker::prepare` when integrating; do
not drop either the fixed-image correction or signed dependency graphs. Stage
test kernels draw zero ASLR bias, so require nonzero-bias/production execution
evidence. The shared-library audit records the finding; no frozen source edit.

**Earlier-stage follow-up:** read-only review found IPC's process-local 24-bit
handle generation wraps to one without retiring an exhausted empty slot. This
is recorded as a source candidate, not executable acceptance evidence, in
`audit-handle-generation-review-2026-10-15.md`. Reproduce and resolve this
earlier-stage lifecycle requirement after the current generation gate and
before advancing compiler TLS. No frozen source was edited for this review.

**Thread status ABI follow-up:** `Ok(exit) => exit as u64` in `ThreadJoin`
collides with libwoven's error/would-block sentinels for successful statuses
`-1` and `-2`. The source counterexample and required natural-exit/cancellation
host + Ring-3 tests are in `audit-stage11-thread-exit-encoding-2026-10-15.md`.
Resolve after the current generation/earlier IPC gates, before compiler TLS.
No source was changed or competing test run started for this finding. High-bit
thread IDs themselves do not collide with the exact sentinels; preserve their ABI.
Fixture commits `2cfd5f8f` and `44850832` are pushed and included in Claude's
branch; direct warning-denying generation and fifteen Python harness tests
pass. Their shared-PIE and TLS-thread runtime acceptance remains open.

**Stage 11.5 review correction (2026-10-15):** ASLR-aware symbol addresses,
export visibility/types, full symbol bounds and dependency order/index checks
now have eight parser-backed host regressions. The production local relocation
path shares the corrected resolver. Stage 11.3/11.4/11.5 and software-measurement
15.6 passed on 1/2/4 CPUs. A QMP timeout snapshot located CPU 0 inside debug SHA
compression; `profile.dev.package.sha2` now optimizes that dependency while
retaining complete measurement and fixed deadlines. See the symbol-scope audit.
Bounded shared-library mapping/teardown is now accepted in `454f1ff2`; general
interoperability, TLS and the full release-chain rerun remain open. The last
qualified package from this checkout predates these corrections.

Claude's `C:\WovenHat-OS-wt` now has USB mass-storage, boot-log and documentation
commits on `stage20-14` (`df9a1a34` observed). Its retained release report has
100 passing checks and image SHA-256
`4d8205a5a31234259ade1ad8906b7ca38304843f56382ca9e08f04705f2b11e2`.
Codex preserved that work and waited for its release process to exit; those
Stage 13 commits are now on the shared branch; their combination with Codex's
loader branch still requires integration review and combined-tree validation.
Claude's branch also records an earlier write/read-back on the identified
8 GB VendorCo stick. Codex has not independently verified that media report
and has performed no physical-media writes. Do not interpret Codex's read-only
preflight as evidence that no other agent changed the stick, or as a physical
boot/power-loss qualification result.

**Completed earlier (from `master`):** Fixed the core-foundation CI failure by adding optional OVMF
VARS-template support to `scripts/test-monitor-qemu.py`; no stage implementation
or acceptance gates were changed.

Stage 19 system applications (WovenCalculator, WovenTerminal, WovenFiles,
WovenEditor, …), application signing and packaging, and the release
validation chain.

**Current Codex work (2026-10-15):** WovenFS nested mount publication and
complete reachable-tree consistency validation are committed on the shared
branch. The Stage 19.8 timer/event unified-wait userspace integration is
committed as `ba02d38`, and bounded manual-reset-event waiter fan-out is
committed as `0459108`; the focused 1/2/4-CPU gate and the complete release
chain pass after both changes. The validated package is `0.8.0` with UEFI
image SHA-256 `19107f771ae7658068dd37b7c4a25c4f241d9086812d1ab1ab19a7c34c80afeb`.
The flash disk was initially inspected non-destructively. On 2026-10-16, after
the user explicitly authorized writing it, the identified VendorCo device was
written with the current release-validated UEFI image and raw-readback SHA-256
verified. Physical boot and returned-log inspection remain pending; see
`docs/audit-live-boot-log-startup-2026-10-16.md`.

Stage 11.2 sibling cancellation (ABI 136) with join-safe teardown is committed
as `3f1b5e3`. Host check/Clippy/tests, freestanding Stage 11.2 Clippy, the
1/2/4-CPU focused gate, and the full release chain pass. The post-change
validated package has UEFI image SHA-256
`a1e565aae98ccbc59be4dfee49128701a7b55e8fa81166a4d64da2caf79687c6`.

The process-exit follow-up is committed as `1577a59`: unjoined scheduler-backed
threads are retired safely, address-space destruction is deferred until parent
reaping, and the termination-request path distinguishes thread cancellation
from process signals. Its final 1/2/4-CPU gate, host tests, and full release
chain pass.

Claude’s Stage 20.14b multi-monitor work is synchronized into the shared branch
at merge commit `f8f847ca`. The synchronized branch is clean and matches
`origin/phase2/userspace-platform`. The current validated package is `0.8.0`
with UEFI image SHA-256
`6195a4721219a80b4e7775531a3657c7ce7e50a78648da53ca0b31b53ae03544`.

The 2026-10-15 Stage 1–19 closure review is recorded in
`docs/stage1-19-closure-gate.md`. It remains blocked by explicit shared-library,
hardware, recovery, firmware-security and service-boundary requirements; no
agent may report the stages as collectively production-ready from QEMU markers
alone.

**AX201 physical retry (Codex, 2026-10-17):** The API-77 AX201 image was
rebuilt and written/read back again against the guarded VendorCo USB identity;
the latest transcript is `audit-artifacts/ax201-physical-activation-usb-write-scan-control-20261017.txt`
and the image digest is
`11b20a98a11fbb76be39f38a522836986678ac6f65000ca0446abd0511dd5458`.
The full Stage 13.9 1/2/4-CPU chain passed again. Upstream review found that
`RFH_QUEUE_CONFIG_CMD` is for extra RX queues; the one-queue owner uses the
context-info default queue, so no speculative post-ALIVE command path was
added. RX completion reclaim is now wired into the physical poller and covered
by the queue-owner self-test. Physical boot/ALIVE evidence is still pending.
