# Handle generation review — bounded IPC correction accepted

## Final serialized IPC acceptance — 2026-10-15

Session 63244 completed with exit 0 and
`=== IPC GENERATION FULL ACCEPTANCE: PASS ===`. Final evidence:
`audit-artifacts/ipc-generation-20261015/revision2/attempt-20261015-185944-027/`.
The unchanged 488-file baseline passed all three revised focused boots;
build, host all-targets Clippy, all host Rust tests (including the durable
actual-source allocator regressions), sixteen libwoven tests and fifteen
Python tests; production kernel Clippy and fourteen affected-feature Clippy
configurations, all with warnings denied; exact full `RUN-STAGE10.7.ps1`
preservation; and all twenty-one Stage 10.8–11.5 runtime boots on 1/2/4 CPUs.
The full launcher evidence is `audit-artifacts/acceptance-20261015-190547-079/`.

Inherited memory stress retained all 10/30/20 one-/two-/four-CPU serial logs
at `stage7-6-artifacts/20261015-190549-058/`; every log contains the new IPC
exhaustion marker. Live network and isolated asynchronous I/O/TCP preservation
also passed on 1/2/4 CPUs. Independent post-run inspection checked all 21 stage
result files, three final focused markers and fourteen feature-lint completions;
all 488 original hashes still matched. No foreign compiler/test process was
found at that inspection. Earlier failed source/test-ordering evidence and
manifests were preserved, not replaced or counted as acceptance.

This accepts permanent retirement of exhausted process-local IPC handle slots,
with five installation-path rollback/teardown coverage, as implemented in
`998abd05` and the durable/runtime regressions in `8d6addea`. The 32-bit handle
ABI, normal 16-slot capacity, authorization/metadata, reference ownership and
existing lock boundaries are preserved. Exhausted slots remain permanently
unavailable within the surviving namespace; a newly registered namespace is
distinct. This is not full Stage 7–11 closure, combined-tree/release acceptance,
or native hardware qualification. Process/thread signed exit-result encoding,
compiler TLS and broader loader interoperability remain open. Pending/read-only
statements below describe the retained investigation timeline, not this final
decision.

## Executable IPC boundary finding — 2026-10-15

After session 79248 completed its thread-generation gate with exit 0, the
serialized actual-source allocator probe reproduced all three exhaustion
defects. Evidence:
`audit-artifacts/thread-generation-20261015/ipc-generation-probe-1792082615316658700/`.
Warning-denying compilation passed; execution exited 101 with three failing
boundary tests and two passing ordinary-capacity/metadata tests. The surviving
namespace reissued stale raw handle `257`. Exhausted-slot skipping and permanent
all-slot refusal also failed. These are now executable defects, not only source
candidates.

The integrated allocator selects an empty slot only when its preserved
generation is below `0x00ff_ffff`, then increments without wrapping. The last
generation remains usable until close; subsequent allocation permanently skips
that exhausted slot. No handle format, rights, lineage, service class, object
reference path, lock, allocation or wait was added or changed. The same five
actual-source probe tests pass with warnings denied in
`ipc-generation-probe-1792082651878246200/`. Each evidence directory retains
source/extraction hashes, compiler version, commands, logs and exit statuses.

This is **not accepted IPC production closure**: persistent repository boundary
regressions, all five installation/rollback paths, owner/waiter/frame cleanup,
host/kernel lint and inherited 1/2/4-CPU runtime preservation remain required.
The original 135-file freeze is now historical evidence of the accepted thread
increment; the IPC edit deliberately begins a new correction baseline. Do not
replace that original manifest or claim its old runtime results cover this edit.
The earlier read-only findings and prepared-diagnostic descriptions below are
retained investigation history.

Formatting and production freestanding kernel Clippy (`-D warnings`) pass.
The first PowerShell wrapper reported exit 1 despite Cargo's successful compile;
its transcript is retained in `ipc-production-clippy.txt`. Repeating through
explicit native-exit capture produced `IPC production Clippy exit: 0`, retained
in `ipc-production-clippy-repeat.txt`. Neither check establishes host-all-target
or affected-feature/runtime acceptance. Those remain required.

## Durable and production-state regressions — in progress

`tests/ipc_handles.rs` now invokes the repository-owned
`scripts/test-ipc-handle-generation.py` from `cargo test`. The runner extracts
the actual allocator unchanged, fails closed if its extraction/configuration
boundaries change, compiles with warnings denied, and runs the five boundary,
capacity and metadata cases. Generated compiler/command/hash evidence uses
unique directories under `audit-artifacts/`. The authorization double panics
if invoked; these tests do not substitute for security or paging acceptance.
The durable host test and `qemu-test` kernel Clippy passed before the final
teardown extension.

