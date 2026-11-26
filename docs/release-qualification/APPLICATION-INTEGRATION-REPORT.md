# Application integration report

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Implemented

WovenEditor and WovenSettings GUI sources in the inspected updated-app/Phase J
branches already match master. WovenFiles PR 141 adds a read-only layout.
The established packaging harness includes signed editor, settings, files,
terminal, monitor and other native application packages.

## Tested and accepted

All eleven signed packages and both services built successfully. The strict
WHPX desktop journey passes on 1/2/4 CPUs, including Files, Settings and
Terminal launch and input. Full editor behavior and reboot persistence remain
open. The one-CPU TCG run timed out before Files launch.

## Blocked and remaining implementation — updated 2026-11-25

The three bounded items previously listed here have been closed on
`feature/native-app-integration`:

- **WovenEditor**: the GUI now has bounded Open, Save and Save As controls plus
  a path field. Save validates the target, checks write and sync results and
  emits distinct serial markers; Open loads a UTF-8-trimmed note. A `WCLS` with
  unsaved edits is held until an explicit Discard, and focusing a field no
  longer marks the buffer dirty. Host logic tests 5/5 and warning-denying
  freestanding Clippy pass. There is no truncate/rename primitive in the
  established ABI, so overwriting a longer note with shorter text retains a
  tail; Save As to a fresh path is clean. This limit is accepted rather than
  papered over with a new syscall.
- **WovenSettings**: persistence was already accepted in current source; the
  genuinely open item was the PR 142 high-contrast hardening. Duplicate
  `high_contrast` keys are now rejected by both the writer and an additive
  shared reader, and the GUI reads the effective value through that reader.
  Host policy tests 5/5 pass. Light/dark remain session previews and live
  propagation to an already-running shell is still a separate increment.
- **WovenFiles**: added a bounded mutation CLI (`--mkdir`, `--rm`, `--rename`,
  `--write`) with `/mnt` or `/tmp` path confinement, distinct error codes and
  serial markers, on top of thin safe libwoven wrappers for the existing
  mkdir/unlink/rename/sync syscalls. No capability or ABI was broadened. A
  gated kernel self-test and `scripts/test-native-app-mutations.py` exercise
  create → write → rename → remove at 1/2/4 CPUs with a disposable FAT32 disk
  attached; the mounted-volume mutation evidence is the kernel's
  `[STORAGE MUTATION] live FAT32 ...: PASSED` marker on the same boot. The app
  mutation sequence itself runs under `/tmp` because a disposable FAT32 image
  without a provisioned `/mnt` directory cannot satisfy the kernel's
  `ensure_path("/mnt")` precondition — that gap is documented here, not hidden.

Validation: workspace tests, all-target and target Clippy are clean; signed
WovenEditor/WovenSettings/WovenFiles harnesses pass at 1/2/4 CPUs; the new
mutation harness passes at 1/2/4 CPUs. The editor relative-click desktop mode
fails identically on the pre-change baseline, so it is recorded as a
pre-existing harness limitation rather than treated as a regression or as
editor GUI acceptance. Full editor GUI behavior under pointer input remains
open.
