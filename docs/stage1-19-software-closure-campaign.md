# Stage 1–19: software closure campaign (hardware deferred)

Date: 2026-10-28. Scope: software-only closure; **not a release acceptance or physical hardware sign-off**.

## Objective and definitions

Close the Stage 1–19 **software acceptance** at one immutable commit SHA while keeping all physical device, firmware, native GPU/radio, real DMA, real power-loss, and measured physical provisioning checks in a **separate hardware qualification ledger**.

- **SOFTWARE CLOSED** means warning-denying Clippy for host/kernel/affected features, unit/negative/concurrency tests, deterministic fault injection, acceptance boots on 1/2/4 CPUs, preserved logs/markers, and independent review have passed **on the same combined tree**.
- **SOFTWARE OPEN** means a required implementation, software integration, test, or evidence item is still missing.
- **HARDWARE PENDING** means software is independently closed but relevant real-device evidence remains outstanding.
- **RELEASE BLOCKED** remains true until physical qualification and full release criteria pass. No stage should be silently greened by changing an assertion, marker, or required test.

## Work queue (software only)

| Stages | Software closure deliverables | Status as of campaign creation |
| --- | --- | --- |
| 1–5 | Re-run full boot, paging, scheduler, storage transaction gates; negative tests for corruption, exhausted resources, torn-write and rollback on *virtual* block devices, including reboot/remount | REVALIDATE |
| 6 | Complete serialized 1/2/4-CPU SMP stress including hotplug/cancellation, lock ordering, memory pressure, scheduler fairness/priority inversion, teardown and TLB shootdown | REVALIDATE |
| 7–10 | Close all Ring-3 fault/teardown, handle-generation, COW, IPC capability/authority, network lifecycle and async cancellation/timeout software cases; exercise both positive and denial paths | REVALIDATE |
| 11 | Broader loader/runtime interoperability, TLS, BSS, signals/PTY/job control and musl/POSIX compatibility against executable signed fixtures, plus negative cases; close only the explicitly versioned supported ABI profile | SOFTWARE OPEN |
| 12 | Native WovenFS mounted writable VFS integration; cache coherency; unmount/remount and reboot persistence; injected crash/flush-failure/corruption/rollback tests; authoritative writable mount ownership | SOFTWARE OPEN |
| 13 | Re-run model/QEMU PCIe/NVMe/AHCI/xHCI/HDA/HID/radio tests incl. hotplug, invalid devices, timeouts and teardown; separate any native physical drivers as hardware | REVALIDATE |
| 14 | Live IPv6 socket lifecycle, routing/policy breadth, connection lifecycle, malformed packet handling and network cancellation tests via virtual NIC | SOFTWARE OPEN |
| 15 | Signed package verification, permission broker and denial, revocation, persistent *virtual* vault/recovery, retained audit ledger, graphical consent flow and negative rollback tests | SOFTWARE OPEN |
| 16–18 | Build/launch signed out-of-kernel applications; dynamic runtime/IPC integration, font/compositor rendering regression, install/update/recovery boot simulation and teardown | SOFTWARE OPEN |
| 19 | Compositor and window-manager isolation, lease/focus/surface lifetime, accessibility/scaling/damage, catalog and application lifecycle, malicious client/denial tests, persistent signed-media update/rollback with simulated slot boot | SOFTWARE OPEN |

These statuses are a work queue, **not** new pass/fail evidence. A recent bounded test can be marked accepted within its documented scope, while the whole stage remains software-open for broader exit criteria.

## Execution order

1. Pin the exact source SHA and branch. Inventory stage feature gates, dependencies, known regressions and old evidence. Capture independent baseline failures before modifying code.
2. Repair Stage 1–10 remaining software edge cases and run serialized gates. Preserve all previously accepted markers and hashes.
3. Finish Stage 11 libc/loader/PTY/signals interoperability and the documented supported API profile; don't label unlimited POSIX compatibility as achieved.
4. Finish Stage 12 native VFS-backed WovenFS durability with virtual crash/reboot/fault injection and negative tests.
5. Revalidate Stage 13 models and close Stage 14 virtual-network protocol/software lifecycle gaps.
6. Finish Stage 15 secure update, provisioning and audit paths under virtual/test-only policy; finish Stage 16–19 application, desktop and recovery integration.
7. Run complete host checks and **every affected and inherited** 1/2/4-CPU QEMU matrix *on the combined SHA*, with logs, expected exits, actual negative assertions and no skips of required functionality.
8. Update the canonical closure ledger with per-stage SHA, command, CI URL, logs, reviewer, known limitations and status. Only then convert verified rows to SOFTWARE CLOSED / HARDWARE PENDING.

## Hardware-only deferrals

Physical USB/SATA/NVMe flush and abrupt power-loss; native PCIe DMA/IOMMU and real NVMe/AHCI/xHCI/HDA; live Wi-Fi/Bluetooth radios; real Ethernet and GPU/display/input; firmware Secure Boot/TPM provisioning; key storage/rotation in real hardware; native boot/install/update/rollback/recovery; APIC >255, NUMA, physical CPU hotplug and supported-device matrix.

**Guardrail:** do not reclassify missing simulated crash, integration, security, install or lifecycle tests as “hardware testing.” Those remain software obligations.

## Acceptance record template

For each stage: stage ID, scoped supported contract, exact SHA, lint/test commands, 1/2/4-CPU run URLs and logs, expected/actual exit status and serial markers, negative/fault/concurrency tests, inherited regression report, remaining deferred hardware IDs, reviewer, decision.

## Current decision

**Campaign opened; Stage 1–19 software closure not yet demonstrated on one combined SHA.** Keep the production closure ledger BLOCKED until software acceptance evidence is real. After software closure, release status stays HARDWARE PENDING rather than production-ready.
