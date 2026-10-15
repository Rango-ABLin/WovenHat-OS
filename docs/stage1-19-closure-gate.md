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
| 7–10 | Core isolation, corrected COW ancestors/child-wait authority/status, IPC with permanent exhausted-handle-slot retirement, capabilities, networking, completion and timer foundations pass bounded gates | Complete signals, dynamic userspace runtime, broader driver/hardware qualification and full cancellation stress |
| 11 | Processes, scheduler-backed threads, signed wait/join results, join/cancel/teardown, permanent exhausted-thread-record retirement, fixed-ET_EXEC/nonzero-PIE placement, real signed compiler shared-PIE execution, deterministic ASLR candidates, W^X, RELRO, bounded main-image compiler local-exec TLS and lifecycle, and bounded signed shared-object graph loading with cross-image relocation | Production ASLR arena/guard geometry, pure-BSS runtime and broader TLS models, general shared-library/toolchain interoperability, library TLS, REL/PLT tables, constructors/destructors, versioning, and physical qualification |
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

1. **Accepted bounded increment:** Stage 11.2 permanent exhausted-thread-record
   retirement, five actual-source host regressions, build and warning-denying
   host/kernel/affected-feature checks, plus complete runtime preservation.
   The failing stale-handle evidence is preserved. Commit `92959b3d` contains
   the correction; the fresh serialized chain in
   `audit-artifacts/thread-generation-20261015/runtime-attempt-20261015-180920-929/`
   exited 0 with its final pass marker after Stage 11.2, full Stage 10.7
   preservation and Stage 10.8–11.5 matrices on 1/2/4 CPUs. All 135 frozen
   hashes matched at that acceptance; the subsequent IPC increment uses its
   own new freeze. This is not full Stage 11 or release acceptance.
2. **Accepted bounded increment:** permanent exhausted IPC handle-slot
   retirement, durable actual-source regressions, real installation/rollback/
   teardown probes, complete host/lint QA and inherited runtime preservation.
   Session 63244 exited 0 after full Stage 10.7 and Stage 10.8–11.5 on 1/2/4
   CPUs, with all 488 frozen inputs unchanged. See
   `audit-handle-generation-review-2026-10-15.md` for final and failed evidence.
3. **Accepted bounded increments, in prerequisite order:** COW ancestor
   permissions (`79b2f5a`), child-wait ownership (`6c57a17`), then signed
   process/thread exit-result payloads (source `ad11e1f1`). The real libwoven
   fixture covers signed boundary statuses, cancellation, foreign/stale/double
   waits/joins and exact resources on 1/2/4 CPUs. Full combined acceptance in
   `audit-artifacts/exit-status-20261015/` exited 0 after all host/lint checks,
   full Stage 10.7 and Stage 10.8–11.5 matrices. All 500 frozen inputs matched.
   See the individual and combined audits for final and original failed evidence.
   This is not full-stage, combined-tree, release or hardware qualification.
4. Finish Stage 11 loader interoperability and TLS above the bounded signed
   `/lib` graph implementation. Claude's `6f02202d` fixed-ET_EXEC policy is now
   adapted in source `a8acbaaf` and accepted after full serialized preservation
   (session 62865, exit 0; all 500 inputs match). The expanded fixture executes
   both fixed ET_EXEC and nonzero PIE candidates on 1/2/4 CPUs; no global ASLR
   disabling or old-loader restoration was used. See the placement audit.
   Legacy REL/PLT tables, constructors/destructors,
   versioned/weak symbols, dynamic library TLS and broader toolchain/stress
   qualification remain open. The real Rust/LLD null-DT_DEBUG rejection is
   corrected in source `da6d16e6`, accepted after full session 73596 passed
   with all 505 inputs unchanged. Actual signed shared imports execute at
   zero/nonzero bias on 1/2/4 CPUs, preserving all original loader assertions.
   Main-image compiler local-exec TLS/TCB and fresh child-template initialization
   are accepted after full session 37505 passed on 515 unchanged inputs, with
   actual compiler lifecycle execution on every online CPU in 1/2/4-CPU boots.
   See the compiler-TLS implementation audit. Production ASLR arena/guard
   geometry is the next prerequisite; pure-BSS runtime and broader TLS models
   remain unqualified.
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
