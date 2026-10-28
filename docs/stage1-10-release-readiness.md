# Stages 1–10 release qualification — 2026-10-16

Status: **NOT RELEASE-READY**. The user has prioritized stages 1–10 for
desktop and laptop release preparation. This does not establish support for
every desktop/laptop, and testing cannot establish an absolute absence of bugs.
No known critical defect or missing mandatory qualification is waived.

The unvalidated Stage 11 planner is preserved separately on
`codex/aslr-layout-draft-20261016`, commit `db49c1a`; it is not part of this
foundation baseline. The confirmed production guard defect remains a release
blocker wherever the affected loader is shipped, regardless of stage label.

## Ordered closure gates

| Order | Required evidence | Open release conditions |
| --- | --- | --- |
| 1–5: boot, memory, paging, tasks, VFS/storage and userspace foundation | Actual source review; host malformed-input/fault tests; kernel/host warning-denying lint; independent 1/2/4-CPU boot and storage gates; optimized-image tests; retained device/recovery evidence | Bounded journal versus arbitrary/full-volume operations; torn-sector, flush ordering and reboot/power-loss recovery; native storage/DMA qualification |
| 6: multicore foundation | 1/2/4-CPU repeated memory/TLB/preemption/migration gates; 2/4-CPU hotplug; lock order and teardown; low-memory/NUMA/high-APIC-ID evidence | Broader cross-layer contention, low-memory and priority-inheritance qualification; physical multicore/NUMA/hotplug matrix |
| 7: isolation | Actual COW/permissions and pointer-copy tests; every relevant CPU; guard-fault and capacity regressions; cancellation and exact reclamation | Confirmed randomized mmap/stack-guard defect must be corrected and accepted before release |
| 8: IPC | Exhausted generations, ownership/escrow/revocation, bounded capacity, copyout/error/cancel/owner-teardown and cross-CPU evidence | Fresh same-source inherited acceptance, broader concurrency and failure stress |
| 9: security | Capability ceilings/lineage, sandbox and resource denial, generation reuse, audited lock order and negative authority tests | Explicit release threat model and hardware trust assumptions; no synthetic test treated as firmware qualification |
| 10: async I/O/network/completion/timers | Exact full Stage 10.7 launcher, all prior assertions, live host-verified network; 10.8/10.9 matrices on 1/2/4 CPUs | Fresh frozen-chain evidence; completion/cancel/close/teardown stress and supported device qualification |
| Final delivery | Immutable source/toolchain/firmware and image identity, artifact checksums, clean integrated tree, release notes, reproducible validation commands | No stale-image packaging; no release claim before all preceding conditions have evidence |

Existing later-stage implementation is not proof that earlier release gates
are closed. Preserve ABI and assertions, diagnose failures, and accept each
correction separately after relevant inherited preservation. Do not enlarge
capacity, disable tests, suppress warnings or increase timeouts to obtain green.

## Evidence integrity correction in progress

The legacy memory/storage/hotplug/network final checks required exit 33 and
success-marker substrings but did not globally reject contradictory failure
or panic output. Shared `scripts/qemu_validation.py` now requires line-anchored
markers, the exact successful exit, and no tagged FAILED/TIMEOUT or kernel
panic. Intentional user faults remain valid isolation evidence. Negative host
tests cover both ordering directions, missing/forged marker substrings, wrong
CPU/exit, detailed successes and every consumer's actual imported validator.
Integration is not full acceptance; fresh boot evidence is still required.

## Desktop and laptop qualification

The current host reports HP Pavilion x360 Convertible 14-dw1xxx (laptop).
Read-only inventory is not a boot or hardware qualification. No desktop model
has been selected or qualified in this pass. Record exact tested firmware,
CPU, RAM, storage/controller, display and input configuration for each supported
model and distinguish supported from experimental devices. Suspend/resume,
battery and lid behavior need explicit laptop evidence if included in the
release contract. USB image read-back alone does not prove physical boot,
durability, power-loss recovery or compatibility. No disk write or erasure is
authorized merely by this document.
