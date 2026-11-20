# M5 Physical Hardware Test Plan

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `5ca5b9b3`

## Scope

Physical qualification of the M5 supported-device set: NVMe, AHCI disk, xHCI
USB HID, MSI/MSI-X device interrupts, and the shared DMA page ownership
contract.

## Status

- `PHYSICAL_NOT_TESTED`. No physical hardware was available or authorized in
  this session.
- Every physical row in the driver matrix and the M5 test results is reported
  as `BLOCKED` / pending; no physical result may be claimed as pass/fail.

## Planned physical tests

1. NVMe: boot on real NVMe controller, mount, read back persisted file.
2. AHCI: boot with real SATA disk, mount, read back.
3. xHCI: real USB keyboard/mouse interrupt delivery through MSI/MSI-X.
4. Shared DMA: real device DMA with ownership/bounds checks.
5. Wi-Fi: physical AX200/AX201 bring-up (blocked on firmware availability).
6. Bluetooth: physical HCI bring-up (blocked on radio availability).

## Evidence requirements before promotion

- Device identity, firmware versions, measured logs, repeatable procedure,
  operator approval (see `tests/hardware-matrix.json` contract).
- Physical run must use the same gated build and the Stage 10.7 acceptance
  launcher (`RUN-STAGE10.7.ps1`).

## Blockers

- No physical hardware in session; physical-hardware authorization workflow not
  completed in this M5 slice.

See `docs/milestones/M5_RELEASE_READINESS.md` for the release decision context.
