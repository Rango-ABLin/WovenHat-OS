# Stage 1–19 production closure gate

**Baseline:** `phase2/userspace-platform`, pre-audit commit `4c0be2d5`.

**Decision: BLOCKED / NOT PRODUCTION-READY.** This ledger is deliberately
conservative. A passing QEMU marker proves the bounded software contract for
that gate; it does not prove native hardware support, crash recovery under
power loss, firmware integration, or production deployment readiness.

## Evidence rules

Every stage is closed only when the same source baseline has:

1. warning-denying host and freestanding-kernel Clippy;
2. host, malformed-input, negative, cancellation, teardown and concurrency
   tests appropriate to the stage;
3. retained 1/2/4-CPU QEMU evidence with the expected marker and exit code;
4. documented ownership, generation reuse, bounded-capacity and lock-order
   review; and
5. hardware, recovery, update, rollback and security evidence where the stage
   claims those capabilities.

Missing evidence is **blocked**, not inferred from an older green run.

## Current closure ledger

| Stages | Current result | Remaining production blockers |
| --- | --- | --- |
| 1–5 | Software gates and bounded storage transactions pass | Native storage/DMA matrix, arbitrary-size/full-volume rollback, torn-write and unclean-shutdown qualification |
| 6 | Bounded SMP, TLB, hotplug and lock foundations pass in QEMU | General multicore stress, NUMA/high-APIC-ID and physical hotplug qualification, broader priority-inheritance/low-memory evidence |
| 7–10 | Core isolation, IPC, capabilities, networking, completion and timer foundations pass bounded gates | IPC handle-generation rollover reproduction/resolution, complete signals, dynamic userspace runtime, broader driver/hardware qualification and full cancellation stress |
| 11 | Processes, scheduler-backed threads, join/cancel/teardown, ASLR, W^X, RELRO, synthetic TLS metadata, and bounded signed shared-object graph loading with cross-image relocation | Thread-generation exhaustion runtime preservation, compiler-compatible main/child TLS, general shared-library/toolchain interoperability, library TLS, REL/PLT tables, constructors/destructors, versioning, and physical qualification |
| 12 | Typed VFS, metadata, crypto, snapshots and mount boundaries pass bounded gates | Native WovenFS as the mounted system filesystem, durable crash/power-loss evidence, full rollback and persistent key provisioning |
| 13 | QEMU driver/input/storage/audio/HID foundations pass their accepted gates | Native PCIe/NVMe/AHCI/xHCI/HDA/Wi-Fi/Bluetooth/input qualification, hotplug and fault recovery |
| 14 | Bounded IPv4/IPv6, DHCP/DNS, sockets and firewall paths pass QEMU gates | Live IPv6 socket lifecycle, routing/policy breadth and physical NIC/radio qualification |
| 15 | Permission broker, signed packages, software secure/measured boundaries, volatile vault and ledger ABI pass bounded gates | Firmware Secure Boot/TPM provisioning, persistent vault, key rotation/revocation, graphical consent and durable security retention |
| 16–18 | Out-of-kernel apps, font/compositing and desktop roadmap software are exercised by retained QEMU gates | Toolchain/runtime expansion, installer/recovery/update integration, native display/GPU/input qualification and platform support policy |
| 19 | Compositor, surfaces, lease/focus, event waits, window manager, scaling, damage, apps and catalog pass bounded software gates | Persistent signed-media update/rollback, service-owned capability provisioning, hardware GPU/display qualification, and complete application lifecycle/security review |

## Hardware preflight

The read-only inventory was captured at:

`audit-artifacts/physical-host-20261015-124831-760/host.json`

It identified two non-boot USB disks. The host was not elevated and the report
states `PhysicalTestPerformed=false`. That preflight modified no disk,
partition, boot entry, or flash media. This is not a claim about later work
by another agent: Claude's storage branch records a USB write/read-back, which
Codex has not independently verified. Neither a read-only inventory nor image
read-back proves physical boot or power-loss recovery. Such qualification remains
blocked until the user explicitly authorizes a selected device and an elevated
test session.

## Required next implementation gates

1. Accept the Stage 11.2 thread-generation exhaustion correction: permanent
   retirement is integrated, and five actual-source host regressions plus
   build and warning-denying host/kernel/affected-feature checks pass. The
   failing stale-handle evidence is preserved. The inherited runtime chain
   remains pending, serialized behind Claude's live acceptance process;
   the correction is not yet accepted or committed.
2. Reproduce and resolve the earlier IPC handle-generation rollover candidate
   before advancing loader work. The source review identifies a 24-bit counter
   wrapping to one in a surviving process-local slot, but executable regression
   evidence is still required. See `audit-handle-generation-review-2026-10-15.md`.
3. Correct the Stage 11.2 signed exit-result sentinel collision for `-1` and
   `-2` through both ordinary exit and cancellation/join. The source finding
   and required host/Ring-3 evidence are in
   `audit-stage11-thread-exit-encoding-2026-10-15.md`; it is not yet a runtime
   acceptance result.
4. Finish Stage 11 loader interoperability and TLS above the bounded signed
   `/lib` graph implementation. Explicitly preserve Claude's pending `6f02202d`
   fixed-`ET_EXEC` placement correction in the replacement graph loader and
   require nonzero-bias/production execution evidence: zero-bias test builds
   do not cover that case. Legacy REL/PLT tables, constructors/destructors,
   versioned/weak symbols, dynamic library TLS and broader toolchain/stress
   qualification remain open. A real Rust/LLD probe also exposes null
   `DT_DEBUG` rejection and an incompatible main-thread TLS/TCB layout;
   compiler TLS execution and fresh child-thread template initialization
   are unaccepted even though synthetic TLS metadata tests pass.
   See the shared-library audit for the exact
   implemented graph, permission and teardown contract.
5. Close the native WovenFS mount/write-through policy and recovery evidence
   on a deliberately selected non-boot device.
6. Add service-owned capability provisioning and signed update/recovery
   rollback evidence before calling the desktop/application stages complete.
7. Re-run the inherited 1/2/4-CPU matrix and release chain after each accepted
   increment; commit each accepted increment separately.

This ledger does not mark any stage complete merely because a bounded gate
passes. It is the release decision record for the current branch.
