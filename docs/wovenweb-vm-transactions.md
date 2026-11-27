# WovenWeb VM transaction ownership

Status: host-tested state machine; production integration remains open.
`kernel/src/vm_lease.rs` is compiled by `tests/vm_lease.rs`. No current mapping
call uses this registry, so it does not yet fix the kernel race.

A process VM identity includes an owner and a non-reused address-space generation.
Callers serialize the registry under the same lock that checks mapping occupancy.
They reserve the complete slot set before dropping that lock to perform a page
operation. A lease is non-Copy, non-Clone and must-use. Completion validates the
identity and transaction sequence before publishing or rolling back. Sequence
exhaustion refuses admission rather than wrapping. A multi-slot lease is atomic
for thread stack/TLS creation and join cleanup.

Fork snapshots and exec replacement require an exclusive lease: no concurrent
mapping mutations and no overlapping snapshot. Retirement immediately closes
admission. Existing operations finish while the space is retained. Only after
all leases drain and actual page teardown succeeds can destruction be recorded.
Ambiguous rollback quarantines its slots; exclusive ambiguity quarantines the
whole space. Quarantined mappings may be destroyed with the owner but not reused.
A dropped/lost token must retain the address space rather than permit destruction;
production integration needs task-death recovery for outstanding leases.

## Required integration

- `mmap_current` and `mmap_file_current`: check Ready process and occupancy,
  reserve slot, perform mapping outside registry lock, publish under identity
  check, finish after successful commit or confirmed rollback.
- `munmap_current` and `msync_current`: reserve the exact current mapping while
  validating bounds/authority. Prevent unmap or reuse until operation completes.
- `create_user_thread` and `join_user_thread`: reserve stack/TLS as one set and
  coordinate scheduler/thread-resource publication. A losing creator must never
  unmap a winner's pages. A losing joiner must never free a reused mapping.
- Lazy pager enqueue/completion, page fault resolution, COW and writeback: retain
  the space and mapping generation across requests; late completions cannot
  update a replaced mapping. Pager work must drain on process death.
- `fork_current`: hold an exclusive snapshot lease through page-table clone and
  child publication/rollback; the child receives a fresh registry identity.
- `exec_current`: retire sibling tasks and drain mutations before replacing the
  root. The existing Process is Copy, so a lease registry must not be copied into
  process snapshots; keep ownership separate or remove snapshot-by-Copy safely.
- Process exit/reap, signals and cancellation: close admission first, cancel or
  finish outstanding page work, wait for remote sibling handoff and leases to
  drain, then destroy page tables and recycle process slots. No blocking under
  scheduler/process-table IRQ locks. Define event-driven wakeup for drained state.

Nine tests cover atomic stack/TLS admission, disjoint mapping work, stale-completion
ABA refusal, owner-generation mismatch, sequence exhaustion, snapshot exclusion,
retirement with in-flight operations, quarantine and 512 two-operation schedules.
These are host model tests, not an SMP kernel or page-table proof. Preserve the
existing 64 KiB/16-slot ceiling until integrated guest tests cover these paths.

## Cancellation journal prerequisite

`kernel/src/vm_journal.rs` adds a bounded per-address-space operation journal,
compiled with the lease model by `tests/vm_lease.rs`. It remains host-only.
The journal owns each non-Copy lease; executors receive opaque generation-tagged
tickets. Losing a task stack therefore does not discard lease/cleanup intent.
Full capacity and sequence exhaustion refuse admission without creating a lease.
Late and foreign tickets cannot mutate a replacement operation.

A scheduler-confirmed quiescence boundary is required before cleanup can claim
an operation. A cancellation request or Dead state without remote CPU handoff
and deferred IO completion is insufficient. Recovery descriptors retain original
intent and the optional recorded application result. If a cleanup worker dies,
its confirmed quiescence permits a new worker to claim a new cleanup generation;
the old worker's completion is stale. Uncertain cleanup quarantines its slots.
Cleanup sequence exhaustion retains ownership and blocks destruction.

The journal does not inspect real page tables or pin backing-file objects. Its
caller must retain those resources, validate actual executor identity/liveness,
and serialize journal plus canonical mapping occupancy under the same lock.
Recovery must compare authoritative mapping generations: an executor can die
after physical mapping or canonical publication but before recording that step.
A missing applied marker is never permission to blindly unmap. Current host
coverage includes 128 crash/publication/cleanup-handoff schedules.

Production integration still needs a registry stored independently of Copy
Process snapshots, pre-operation durable intent, canonical mapping generations,
real scheduler/pager quiescence witnesses, backing-resource retention, event-driven
drain, idempotent page cleanup, and thread/fork/exec/exit/reap gates. The existing
production mapping race is not repaired by adding this host model.
