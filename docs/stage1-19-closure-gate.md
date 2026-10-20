# Stage 1–19 production closure gate

**Baseline:** `phase2/userspace-platform`, accepted TLS increment `a2a9b35`.

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
| 11 | Processes, scheduler-backed threads, signed wait/join results, join/cancel/teardown, permanent exhausted-thread-record retirement, fixed-ET_EXEC/nonzero-PIE placement, real signed compiler shared-PIE execution, deterministic ASLR candidates, corrected bounded ASLR arena/guard geometry, W^X, RELRO, bounded main-image and shared-library TLS, lifecycle callbacks, REL/PLT tables, symbol versioning, weak symbols, and bounded signed shared-object graph loading with cross-image relocation | Pure-BSS runtime and broader TLS models, entropy/stress qualification, general untested toolchain interoperability, and physical qualification |
| 12 | Typed VFS, metadata, crypto, snapshots and mount boundaries pass bounded gates | Native WovenFS as the mounted system filesystem, durable crash/power-loss evidence, full rollback and persistent key provisioning |
| 13 | QEMU driver/input/storage/audio/HID foundations pass their accepted gates | Native PCIe/NVMe/AHCI/xHCI/HDA/Wi-Fi/Bluetooth/input qualification, hotplug and fault recovery |
| 14 | Bounded IPv4/IPv6, DHCP/DNS, sockets and firewall paths pass QEMU gates | Live IPv6 socket lifecycle, routing/policy breadth and physical NIC/radio qualification |
| 15 | Permission broker, signed packages, software secure/measured boundaries, volatile vault and ledger ABI pass bounded gates | Firmware Secure Boot/TPM provisioning, persistent vault, key rotation/revocation, graphical consent and durable security retention |
| 16–18 | Out-of-kernel apps, font/compositing and desktop roadmap software are exercised by retained QEMU gates | Toolchain/runtime expansion, installer/recovery/update integration, native display/GPU/input qualification and platform support policy |
| 19 | Compositor, surfaces, lease/focus, event waits, window manager, scaling, damage, apps and catalog pass bounded software gates | Persistent signed-media update/rollback, service-owned capability provisioning, hardware GPU/display qualification, and complete application lifecycle/security review |

## Hardware preflight

The latest read-only inventory was captured at:

`audit-artifacts/physical-host-20261020-171457-651/host.json`

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
   Legacy REL/PLT tables, constructors/destructors, versioned/weak symbols,
   and dynamic library TLS are now accepted in bounded signed fixtures. The
   real Rust/LLD null-DT_DEBUG rejection is
   corrected in source `da6d16e6`, accepted after full session 73596 passed
   with all 505 inputs unchanged. Actual signed shared imports execute at
   zero/nonzero bias on 1/2/4 CPUs, preserving all original loader assertions.
   Main-image compiler local-exec TLS/TCB, shared-library PT_TLS/TPOFF64, and
   fresh child-template initialization are accepted after the final compiler
   fixture passed on every online CPU in 1/2/4-CPU boots. See the compiler-TLS
   and shared-library audits. The bounded ASLR arena/guard geometry correction
   is also accepted with dedicated 1/2/4-CPU evidence. Pure-BSS runtime,
   broader TLS models, entropy/stress qualification, and physical validation
   remain unqualified.
   See the shared-library audit for the exact
   implemented graph, permission and teardown contract.
5. Close the native WovenFS mount/write-through policy and recovery evidence
   on a deliberately selected non-boot device.
6. The bounded A/B update transition, signed `WUPD` envelope verification, and
   scratch-device durable `WUAB` record acceptance are implemented and
   accepted in `docs/audit-stage27-1-update-ab-state-2026-10-20.md`. Still add
   a long-lived service lifecycle, power-loss/recovery evidence, boot-slot and
   installer integration before calling the desktop/application stages
   complete.
7. Re-run the inherited 1/2/4-CPU matrix and release chain after each accepted
   increment; commit each accepted increment separately.

This ledger does not mark any stage complete merely because a bounded gate
passes. It is the release decision record for the current branch.

## Earlier closure policy and evidence (merged from `master`)

