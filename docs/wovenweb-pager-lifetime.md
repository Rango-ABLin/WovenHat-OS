# Production pager ownership boundary

The production pager now retains an exited process's address space while its
requests are queued or active. Queue pop atomically transfers ownership to the
single worker; a sequence-qualified completion releases it only after page work
and canonical publication. Admission reserves eventual completion capacity within
the existing 16-outstanding-work bound. Sequence exhaustion refuses admission.
Reaping removes orphan completions only after scheduler quiescence and no pending
owner work. ABI, capabilities, allocator defaults and VM ceilings are unchanged.

Fault admission holds PAGER (rank 10) through process validation (rank 20) and
Blocked publication under SCHEDULER (rank 10). All IRQ guards are dropped before
IPI delivery or stack switching. The worker performs page operations outside the
queue lock. Reaping checks scheduler quiescence, drains owner metadata under PAGER,
then rechecks ownership and Exited state under PROCESS_TABLE before destruction.

## Focused qualification

Seven exact-production-module host tests pass, including 64 owner-pair schedules,
wrong/stale completion identities, reserved capacity and exhaustion. A separate
wovenweb-vm-test gate runs after the existing real Ring-3 pager workload. On 1/2/4
CPUs that workload completed eight of eight faults; the new fixture then proves
queued and active work prevent zombie reaping, and matching completion permits
exact frame/mapping/process baseline recovery. The fixture uses a real child and
mapping but synthetic ownership metadata, without fake page I/O. Its pause hook
exists only in the test feature. All three runs use the same recorded image.

Current-source heap, TCP/SOCKS, C++ ABI and legacy default-allocator GUI
regressions also pass on 1/2/4 CPUs.
Default production kernel, four affected WovenWeb feature configurations and the
Rust probe pass warning-denying Clippy. Root host lint and SDK unit execution are
still blocked locally by Windows signing policy; no policy was bypassed. Hosted
queue tests/lint passed on 3695a253 alongside 179 SDK and three compile-fail checks.
Complete Rust analysis and the PR CodeQL check also succeeded with no new alerts.
See [machine-readable evidence](wovenweb-pager-evidence.json) for logs and hashes.

## Remaining boundaries

This closes a pager-versus-reaper lifetime boundary, not all address-space races.
Exec, raw unmap, fork, mapping reservations/capacity and full VM journal integration
remain open. Thread-fault canonical publication still uses the existing leader
lookup and needs separate correction. This increment relies on existing scheduler
quiescence semantics and does not qualify a new CPU handoff protocol. The host-only
lease/journal model is still separate from production mappings. Native libc/C++,
LibCore, TLS/dependencies, engine, UI and Tor are unfinished. There is no VPN gateway
and no complete or untrackable browser claim.

## Continuous guest qualification

The focused workflow now includes separate Linux 1/2/4-CPU pager guest jobs,
with pinned Rust, warning-denying kernel lint and retained serial/build/provenance
artifacts. The harness accepts explicit OVMF CODE and an optional VARS template;
only a per-run VARS copy is writable. Windows adjacent-firmware defaults remain.
The Linux matrix is newly added and awaits hosted qualification.

Broader release validation on 3695a253 failed the Stage20.2 focused taskbar pixel
assertion and Stage20.15 launcher pixel assertion on one CPU. Those failures are
unresolved; their causes have not been attributed to this increment. Existing
assertions and production rendering are preserved. Focused pager acceptance is
not complete release acceptance.