`kernel/src/ipc_generation_tests.rs` is guarded by `qemu-test`, including its
call inside the existing Stage 8.1 post-SMP probe. It uses the real global state,
authorization and public shared-frame allocator. Only empty counters in newly
registered synthetic namespaces are seeded; no live generation is modified,
no exhausted counter is reset, and cleanup only touches registered test owners.
The test covers valid last-generation handles; repeated endpoint, delegation,
discovery and shared-memory allocation failures; exact reference rollback;
failed transfer FIFO/escrow/waiter preservation; a successful retry into an
ordinary freed slot; stale handles and metadata; publisher/service exit; an
actual pending owner waiter; abandoned queued escrow; receiver teardown; and
exact object/service/lineage/frame baselines. Synthetic waiter IDs are inspected
only through private State return values and are never signaled to the scheduler.
There is no new unsafe code, production syscall, capability or lock. Frame
allocation is outside the existing state guard; established higher-rank frame
release paths are unchanged. No scheduling occurs under that guard.

The initial focused matrix passed on 1/2/4 CPUs at
`audit-artifacts/ipc-focused-20261015-185039-168/`, but review extended the probe
to retain queued escrow and an actual waiter during publisher teardown. The
first extended run failed on one CPU and stopped before host/full acceptance:
`audit-artifacts/ipc-generation-20261015/attempt-20261015-185442-871/`.
The diagnostic counterpart, source and logs are retained in
`diagnostic-waiter-order-20261015-1858/`. It showed correct reference counts
and exact cleanup baselines, but a second send had legitimately consumed the
receive waiter before the test expected close to return it again. The test now
asserts close-time waiter removal on the empty original channel and uses a
replacement channel for independent escrow/owner teardown. This retains the
waiter assertion and adds stale-channel validation; no production behavior,
timeout or prior assertion was disabled to obtain a pass.

A new 488-file freeze and full serialized attempt are under
`audit-artifacts/ipc-generation-20261015/revision2/`. They do not overwrite
either earlier manifest or failed evidence. Focused 1/2/4 boots, build, host
all-targets Clippy, host/libwoven/Python tests, production and affected-feature
Clippy, exact full Stage 10.7 preservation and Stage 10.8–11.5 matrices are
required before acceptance. This final revised chain is still running.

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

## IPC boundary and runtime regression requirements

The IPC encoding differs from the thread registry: all 24 generation bits can
be used in a valid live handle, including `0x00ff_ffff`. The exhausted state is
an **empty** slot whose preserved generation already equals that maximum.
Keep the last issued handle valid until close; skip its slot on subsequent
allocation rather than wrapping or rejecting the last live generation. Normal
capacity remains 16, while a surviving namespace can permanently lose exhausted
slots. `count()` measures live entries, not remaining allocatable capacity.

For the next correction, seed generation counters only in synthetic namespaces
owned by the regression, under the existing IPC state lock. Never rewrite the
generation of a live entry or reset exhausted counters to manufacture cleanup.
An endpoint receive handle needed to test transfer must remain in a separate,
non-exhausted live slot. Required production-path assertions are:

- Endpoint creation into an all-retired namespace returns `HandleTableFull`
  without changing endpoint-object count; repeated failures remain bounded.
- Failed delegated installation preserves source authority, rights and lineage
  and returns its temporary object reference. Closing the source afterward
  releases the object exactly once.
- Failed shared-memory installation leaves no registered shared object. Test
  frame ownership at the public allocation wrapper as well as the private
  install method: the latter does not itself own the caller's failure cleanup.
- Failed transfer receive preserves the queue head, payload, transferred
  object's escrow reference and sender waiters. Freeing an ordinary receiver
  slot permits exactly one successful dequeue; an exhausted slot must not be
  revived. Verify installed rights/lineage and stale-handle rejection afterward.
- Failed service discovery preserves publication and its owning escrow while
  releasing the attempted client reference. Publisher teardown removes service
  discoverability, and surviving delegated/queued references follow their
  existing independent lifetimes.
- Unregister drains both live last-generation handles and ordinary handles,
  removes owner waiters, and restores object/frame baselines. A different newly
  registered process-local namespace may start at generation one; that is not
  permission to recycle handles inside a surviving namespace.

The existing boot-time handle self-test and post-SMP Stage 8.1 probe cover
ordinary reuse only; neither currently seeds the 24-bit boundary. Retain those
assertions and the Stage 8.2–8.6 messaging, blocking, shared-memory, transfer and
service acceptance. Extend boundary coverage without adding a production
syscall for generation mutation, new capabilities, lock nesting, or scheduling
under the IPC lock. These are reviewed requirements, not executed test results.

## Compared earlier-stage implementations

- `async_op::Table::allocate` skips free slots at `u32::MAX` before increment.
- `completion_port::create` requires an unowned slot below `u32::MAX` before
  incrementing under its allocation lock.
- `async_events::create` likewise skips exhausted unowned timer/event slots.

These are source-level observations only, not a new runtime qualification or
an exhaustive audit of every kernel authority. Other wrapping generation and
object-ID counters require their own lifetime/collision analysis; a wrapping
counter alone does not prove exploitable stale authority.
