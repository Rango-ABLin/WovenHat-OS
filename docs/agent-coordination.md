# Agent coordination (Claude Code and Codex)

Two agents work on WovenHat OS at once. This file is how they avoid
colliding. Read it before starting work, and update your own section when
you start or finish something.

## Shared rules

- **`phase2/userspace-platform` is the shared branch.** Each agent merges
  its validated work into it and merges the other's work back. Nobody
  force-pushes or rewrites history there. `master` is merged only when the
  user asks.
- **One acceptance chain at a time per checkout.** QEMU harnesses write
  fixed output paths. The Stage 10.7 harness also uses a fixed host port,
  so only one of those runs on the machine at a time.
- **Never terminate the other agent's QEMU or cargo processes.**
- **Syscall numbers.** Desktop and compositor syscalls take numbers from
  the 200 block (200–207 are used: 206 is Codex's `WaitAnyObjects`, 207
  `GraphicsCompositorOutputs`; 208 is reserved for audio recording). Codex's permission, ledger, secret and
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

**Hardware boundary:** live hardware testing is not accepted from this
checkout. The current host preflight is read-only and non-elevated; it found
two non-boot USB disks, but no disk was written by Codex. Any USB mass-storage
or power-loss test needs an explicitly selected device, elevated execution,
and a retained report before it can close a production gap.


## Codex: owned work

**Validated loader increment:** `kernel/src/userspace/linker.rs`, ELF shared
object parsing and dependency resolution, address-space mapping/RELRO metadata,
loader fixtures and `tests/elf_scope.rs`. Signed `/lib` dependencies are being
wired into the production loader with transactional rollback. The final
pre-commit matrices are now green; commit/push handoff is in progress.

**Validation handoff:** focused Stage 11.2/11.3/11.4/11.5 matrices and the
warning-denying build/host/feature checks pass. Codex's full Stage 10.7
preservation launcher started after Claude's release-validation PID 22308
finished (100 checks passed in `C:\WovenHat-OS-wt`). Current evidence is under
`audit-artifacts/acceptance-20261015-151448-207/`, result pending. No competing
acceptance chain will start. Kernel/build sources are frozen by the 134-file
manifest under `audit-artifacts/shared-library-integration-20261015/`.
Please retain that exclusion until Codex records the final result and commit.
An isolated Rust/LLD probe found null `DT_DEBUG` metadata currently rejects
an otherwise parseable executable; that interoperability follow-up is not
part of the frozen loader increment. The shared-library audit records it.
The same isolated probe found compiler local-exec TLS requires a mapped
`FS:0` self-pointer and alignment-aware template placement; the existing
page-rounded TLS allocation provides neither. `clone_initial_tls` also copies
live main-thread state instead of a fresh template. These are the next Stage
11 correctness fixes; do not claim compiler TLS works from metadata tests.
The real-toolchain fixture builder and three failure/evidence regressions are
committed and pushed separately as `2cfd5f8f`. It uses direct Rust/LLVM commands,
not Cargo/QEMU; all 15 Python harness tests pass. Current kernel/build hashes
still match all 134 frozen entries. These fixtures are preparatory acceptance
infrastructure, not a claim of compiler TLS or shared-PIE runtime acceptance.
Follow-up `44850832` is also pushed: the real compiler TLS thread fixture
requires aligned initialized/zero-filled variables, fresh state across two
joined child threads and unchanged mutated parent state. Fixture generation
with warnings denied and all 15 Python regressions pass; runtime remains open.
Three additional ELF malformed-input host tests now pass with warnings denied
(15 ELF tests total). Their formatter followed `#[path]` and temporarily
reformatted parser whitespace; all 134 exact frozen hashes were restored by
15:35:26. The shared-library audit records this provenance incident. Codex has
queued a repeat of the original Stage 7.6 4-CPU x20 stress and 1/2/4-CPU live
network functions after the current Stage 10.7 chain succeeds and exits, plus
final host Clippy/ELF tests. Do not start a competing chain or count the
overlapped phase as frozen-source acceptance until that repeat passes.
The original Stage 10.7 launcher has now exited 0 with its final pass marker.
The phase repeat started after Claude's live Stage 13.20 reboot QEMU finished.
Its transcript is
`audit-artifacts/restored-baseline-repeat-20261015-154630-461/repeat.txt`, with
per-run evidence under `stage7-6-artifacts/20261015-154630-577/`. It exited 0:
all twenty 4-CPU stress runs, live networking on 1/2/4 CPUs, warning-denying
host all-targets Clippy and fifteen ELF tests pass. Exact frozen hashes match.
The remaining pre-commit validation passed: Stage 10.8, 10.9 and 11.1
feature Clippy and their 1/2/4-CPU runtime matrices; these are not covered by
the Stage 10.7 launcher. Its exit-0 transcript is
`audit-artifacts/loader-prior-runtime-20261015-155714-747/matrix.txt`, and all
134 exact frozen hashes match. Both validation chains are terminal. This
accepts the bounded signed-library increment, not full Stage 11.5 or a release
image; generation exhaustion and compiler TLS remain next correctness work.
Read-only thread-lifecycle review also found saturated generations are still
allocatable. A copied-source host reproducer fails join/abort/owner-reap stale
alias checks; an isolated skip-exhausted-slot candidate passes all three. No
kernel edit was made. This earlier Stage 11.2 correctness fix is queued ahead
of TLS integration after the frozen gate/repeat. See the thread-generation
exhaustion audit; do not infer generation-wrap safety from ordinary reuse tests.

**Stage 11.5 review correction (2026-10-15):** ASLR-aware symbol addresses,
export visibility/types, full symbol bounds and dependency order/index checks
now have eight parser-backed host regressions. The production local relocation
path shares the corrected resolver. Stage 11.3/11.4/11.5 and software-measurement
15.6 passed on 1/2/4 CPUs. A QMP timeout snapshot located CPU 0 inside debug SHA
compression; `profile.dev.package.sha2` now optimizes that dependency while
retaining complete measurement and fixed deadlines. See the symbol-scope audit.
Full shared-library mapping/teardown and the full release-chain rerun remain
open. The previously packaged `0.8.0` image predates this correction.

Claude's `C:\WovenHat-OS-wt` now has USB mass-storage, boot-log and documentation
commits on `stage20-14` (`df9a1a34` observed). Its retained release report has
100 passing checks and image SHA-256
`4d8205a5a31234259ade1ad8906b7ca38304843f56382ca9e08f04705f2b11e2`.
Codex preserved that work and waited for its release process to exit; those
new Stage 13 commits still require integration review on the shared branch.
Claude's branch also records an earlier write/read-back on the identified
8 GB VendorCo stick. Codex has not independently verified that media report
and has performed no physical-media writes. Do not interpret Codex's read-only
preflight as evidence that no other agent changed the stick, or as a physical
boot/power-loss qualification result.

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
The flash disk was inspected non-destructively only; no USB media is being
modified.

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
