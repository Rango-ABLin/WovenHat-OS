# M00 - Inventory and closure baseline

Date: 2026-10-24
Baseline commit: `3eee80b8` (`test: harden desktop login settling`)
Branch: `master` (clean before M00 documentation)
Toolchain: repository-defined Rust/Cargo toolchain; Windows PowerShell host
QEMU: `C:\Program Files\qemu\qemu-system-x86_64.exe`
Firmware: `C:\Program Files\qemu\share\edk2-x86_64-code.fd`

## Evidence inventory

| Area | Current state | Evidence | Classification |
|---|---|---|---|
| Stages 1-19 and accepted Stage 20+ history | Historical acceptance is recorded in `docs/stage-status.md`; current release chain is bounded software evidence | `target/release-validation/results.json` | Partial: historical evidence must be rerun for final release |
| Release validation | 158/158 checks passed on the recorded release chain | `target/release-validation/results.json` | QEMU/host software only |
| Installed disk boot | Exact installed-image boot passed on 1/2/4 CPUs; source and installed image digests were recorded | `target/installed-disk-release-*` | Pass for installed-image boot; supported installer transaction still open |
| Desktop qualification | Strict harness exists; intermittent QEMU desktop journey remains unaccepted | `target/desktop-qualification-*` | Fail/not accepted |
| Virtual trust chain | Fail-closed preflight is implemented | `target/virtual-trust-qualification/current-preflight.json` | Blocked: OVMF VARS, swtpm, development key/certificate |
| Physical qualification | Read-only inventory and 17-category plan exist | `scripts/physical-qualification-preflight.py`, `tests/hardware-matrix.json` | `PHYSICAL_NOT_TESTED` |
| CI | Windows, kernel, runner, and hardware-qualification workflows are present | `.github/workflows/` | Requires current-head CI execution evidence |

## Closure classification

- COMPLETE: implemented with current, reproducible evidence and no known unmet
  acceptance predicate in the scoped gate.
- PARTIAL: source and focused tests exist, but the mandate acceptance evidence
  is incomplete or comes from an older baseline.
- BLOCKED: the gate requires a missing tool, credential, hardware device,
  external review, or other unavailable precondition.
- FAIL: a current reproducible acceptance test fails.

## Priority blockers

1. M02/M07/M08/M09: obtain a stable boot-to-desktop interaction run, including
   keyboard, mouse, window composition, application lifecycle, and recovery.
2. M04/M11: qualify persistent storage mutation, remount, crash recovery,
   installer transaction, rollback, and backup/restore paths.
3. M10/M14: provide disposable OVMF VARS, swtpm, development signing material,
   SBOM/provenance and clean-room build evidence where tools are available.
4. M05/M06/M13: expand device matrices and obtain real-device evidence. These
   cannot be closed by QEMU-only runs.
5. M15: final 1/2/4-CPU matrix, soak, security review, and GO/NO-GO audit.

## M00 exit decision

**PASS for M00 documentation scope.** The ledger, dependency graph,
reproducibility record, and checkpoint are present. This is not a release
qualification decision and does not override the failed/blocked gates above.

## M01 result

M01 passed on the current integration branch after correcting the desktop's
default compositor mode. `Desktop::INITIAL` now starts in the accepted 2D path;
3D remains an explicit setting until it has a complete pixel-accurate matrix.
The mandate's Stage 20.5 focused-shadow predicate, Stage 20.15 launcher/session
journey, NVMe and AHCI FAT32 persistence, and Stage 31 write-fault containment
were run on 1, 2 and 4 CPUs. The fault-injection gate passed by observing the
expected fail-closed batch-persist marker.

Evidence:

- `target/woven-shell-depth-{1,2,4}cpu`
- `audit-artifacts/stage20.15-{1,2,4}cpu-*`
- `audit-artifacts/stage13.14-nvme-{1,2,4}cpu-*`
- `audit-artifacts/stage13.14-ahci-{1,2,4}cpu-*`
- `audit-artifacts/stage31-storage-write-fault-{1,2,4}cpu-*`

M01 is a software/QEMU acceptance result. It does not close physical-device,
Secure Boot/TPM, signing-key, or external-review prerequisites.
