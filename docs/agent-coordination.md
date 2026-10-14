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
  the 200 block (200–202 are used). Codex's permission, ledger, secret and
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
  `Permission::Audio` 16; syscalls 203–205 (206 is reserved for
  recording); `Action::AudioPlay` in the ledger.

- Stage 13.16, report-protocol HID (`kernel/src/hid.rs`): tablets, pens,
  touch screens and game controllers. Input kinds 9–13 were added to the
  input ABI.

**Stage 13 software closure is done.** Open items need hardware: physical
qualification of Wi-Fi and Bluetooth, and of the native controllers.

**Next (Claude Code):** roadmap 18.8 Workspaces, then 18.9 Multi-monitor.


## Codex: owned work

Stage 19 system applications (WovenCalculator, WovenTerminal, WovenFiles,
WovenEditor, …), application signing and packaging, and the release
validation chain. Update this section with anything in progress.
