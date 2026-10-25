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
remain open. Sibling-thread canonical publication now uses retained process
ownership, as qualified below. Mapping-generation serialization remains a separate
prerequisite. This increment relies on existing scheduler quiescence semantics and does not qualify a new CPU handoff protocol. The host-only
lease/journal model is still separate from production mappings. Native libc/C++,
LibCore, TLS/dependencies, engine, UI and Tor are unfinished. There is no VPN gateway
and no complete or untrackable browser claim.

## Continuous guest qualification

The focused workflow now includes separate Linux 1/2/4-CPU pager guest jobs,
with pinned Rust, warning-denying kernel lint and retained serial/build/provenance
artifacts. The harness accepts explicit OVMF CODE and an optional VARS template;
only a per-run VARS copy is writable. Windows adjacent-firmware defaults remain.
The original Linux matrix passed on dac3686e; the expanded sibling gate awaits
qualification on its new revision.

Broader release validation on 3695a253 failed the Stage20.2 focused taskbar pixel
assertion and Stage20.15 launcher pixel assertion on one CPU. Those failures are
unresolved; their causes have not been attributed to this increment. Existing
assertions and production rendering are preserved. Focused pager acceptance is
not complete release acceptance.

## Sibling-thread publication regression

The pager previously looked up a faulting task as the process leader. A sibling
could therefore populate real pages without updating the owning process's
resident bitmap. Production publication now uses the process identity retained
by the queued work. The regression creates a read-only two-page lazy mapping,
closes the descriptor, reads each page from a raw-entry sibling, joins it, and
exits. The kernel independently checks two canonical resident pages and exactly
two pager requests/completions before reaping, then removes the fixture file,
reclaims unused cache pages and checks exact frame/mapping/process baselines.

The original production lookup fails this new fixture with zero canonical
resident pages (negative serial retained); the corrected lookup passes on
1/2/4 CPUs. The sibling uses existing default placement; the earlier real pager
workload separately exercises remote leader faults on SMP. No simultaneous
unmap/exec/fork claim follows from this fixture. The resident inspection helper
and test calls compile only in wovenweb-vm-test. Existing authority stays fixed.

The preceding portable gate revision dac3686e passed all three Linux guest jobs,
SDK checks and Rust/PR CodeQL checks. The expanded sibling gate now awaits its
own hosted qualification; historical success is not assigned to a new head.

Current-source heap, TCP/SOCKS, C++ ABI and legacy GUI regression matrices also
pass on 1/2/4 CPUs. Default kernel and all four affected WovenWeb feature lint
gates pass with -D warnings; the Rust probe release lint passes. The local lint
driver initially omitted the C++ fixture image environment variable; its failure
log is retained and the identical lint passed with the real fixture supplied.
Root host executables remain policy-blocked and were not retried.