> Preserved from the `master` ledger (baseline `a54ae319`) when `master` was
> merged into `phase2/userspace-platform`. The closure ledger above
> supersedes its per-stage status table. Its decision rules, cross-cutting
> acceptance list, closure record template and the Stage 12 and 15.2 evidence
> below still apply.

**Status: BLOCKED / NOT ACCEPTED.** This is a closure policy and evidence ledger, **not** a claim that stages are complete. Baseline is accessible `master` at `a54ae319`; reconcile any later Stage 19 source and exact commit SHA before release decisions.

### Decision rules
- **Implemented**: source exists. **Verified**: reproducible positive, negative and concurrency tests pass at a named SHA. **Closed**: every stage exit criterion and regression gate passes with retained evidence, no open P0/P1 defects. **Production-ready**: additionally hardware, power-loss, security, installer/recovery, updates and rollback validated on a documented support matrix.
- No stage may be marked **Closed** from a marker, old green run, documentation, or QEMU exit 33 alone.
- Every entry must identify owner, source SHA, exact commands, CI run URL, artifacts, negative tests, review approval, known limitations and disposition. Unknown evidence = **BLOCKED**.
- Stage 20 feature development may be prototyped separately but must not be declared an accepted successor until this gate passes.
- Any failure invalidates closure for affected stage and downstream dependencies. Fix on isolated branches, rerun affected + inherited regressions, update this ledger, then review.

### Mandatory cross-cutting acceptance
1. Pin exact source/branch and compare to reported Stage 19.7 source. Verify build, `cargo clippy` with `-D warnings`, host/unit/property tests, CI workflow coverage and 1/2/4 CPU QEMU gates.
2. Preserve per-stage artifacts and serial markers with SHA, workflow attempt, logs, machine configuration and exit codes. Test failure paths and malformed inputs, not just happy paths.
3. Check ownership, cancellation, generation-tag reuse, process exit, concurrent teardown, SMP races, lock ordering and bounded resource exhaustion.
4. Prove WovenGuard authorization at every actual syscall, VFS operation, IPC/handle transfer, DMA mapping and shared-window surface mapping; test denial and recursive revocation.
5. Demonstrate durable WovenFS via typed VFS across reboot/remount, journal crash injection, corrupt volume refusal, exclusive writable mounts and safe unmount. Qualify FAT32 recovery and persistent snapshots separately.
6. Exercise real supported hardware for storage power-loss/flush/DMA, network and radio drivers, input, graphics and boot; publish unsupported devices.
7. Demonstrate userspace ABI/toolchain, ELF applications, compositor isolation, GUI surface lifecycle, installer, signed updates, rollback, recovery, security audit and release reproducibility before claiming **production-ready**.

### Stage ledger — initial conservative status
| Stage | Scope | Current closure decision | Required evidence to close |
|---|---|---|---|
| 1–5 | Boot, paging, scheduler, VFS, FAT32 | BLOCKED | Current SHA CI, durable crash recovery, physical storage qualification |
| 6 | SMP, TLB shootdown, hotplug | BLOCKED | Dedicated CI, races and 1/2/4 CPU stress |
| 7 | Ring-3, scheduler/pager safety | BLOCKED | Dedicated CI, teardown and fault injection |
| 8 | IPC, shared memory, capabilities | BLOCKED | Dedicated CI, stale handles and transfer/revocation tests |
| 9 | WovenGuard | BLOCKED | End-to-end enforcement and negative tests |
| 10 | Async TCP, completion, timers | BLOCKED | Current full CI and cancellation/teardown tests |
| 11 | Userspace ABI, ELF, libwoven | BLOCKED | Userspace SDK and real ELF application execution |
| 12 | WovenFS, crypto, snapshots, mounts | BLOCKED | VFS persistence, durable snapshots, power-loss, all stage gates |
| 13 | PCIe and device drivers | BLOCKED | PCIe topology/hotplug, hardware DMA, device matrix |
| 14 | WovenNet and firewall | BLOCKED | Current full CI, live IPv6 and policy enforcement coverage |
| 15 | Native WovenFS and integration | BLOCKED | Mount namespace, VFS integration, crash and remount |
| 16 | Stage 16 deliverables | UNVERIFIED | Identify source SHA, specification and acceptance artifacts |
| 17 | Stage 17 deliverables | UNVERIFIED | Identify source SHA, specification and acceptance artifacts |
| 18 | Stage 18 deliverables | UNVERIFIED | Identify source SHA, specification and acceptance artifacts |
| 19 | Compositor/shared surfaces (reported 19.7) | UNVERIFIED | Identify source; secure mapping, publication, revocation, SMP and compositor tests |

