# WovenFS atomic batch validation - 2026-11-27

## Status

**IN PROGRESS / NOT ACCEPTED.** Source commit `73d2e026` was independently
created and pushed while validation was running; it is not evidence that the
remaining gates passed. Physical durability, full stage closure and
production readiness are not claimed. This audit and the latest handoff/status
updates remain uncommitted.

## Change and boundaries

The WovenFS branch of `storage::persist_paths_atomic` now mounts and locks the
volume once and stages all file creation and exact-length replacement into one
existing journal transaction. Duplicate or malformed paths, missing parent
directories and sector-capacity exhaustion reject the batch before commit.
The existing 32-sector journal capacity and 4-KiB per-file limit remain.
An error after the commit point requires remount/recovery to determine the
outcome. This persistence API does not publish changes to existing VFS nodes.

## Evidence

- `cargo build`, default and affected-feature kernel Clippy with warnings
  denied, all-target host Clippy, and workspace tests pass.
- `cargo test --workspace` passes, including 23 WovenFS crash tests. Multi-file
  coverage enumerates every sector-write cut and verifies validation/capacity
  rejections do not write to the test device. The added test also interrupts
  replay at every sector-write boundary after each post-commit write boundary,
  verifies all files, and checks a further mount makes no changes. It exercises
  truncation and reuse of freed blocks. This is a RAM-device sector model, not a
  physical power-loss result.
- `python -m unittest tests.test_installed_disk_qemu` passes 9 tests.
- Installed-image normal reboot and descriptor/staged interruption matrices
  pass on 1/2/4 CPUs under `target/wfs-atomic-20261127-{normal,descriptor,staged}/`.
  The installed test verifies the native batch adapter on every boot; its
  interruption cuts still exercise the existing single-file qualification.
- Stage 1-5, 12.2 and 12.5 runtime regressions pass at 1/2/4 CPUs.
- The full Stage 10.7 preservation launcher passes with evidence under
  `target/stage10-output/audit-artifacts/acceptance-20261127-081723-575/`,
  and again with the replay test under
  `target/stage10-output/audit-artifacts/acceptance-20261127-085637-330/`.
- The initial commit-interruption attempt failed before guest startup with
  `cannot set up guest memory 'pc.ram'` on some boots; logs are preserved under
  `target/wfs-atomic-20261127-commit/`. After host memory recovered, commit,
  target, target-flush, checkpoint and retirement matrices passed at 1/2/4
  CPUs under `target/wfs-atomic-rerun-20261127-*/`. As above, the interruption
  fixture covers the single-file operation, not the multi-file batch.
- Stage 15.2 disposable scratch-device acceptance passes at 1/2/4 CPUs under
  `target/wovenfs-device-scratch-{1,2,4}cpu-17957651*`. This attaches a
  regular image file, not a physical disk.

## Remaining work

Exercise a multi-file crash/replay path in the installed guest and obtain the
independent review required by the roadmap. Native hardware, controller-cache
and physical power-loss qualification remain separate release prerequisites.
