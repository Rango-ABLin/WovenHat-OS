# Stage 27.5 WovenRecovery environment audit - 2026-10-22

## Scope

This audit covers the bounded WovenRecovery environment foundation: signed
Ring-3 application loading, capability and boot-mode authorization, status,
filesystem-check, serial-log retrieval, and fail-closed reset/reinstall
admission. It does not claim destructive recovery, installation, boot-slot
handoff, or physical-device qualification.

## Implementation

- `Capability::Recovery` is included in the bounded system-service ceiling.
- Syscall 240 validates capability, authorization mode, operation, and output
  length before copying results to userspace.
- Status and filesystem results use fixed 32-byte records; log reads are
  bounded to 4096 bytes.
- Reset and reinstall return `ADMISSION_NOT_IMPLEMENTED`; no disk mutation is
  reachable through this foundation.
- The recovery application is signed as WAPP and reports denied,
  unavailable, and successful outcomes distinctly.

## Acceptance evidence

- `python scripts/test-stage27-5-recovery.py --cpus 1`: PASS, exit 33.
- `python scripts/test-stage27-5-recovery.py --cpus 2`: PASS, exit 33.
- `python scripts/test-stage27-5-recovery.py --cpus 4`: PASS, exit 33.
- Recovery app freestanding Clippy with `-D warnings`: PASS.
- Kernel Clippy with `stage27-5-recovery-test` and `-D warnings`: PASS.
- Workspace all-target Clippy with `-D warnings`: PASS.
- `cargo test --offline -p libwoven`: 115 passed, 0 failed.

Serial, QEMU, and disk-image evidence is retained under
`audit-artifacts/stage27.5-recovery-{1,2,4}cpu`.

## Limits and next prerequisites

Reset/reinstall are admission-only and intentionally not release-complete.
The next work must implement and independently test safe destructive recovery,
installer integration, update-slot sizing and bootloader handoff, power-loss
recovery, and physical qualification before release-certification stages.
