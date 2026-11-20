# M5 Release Readiness

Date: 2026-11-20
Branch: `m5/hardware-qualification` at `5ca5b9b3`

## M5 status

- `IN PROGRESS (NOT ACCEPTED)`. No stage acceptance has been granted.
- Full M5 acceptance gate: supported-device tests pass on 1/2/4 CPUs in QEMU
  where a QEMU backend exists; physical-hardware tests reported `BLOCKED` until
  real hardware evidence exists.

## Current readiness by area

| Area | State | Blocking for acceptance |
|---|---|---|
| Shared DMA + NVMe + AHCI | Emulated QEMU PASS (1/2/4 CPUs where applicable) | No, within current scope. |
| xHCI/USB HID input | Emulated QEMU PASS (1 CPU; layer needs 2/4 CPU evidence) | Needs 2/4 CPU run of Stage 13.13 if required. |
| PCI/MSI-X hardening | Code landed; QEMU gates PASS at 1 CPU. Runtime delivery/storm/capability coverage open. | Yes for full M5 acceptance. |
| Physical hardware | BLOCKED / PHYSICAL_NOT_TESTED. | Yes for physical claim. |
| Docs deliverables | In progress: implementation, driver matrix, physical plan, security, performance reports written this pass; release readiness and machine summary pending. | Ongoing. |

## Release decision

M5 is not accepted for release yet. The milestone remains open until:

1. The remaining PCI/MSI-X hardening gaps are closed with tests.
2. Remaining supported-device QEMU gates (2/4 CPU) pass.
3. Physical qualification is executed and recorded without fabrication.
4. The M5 release-readiness and machine-readable summary are finalized.

Next prerequisite: finish the audit-listed hardening gaps, then finalize
deliverables and re-run the Stage 10.7 full Windows gate on the M5 branch.
