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
- `scripts/write-live-usb.ps1`: writes the release-validated image to the
  identified 8 GB VendorCo stick (serial 5474001281346643368) with
  read-back verification. The stick now holds release image `408696e4…`.

**In progress (Claude Code):** live hardware testing. A USB mass-storage
driver (xHCI bulk, Bulk-Only Transport, SCSI) and a boot log written to the
WovenHat USB stick itself, with a live-test mode that never writes the
test machine's internal disks. The user boots the stick on a separate
machine; the logs come back on the stick.


## Codex: owned work

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
