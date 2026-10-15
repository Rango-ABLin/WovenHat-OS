# Handle generation review — source findings, not acceptance

This read-only review was performed while the thread-generation runtime chain
waited behind Claude's live release validation. No kernel source or frozen
manifest was changed for this review. No additional test chain was started.

## IPC process-local handles: rollover candidate

Reviewed `kernel/src/ipc.rs` SHA-256:
`95ca75ca48176d9522455ee199abbff4f271ca5f8088bc9972499a5cb0bccddb`.

`Handle` is a 32-bit ABI value: the low byte encodes slot plus one; the upper
24 bits encode generation. `HandleSpace::allocate_tagged` selects the first
empty entry regardless of its previous generation, computes
`wrapping_add(1) & 0x00ff_ffff`, and changes zero to one. `remove` clears the
entry but preserves its generation counter. `lookup_raw` checks only the
selected entry's current generation against the supplied handle generation.

Source inspection therefore predicts that repeatedly closing and allocating
the same slot in a surviving process can eventually reissue an old handle.
This is a candidate violation of the documented stale-authority guarantee,
not yet a passing or failing executable regression. Cross-process numeric
handle reuse is a separate matter because lookups are process-local.

Before advancing compiler-TLS work, after the already integrated thread
correction's acceptance, reproduce this boundary against actual production
code. Exercise ordinary capacity/reuse and exhausted-slot refusal; cover
endpoint, shared-memory, delegation, transfer, service lookup, failed allocation
reference rollback, and owner teardown where they share this allocation path.
Preserve the 32-bit ABI, rights and lineage checks. Review all call sites before
choosing permanent slot retirement or another non-reissuing design. Run the
earlier IPC/security preservation gates as well as inherited 1/2/4-CPU gates;
do not treat low-generation SMP loops as exhaustion evidence.

### Allocation and rollback call-site review

The configured process-local capacity is 16 slots. The allocator is shared by
five production installation paths, so an exhaustion fix must preserve more
than direct endpoint creation:

| Path | Existing failure/ownership behavior to preserve |
| --- | --- |
| `create_endpoint_object` | Removes the newly installed object on handle allocation failure. |
| `grant_handle` | Retains the delegated object first, then releases that reference if target allocation fails. Rights, lineage and service class remain attached. |
| `install_shared_memory` | Removes the new shared-object entry on handle allocation failure; frame ownership must still follow the caller's existing rollback contract. |
| `receive_handle` | Allocates before dequeue; allocation failure leaves the message and owning transfer escrow untouched. Successful install consumes that escrow reference directly. |
| `discover_service` | Retains the service object first, then releases that reference if client allocation fails. |

`close_handle` removes the entry, removes waiters keyed by owner and the exact
handle, then releases the object reference. `unregister` drains the owner's
handle space and removes owner waiters; a subsequent process-local handle space
starts empty. Those source relationships explain why the immediate regression
must retain one surviving process/handle space: it is the same owner's stale
handle, not merely equal integers issued in different process-local namespaces.
No claim about full rollback correctness is made from this read-only review;
the table records the behaviors that executable exhaustion tests must exercise.

### Prepared diagnostic, not executed

`audit-artifacts/thread-generation-20261015/prepare-ipc-generation-probe.py`
is prepared outside Cargo inputs and the 135-file source freeze. It requires
an explicit expected IPC source SHA-256, extracts the actual `Handle`, object
reference, handle entry and handle-space definitions/methods without rewriting
their allocator logic, and checks the configured 16-slot capacity. It will
compile with warnings denied and preserve command output, extracted/probe
hashes and exit status in a unique evidence directory. It does not change the
production allocator.

Five cases cover ordinary capacity/reuse, metadata round-trip, same-process
stale-handle reissue after seeded rollover, skipping an exhausted slot and
permanent all-slot exhaustion refusal. WovenGuard authorization deliberately
panics if used: no stub can accidentally stand in for security acceptance.
The probe does not model paging, scheduling, reference counting or teardown.
It has **not been executed or accepted**; run it only in a serialized slot,
after the current thread correction gate, then add production-source and
inherited runtime coverage before accepting any IPC correction.

## Compared earlier-stage implementations

- `async_op::Table::allocate` skips free slots at `u32::MAX` before increment.
- `completion_port::create` requires an unowned slot below `u32::MAX` before
  incrementing under its allocation lock.
- `async_events::create` likewise skips exhausted unowned timer/event slots.

These are source-level observations only, not a new runtime qualification or
an exhaustive audit of every kernel authority. Other wrapping generation and
object-ID counters require their own lifetime/collision analysis; a wrapping
counter alone does not prove exploitable stale authority.
