# Physical hardware qualification — release gate

Status: **BLOCKED until physical evidence exists**. This is a test protocol, not a certificate.

## Safety rules
- Default is read-only inventory. Never automatically partition, format, flash, erase, update firmware, change Secure Boot keys, or write boot entries.
- Identify the machine by model, board revision, firmware version and serial fingerprint (redacted in public artifacts).
- Every storage write test requires a dedicated, explicitly identified non-boot device, operator confirmation, backup, and a second independent check that the selected disk is not a system disk.
- No destructive power interruption on a device containing valuable data. Use disposable test hardware and supervised controlled power switching.
- A failure or missing evidence is BLOCKED, never PASS. Do not merge qualification claims based only on QEMU.

## Hardware support matrix
For each exact machine/firmware configuration record: CPU topology; RAM; UEFI/ACPI; GPU and display outputs; PCIe; NVMe/AHCI; USB/xHCI; keyboard, touchpad and mouse; Ethernet/Wi-Fi/Bluetooth; audio; RTC; TPM/Secure Boot; thermal/power controls.

## Execution order
1. **Inventory/preflight**: capture read-only host inventory, boot disk identification, disk serial fingerprints, firmware mode, device IDs and OS build. Store sanitized JSON and command transcript.
2. **Boot/install**: test signed installation media, UEFI boot, automatic graphical login without F1, cold boot, warm restart, shutdown and boot recovery. Capture serial logs, photographs/video and boot timings.
3. **Desktop/input**: test framebuffer/native display modes, scaling, cursor, keyboard layout, launcher catalog, file manager, focus, shadow depth, resize and multi-monitor. Measure responsiveness and record dropped input and compositor crashes.
4. **Sleep/resume**: run supported sleep states and resume cycles; verify storage flush, timers, CPU bring-up, graphics restoration, network reconnection and data integrity. Unsupported states must be marked N/A with evidence, not PASS.
5. **Storage**: on an authorized disposable disk, create files with recorded hashes, sync, remount, reboot, verify hashes and metadata; test full-disk, interrupted writes and recovery. Separately run supervised power-loss testing with written consent and recovery logs.
6. **Peripherals**: enumerate and exercise PCIe, USB hotplug, input, audio, network and radio devices; capture errors and recoverability. Record unsupported hardware explicitly.
7. **Security**: verify boot-chain signatures, TPM measurements, key enrollment/revocation, permissions, WovenGuard isolation, malicious-device rejection, update signing and rollback. Do not weaken controls for testing.
8. **Endurance**: repeat cold boot/restart, sustained I/O, concurrent 1/2/4 CPU workloads, memory pressure, hotplug, desktop use and recovery. Record counts, durations, failure rate, thermals and traces.
9. **Release review**: independently review exact source SHA, CI runs, warning-denied Clippy, 1/2/4-CPU QEMU, physical artifacts, severity of open issues and rollback. Sign off only for tested configurations.

## Per-test evidence record
Each test must retain: test ID; date/time; operator; machine fingerprint; firmware version; source SHA; media/image hash; command or procedure; expected outcome; observed outcome; exit codes; serial and device logs; artifact hashes; PASS/FAIL/BLOCKED/N/A; linked issue; reviewer approval.

## Acceptance criteria
- No unexplained crash, unexpected data loss, silent corruption, unauthorized access, or boot path requiring F1.
- All mandatory tests PASS on the declared support matrix, including negative and recovery cases.
- No open release-blocking defect; all deviations and unsupported devices documented.
- CI and QEMU gates are necessary but insufficient; physical evidence and explicit release approval are mandatory.

## Current decision
**BLOCKED / NOT PRODUCTION-READY.** The repository's Stage 1–19 ledger reports a read-only physical inventory only. No complete physical qualification matrix is evidenced by this document.
