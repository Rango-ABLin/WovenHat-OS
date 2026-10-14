# Stage 13.4–13.7 validation audit — 2026-10-14

The current tree revalidated the software-backed storage, USB-core, USB-HID,
and unified-input gates in order. Each harness exited through the debug-exit
success path on 1, 2, and 4 CPUs.

## Evidence

- Stage 13.4 AHCI/SATA DMA block I/O:
  `stage13.4-1cpu-1791981605010453300`,
  `stage13.4-2cpu-1791981675679929300`,
  `stage13.4-4cpu-1791981716077680700`.
- Stage 13.5 xHCI USB core:
  `stage13.5-1cpu-1791981759297896800`,
  `stage13.5-2cpu-1791981820766639900`,
  `stage13.5-4cpu-1791981864694609800`.
- Stage 13.6 USB HID:
  `stage13.6-1cpu-1791981914079760100`,
  `stage13.6-2cpu-1791981975767952400`,
  `stage13.6-4cpu-1791982020550370400`.
- Stage 13.7 WovenInput:
  `stage13.7-1cpu-1791982061927761800`,
  `stage13.7-2cpu-1791982127301158800`,
  `stage13.7-4cpu-1791982174067000900`.

These are QEMU software/contract gates. They do not prove native AHCI,
xHCI, USB-HID, or input-device interoperability on the inserted physical
machine. The current Stage 13.6 fixture exercises the bounded HID keyboard
path; USB mouse report decoding remains a separate production gap documented
by the stage status.