### Known blocking defects / evidence gaps
- Latest observed PR #42 release-validation rerun failed; inspect workflow steps that mutate `storage_manager.rs` and fix at the actual failing source before reacceptance.
- WovenFS native `wfs.rs` now has a bounded writable VFS adapter with explicit
  publication/unmount invalidation and 1/2/4-CPU scratch-device evidence, but
  it is not yet the mounted system filesystem and has no physical flash or
  power-loss qualification.
- WovenGuard enforcement and revocation coverage is not established across all boundary types.
- Real device qualification, full installer/recovery/update path and production application toolchain remain unproven.
- The connected `master` is older than the user's reported Stage 19.7 tree. **Do not merge older patches over newer code without comparison.**

### Closure record template
```
Stage:
Owner:
Baseline branch / commit SHA:
Scope / exit criteria:
Build + Clippy command and result:
Host / negative / concurrency test evidence:
QEMU 1 CPU run/artifact:
QEMU 2 CPU run/artifact:
QEMU 4 CPU run/artifact:
Hardware / power-loss evidence or explicitly documented exemption:
Security review / denial tests:
Open P0/P1 defects:
CI URL / attempt:
Architecture & Codebase Guide update:
Reviewer / approval date:
Decision: BLOCKED | VERIFIED | CLOSED
```

**Stage 20 unlock condition:** every Stage 1–19 entry is CLOSED on the same reconciled source baseline, all inherited CI gates are green, and independent security/release reviewers approve the closure ledger. Production-ready is a separate, stronger signoff.

### Stage 12 evidence review — CI run 38373420895 (2026-10-13)
- **Source SHA:** `de9ee133a023aae6ced38733d7d58c5b6c64b71e` on the closure branch.
- **Job:** `release-validation`, job `115215724350`, **success**.
- **Stage 12.1–12.5:** each passed QEMU **1, 2 and 4 CPU** acceptance with expected exit **33** (15/15 executions).
- **Clippy:** `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage12-{1..5}-test -- -D warnings` executed successfully for each stage (feature strings expanded separately).
- **Source integrity:** per-stage `storage_manager.rs` SHA checks passed; the earlier undefined `i` error did not recur. This is evidence of reproducibility in this run, not proof of the earlier failure's root cause.
- **Disposition:** Stage 12 **software gate VERIFIED** on this SHA; Stage 12 **closure BLOCKED**. Hardware qualification, durable snapshots, and native filesystem/VFS integration remain unproven.
- **Artifacts:** GitHub Actions run https://github.com/Rango-ABLin/WovenHat-OS/actions/runs/38373420895 ; retained job artifact `release-validation` and per-CPU Stage 12 audit directories.

### Stage 15.2 native WovenFS integration findings
- `kernel/src/wfs.rs` implements a genuine journaled on-disk `Volume` with format, mount, create, read, write and unlink primitives.
- `mount_ata_into_vfs()` imports validated entries into `/wfs` and owns a
  serialized publication lifecycle. Remount invalidates cached descendants and
  open descriptions before importing the fresh journal-replayed tree.
- Existing import treats `AlreadyExists` as imported without refreshing contents; a remount must not silently retain stale cached data. Define mount namespace ownership and unmount/refresh semantics before enabling write-back.
- `with_ata_volume()` mounts for the duration of a closure, but this is not an end-to-end VFS-backed writable mount.
- **Remaining evidence:** exercise reboot/remount, stale handles, failure
  injection and crash recovery on the normal VFS path, then qualify a selected
  non-boot physical device. The bounded software adapter and mount lifecycle
  are implemented; this does not close the hardware or system-filesystem gate.
