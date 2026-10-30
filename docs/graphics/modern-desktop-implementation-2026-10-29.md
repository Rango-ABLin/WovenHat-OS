# Modern desktop implementation - 2026-10-29

Status: IMPLEMENTING. The user authorized the complete appearance and everyday
workflow proposal. This record tracks that scope; it does not accept later
roadmap stages or supersede the master roadmap's prerequisite order.

## Ordered implementation

| Work | Existing boundary / required completion | Status |
|---|---|---|
| Input and accessibility reliability | Graphics Stage 10; Stage 20.19 capture, cancellation, occlusion, keyboard focus and client controls | Current increment; acceptance open |
| Shared visual identity | Graphics Stage 11 tokens, palettes, spacing, focus and control states; preserve accepted app behavior | Pending prerequisite |
| Typography and icons | Extend accepted graphics Stage 05/06 boundaries with scalable assets, complete shaping/fallback and application integration | Pending prerequisite |
| Persistent personalization | Authorized per-user theme, accent, wallpaper, scale and accessibility writes, reboot readback and compositor propagation | Pending prerequisite; no broad /etc write capability |
| Window and desktop navigation | Snap previews, searchable launcher, pinned/recent items, workspace overview and focus restoration | Pending prerequisite |
| Settings and Files workflows | Searchable settings backed by services; file navigation, associations, thumbnails, cancellable operations and safe restore | Pending prerequisite; FAT32 remains the primary system filesystem |
| Network, applications and updates | Complete network/browser/download/open and signed install/update/remove/recover journeys | Pending prerequisites in the master roadmap |
| Media and laptop integration | Real service/device selection, playback, brightness, battery, touchpad and qualified suspend/resume | Pending software and named-device qualification |
| Accessibility and localization | All-surface keyboard/screen-reader acceptance, high contrast, reduced motion and translations including Chichewa | Pending integration acceptance |
| Performance and recovery | Measured frame timing and resource budgets, application/service restart and recoverable errors | Pending prerequisite; native evidence remains separate |

## Current source increment

`Button::pointer_move` updates hover separately from primary-button edges.
Eight existing application motion callers now use it. Motion cannot arm a
control whose press began elsewhere, consume a release, or discard a captured
press merely because the pointer left the control. `cancel_pointer` explicitly
discards pending activation. Host tests exercise outside-to-inside dragging,
leave/re-enter, duplicate release, cancellation and disable/re-enable.

Settings now owns one persistent Button rather than reconstructing a display
copy each repaint and activating on any release. Files and Settings accept
Enter, Space and R through authenticated existing shell messages, and show the
keyboard focus indicator. This adds no filesystem-write authority.

Each production client sends `WCAP` version 1 after opening. The shell binds
this opt-in to the authenticated window owner, resets it on slot reuse, and
sends `WCAN` only to opted-in clients before the existing synthetic `WPBT`
release when locking or changing workspace. All eleven production pointer clients handle
the cancellation before interpreting the release and repaint cleared state.
Security and Installer additionally require a preceding held state for a
release action. If WCAN cannot be queued, the shell avoids sending an
activating synthetic release. Legacy clients receive only their established release and are never sent an
unknown tag. An unchanged strict legacy-client QEMU fixture caught the initial
unsolicited-tag defect; those failed logs remain preserved. Negotiation has no
acknowledgement and its readiness race is not separately qualified.

The shell and clients remain single-threaded userspace programs. No syscall,
kernel object, unsafe block, lock, pointer lifetime or capability was added.
Messages remain bounded and shell-sender authenticated. Window-slot removal
continues to discard capture before reuse. Saturated-mailbox cancellation now retries a bounded number of times (3)
before falling back to the previous fail-closed behaviour (skip the
synthetic WPBT release). Client-side `cancel_pointer` is covered by host
tests; a persistent full mailbox remains fail-closed by design.

## Acceptance work

The real-QEMU desktop journey now checks Files workspace cancellation without
an extra refresh action, Files Enter activation with a framebuffer change,
and Settings launch/Enter activation/framebuffer change/close before its
existing Terminal journey. New timings use the existing input/frame budgets.

The prescribed Windows baseline initially failed before QEMU because a cleanup
commit removed the reachable Stage 7.6-9.5 runners while retaining their
callers. Twenty runners were restored from `b5a2e571^`. Nineteen remain byte-identical;
Stage 7.6 now runs host all-targets Clippy and explicit freestanding kernel
Clippy, because its former host-target kernel lint conflicts with the current
stdlib-backed host codec dependency. No assertions, stress counts or timeouts
were changed. The original failure
is retained at `audit-artifacts/acceptance-20261029-082647-830/`.

Independent source review identified cancellation and cascade-coordinate gaps;
the implementation and runtime journey were corrected. Validation evidence is
being collected under `audit-artifacts/modern-desktop-20261029-084228/`; earlier failures remain at
`audit-artifacts/modern-desktop-20261029-083456/`.
Final-source host/kernel builds, warning-denying Clippy, host tests and all
twelve affected app release builds/lints pass. The unchanged Stage20.19 legacy
input fixture passes on 1/2/4 CPUs after negotiation. Full Windows, expanded
production desktop journeys and inherited matrices must pass before acceptance. The earlier interrupted 90-case graphics run is
not reused as a passing gate. No complete modernization or hardware release
claim is made.
