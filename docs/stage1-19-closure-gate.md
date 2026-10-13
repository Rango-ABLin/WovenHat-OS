# Stage 1–19 closure gate (mandatory before Stage 20)

**Status: BLOCKED / NOT ACCEPTED.** This is a closure policy and evidence ledger, **not** a claim that stages are complete. Baseline is accessible `master` at `a54ae319`; reconcile any later Stage 19 source and exact commit SHA before release decisions.

## Decision rules
- **Implemented**: source exists. **Verified**: reproducible positive, negative and concurrency tests pass at a named SHA. **Closed**: every stage exit criterion and regression gate passes with retained evidence, no open P0/P1 defects. **Production-ready**: additionally hardware, power-loss, security, installer/recovery, updates and rollback validated on a documented support matrix.
- No stage may be marked **Closed** from a marker, old green run, documentation, or QEMU exit 33 alone.
- Every entry must identify owner, source SHA, exact commands, CI run URL, artifacts, negative tests, review approval, known limitations and disposition. Unknown evidence = **BLOCKED**.
- Stage 20 feature development may be prototyped separately but must not be declared an accepted successor until this gate passes.
- Any failure invalidates closure for affected stage and downstream dependencies. Fix on isolated branches, rerun affected + inherited regressions, update this ledger, then review.

## Mandatory cross-cutting acceptance
1. Pin exact source/branch and compare to reported Stage 19.7 source. Verify build, `cargo clippy` with `-D warnings`, host/unit/property tests, CI workflow coverage and 1/2/4 CPU QEMU gates.
2. Preserve per-stage artifacts and serial markers with SHA, workflow attempt, logs, machine configuration and exit codes. Test failure paths and malformed inputs, not just happy paths.
3. Check ownership, cancellation, generation-tag reuse, process exit, concurrent teardown, SMP races, lock ordering and bounded resource exhaustion.
4. Prove WovenGuard authorization at every actual syscall, VFS operation, IPC/handle transfer, DMA mapping and shared-window surface mapping; test denial and recursive revocation.
5. Demonstrate durable WovenFS via typed VFS across reboot/remount, journal crash injection, corrupt volume refusal, exclusive writable mounts and safe unmount. Qualify FAT32 recovery and persistent snapshots separately.
6. Exercise real supported hardware for storage power-loss/flush/DMA, network and radio drivers, input, graphics and boot; publish unsupported devices.
7. Demonstrate userspace ABI/toolchain, ELF applications, compositor isolation, GUI surface lifecycle, installer, signed updates, rollback, recovery, security audit and release reproducibility before claiming **production-ready**.

## Stage ledger — initial conservative status
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

## Known blocking defects / evidence gaps
- Latest observed PR #42 release-validation rerun failed; inspect workflow steps that mutate `storage_manager.rs` and fix at the actual failing source before reacceptance.
- WovenFS native `wfs.rs` is not established as a production VFS mount; legacy `wovenfs.rs` metadata is not a substitute.
- WovenGuard enforcement and revocation coverage is not established across all boundary types.
- Real device qualification, full installer/recovery/update path and production application toolchain remain unproven.
- The connected `master` is older than the user's reported Stage 19.7 tree. **Do not merge older patches over newer code without comparison.**

## Closure record template
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
