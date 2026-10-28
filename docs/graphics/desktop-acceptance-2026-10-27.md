# Desktop software graphics acceptance - 2026-10-27

Status: ACCEPTED AND CLOSED for the software/QEMU desktop scope.
Accepted source: `cea33d1a2f6ef1ddb5cdb71a8f99d393952c0df7`. Closure documentation is a separate commit.

## Boundary

This closes the reported desktop graphics errors and the software regression
acceptance gate. It covers secure compositor surfaces, desktop behavior,
software 3D presentation, emulated VirtIO GPU scanout/recovery, monitor layout,
session behavior, and real QEMU desktop input/performance on 1/2/4 CPUs.
It does not certify native GPUs, native HID, GPU-accelerated 3D, GFX-08,
or the complete graphics modernization mandate. WHPX accelerates the CPU.
No next modernization stage is promoted by this repair.

## Repairs

- Stages 20.2/3/4/6/8/9/13 explicitly select their flat behavior contract and
  use the current independent wallpaper/taskbar/panel palette. Dedicated
  Stage 20.17/18 software 3D tests remain enabled and pass.
- Launcher tests compare the complete panel and independent generic icon
  masks. Taskbar, failed-launch notifications and Notification Center retain
  exact border/text/translucency checks; background-only toast tolerance is gone.
- Stage 20.18 compares every projected notification pixel and requires visible
  text, in addition to its existing title-bar geometry assertions.
- Stage 20.14b uses the current flat contract and checks the whole secondary
  taskbar strip remains wallpaper, plus cross-monitor drag, host captures,
  presentation and teardown.
- Stage 20.6 now compares a 19x44 completed-frame strip over the window body
  and shadow. It checks the opaque header, border and translucent body. Its
  old three-sample assertion incorrectly expected translucency in the opaque
  header and could pass a partially painted frame. Exact diagnostics exposed
  that defect; the original limit is unchanged.
- Repaint handles a client releasing its shared surface after liveness checks:
  it confirms disappearance, invokes existing window/capture/focus teardown,
  then rebuilds before presentation. Each retry removes a window and is bounded
  by MAX_WINDOWS. Unexpected errors on valid surfaces remain fatal and logged;
  the existing bounded resize replacement grace is preserved.
- Normal-boot host fixtures require the signed-app licensed-wallpaper readback
  marker and explicitly reject the obsolete shorter marker.

The responsiveness/input fixes use bounded event batches, separate cursor and
close damage, absolute tablet coordinates, cleared dead-window pointer capture,
retained shortcuts, empty initial Calculator input, and speed-optimized shell
builds. The demo selects flat composition and reduced motion; RUN-DESKTOP.ps1
uses WHPX by default, supports TCG, and snapshots its read-only boot image.

No ABI, capability, credential work factor, warning policy, stress count,
assertion, or timeout was relaxed. No new kernel object, unsafe code, lock,
asynchronous pointer or blocking operation was added. The between_frames
helper uses explicit cfg(any(stage20-6-test, stage20-14b-test)) guards; feature
graphs are unchanged. Existing owner/generation checks remain authoritative.

## Source changes

This closure changes `kernel/src/main.rs` (independent acceptance fixtures),
`kernel/src/compositor.rs` (test-only completed-frame helper guards),
`apps/woven-shell/src/main.rs` (bounded released-surface recovery), and
`tests/test_normal_boot_serial_safety.py` (strict installation-marker fixtures).
Launcher/qualification changes and the earlier responsiveness fixes are retained
in the integrated history. Architecture, status, roadmap and audit records are
updated here; there are no syscall or application-protocol changes.

## Acceptance results

- 87/87 required QEMU cases PASS on 1/2/4 CPUs: Stages 08, 09, 19.12/13/14,
  20.1-13, 20.16/17/18, 24.59, GPU single-output/recovery/two-head scanout,
  multi-monitor desktop, session, and TCG/WHPX real desktop qualification.
- Strict clean-source GFX-07 checker: 9/9 PASS, unchanged fatal-marker detector.
- Kernel build; warning-denying host/kernel/app Clippy; Rust host tests PASS.
  Initial static sweep covers 29 kernel configurations; affected/default/lower
  features and host checks were rerun after the test-only helper guard repair.
  Final shell release build and warning-denying target Clippy PASS.
- Full Python host suite: 205 tests PASS, no skips. Windows elevation was needed
  solely to create real symbolic-link security fixtures. Earlier privilege
  failures remain preserved; no security test was mocked or weakened.

## Evidence and provenance

Final matrix: `audit-artifacts/graphics-close-final-20261027-175843/manifest.json`.
Strict checker: `audit-artifacts/graphics-close-gfx07-final-20261027/manifest.json`.
Static checks: `audit-artifacts/graphics-close-static-20261027-163001/`, including
`python-host-tests-elevated.log` and its exit-0 result. Multi-monitor repair:
`audit-artifacts/graphics-close-multi-20261027-173040/`. Completed-frame fixture:
`audit-artifacts/graphics-close-translucency-20261027-175506/`.

The final matrix contains 51 newly run cases and 36 unaffected passing cases
from source a84ae4eb. Before reuse, the runner verified every other build-input
hash and byte-for-byte equality outside the Stage20.6 fixture and its test-only
helper cfg extension. Every old Stage20.6 result was excluded. Commands, source
revisions, log hashes, exact exits and this proof are in the manifest; final
source hashes remained unchanged throughout the resumed chain.

The initial 84/87 sweep, strict-checker close-race failure, interrupted sweep,
and exact header-pixel diagnostic are retained under graphics-close-matrix-*,
graphics-close-gfx07-20261027-1734, and graphics-close-translucency-*.
Only the recorded task-owned interrupted process tree was stopped. Historical
failures and unrelated QEMU processes were not deleted or terminated.

Native hardware, full text/image/widget coverage, accelerated graphics and
independent production review remain separate roadmap prerequisites.
