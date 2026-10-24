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

1. M04/M11: qualify persistent storage mutation, remount, crash recovery,
   installer transaction, rollback, and backup/restore paths.
2. M10/M14: provide disposable OVMF VARS, swtpm, development signing material,
   SBOM/provenance and clean-room build evidence where tools are available.
3. M05/M06/M13: expand device matrices and obtain real-device evidence. These
   cannot be closed by QEMU-only runs.
4. M15: final 1/2/4-CPU matrix, soak, security review, and GO/NO-GO audit.

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

## M02 result

M02 passed for software/QEMU scope. Strict desktop qualification passed on
1/2/4 CPUs with 5 GiB configured guests (at least 4 GiB usable) after
correcting the harness's cascade-window close coordinates and
fixing full-screen recomposition when a client surface is released. Two
sequential shell boots and the guest reset gate also passed on each of 1/2/4
CPUs. This demonstrates bounded cold-boot/session and guest-reset behavior;
it is not physical power-loss or native hardware evidence.

## M03 result

M03 passed for current-head software safety and concurrency scope. The full
Rust all-target test suite, 204-test Python host suite, warning-denying
kernel-target Clippy, and warning-denying host all-targets Clippy all passed.
The evidence covers bounded ownership/generation reuse, IPC and IRQ mailbox
concurrency, allocator and page validation, PCI resource transactions,
filesystem crash atomicity, and input/graphics contracts. This does not close
the mandate's physical, arbitrary-fault, TPM/Secure Boot, signing, or external
security-review gates.

## M05 result

M05 passed for deterministic QEMU/device-model scope. Controller-owned
NVMe/AHCI/xHCI/HDA restart, dual-head graphics scanout, and USB input gates
passed on 1/2/4 CPUs. The isolated repeated-shell reruns also report at least
4 GiB usable memory in every boot. This does not claim native driver crash
recovery, physical graphics/input/storage/audio/network/Bluetooth behavior,
or hardware certification.

## M06 result

M06 passed for software and QEMU scope. The network/link-loss matrix and the
Stage 13.10 Wi-Fi/Bluetooth contract matrix passed on 1/2/4 CPUs. This covers
bounded protocol, security, transport, firmware, and recovery contracts; it
does not provide native radio, RF, physical interoperability, or certification
evidence.

## M04 result

M04 passed for disposable file-image and QEMU scope. The current run passed
Stage 28 GPT/FAT32/EFI/provisioning with rollback, installed-image boot and
exact digest verification on 1/2/4 CPUs, 45 focused installer/storage tests,
and Stage 27.5 recovery on 1/2/4 CPUs. Together with the M01 NVMe/AHCI FAT32
persistence and bounded write-fault evidence, this closes the repository's
software storage/installer path while leaving physical durability, power-loss
media qualification, and trust-boundary prerequisites open.

## M07 result

M07 passed for bounded software/QEMU graphics scope. Dual-head scanout,
single-output pixels, and GFX-02 CPU renderer feature tests passed on 1/2/4
CPUs. Native GPU driver behavior, physical scanout, acceleration performance,
and device-loss recovery remain unqualified.

## M08 result

M08 passed for software/QEMU UX scope through the strict desktop and guest
reset matrices on 1/2/4 CPUs. Input, sign-in, window composition, teardown,
frame restoration, and the bounded session journey are covered. Native
accessibility devices and physical display qualification remain open.

## M09 result

M09 passed for software/QEMU scope. The current-head release chain completed
158/158 checks, including the signed application catalog, app store, package
transactions, update/security services, AI features, service supervision,
system-app boots, and app linting on 1/2/4 CPUs. This does not close trust
provisioning, production signing, or native certification.
