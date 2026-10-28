# Live boot log startup candidate — 2026-10-16

## Finding

The production path called `livelog::start()` synchronously after USB driver
bring-up but before `session::start()`. `start()` scans the attached disk's
GPT/FAT structures, creates and preallocates a 512 KiB file, writes the initial
summary and inventory, and flushes it. A slow or wedged USB operation can hold
the boot thread there before the desktop session starts. If setup never reaches
file creation, the stick returns without a boot log, leaving no evidence of the
stall.

The attached VendorCo 8 GB stick is identified as FAT32 `WOVENLOG`. After the
authorized write, Windows mounted the healthy volume read-only for inspection;
the root contains no log files or `LOGS` directory. The empty volume confirms
that no boot log was returned, but does not establish which boot step stalled.

## Candidate change

- Production boot schedules live-log initialization as a background I/O
  service after device bring-up and immediately before attempting to start the
  WovenShell session. A stall during session creation can therefore still be
  captured once the delayed worker runs.
- The worker waits 10 timer ticks before touching USB/FAT, then
  records the already-buffered kernel log and retries setup failures every 30
  seconds with a serial diagnostic.
- The retry worker is queued even if xHCI has not reported USB storage yet;
  `NoUsbDisk` is retried instead of permanently skipping logging at boot.
- The synchronous `livelog::start()` entry remains for the Stage 13.19
  acceptance self-test.

This isolates slow live-log work from the session launch path. It does not fix a
stall in earlier xHCI/driver initialization, which still occurs before the
session and before the live-log worker can run.

## Earlier driver bring-up diagnostics

Driver initialization now emits a begin marker before the call that may stall,
and xHCI emits progress checkpoints across PCI enable, controller halt/reset,
controller start, connected-port discovery/reset, and slot enable. UART
transmit polling is bounded in every build; bytes are recorded to the in-memory
serial ring before transmission, so a missing UART cannot itself freeze the
boot and later successful USB logging can include those checkpoints. If xHCI
never becomes usable and the machine remains hung, the USB logger cannot write
through that same controller; an external serial capture is still required to
retrieve those earliest markers. This is diagnostics, not proof of a hardware
hang fix.

## Validation state

The candidate passed the exact Windows Stage 10.7 gate and the full release
validation chain. Warning-denying Clippy passed for the affected stage feature
sets. Stage 13.19 test, production and unlabelled paths and the Stage 20.15
desktop path passed on 1/2/4 CPUs. The normal release desktop/system-app boot
also passed. `git diff --check` passed. A direct
`rustfmt --check` invocation traversed the module tree and reported formatting
differences across pre-existing, untouched files, so it was not a useful
candidate-only format result; no broad formatting rewrite was applied.
These software gates do not prove physical USB reliability or resolve the
reported hang on real hardware. The live stick must be booted and its returned
log read to confirm whether the desktop appears and startup completes.

## Authorized media write

On 2026-10-16, the user explicitly authorized writing the attached disk. The
unique target was confirmed by model `VendorCo ProductCode USB Device`, serial
`5474001281346643368`, and physical size 8,052,549,120 bytes (disk index 1;
not the system or boot disk). The release-validated UEFI image was written;
raw-device readback SHA-256 matched
`44d668ef2a463941d39d9415b0d4c4af989b0e958d6ef6fad9adc0e45a13231c`.
The 13,697,024-byte image is the current candidate build. The EFI and healthy
FAT32 `WOVENLOG` partitions were created; the log partition is 7,665 MiB.
The writer transcript is `target/live-usb-write-20261016-final.txt` and the
full release report is `target/release-validation/results.json` (generated,
Git-ignored). The release validator's source/check report and image manifest
are in `target/release-validation/`. Physical boot and returned-log inspection
have not yet occurred.

## Follow-up correction

Physical production boots no longer enumerate USB input and mass storage
synchronously inside driver bring-up. The xHCI controller is initialized
there, while device enumeration is queued on the audited I/O service. This
prevents a slow or non-responsive USB device from delaying session startup;
the live-log worker retries until the storage device becomes available. The
QEMU Stage 13.19 test and production modes passed on 1 CPU, as did host
warning-denying checks for the changed paths. A fresh system-apps image was
built and passed the normal release shell smoke test. Physical boot remains
the required confirmation.

## Physical retest and bounded enumeration correction — 2026-10-17

The user booted a second, freshly written 14.65 GB VendorCo USB stick. Its
healthy FAT32 `WOVENLOG` partition returned with only Windows
`System Volume Information`; neither `LOGS` nor `BOOT0001.TXT` existed. This
reproduced the missing-log result and ruled out the first stick's media size as
the sole explanation. The temporary Windows `R:` mount was removed after the
inspection.

Source inspection found that the production deferred USB worker called
`start_input()` once and exited. A not-yet-ready mass-storage device therefore
left `with_usb_disk()` empty forever. Additionally, the old early return when a
HID interrupt vector already existed prevented a later retry from discovering
storage on another port. The correction keeps the worker on the existing
event-driven I/O service, retries every 300 timer ticks, and stops only after a
USB disk is published. `start_input()` now still enumerates ports when the HID
worker already exists, without reinstalling its vector or spawning a second
input worker.

Focused validation after the correction:

- warning-denying kernel-target Clippy and host all-targets Clippy passed;
- Stage 13.18 USB mass storage passed on 1 CPU;
- Stage 13.19 production passed on 1/2/4 CPUs;
- Stage 13.19 test and unlabelled paths passed on 1 CPU.

These QEMU gates do not yet prove physical recovery. The full release
validation passed after the correction, and the authorized second stick was
rewritten with the guarded image (SHA-256
`6ad538d2a7c94017a4d5cdd312b62521128b1bad9d2fceb6ce0d9abe9b32ba8d`); raw
readback matched. The next prerequisite is a physical boot and returned-log
inspection. If it remains empty, external serial capture is required because
a controller stall can occur before the live-log worker can write through that
same USB controller.
