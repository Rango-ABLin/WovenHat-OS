# Stage 13.5 USB core candidate audit

## Scope

Stage 13.5 introduces WovenHat's first native USB host-controller foundation. The implementation discovers PCI USB class 0Ch/subclass 03h/programming-interface 30h xHCI controllers, enables MMIO decoding and bus mastering, maps the controller BAR uncached, performs halt/reset/start sequencing, configures DCBAA, command ring, event ring and event-ring segment table, discovers a connected root-hub port, resets it, submits Enable Slot, and validates the matching command-completion event.

## Architecture

The controller is serialized behind the existing rank-10 IRQ-safe mutex. DMA structures are allocator-owned physically contiguous 4 KiB pages accessed through the bootloader direct map. Completion is deliberately polling-based because WovenHat does not yet expose a general MSI/MSI-X vector allocator; the audit therefore does not claim interrupt-driven USB completion. Existing ATA, AHCI, NVMe and VirtIO paths are preserved.

## Acceptance

`run-stage13-5-acceptance.ps1` requires a full build, freestanding Clippy with `-D warnings`, and isolated QEMU boots on 1/2/4 CPUs. The runtime harness attaches a QEMU xHCI controller plus a USB keyboard. Every boot must discover xHCI, initialize its core DMA structures, observe a connected port, reset the port, enable a device slot, receive the matching command-completion TRB, and exit through the standard debug-exit success path.

## Production gaps

Address Device, endpoint-context programming, EP0 control transfers, descriptor parsing, hubs, HID class support, mass storage, hotplug, suspend/resume, MSI/MSI-X interrupt delivery, bandwidth scheduling, streams, multiple controllers and broad physical-hardware qualification remain later work.

## Acceptance correction R2

The first 1-CPU QEMU run reached the xHCI controller and failed `Enable Slot` with `CommandFailed`. The root cause was event ordering: resetting the connected root port queues a Port Status Change Event before the later Command Completion Event. The original `enable_slot` path incorrectly treated the first event of any type as the Enable Slot completion.

R2 consumes unrelated xHCI events while waiting for the matching Command Completion TRB, while retaining bounded polling and strict command-pointer, completion-code, and non-zero slot validation. No acceptance assertion or timeout was weakened.


## Final acceptance record

Stage 13.5 was accepted on 2026-09-30 from commit `e8ed974b2166f4f4afb01c44034443e5819fb50d` by GitHub Actions run `36705547487`.

The full release-validation suite passed together with the retained Stage 13.3 NVMe and Stage 13.4 AHCI/SATA gates. Dedicated xHCI USB-core results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.5-1cpu-*`, `stage13.5-2cpu-*`, and `stage13.5-4cpu-*`.

## Milestone status

**Stage 13.5 native xHCI USB core: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers native xHCI discovery and bring-up, DMA ring structures, root-port discovery/reset, Enable Slot submission, correct filtering of unrelated events while awaiting the matching command completion, and successful 1/2/4-CPU execution. Device addressing, endpoint configuration, HID and other USB classes remain subsequent stages.
