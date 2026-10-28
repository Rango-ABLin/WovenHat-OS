# WovenHat OS consolidated stage-audit archive

Generated from historical stage-audit records on 2026-10-25. Each source path is retained as a section heading; no audit content was discarded.

---

## Stage 32.11 GDB session state machine - 2026-10-28

The bounded session lifecycle passed 1/2/4-CPU QEMU self-tests. Stop replies,
running-state command refusal, continue/step action translation, and kill
deactivation are covered. Target and host warning-denying Clippy, host tests,
and Python syntax validation passed. Trap ownership, task binding, transport
integration, and execution control remain outside this acceptance.

## Stage 32.10 GDB register context and command dispatch - 2026-10-28

The pure GDB command boundary passed 1/2/4-CPU QEMU self-tests. Register
encoding, bounded memory reads, malformed/out-of-range refusal, feature
negotiation, and continue/step/kill action classification are covered. Target
and host warning-denying Clippy plus host tests passed. Live trap/session
integration remains outside this acceptance.

## Stage 32.9 second-UART debug serial - 2026-10-27

The bounded COM2 16550 transport passed its loopback and second-QEMU-serial
marker gate at 1/2/4 CPUs with exit 33. Target and host warning-denying Clippy
and host tests passed. The result is transport-boundary evidence only; live
GDB command handling and trap/debug lifecycle remain outside this increment.

## Stage 20.8 resize-surface diagnostic cleanup - 2026-10-27

The WovenShell repaint path previously logged expected shared-surface blit
failures while a client was between the old and replacement surfaces during
`WSIZ`/`WRSZ`. The path now uses the existing bounded resize grace deadline;
post-deadline failures remain observable. Stage 20.8 passed at 1/2/4 CPUs,
with target and host warning-denying Clippy plus host tests passing.

## Stage 20 reported-gate revalidation - 2026-10-27

The seven gates reported red by the desktop UX sweep were rerun against
current commit `744b5839`: Stage 20.2, 20.3, 20.4, 20.6, 20.8, 20.9, and
20.13. Every gate passed at 1, 2, and 4 CPUs with QEMU exit 33. The affected
Stage 20 image-backed feature gates, production kernel lint, and host
`--all-targets` Clippy passed. No assertion, timeout, or source behavior was
weakened; the earlier 2-CPU 20.6 result was an intermittent failure and passed
on a clean rerun. Evidence remains in the corresponding `target/woven-shell-*`
CPU directories.

## Source: AUDIT-STAGE10.1-KERNEL-EVENT-ASYNC-IO.md

# WovenHat OS Stage 10.1 â€” Kernel Event + Async I/O Foundation

## Objective
Stage 10.1 converts the already-worker-backed block-I/O queue into a genuinely
event-driven scheduler path. The previous implementation queued work on a
storage worker but the submitting task repeatedly called `yield_now()` until
its request became complete. The worker was asynchronous; the waiter was still
polling.

## New control flow

```text
caller task
   |
   | queue request + TaskId waiter
   | (completion event armed before worker wake)
   v
block I/O queue ----------------------+
   |                                  |
   | wake worker                      | caller: wait_for_event()
   v                                  |        -> Blocked
storage worker                        |
   |                                  |
   | perform ATA operation            |
   | publish RequestState::Complete   |
   | signal_event(waiter TaskId) -----+
   v
scheduler marks waiter Ready / latches pending permit
   |
   v
caller rechecks its request and consumes result
```

## Race property
The Stage 8.3 event is scheduler-latched. If completion occurs before the task
actually blocks, `signal_event` stores the pending permit in the TCB. The later
`wait_for_event` consumes that permit instead of sleeping. If the task is
already blocked, the same signal makes it Ready. Thus completion cannot vanish
in the check/sleep window.

Each queued block request is armed with its waiter before the worker is woken,
which also prevents completion from occurring without a corresponding event
for the submitter. Unrelated task events are treated as spurious wakeups: the
request state is rechecked and the task waits again if necessary.

## Acceptance proof
`stage10_1_event_driven_completion_valid()` executes a real queued ATA request
through the block worker and requires all of these counters to advance:

- queued requests
- completed requests
- scheduler event waits
- worker completion signals

It also requires the request table to return to zero active entries.

The Stage 10.1 acceptance wrapper first runs the complete validated Stage 9.5
acceptance chain unchanged, then requires the Stage 10.1 production marker in
1-, 2-, and 4-CPU memory-regression serial logs.

## Security boundary
Stage 10.1 does not bypass Stage 9.5. Resource authorization remains at the
existing WovenGuard device/network/storage gates. This stage changes scheduling
and completion mechanics, not who is authorized to perform I/O.
---

## Source: AUDIT-STAGE10.2-GENERIC-ASYNC-REQUEST-COMPLETION.md

# WovenHat OS Stage 10.2 â€” Generic Async Request / Completion API

## Goal
Generalize the Stage 10.1 event-driven block-I/O proof into one bounded kernel asynchronous-operation substrate that future storage, network, device, service, graphics, input, USB, and AI-service paths can reuse.

## Architecture
Stage 10.2 adds `kernel/src/async_op.rs` with:

- a fixed-capacity, allocation-free global operation table;
- generation-tagged `Handle` values to reject stale references;
- owner `TaskId` binding;
- explicit `AsyncClass` identity (`Block`, `Network`, `Device`, `Service`);
- `Pending` and `Complete` lifecycle state;
- compact completion payload (`status`, `value`);
- scheduler-event-backed blocking waits;
- producer completion from a different task/worker;
- rollback release when a producer cannot enqueue its real work;
- counters for allocations, completions, waits, scheduler signals, releases, stale-handle rejection, and live operations.

## Lost-wakeup rule
Wait registration and completion are serialized by the async table lock. If the owner registers first, completion signals that task through the Stage 8.3 scheduler event. If completion occurs first, the `Complete` state itself is observed by the later wait, so no completion is lost.

Unrelated scheduler events may wake a task; the waiter therefore rechecks its generation-tagged operation state until its own completion is visible.

## Real block-I/O integration
`kernel/src/block_io.rs` no longer owns its own task waiter identity. A queued block request now stores an `async_op::Handle`.

1. submitter allocates `AsyncClass::Block`;
2. request is inserted into the bounded storage queue;
3. enqueue rollback releases the generic handle if the storage queue is full;
4. storage worker writes the request result/data into the storage queue;
5. storage worker calls `async_op::complete`;
6. owner calls `async_op::wait` and then consumes the block result.

This preserves Stage 10.1's non-polling storage behavior while moving completion ownership into the generic substrate.

## Stage 10.2 production proof
`stage10_2_generic_async_completion_valid()` requires:

- all four async classes to allocate and round-trip completion metadata;
- completion-before-wait to succeed;
- a stale consumed handle to be rejected;
- generic allocation/completion/release/stale counters to advance;
- a real queued ATA request to pass through the generic API;
- both generic async and block-I/O active tables to drain back to zero.

Kernel marker:

```text
[S10.2] generic async request/completion API: PASSED
```

## Preservation contract
`run-stage10-2-acceptance.ps1` runs the complete validated Stage 10.1 acceptance chain unchanged first, then requires the Stage 10.2 production marker on the existing 1/2/4 CPU production logs.

No existing timeout, stress count, warning policy, network test, WovenGuard test, IPC test, or SMP criterion is weakened.


## SMP runtime hardening
A later 2-CPU stress timeout exposed two worker-wakeup hazards. The worker is
now identified by TaskId rather than a global IN_WORKER flag, and producer
notification uses signal_event()/wait_for_event() instead of wake_task() plus
periodic sleep. See AUDIT-STAGE10.2-SMP-RUNTIME-FIX.md.
---

## Source: AUDIT-STAGE10.2-SMP-RUNTIME-FIX.md

# WovenHat OS Stage 10.2 SMP Runtime Fix

## Trigger
The strict Stage 10.2 acceptance run passed host tests, all ten 1-CPU boots,
and 29 of 30 2-CPU boots. Run 30 timed out. The failure is intermittent and
therefore consistent with an SMP scheduling/wakeup race rather than a build or
lint defect.

## Root concurrency defects addressed

### 1. Global `IN_WORKER` was not task-local
`block_io::should_queue()` used a global AtomicBool set while the block-I/O
worker was performing ATA I/O. On SMP, another task on another CPU could
observe that flag and incorrectly take the direct-I/O path even though it was
not the worker. This breaks the intended serialization model and can expose
rare cross-CPU timing and device-access contention.

The global flag is removed. Queue bypass now depends on identity: the current
task bypasses queueing only when it is the actual registered block-I/O worker
(or when queueing is unsafe because interrupts are disabled / no normal task is
running).

### 2. Producer-to-worker wake had an enqueue-vs-sleep race
The worker previously did:

    process_one_primary() == false
    -> sleep_current(16)

A producer did:

    enqueue request
    -> wake_task(worker)

If enqueue happened after the worker's empty-queue observation but before the
worker published Sleeping, `wake_task` could legally return false because the
worker was still Running. The worker would then sleep despite queued work.
Repeated unlucky timing under SMP stress could stretch progress sufficiently
to hit the boot acceptance timeout.

The worker now uses the Stage 8.3 lost-wakeup-safe scheduler event protocol:

    worker: drain queue -> wait_for_event()
    producer: enqueue -> signal_event(worker)

If the signal arrives before the worker blocks, `event_pending` is latched. If
it arrives after Blocked is published, the signal makes the worker Ready. No
third lost-wakeup state exists.

## Acceptance policy
No timeout, stress count, marker, or preservation check was weakened. Run the
same command:

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\RUN-STAGE10.2.ps1

---

## Source: AUDIT-STAGE10.3-USERSPACE-ASYNC-COMPLETION-ABI.md

# WovenHat OS Stage 10.3 Audit â€” Userspace Async Completion ABI

## Objective

Expose the Stage 10.2 generic request/completion substrate safely to Ring 3 while
preserving event-driven waiting, WovenGuard least privilege, generation safety,
and deterministic process teardown.

## Architectural changes

### `kernel/src/async_op.rs`

- Adds stable `Handle::to_raw/from_raw` encoding with reserved-bit validation.
- Makes the completion ABI explicitly 16 bytes (`status`, reserved, `value`).
- Adds non-consuming `peek_current` and `wait_ready` so syscall copyout can fail
  without destroying a ready result.
- Adds owner cancellation and task-owner reclamation.
- Adds owner-managed completion only for `AsyncClass::Service`.
- Keeps general `complete()` available to trusted kernel producers such as
  block I/O workers.

### `kernel/src/syscall.rs`

Adds syscalls 64â€“68: create, poll, wait, cancel and service-complete. Block,
Network and Device creation is gated through the existing Stage 9.5 resource
policy; Service uses current effective IPC authority. Poll/wait recheck current
authority so a previously created handle cannot bypass later policy tightening.
Cancellation intentionally remains owner-available after tightening so teardown
cannot be blocked by revocation.

### `kernel/src/userspace.rs`

The real bootstrap Ring-3 program exercises the ABI through `int 0x80`, including
pending poll, completion-before-wait, completion copyout, stale rejection,
cancellation, and one intentionally abandoned operation. libc-style stubs are
also exported for syscalls 64â€“68.

### `kernel/src/task.rs`

Both normal process exit and scheduler/signal-driven termination call
`async_op::release_owner(task_id)`, preventing leaked operation-table capacity.
The async table lock is the lifecycle linearization point versus producer
completion.

### `kernel/src/main.rs`

Boot acceptance requires Ring-3 syscall coverage, zero live async operations,
and owner-reap evidence before emitting the Stage 10.3 marker.

## Safety invariants

1. A raw handle cannot name generation zero or use reserved bits.
2. Only the owning task may inspect, wait, cancel, or service-complete a handle.
3. Hardware-class completion remains kernel-only.
4. Completion-before-wait is latched in table state.
5. Copyout failure leaves the completed slot intact for retry/cancel.
6. Cancellation and exit teardown invalidate the generation before late producer
   completion can affect a future occupant.
7. Current WovenGuard authority is re-evaluated on create/poll/wait.
8. Policy revocation cannot prevent owner cleanup.

## Acceptance

`RUN-STAGE10.3.ps1` preserves the complete Stage 10.2 chain and then requires the
Stage 10.3 production marker in 1-, 2- and 4-CPU serial logs.

## SMP preservation fix â€” 2026-09-14
A Stage 7.6 preservation stress run timed out on 2 CPUs at run 8 after seven consecutive passes. The Stage 10.3 Ring-3 probe originally allocated and unmapped a dedicated page solely to hold the 16-byte completion record. That unnecessarily coupled the new async ABI test to the VM allocator during every legacy userspace bootstrap. The probe now uses 32 bytes of scratch space below the existing mapped userspace stack pointer. This preserves the exact Ring-3 copyout validation while removing unrelated mmap/munmap paging churn from the preservation stress path.
---

## Source: AUDIT-STAGE10.4-PRESERVATION-ISOLATION-FIX.md

# Stage 10.4 Preservation Isolation Fix

The first Stage 10.4 package executed its new real block-I/O Ring-3 probe in every `qemu-test` production boot. That meant the Stage 7.6 preservation stress suite was no longer testing the accepted Stage 10.3 workload: each legacy boot also performed new Stage 10.4 storage work. The first 1-CPU preservation boot timed out before a Stage 10.4 marker was produced.

This fix introduces the compile-time feature `stage10-4-test`. Ordinary `qemu-test` builds preserve the exact Stage 10.3 runtime path and do not spawn the Stage 10.4 storage service. The Stage 10.4 acceptance runner first completes the entire Stage 10.3 preservation chain, then performs separate 1/2/4 CPU boots with `stage10-4-test` enabled and requires the Stage 10.4 marker.

This is test isolation, not a timeout increase or weakened assertion. The Stage 10.4 implementation, WovenGuard StorageIo boundary, bounce buffers, cancellation, stale-handle checks and teardown remain intact.
---

## Source: AUDIT-STAGE10.4-USERSPACE-ASYNC-BLOCK-IO.md

# Stage 10.4 Audit â€” Userspace Asynchronous Block I/O

## Boundary introduced
Stage 10.4 connects Ring-3 to the existing real block worker without exposing user
virtual addresses to asynchronous worker context. The API is intentionally a
privileged storage-service surface, not a general application raw-disk API.

## Invariants
- User-domain capability ceiling is unchanged; raw storage requires `StorageIo`.
- Block handles are minted only by `AsyncBlockRead`/`AsyncBlockWrite`.
- Every request has a TaskId owner and generation-tagged async handle.
- The worker owns/copies a 512-byte kernel bounce buffer; it never retains a user pointer.
- Completion copyout occurs before queue/handle consumption and is retry-safe.
- Cancellation removes the queue association before invalidating the async handle.
- Exit and forced termination remove owner queue entries before generic async owner reap.
- Queue slot reuse is protected by request-id matching; late workers cannot attach to a new request.
- The acceptance write targets LBA `u64::MAX`; ATA bounds validation rejects it before media I/O.

## Security
A new `spawn_user_system_service` path creates Ring-3 service tasks only when the
requested capability set is within the SystemService domain ceiling. Stage 10.4's
probe receives only `StorageIo`. Ordinary User-domain programs keep the existing
userspace capability set and cannot call the raw block API successfully.

## Acceptance evidence required
The Stage 10.4 serial marker is emitted only after a dedicated Ring-3 service exits
0 and telemetry proves at least four queued operations, at least two worker
completions, zero residual block queue entries, zero generic async slots, and an
owner-reap increment. The outer PowerShell acceptance then requires that marker on
1, 2 and 4 CPU production boots after the full Stage 10.3 regression chain.


## Preservation build dead-code fix
The isolated Stage 10.4 probe is feature-gated. Its helper spawn API, ELF-construction function, and linker-symbol declarations are now gated with `#[cfg(feature = "stage10-4-test")]` as well. This keeps ordinary qemu-test preservation builds warning-free under `-D warnings` while compiling the helpers normally in dedicated Stage 10.4 boots. No `allow(dead_code)` suppression was added.

## 1-CPU blocking-syscall handoff fix
The isolated Stage 10.4 run exposed the first genuinely pending Ring-3 async wait. `int 0x80` enters through an interrupt gate with IF cleared, while WovenHat's stack switch saves RSP/callee-saved registers but not RFLAGS. `task::wait_for_event()` previously switched to the replacement task with IF still cleared, allowing the replacement/bootstrap task to reach HLT with interrupts disabled and stall indefinitely on one CPU.

The event-wait handoff now enables interrupts only for the switched-away execution interval. When the blocked syscall task resumes after `switch_stacks`, its original IF state is restored before returning through the syscall frame. This preserves the syscall's interrupt-gate semantics while ensuring the scheduled replacement task can receive timer/IPI interrupts.

Dedicated `stage10-4-test` boots now exit through the QEMU debug-exit port immediately after the Stage 10.4 marker. Full Stage 10.3 production validation is run first by the acceptance chain, so later legacy boot assertions are not contaminated by the extra storage-service process. The QEMU runner also prints the serial tail automatically on any future timeout.

The same IF-handoff correction is applied to `sleep_current()`, because Ring-3 nanosleep reaches the scheduler through the same interrupt-gate boundary and depends on timer interrupts to wake. This fixes the root scheduler class rather than special-casing block I/O.

## Final scheduler IF isolation fix

The first blocking Ring-3 block-I/O wait exposed that the low-level scheduler context switch saved only RSP plus callee-saved registers, not RFLAGS. Because syscall/interrupt gates clear IF, a resumed task could inherit another task's interrupt-disabled state. An attempted high-level workaround enabled interrupts before `switch_stacks`, but that violated the pre-existing GDT/TSS invariant requiring the privilege-stack hand-off to happen with interrupts disabled.

The final fix is architectural: `wovenhat_context_switch` now saves/restores RFLAGS as part of every schedulable kernel context. All synthetic first-dispatch stacks (kernel task, user task, fork child) include the corresponding RFLAGS slot with IF initially clear. High-level event/sleep paths no longer enable interrupts before `switch_stacks`; they preserve the atomic GDT/CR3/stack transition and rely on the incoming task's own saved flags. This keeps interrupt state task-local and preserves the scheduler's existing critical-section contract.


FINAL SUBMISSION-PATH FIX
- Ring-3 async block submission no longer calls the synchronous `should_queue()` predicate.
  int 0x80 enters with IF clear by design; asynchronous submission may safely copy and enqueue
  kernel-owned request state with IF=0, then return or block later through AsyncBlockWait.
- Stage 10.4 acceptance failures exit QEMU immediately with isa-debug-exit failure status instead
  of halting until the host timeout, so future faults are reported directly rather than disguised
  as hangs.
---

## Source: AUDIT-STAGE10.5-USERSPACE-ASYNC-VFS-FILE-IO.md

# Stage 10.5 Audit â€” Userspace Asynchronous VFS/File I/O

## Security boundary
Stage 10.5 does not grant `StorageIo` to ordinary applications. File requests are authorized through the existing `FileRead` / `FileWrite` capabilities and WovenGuard file-scope policy.

## Stable object identity
Submission resolves the process fd once and clones the underlying refcounted `OpenFileId`. The request therefore survives `close(fd)` and cannot be redirected if the same descriptor number is reused. The acceptance probe closes the descriptor immediately after both write and read submission and requires the correct file contents to be produced.

## Pointer lifetime
No userspace pointer is retained. Write bytes are copied into a bounded kernel request buffer before queue publication. Reads stay in a kernel buffer until the owner collects the completed operation. Completion/data copyout occurs before queue/handle consumption, preserving retry safety.

## Offset semantics
The first API is intentionally positional. `vfs::read_at` and new `vfs::write_at` avoid races on the shared open-file seek position and make concurrent operations deterministic.

## Cancellation and teardown
Pending or completed cancellation releases the pinned file immediately. If the worker already owns a copied request, cancellation/owner teardown detaches the generic completion but leaves the queue slot and VFS reference alive until the worker finishes; the worker then releases the reference without signalling a stale async handle.

## SMP validation
The Stage 10.5 probe runs only under `stage10-5-test`, after the complete Stage 10.4 acceptance is preserved. Dedicated 1/2/4 CPU boots validate read/write, fd pinning, stale-handle rejection, cancellation, owner teardown, and final request-table drain.

## Preservation-build dead-code correction

The first external acceptance attempt stopped during the ordinary Stage 10.4 preservation build because Stage 10.5-only telemetry (`Queue::active`, `Stats`, and `stats`) remained compiled while their only consumers were correctly isolated behind `stage10-5-test`. With `-D warnings`, Rust therefore rejected those three unused items before any QEMU boot occurred.

The correction keeps the isolation model intact: those telemetry definitions are now themselves compiled only with `stage10-5-test`. No warning suppression (`allow(dead_code)` / `expect(dead_code)`) is used, and no Stage 10.5 worker or probe is enabled in legacy/preservation boots. Runtime async-file submission, cancellation, teardown, VFS pinning, and WovenGuard behavior are unchanged.
---

## Source: AUDIT-STAGE10.6-USERSPACE-ASYNC-NETWORKING.md

# WovenHat OS Stage 10.6 Audit â€” Userspace Asynchronous Networking Foundation

## Scope
Stage 10.6 extends the accepted Stage 10.5 generic async substrate to ordinary User-domain UDP/socket I/O. It does not expose raw NIC rings or DMA to Ring 3 and does not weaken the `NetworkIo` capability gate.

## ABI
New syscalls:
- 77 `AsyncNetSend(socket, user_buffer, length)`
- 78 `AsyncNetRecv(socket, capacity)`
- 79 `AsyncNetPoll(handle, completion_ptr, data_ptr)`
- 80 `AsyncNetWait(handle, completion_ptr, data_ptr)`

All operations use the existing generation-tagged `AsyncClass::Network` generic handle. Completion is the stable 16-byte Stage 10.3 record (`status`, `reserved`, `value`), with `value` equal to bytes sent/received. Receive payload bytes are copied separately to the caller-provided data pointer only after successful completion.

## Pointer ownership
No Ring-3 pointer survives syscall submission. Send bytes are copied into a bounded kernel request immediately. Receive requests retain only capacity; data remains in the kernel request until collection. Completion/data copyout happens before request/handle consumption, preserving retry safety on a bad user pointer.

## Socket identity and close races
The process socket table now assigns each socket a monotonically changing generation and tracks async references. Stage 10.6 pins `(slot, generation, owner)` at submission. `close()` on a pinned socket marks it closing and prevents new synchronous or async use, but the underlying smoltcp socket remains alive until the last async reference drops. Slot reuse therefore cannot redirect an in-flight operation.

## Worker/readiness model
The bounded async-network queue has Pending/InProgress/Complete states. The worker tries socket send/receive. `WouldBlock`/buffer backpressure returns the request to Pending and the worker sleeps on the scheduler event. `network::poll()` signals the worker after smoltcp/device progress, so readiness retries are not a userspace busy-poll loop. Ring 3 sleeps independently on the generic async completion event.

## Cancellation and teardown
Owner cancellation detaches Pending requests immediately; if the worker has already copied an InProgress request, the queue clears its completion handle and the worker performs final socket-unpin cleanup when it returns. Process teardown calls `async_network::release_owner()` before generic async handle teardown. Normal process socket close marks any still-pinned descriptors closing, so final async unpin removes the underlying socket deterministically.

## WovenGuard
Submission and result collection both recheck `DeviceClass::Network`, which maps to `Capability::NetworkIo`. Cancellation remains owner-only and does not require a renewed authority check, preserving safe cleanup if policy tightens after submission.

## Acceptance isolation
`stage10-6-test` implies `qemu-test` but suppresses the older live NETTEST block during the dedicated boot so it does not compete for host traffic. The Stage 10.6 harness attaches a VirtIO NIC and QEMU user network, injects a real 16-byte UDP payload through a host-forward into guest port 7001, and validates 1/2/4 CPU boots. Before these dedicated boots, the complete Stage 10.5 acceptance chain is rerun unchanged.

The Ring-3 probe exercises:
1. asynchronous UDP send followed by immediate socket close, proving send-side socket pinning;
2. asynchronous UDP receive followed by immediate close, real host packet delivery, and byte-for-byte payload validation;
3. stale-handle rejection after consumption;
4. explicit cancellation of a pending receive;
5. an abandoned pinned receive reclaimed by process teardown;
6. zero leaked async operations and zero leaked user sockets at the end.

## Validation command
`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\RUN-STAGE10.6.ps1`

Stage 10.6 is not closed until that acceptance prints `=== STAGE 10.6 ACCEPTANCE: PASS ===`.

## Runtime fix: connected UDP auto-bind and permanent-error isolation

The first isolated Stage 10.6 boot exposed a liveness defect in the new async UDP send path. The Ring-3 probe opened a UDP socket and connected it to the QEMU DNS proxy without first binding a local port. smoltcp requires a local UDP endpoint before transmission. The pinned async send path collapsed every send error into `WouldBlock`, so an unbound/addressing failure was treated as transient readiness backpressure and retried forever.

The socket layer now gives connected UDP sockets normal implicit-bind semantics: `socket_connect()` allocates an ephemeral local port when a datagram socket is still unbound, retrying a bounded number of ports before returning `Address`. The pinned send path also verifies that the UDP socket is open before attempting transmission, so a permanent addressing defect cannot enter the async retry loop.

This is an architectural liveness fix rather than a probe-only workaround. It preserves generation pinning, deferred close, owner binding, WovenGuard `NetworkIo`, kernel-owned buffers, cancellation, and readiness-driven retries for genuinely transient send-buffer pressure.
---

## Source: AUDIT-STAGE10.7-FINAL-PRESERVATION-FIX.md

# Stage 10.7 final preservation fix

## Failure class
Stage 10.6 preservation could stall before the Ring-3 UDP readiness marker after Stage 10.7 development changed the async-network readiness bridge.

## Root cause
The accepted Stage 10.6 worker was woken only when the bounded async-network queue contained a Pending request. A later Stage 10.7 SMP experiment changed `network_progress()` to signal the worker on every `network::poll()`. Because scheduler events are latched, this could leave a stale event permit while a network request was retrying. On a single CPU the worker could consume that permit and retry before smoltcp had useful new progress, disturbing the proven Stage 10.6 scheduling/readiness behavior.

A second test-contract weakness was that the host readiness marker was published after `bind(7001)` but before `AsyncNetRecv` was submitted. The final handshake now publishes readiness only after both the bind and async receive submission succeed.

## Fix
1. Restore Stage 10.6's accepted `QUEUE.has_pending()` gate in `network_progress()`.
2. Keep Ring-3-owned readiness, but emit it after syscall 78 successfully creates the pinned async receive request.
3. Keep the Stage 10.7 dedicated test isolated from the legacy live network regression.
4. Make `RUN-STAGE10.7.ps1` set persistent project-local Cargo build/target directories so Windows temporary-directory cleanup cannot invalidate bootloader compilation.

No timeout, security policy, capability, assertion, or acceptance criterion was weakened.
---

## Source: AUDIT-STAGE10.7-HOST-HARNESS-TIMED-WAIT-COMPATIBILITY.md

# Stage 10.7 Host Harness Timed-Wait Compatibility Repair

Date: 2026-09-18

## Scope

The Stage 10.7 SMP liveness repair changed the production async-network worker to use the kernel's timed scheduler wait (`timer::ticks()` plus `task::wait_for_event_until()`) as a bounded liveness backstop after each queue pass.

The production 4-CPU Stage 10.7 QEMU test passed, but the preservation acceptance chain failed earlier in the host `async_network` test because that test compiles the production worker against local scheduler/timer doubles. Those doubles still exposed only the older `wait_for_event()` surface and no `timer` module.

## Repair

`tests/async_network.rs` now supplies the two production interfaces required by the worker:

- `timer::ticks() -> u64`, returning a deterministic host-test tick value.
- `task::wait_for_event_until(u64)`, using the same `Parked` unwind boundary as the existing `wait_for_event()` double.

This is a host-harness compatibility repair only. It does not alter the production networking, scheduler, async-operation ABI, socket semantics, cancellation, teardown, or SMP liveness implementation.

## Validation requirement

Run the authoritative Windows Stage 10.7 gate. The repair is not accepted until the host tests and the preservation/current 1/2/4-CPU QEMU gates pass.
---

## Source: AUDIT-STAGE10.7-NETWORK-WAKEUP-HANDOFF-FIX.md

# Stage 10.7 network wakeup handoff fix

## Failure evidence
The restored Stage 10.6 preservation probe passed on 1 and 2 CPUs but could hang on 4 CPUs after the UDP target-ready marker. This localized the defect to SMP readiness delivery after socket setup, not boot, userspace startup, or host injection.

## Root cause
The async network worker changes a request from `Pending` to `InProgress` while attempting the socket operation. `network_progress()` historically signaled the worker only when it observed a `Pending` request. On SMP, smoltcp could publish readiness on another CPU while the request was `InProgress`; the progress signal was then suppressed. If the worker's socket attempt still returned `WouldBlock`, it put the request back to `Pending` and slept with the readiness edge already lost.

## Fix
A small `AtomicBool` bridge, `NETWORK_WORK_IN_PROGRESS`, is armed before a pending request is removed from the queue and disarmed only after the request has either returned to `Pending` or completed. `network_progress()` signals when either a queued request is `Pending` or the worker is inside this protected InProgress window.

This provides continuous wakeup coverage across the state transition:

`Pending -> InProgress -> Pending -> wait_for_event`

A progress edge can therefore never fall into a state where both the queue test and the handoff bridge are false. The existing scheduler event latch still protects the final signal-before-wait window.

## Preserved properties
- no timeout relaxation
- no busy polling introduced
- no capability or WovenGuard weakening
- Stage 10.6 host harness remains the last accepted baseline
- Stage 10.7 TCP uses the same corrected worker path
---

## Source: AUDIT-STAGE10.7-S10.6-RING3-READINESS-FIX.md

# Stage 10.7 preservation fix: Stage 10.6 Ring-3 UDP readiness handshake

## Failure
During Stage 10.7 preservation, the previously accepted Stage 10.6 isolated UDP test could hang on SMP after the kernel printed its receive-ready marker. The marker was emitted immediately after spawning the userspace probe, before Ring-3 had actually created and bound UDP port 7001.

## Root cause
The host harness began UDP injection on a process-spawn marker rather than an actual socket-readiness event. SMP scheduling changes the interval between process spawn and the Ring-3 bind syscall. QEMU user networking could therefore observe forwarded UDP traffic before the destination socket existed, making preservation timing-dependent.

## Fix
The Stage 10.6 Ring-3 probe now writes `[S10.6] Ring-3 UDP/7001 bound: READY` through the normal userspace write syscall only after `bind(..., 7001)` succeeds. The host harness waits for this Ring-3 marker before sending the acceptance datagram. The old kernel marker was renamed to state only that the probe process was spawned.

The previously applied async-network wakeup hardening remains in place: network progress always signals the worker, relying on the scheduler's latched event semantics.

## Safety properties preserved
- No timeout increase.
- No assertion or acceptance weakening.
- No WovenGuard capability changes.
- No socket ownership/generation changes.
- No busy-polling added.
- Stage 10.6 still uses a real host-to-guest UDP datagram.
- Stage 10.7 still preserves the complete Stage 10.6 acceptance chain before its dedicated TCP tests.
---

## Source: AUDIT-STAGE10.7-USERSPACE-ASYNC-TCP.md

# Stage 10.7 â€” Userspace Asynchronous TCP Audit

## Goal
Extend the accepted Stage 10.6 generation-pinned async socket foundation from
UDP to TCP connection-state transitions without introducing polling in Ring 3.

## Design
- Syscall 81 `AsyncTcpConnect(socket, packed_endpoint)` allocates an owner-bound
  `AsyncClass::Network` handle and pins the exact socket generation.
- The async-network worker owns connect readiness progression. `network::poll()`
  signals the worker after smoltcp progress; connect retries only while the TCP
  socket is active but not yet send-capable.
- TCP send distinguishes genuine connection closure from temporary TX
  backpressure.
- TCP receive returns `Ok(0)` after remote FIN once no buffered bytes remain,
  giving userspace conventional EOF semantics.
- Existing Stage 10.6 deferred close means descriptors can be closed while an
  async TCP operation still references the socket; the object is removed after
  its final async reference is released.
- Send bytes are copied synchronously into a kernel buffer. Receive bytes stay
  kernel-owned until retry-safe completion copyout. No Ring-3 pointer survives
  an asynchronous operation.
- `NetworkIo` remains required at submission and collection. Cancellation is
  owner-permitted so policy tightening cannot prevent safe teardown.

## Acceptance
1. Preserve the complete accepted Stage 10.6 chain.
2. On 1/2/4 CPUs, connect to a real host TCP server through QEMU user networking.
3. Connection A proves async connect + async send + close-before-completion.
4. Connection B proves async connect/send/receive and exact host-returned bytes.
5. Host half-close/close proves async receive completes with zero-byte EOF.
6. An unreachable endpoint proves deterministic cancellation and stale-handle
   rejection.
7. A second abandoned unreachable connect proves owner teardown reclaims both
   the generic async operation and pinned socket reference.

Stage 10.7 is closed only after `=== STAGE 10.7 ACCEPTANCE: PASS ===` on the
user's QEMU environment.

## Isolation fix after first acceptance run

The first isolated Stage 10.7 boot reached the legacy qemu-test live-network regression before the Stage 10.7 block. Because the Stage 10.7 host harness services the dedicated TCP scenario rather than the legacy UDP port 7000 echo, the kernel stopped at `[NETTEST] UDP: TIMEOUT` and never reached the asynchronous TCP probe.

The runtime guard in `kernel/src/main.rs` now excludes both `stage10-6-test` and `stage10-7-test` from the legacy NETTEST block. Stage 10.7 still initializes the same VirtIO/smoltcp stack inside its own dedicated block, starts the async-network worker there, and then executes only the Stage 10.7 TCP workload. The ordinary `network-test` regression and the complete Stage 10.6 preservation acceptance remain unchanged and still validate DHCP/DNS/ICMP/UDP/TCP separately.

This is an acceptance-isolation correction only; it does not alter TCP state handling, socket generation pinning, WovenGuard authority, cancellation, EOF semantics, or teardown behavior.

## SMP readiness wakeup correction

During Stage 10.7 preservation, the previously accepted Stage 10.6 UDP receive could
intermittently stall on 4 CPUs after the host-injection readiness marker. The root
cause was a check-then-signal race in `async_network::network_progress()`: it only
signaled the worker when the queue happened to contain a `Pending` request. The
worker temporarily marks a request `InProgress` while probing smoltcp. If network
progress arrived during that interval, the wakeup was suppressed; the worker could
then restore `Pending` after `WouldBlock` and sleep with the readiness edge already
lost.

The readiness bridge now signals the async-network worker unconditionally whenever
`network::poll()` reports progress. WovenHat scheduler events already latch a signal
that arrives before `wait_for_event()`, so the signal survives the worker's
`InProgress -> Pending -> wait` transition. This fixes the SMP lost-wakeup class for
both UDP (Stage 10.6) and TCP (Stage 10.7) without introducing a busy retry loop.
---

## Source: AUDIT-STAGE13.10A-BLUETOOTH-HCI-ACCEPTANCE.md

# Stage 13.10A Bluetooth HCI Core Acceptance

Accepted: 2026-09-30

Roadmap Stage 13.10 is Bluetooth. Historical Wi-Fi labels named Stage 13.10Aâ€“AC belong to the Stage 13.9 AX200/WovenWiFi development line and do not represent roadmap Bluetooth completion.

## Accepted scope

Stage 13.10A establishes the hardware-independent Bluetooth HCI command/event foundation in `kernel/src/bluetooth_hci.rs`.

The accepted slice provides bounded HCI command encoding, payload validation, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests. The kernel acceptance marker is:

`[S13.10A] Bluetooth HCI core foundation: PASSED`

The feature boundary is `stage13-10-test` at both workspace and kernel levels.

## Acceptance evidence

Authoritative GitHub Actions run: 36718370038
Accepted source SHA: 94565ad73b7401445ca62701b0c4f62e637cfdfe
Job: 109896881823

Dedicated Stage 13.10A acceptance passed on:
- 1 CPU, exit 33: `audit-artifacts/stage13.10-1cpu-1790773606550489976`
- 2 CPUs, exit 33: `audit-artifacts/stage13.10-2cpu-1790773610193015780`
- 4 CPUs, exit 33: `audit-artifacts/stage13.10-4cpu-1790773614608965589`

The same run also passed the release validation chain and Stage 13.3 through Stage 13.9 preservation gates.

## Architectural boundary

This acceptance does not claim a Bluetooth hardware driver or usable Bluetooth connection. Stage 13.10A intentionally does not activate xHCI. USB Bluetooth transport, controller capability discovery, ACL transport, discovery/scanning, L2CAP, pairing/security, ATT/GATT/BLE services, lifecycle/hotplug, and physical hardware qualification remain later work.

Next roadmap slice: Stage 13.10B, Bluetooth USB HCI transport over the existing xHCI foundation.
---

## Source: AUDIT-STAGE13.10B-BLUETOOTH-USB-HCI-ACCEPTANCE.md

# Stage 13.10B â€” Bluetooth USB HCI Transport Contract Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10B establishes the USB Bluetooth HCI transport contract beneath the hardware-independent Stage 13.10A HCI core.

Accepted behavior:
- Bluetooth USB interface recognition for class/subclass/protocol E0/01/01.
- Interrupt-IN endpoint discovery for HCI events.
- Bulk-IN and bulk-OUT endpoint discovery for ACL traffic.
- Endpoint packet-size and interrupt-interval capture.
- Bluetooth class control-request setup construction for HCI commands.
- Deterministic descriptor and command-setup self-test.
- Stage 13.10 feature ownership of the existing xHCI PCI MMIO/bus-master helper.
- Runtime acceptance marker: `[S13.10B] Bluetooth USB HCI transport contract: PASSED`.

## Authoritative validation

GitHub Actions run: 36722602921
Accepted source SHA: `0ba3ce257f93775afe67ed975f8549fcdbb78506`
Release-validation job: 109911272075
Result: SUCCESS

The run passed lint and host regressions, the 1/2/4-core boot matrix, Stage 13.3 NVMe, Stage 13.4 AHCI/SATA, Stage 13.5 xHCI, Stage 13.6 USB HID, Stage 13.7 WovenInput, Stage 13.8 WovenAudio, Stage 13.9 WovenWiFi, and the Stage 13.10 runtime gate. The preserved release-validation artifact has digest `sha256:69a5693202ab9f8658d9a79e81da05a28a0fedb0fdd36b35d0a731df3e922805`.

The workflow step name still reads â€œStage 13.10A Bluetooth HCI core 1/2/4-core acceptanceâ€; this is a naming debt only. The runtime harness for stage 13.10 requires the Stage 13.10B marker, so the accepted run exercised the new contract.

## Acceptance boundary

This milestone does not claim a working physical Bluetooth controller. Dedicated xHCI endpoint contexts/rings for Bluetooth, DMA buffers, live HCI command transfer, interrupt event reception, ACL transfer execution, controller initialization/capability discovery, Bluetooth discovery, L2CAP, pairing/security, ATT/GATT, hotplug/lifecycle recovery, and physical hardware qualification remain later work.

## Next

Stage 13.10C should implement live USB HCI transfer execution and controller initialization while keeping USB ownership in xHCI and HCI protocol/state handling in `bluetooth_hci.rs`.
---

## Source: AUDIT-STAGE13.10C-BLUETOOTH-HCI-TRANSFER-ACCEPTANCE.md

# Stage 13.10C â€” Bluetooth HCI Transfer Execution Contract Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10C establishes a bounded transaction/execution contract between the hardware-independent HCI state machine and the Stage 13.10B USB transport description.

Accepted behavior:
- non-copyable HCI transaction state;
- one outstanding HCI command governed by controller command credits;
- bounded command encoding;
- matching Command Complete opcode/status validation;
- Reset and Read Local Version transaction sequencing in deterministic tests;
- bounded xHCI Bluetooth command, event, ACL-IN and ACL-OUT transfer plans;
- ACL-OUT rejection above the discovered endpoint packet capacity;
- runtime marker `[S13.10C] Bluetooth HCI transfer execution contract: PASSED`.

## Authoritative validation

GitHub Actions run: 36729724615
Accepted source SHA: `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`
Release-validation job: 109935801913
Result: SUCCESS
Artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`

The run passed the generic lint/host/1-2-4 CPU matrix, Stage 13.3 through Stage 13.9 regression gates, and the dedicated Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance gate.

## Boundary

This acceptance is deterministic software-contract validation. It does not claim live physical USB Bluetooth operation. Dedicated Bluetooth endpoint contexts/rings, DMA-backed control-OUT command transfer, interrupt-IN event reception, bulk ACL execution, hardware Reset/Read Local Version, teardown/recovery, and physical qualification remain open.

## Next

Bind the accepted transfer plans to dedicated xHCI rings and DMA ownership, route transfer completions into the HCI transaction state, and execute the initial controller Reset / Read Local Version sequence when a real Bluetooth USB interface is present.
---

## Source: AUDIT-STAGE13.10D-BLUETOOTH-LIVE-HCI-ACCEPTANCE.md

# Stage 13.10D â€” Bluetooth Live USB HCI and Controller Initialization Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10D materializes the Stage 13.10B/C contracts into bounded xHCI-owned Bluetooth transport mechanics and a transport-independent controller initialization state machine.

Accepted behavior:
- endpoint-zero control-OUT-with-data path for HCI commands;
- Bluetooth E0/01/01 configuration enumeration and SET_CONFIGURATION;
- direction-aware xHCI DCI mapping;
- dedicated interrupt-IN event, bulk-IN ACL and bulk-OUT ACL endpoint contexts;
- separate rings and DMA buffers for Bluetooth event and ACL paths;
- bounded HCI event receive, ACL receive and ACL transmit primitives;
- Reset -> matching Command Complete -> Read Local Version -> matching Command Complete -> Ready sequencing;
- runtime marker `[S13.10D] Bluetooth controller initialization sequence: PASSED`.

## Authoritative validation

GitHub Actions run: 36747805657
Accepted source SHA: `41e009543e01afc6a348122ff3a2c4f0be16e351`
Release-validation job: 109998346871
Result: SUCCESS
Artifact: 11114005793
Artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`

The dedicated â€œStage 13.10D Bluetooth controller init 1/2/4-core acceptanceâ€ step passed together with the full regression chain and evidence-preservation step.

## Boundary

This acceptance proves deterministic software architecture and xHCI/HCI ownership boundaries. The QEMU acceptance configuration does not provide a Bluetooth radio, so this audit does not claim physical Bluetooth-controller qualification. Real-device endpoint behavior, timing, hotplug, teardown/recovery and interoperability remain hardware qualification work.

## Next

Stage 13.10E should build Bluetooth controller capability discovery and device discovery/scanning above the accepted HCI initialization boundary. xHCI should remain transport-only; discovery policy and protocol parsing belong above it.
---

## Source: AUDIT-STAGE13.10E-BLUETOOTH-DISCOVERY-ACCEPTANCE.md

# Stage 13.10E â€” Bluetooth Discovery and Controller Capability Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10E establishes bounded classic BR/EDR discovery and local-controller capability parsing above the accepted Stage 13.10D initialization boundary.

Accepted behavior:
- GIAC Inquiry command construction with bounded parameters;
- Inquiry Result and Inquiry Complete event parsing;
- bounded storage of 16 unique discovered Bluetooth addresses;
- retained page-scan repetition mode, class-of-device and clock offset;
- duplicate-device suppression;
- fail-closed malformed/truncated event handling;
- Read Local Version typed parsing;
- Read BD_ADDR typed parsing;
- Read Local Supported Commands 64-byte bitmap parsing;
- expected-opcode, controller-status and payload-length validation;
- runtime gates for Inquiry discovery and controller capability discovery.

## Authoritative validation

GitHub Actions run: 36753355665
Accepted source SHA: `98020bfbfa71dd68f0fc54b234761490947fe346`
Release-validation job: 110017294490
Result: SUCCESS
Artifact: 11115452902
Artifact digest: `sha256:dd94f81eed0b3916135051b78c0c27944a33084fba8405c1045dcf49e8196fe2`

The Stage 13.10E 1/2/4-core acceptance gate and evidence-preservation step passed. At the accepted SHA the runtime harness requires `[S13.10E] Bluetooth controller capability discovery: PASSED`, which executes after the Inquiry discovery self-test.

## Boundary

This acceptance is deterministic software validation. It does not claim physical Bluetooth-radio scanning or interoperability. BLE advertising reports, remote-name discovery, connection establishment, link teardown/recovery, L2CAP, pairing/security, ATT/GATT, hotplug and physical qualification remain open.

## Next

Stage 13.10F should consume bounded discovered-device records to establish and track classic Bluetooth ACL links, with explicit connection-handle ownership and disconnect/failure lifecycle before higher protocols are added.
---

## Source: AUDIT-STAGE13.10F-BLUETOOTH-ACL-LIFECYCLE-ACCEPTANCE.md

# Stage 13.10F â€” Bluetooth ACL Connection and Link Lifecycle Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10F establishes bounded classic BR/EDR connection and ACL ownership above the accepted Stage 13.10E discovery boundary.

Accepted behavior:
- Create Connection command construction from a discovered-device record;
- Connection Complete validation and bounded live-link allocation;
- 12-bit controller-handle validation;
- Bluetooth-address/handle ownership pairing;
- idempotent identical connection completion;
- fail-closed conflicting handle or address reuse;
- Disconnect command construction and Disconnection Complete cleanup;
- bounded HCI ACL packet framing/parsing with a 1024-byte software payload ceiling;
- inbound and outbound ACL data restricted to currently owned handles;
- stale ACL authority rejected immediately after disconnect;
- malformed/truncated ACL data rejected.

## Authoritative validation

GitHub Actions run: 36761870548
Accepted source SHA: `79cecd7b0e3bab5db4426e2f6fed3eaa451c0a2b`
Release-validation job: 110046196796
Result: SUCCESS
Artifact: 11118843876
Artifact digest: `sha256:878f9d8866f09fd6e202062dbfa0784073ae7dbed5e196eb223387c26b40410c`

The Stage 13.10F Bluetooth ACL link lifecycle 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim physical Bluetooth interoperability, controller ACL credit/flow-control integration, real-radio timing, hot-unplug, or transport recovery. Higher protocols such as L2CAP, pairing/security, ATT/GATT and profiles remain later stages.

## Next

Stage 13.10G should build bounded L2CAP framing and channel lifecycle on top of the owned ACL link authority established here.
---

## Source: AUDIT-STAGE13.10G-BLUETOOTH-L2CAP-ACCEPTANCE.md

# Stage 13.10G â€” Bluetooth L2CAP Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10G adds a bounded L2CAP Basic Mode and dynamic-channel ownership layer above the accepted Stage 13.10F ACL link lifecycle.

Accepted behavior:
- exact L2CAP Basic header length and CID parsing;
- rejection of CID zero and malformed/truncated frames;
- fixed-capacity L2CAP payload storage;
- L2CAP frames bound to an owned live ACL handle;
- Connection Request/Response signaling;
- dynamic local CID allocation beginning at 0x0040;
- bounded eight-channel ownership table;
- Disconnection Request/Response and deterministic channel reclamation;
- outbound data restricted to an established channel's peer CID;
- inbound data restricted to the established local CID on the same ACL handle;
- stale channel transmit/receive authority rejected immediately after teardown.

## Authoritative validation

GitHub Actions run: 36785295506
Accepted source SHA: `c2dff3c12c9c9387a10493900b3846489e8539a6`
Release-validation job: 110125141796
Result: SUCCESS
Artifact: 11129384616
Artifact digest: `sha256:6ac9ff255444ffa36128cd03d75f3d2c89eff086e308618ed21fed463c4a8eab`

The Stage 13.10G Bluetooth L2CAP framing 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim L2CAP configuration negotiation, segmentation/reassembly, enhanced retransmission/streaming modes, controller ACL credit integration, physical-radio timing, or peripheral interoperability.

## Next

Stage 13.10H should add bounded Bluetooth pairing/authentication/encryption state while preserving the ACL and L2CAP ownership boundaries established in 13.10F/G.
---

## Source: AUDIT-STAGE13.10H-BLUETOOTH-SECURITY-ACCEPTANCE.md

# Stage 13.10H â€” Bluetooth Pairing, Authentication and Encryption Acceptance

Accepted: 2026-10-01

## Scope accepted

Stage 13.10H adds deterministic BR/EDR link-security authority above the accepted Stage 13.10F ACL and 13.10G L2CAP ownership boundaries.

Accepted behavior:
- per-handle authentication/encryption state progression;
- Authentication Requested and Authentication Complete handling;
- Set Connection Encryption and Encryption Change handling;
- failures do not grant secured authority;
- bounded link-key ownership and replacement;
- Link Key Request positive reply for known peers and negative reply for unknown peers;
- Link Key Notification ingestion and explicit key revocation;
- IO Capability Request reply;
- User Confirmation Request parsing with explicit accept/reject;
- User Passkey Request parsing with bounded six-digit input and explicit rejection;
- Simple Pairing Complete success/failure parsing;
- trusted-secured authority requires the live ACL handle, matching peer identity with a stored link key, successful authentication, and controller-confirmed encryption;
- key removal and ACL teardown revoke trusted authority fail closed.

## Authoritative validation

GitHub Actions run: 36791065311
Accepted source SHA: `dda2933eef36026ab70cefbe85898bcd62256f06`
Release-validation job: 110143823518
Result: SUCCESS
Artifact: 11131973476
Artifact digest: `sha256:f731dbd2f52dc7c2743aa052eaa7f801eef99dc0fc6050f13e6fa71518a07405`

The Stage 13.10H Bluetooth security 1/2/4-core acceptance gate and evidence-preservation step passed.

## Boundary

This acceptance is deterministic software validation. It does not claim persistent encrypted key storage, full asynchronous HCI security orchestration, physical Bluetooth radio pairing/interoperability, BLE SMP, or hardware-backed key protection. The controller remains responsible for Bluetooth cryptographic link operations; WovenHat validates and gates authority from the resulting HCI state.

## Next

Continue Stage 13.10 with the BLE foundation / LE advertising and connection model while preserving the existing ACL, L2CAP and security ownership boundaries.
---

## Source: AUDIT-STAGE13.10I-BLUETOOTH-LE-FOUNDATION-ACCEPTANCE.md

# Stage 13.10I Bluetooth LE Foundation Acceptance Audit

Date: 2026-10-01

## Accepted source checkpoint

- Branch: `stage13.10i-bluetooth-le-foundation`
- Accepted implementation SHA: `edcfe31b5cd1406c4feb2d001dbc0e2d2f6d7a9f`
- GitHub Actions run: `36810460362`
- Acceptance job: Stage 13.10I Bluetooth LE foundation 1/2/4-core acceptance â€” success
- Evidence preservation â€” success
- Artifact: `release-validation` / ID `11138589418`
- Artifact digest: `sha256:f14abafba66e1a599c2410e7e458c3cd2447ca783780deb688afffbfb26b5251`

## Accepted scope

Stage 13.10I accepts the deterministic software contract for:

- LE scan-parameter and scan-enable/disable HCI command construction.
- LE Meta Advertising Report framing and exact declared-length validation.
- Public/random peer address-type ownership.
- Fixed-capacity LE discovery state with bounded legacy advertising payloads and RSSI.
- Duplicate advertising-report update without unbounded table growth.
- LE Create Connection command construction from an owned discovery record.
- LE Connection Complete parsing and fixed-capacity LE handle ownership.
- Conflict rejection for aliased/reused live handles or peer identities.
- Disconnection Complete teardown and stale-handle rejection.
- 1/2/4-core regression acceptance while preserving earlier roadmap markers.

## Fail-closed properties

Malformed event lengths, unsupported address/role values, out-of-range controller handles, conflicting live ownership and stale disconnect events are rejected. Disconnect removes LE handle authority immediately.

## Boundary / not claimed

This audit does not claim physical Bluetooth LE radio interoperability, extended advertising, LE privacy/resolving-list behavior, connection-update orchestration, ATT/GATT, BLE SMP, persistent bonding, long-term-key protection or hardware-backed key storage.

BR/EDR link-key state remains separate and is not treated as BLE bonding authority.

## Next stage

Stage 13.10J: bounded ATT/GATT foundation above live LE link ownership.
---

## Source: AUDIT-STAGE13.2-PCIE-CANDIDATE.md

# Stage 13.2 PCI/PCIe â€” implementation candidate

Status: **implemented; acceptance pending on the Windows/QEMU gate**.

## Scope implemented

- ACPI MCFG allocation parsing with checksum/range validation inherited from the ACPI table walker.
- Bounded ECAM region inventory with segment group and bus-range metadata.
- ECAM configuration-space address calculation and MMIO access, with legacy PCI configuration mechanism #1 fallback when MCFG is unavailable.
- SMP-safe serialization of legacy CF8/CFC configuration transactions and configuration read/modify/write operations.
- Rich PCI function identity: segment/BDF, vendor/device, revision, programming interface, class/subclass, header type, command/status.
- Assigned BAR decoding for I/O, 32-bit MMIO and 64-bit MMIO BARs.
- Bounded standard capability traversal for PM, MSI, MSI-X and PCIe capability discovery, including malformed-chain detection.
- Existing `read_config_dword`, `write_config_dword`, `enable_io_bus_master`, `bar0_io_base`, and inventory APIs retained for legacy VirtIO networking compatibility.
- Stage 13.2 QEMU feature, marker, runtime harness support and 1/2/4 CPU acceptance script.

## Deliberately not claimed in this candidate

- BAR size probing/resource allocation or rebalance.
- PCIe extended capability traversal beyond the first 256-byte conventional capability list.
- MSI/MSI-X vector allocation/programming.
- PCI hot-plug and power-management policy.
- IOMMU/DMA isolation.
- NVMe/AHCI/USB drivers; those remain later driver milestones.

## Acceptance gate

Run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-stage13-2-acceptance.ps1`.
Do not mark Stage 13.2 accepted or update the accepted-stage status documents until build, warning-denying Clippy and QEMU 1/2/4 CPU runs all pass.
---

## Source: AUDIT-STAGE13.3-NVME-CANDIDATE.md

# Stage 13.3 NVMe candidate audit

## Scope

Stage 13.3 adds the first native WovenHat NVMe path on top of the accepted Stage 13.2 PCI/PCIe foundation. The implementation discovers PCI class 01h/subclass 08h/programming-interface 02h controllers, enables PCI memory decoding and bus mastering, maps BAR MMIO uncached, initializes NVMe admin and I/O queue pairs, identifies controller/namespace 1, and exposes a 512-byte namespace through the existing `BlockDevice` contract.

## Architecture

The driver owns physically contiguous 4 KiB DMA pages for admin SQ/CQ, I/O SQ/CQ, identify data, and a bounded transfer buffer. Queue entries use command IDs and NVMe completion phase bits. Release/acquire fences bracket submission/completion visibility. The controller is serialized behind the existing IRQ-safe mutex so Stage 13.3 does not introduce an unsynchronized cross-CPU queue path.

The stage deliberately preserves ATA and VirtIO storage paths. NVMe is an additional block backend rather than a replacement.

## Acceptance

`run-stage13-3-acceptance.ps1` requires build, warning-denying freestanding Clippy, and isolated QEMU boots on 1/2/4 CPUs. The runtime harness attaches a dedicated 16 MiB QEMU NVMe namespace. Each boot must discover and initialize the controller, identify the namespace, perform a write/flush/read round trip through `BlockDevice`, restore the original sector, and exit through the standard debug-exit success path.

## Safety and production gaps

Unsafe code is restricted to volatile MMIO and exclusive direct-map access to allocator-owned DMA pages. Commands and queues are bounded. This stage uses polling for bounded controller bring-up and synchronous queue completion; MSI/MSI-X, interrupt-driven completion, multi-queue scaling, multiple namespaces/controllers, larger transfers/PRP lists/SGLs, reset recovery, power management, hotplug, and broad physical-hardware qualification remain explicit later production work.


## Final acceptance record

Stage 13.3 was accepted on 2026-09-30 from commit `b8bafb3a44c1ae0461304f982ffc970fdc62a92e` by GitHub Actions run `36703337710`.

The release-validation job passed the repository's lint, host regressions, normal debug 1/2/4-core boot/storage/network matrix, release boot/network gates, and the dedicated Stage 13.3 NVMe acceptance gate. The dedicated NVMe runs completed with the standard QEMU debug-exit success code:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Per-run evidence was preserved under `audit-artifacts/stage13.3-1cpu-*`, `stage13.3-2cpu-*`, and `stage13.3-4cpu-*` by the workflow artifact upload.

## Milestone status

**Stage 13.3 native NVMe: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers native controller discovery, BAR/MMIO access, DMA-backed admin and I/O queues, controller/namespace identification, namespace-backed `BlockDevice` read/write/flush, destructive round-trip verification with restoration of the original sector, and successful 1/2/4-CPU execution.

Physical-hardware qualification and the production extensions listed above remain later work and are not implied by this QEMU acceptance.
---

## Source: AUDIT-STAGE13.4-AHCI-SATA-CANDIDATE.md

# Stage 13.4 AHCI/SATA candidate audit

## Scope

Stage 13.4 adds a native AHCI/SATA block backend on top of the accepted Stage 13.2 PCIe and Stage 13.3 NVMe foundations. It discovers PCI class 01h/subclass 06h/programming-interface 01h controllers, enables PCI memory decoding and bus mastering, selects an implemented active SATA disk port, programs command-list and received-FIS DMA bases, and exposes 512-byte SATA media through the existing `BlockDevice` contract.

## Architecture

The driver uses allocator-owned 4 KiB DMA pages for the command list, received FIS area, one command table/PRDT and a bounded transfer buffer. It stops the AHCI command engine before rebasing, clears port errors/status, restarts the engine, issues IDENTIFY DEVICE, requires 48-bit LBA support, and uses READ DMA EXT / WRITE DMA EXT / FLUSH CACHE EXT for block I/O. Access is serialized by the existing IRQ-safe controller mutex.

Legacy ATA PIO and NVMe are preserved as independent backends. Stage 13.4 does not replace either path.

## Acceptance

`run-stage13-4-acceptance.ps1` requires build, warning-denying freestanding Clippy, and isolated QEMU boots on 1/2/4 CPUs. The runtime harness attaches a dedicated 16 MiB SATA disk to an ICH9 AHCI controller. Each boot must discover AHCI, initialize a SATA disk, perform write/flush/read round-trip through `BlockDevice`, restore sector zero, and exit through the standard debug-exit success path.

## Production gaps

The stage deliberately uses one command slot and bounded polling for synchronous completion. Interrupt-driven completion, NCQ/multi-slot scaling, ATAPI, hotplug, staggered spin-up, enclosure management, controller reset recovery, multiple disks/controllers, advanced power management, TRIM/DSM, and broad real-hardware qualification remain later production work.


## Final acceptance record

Stage 13.4 was accepted on 2026-09-30 from commit `a4c00388187f63cc801716979b21e4841dd0945f` by GitHub Actions run `36704495845`.

The complete release-validation gate passed, including the accepted Stage 13.3 NVMe regression and the dedicated Stage 13.4 AHCI/SATA runtime gate. Dedicated AHCI/SATA results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.4-1cpu-*`, `stage13.4-2cpu-*`, and `stage13.4-4cpu-*`.

## Milestone status

**Stage 13.4 native AHCI/SATA: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers native PCI AHCI discovery, BAR/MMIO access, DMA command structures, SATA IDENTIFY with 48-bit LBA validation, READ DMA EXT / WRITE DMA EXT / FLUSH CACHE EXT through `BlockDevice`, round-trip sector verification with restoration, and successful 1/2/4-CPU execution.

The production extensions listed above and physical-hardware qualification remain later work.
---

## Source: AUDIT-STAGE13.5-USB-CORE-CANDIDATE.md

# Stage 13.5 USB core candidate audit

## Scope

Stage 13.5 introduces WovenHat's first native USB host-controller foundation. The implementation discovers PCI USB class 0Ch/subclass 03h/programming-interface 30h xHCI controllers, enables MMIO decoding and bus mastering, maps the controller BAR uncached, performs halt/reset/start sequencing, configures DCBAA, command ring, event ring and event-ring segment table, discovers a connected root-hub port, resets it, submits Enable Slot, and validates the matching command-completion event.

## Architecture

The controller is serialized behind the existing rank-10 IRQ-safe mutex. DMA structures are allocator-owned physically contiguous 4 KiB pages accessed through the bootloader direct map. Completion is deliberately polling-based because WovenHat does not yet expose a general MSI/MSI-X vector allocator; the audit therefore does not claim interrupt-driven USB completion. Existing ATA, AHCI, NVMe and VirtIO paths are preserved.

## Acceptance

`run-stage13-5-acceptance.ps1` requires a full build, freestanding Clippy with `-D warnings`, and isolated QEMU boots on 1/2/4 CPUs. The runtime harness attaches a QEMU xHCI controller plus a USB keyboard. Every boot must discover xHCI, initialize its core DMA structures, observe a connected port, reset the port, enable a device slot, receive the matching command-completion TRB, and exit through the standard debug-exit success path.

## Production gaps

Address Device, endpoint-context programming, EP0 control transfers, descriptor parsing, hubs, HID class support, mass storage, hotplug, suspend/resume, MSI/MSI-X interrupt delivery, bandwidth scheduling, streams, multiple controllers and broad physical-hardware qualification remain later work.

## Acceptance correction R2

The first 1-CPU QEMU run reached the xHCI controller and failed `Enable Slot` with `CommandFailed`. The root cause was event ordering: resetting the connected root port queues a Port Status Change Event before the later Command Completion Event. The original `enable_slot` path incorrectly treated the first event of any type as the Enable Slot completion.

R2 consumes unrelated xHCI events while waiting for the matching Command Completion TRB, while retaining bounded polling and strict command-pointer, completion-code, and non-zero slot validation. No acceptance assertion or timeout was weakened.


## Final acceptance record

Stage 13.5 was accepted on 2026-09-30 from commit `e8ed974b2166f4f4afb01c44034443e5819fb50d` by GitHub Actions run `36705547487`.

The full release-validation suite passed together with the retained Stage 13.3 NVMe and Stage 13.4 AHCI/SATA gates. Dedicated xHCI USB-core results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.5-1cpu-*`, `stage13.5-2cpu-*`, and `stage13.5-4cpu-*`.

## Milestone status

**Stage 13.5 native xHCI USB core: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers native xHCI discovery and bring-up, DMA ring structures, root-port discovery/reset, Enable Slot submission, correct filtering of unrelated events while awaiting the matching command completion, and successful 1/2/4-CPU execution. Device addressing, endpoint configuration, HID and other USB classes remain subsequent stages.
---

## Source: AUDIT-STAGE13.6-USB-HID-CANDIDATE.md

# WovenHat OS Stage 13.6 â€” USB HID Candidate

## Scope
Stage 13.6 builds a bounded USB HID class path on the accepted Stage 13.5 xHCI controller core. It adds xHCI device addressing, EP0 control transfers, USB device/configuration/interface/endpoint descriptor parsing, HID boot-protocol selection, interrupt-IN endpoint configuration, and boot-report reception.

## Acceptance target
The QEMU acceptance harness attaches `qemu-xhci` plus `usb-kbd`. The kernel must enumerate the HID keyboard and complete a real interrupt-IN transfer, preserving the existing SMP and Stage 10.3 regression markers on 1, 2, and 4 CPUs.

## Safety / architecture
- DMA buffers are kernel-owned contiguous pages and remain alive for controller lifetime.
- xHCI MMIO remains behind the existing paging MMIO mapping boundary.
- Controller state is serialized by the existing ranked IRQ mutex.
- Command and transfer waits are bounded; unrelated event-ring entries are demultiplexed instead of treated as immediate command failure.
- Completion remains polling-based because the current kernel does not yet expose a generic MSI/MSI-X allocation/delivery API.
- Stage 13.7 remains responsible for a unified PS/2 + USB input framework.

## Deliberate production gaps
This focused stage does not claim hub support, arbitrary HID report-descriptor parsing, hot-unplug recovery, multiple simultaneous HID devices, interrupt-driven xHCI completion, or a unified input event service. Mouse/touchpad protocol classification is represented, but this acceptance specifically proves the QEMU USB boot keyboard path.


## Final acceptance record

Stage 13.6 was accepted on 2026-09-30 from commit `36ead75ccad4323867ba9e25526c64ae6695fe85` by GitHub Actions run `36707350992`.

The full release-validation suite and retained Stage 13.3 NVMe, Stage 13.4 AHCI/SATA, and Stage 13.5 xHCI-core gates passed. Dedicated USB HID results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.6-1cpu-*`, `stage13.6-2cpu-*`, and `stage13.6-4cpu-*`.

## Milestone status

**Stage 13.6 USB HID: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers xHCI device addressing, EP0 control transfers, USB descriptor enumeration, HID boot-protocol setup, interrupt-IN endpoint configuration, and a real HID report transfer from the QEMU USB keyboard on 1/2/4 CPUs. Unified input delivery remains Stage 13.7.
---

## Source: AUDIT-STAGE13.7-INPUT-FRAMEWORK-CANDIDATE.md

# Stage 13.7 â€” WovenInput Candidate

Baseline: `864f94c` (accepted Stage 13.6 USB HID).

This candidate introduces the device-independent WovenInput event layer and routes the existing PS/2 keyboard path through it while preserving the foreground-terminal single-consumer rule.

Implemented:
- bounded unified input event queue
- device-independent keyboard, pointer, touch, pen and game-controller event types
- PS/2 scancode decoder publishes logical key events into WovenInput
- existing kernel shell/userspace stdin APIs remain compatible
- deterministic queue/self-test coverage
- Stage 13.7 1/2/4 CPU acceptance harness

Scope note: this is the Stage 13.7 framework foundation. USB HID report producers remain in the Stage 13.6 xHCI layer until their runtime service path is moved into the common driver/event pipeline; mouse/touchpad/touchscreen/pen/game-controller event types are defined now so later drivers target WovenInput rather than UI consumers directly.


## Final acceptance record

Stage 13.7 was accepted on 2026-09-30 from commit `7be1916e7feb661b7e29a247ad6e682e4406f68e` by GitHub Actions run `36708518537`.

The full release-validation suite and retained Stage 13.3 through Stage 13.6 hardware gates passed. Dedicated WovenInput results were:

- 1 CPU: PASS, exit 33
- 2 CPUs: PASS, exit 33
- 4 CPUs: PASS, exit 33

Evidence was preserved under `audit-artifacts/stage13.7-1cpu-*`, `stage13.7-2cpu-*`, and `stage13.7-4cpu-*`.

## Milestone status

**Stage 13.7 WovenInput framework foundation: COMPLETE at its declared integration boundary.**

The accepted milestone establishes the bounded device-independent input queue and PS/2 keyboard producer while preserving terminal input compatibility. USB HID-to-WovenInput runtime routing and additional device producers remain subsequent integration work and are not claimed by this acceptance.
---

## Source: AUDIT-STAGE13.8-WOVENAUDIO-CANDIDATE.md

# Stage 13.8 â€” WovenAudio / Intel HDA Foundation

Baseline: `ec91086` (accepted Stage 13.7 WovenInput).

Implemented in this candidate:
- WovenAudio device abstraction
- new `DeviceKind::Audio`
- PCI class/subclass discovery for High Definition Audio controllers
- MMIO BAR validation
- PCI memory decoding + bus mastering enablement
- HDA GCAP parsing for input/output/bidirectional stream counts and 64-bit DMA capability
- controller reset handshake through GCTL.CRST
- WovenDriver registration/binding for `hda0`
- bounded, lock-serialized controller ownership
- isolated Stage 13.8 acceptance on 1/2/4 CPUs

Scope:
This candidate proves the controller and subsystem foundation. It does not yet claim audible PCM playback, recording, codec topology discovery, mixer/volume control, DMA BDL programming, or userspace streams. Those belong to the next Stage 13.8 slices after controller initialization is accepted.

## Codec command transport slice
- 256-entry CORB/RIRB DMA rings
- ring-size validation and CORB read-pointer reset
- HDA verb transport with codec-address response validation
- STATESTS codec discovery
- Get Parameter vendor/revision/root-node discovery
- acceptance requires a real codec response

### CORB/RIRB polling correction
The first codec response proved DMA transport was active, but RINTCNT=1 caused the
controller/emulator to stop consuming further CORB commands once one response had
been produced. The polling bootstrap transport now uses a 255-response window.
This keeps interrupts disabled while allowing bounded multi-verb codec discovery.
A later interrupt-driven audio transport should explicitly service/clear RIRB
status and reset response accounting.

## Codec/widget topology slice
- discovers Audio Function Group nodes through Get Parameter(Function Group Type)
- discovers widget ranges through Get Parameter(Subordinate Node Count)
- classifies widget capabilities into DAC/audio-output, ADC/audio-input, mixer,
  selector, pin-complex, power and volume-knob categories
- rejects empty/meaningless codec topology
- Stage 13.8 acceptance requires non-empty HDA topology on 1/2/4 CPUs

## PCM DMA playback slice
- selects the first generic HDA Audio Output Converter discovered from codec topology
- allocates DMA-visible PCM and Buffer Descriptor List pages
- programs one output stream descriptor after the input-stream descriptor range
- uses 48 kHz / 16-bit / stereo bootstrap PCM format
- assigns stream tag 1 to the converter
- starts the HDA output DMA engine and verifies LPIB position advancement
- acceptance requires real DMA consumption; audible host output is not required

## PCM DMA capture slice
- discovers the first HDA Audio Input Converter (ADC)
- allocates a capture BDL and DMA PCM page
- fills the capture page with a sentinel before DMA begins
- programs input stream descriptor 0 for 48 kHz / 16-bit / stereo
- assigns stream tag 2 to the ADC
- requires LPIB position advancement
- requires DMA to overwrite at least one sentinel byte in the guest capture buffer
- acceptance remains headless and does not require a host microphone

### Capture routing correction R4
R3's global stream-tag search incorrectly counted the already-accepted playback
stream-tag command together with the capture command. R4 avoids global matching:
it atomically replaces only capture_smoke_test. Capture now discovers an ADC and
an input-capable pin inside the function, enables the pin, selects ADC connection
zero, binds stream tag 2, then starts input stream descriptor zero. Acceptance
remains unchanged: LPIB must advance and DMA must mutate the sentinel buffer.

### Converter-format correction R5
R4 proved that selecting an input-capable pin and enabling the ADC route was not
sufficient: input LPIB still remained stationary. The remaining protocol defect
was converter format programming. SDnFMT configures the controller stream, but
the HDA codec converter must also receive Set Converter Format. That verb uses
the 4-bit-verb/16-bit-payload encoding, which the original 12-bit/8-bit command
helper could not express. R5 adds command16() and programs PCM format 0x0011 on
both DAC and ADC before assigning stream tags. Acceptance is unchanged.

## Stage 13.8G Ã¢â‚¬â€ WovenAudio stream/API integration

WovenAudio now acts as the hardware-independent faÃƒÂ§ade above HDA. It exposes:
- playback/capture direction and a common stream descriptor;
- a stable 48 kHz, 16-bit, stereo format descriptor for the current backend;
- one-shot playback and capture operations through WovenAudio rather than direct HDA calls;
- mixer state translated into a WovenAudio-facing structure;
- an integrated smoke test that initializes the registered audio device and exercises mixer, playback, and capture through the faÃƒÂ§ade.

This stage intentionally keeps HDA-specific verbs and DMA implementation inside `hda.rs`. It is a boundary milestone, not yet a general asynchronous userspace audio server or arbitrary-format streaming API.

## Final acceptance and hardening record

Stage 13.8 was accepted on 2026-09-30 from commit `e78a349da6353f0f3d3e4721f787742d6fbeb19f` by GitHub Actions run `36710786817`.

The full release-validation suite and retained Stage 13.3 through Stage 13.7 gates passed. Dedicated WovenAudio/HDA acceptance passed on 1, 2, and 4 CPUs with exit 33.

The Stage 13.8H hardening matrix then completed nine additional successful boots:
- 1 CPU: 3/3 PASS
- 2 CPUs: 3/3 PASS
- 4 CPUs: 3/3 PASS

Each hardening boot exited through the expected debug-exit code 33 and preserved its Stage 13.8 evidence under `audit-artifacts/stage13.8-*`.

## Milestone status

**Stage 13.8 WovenAudio / Intel HDA integration and hardening: COMPLETE at the QEMU integration boundary.**

The accepted milestone covers HDA controller bring-up, codec command transport and topology discovery, PCM DMA playback/capture, and the WovenAudio hardware-independent stream/API boundary. It passed both the standard SMP acceptance matrix and the additional 9-boot hardening matrix. Broader physical-hardware qualification and later asynchronous/userspace audio services remain outside this milestone.
---

## Source: AUDIT-STAGE13.9-WOVENWIFI-ACCEPTANCE.md

# Stage 13.9 â€” WovenWiFi Full-Contract Acceptance

## Scope

Stage 13.9 validates the integrated synthetic WovenWiFi contract already implemented in the kernel: PCI classification, IEEE 802.11 frame/IE handling, scanning, Open System authentication and association, RSN/EAPOL foundations, WPA2 cryptography and four-way handshake, GTK/group rekey lifecycle, CCMP protected data paths, WovenNet integration, backend lifecycle, entropy boundaries, session handoff, RX integration, smoltcp adaptation, and selectable transport integration.

The runtime harness requires the complete Stage 13.9 marker set (Aâ€“V plus X/Y/Z), rejects panic/failure/timeout markers, and requires QEMU debug exit 33.

## Final acceptance record

Stage 13.9 was accepted on 2026-09-30 from commit `f245e67083cb61a51d4bf3e8876b6c8cfb140358` by GitHub Actions run `36713418548`.

The full release-validation suite and retained Stage 13.3 through Stage 13.8 gates passed. Dedicated WovenWiFi results were:

- 1 CPU: PASS, exit 33 â€” `audit-artifacts/stage13.9-1cpu-1790770950803140999`
- 2 CPUs: PASS, exit 33 â€” `audit-artifacts/stage13.9-2cpu-1790770961245961884`
- 4 CPUs: PASS, exit 33 â€” `audit-artifacts/stage13.9-4cpu-1790770972555282660`

The Stage 13.9 PowerShell acceptance labels were corrected so this gate is no longer mislabeled as Stage 13.10AC.

## Milestone status

**Stage 13.9 WovenWiFi full synthetic contract: COMPLETE at the QEMU integration boundary.**

This acceptance does not claim physical Intel AX200 hardware qualification. Physical firmware loading, device-specific transport behavior, RF/association behavior, interrupt/DMA behavior, and real access-point interoperability remain separate hardware qualification requirements.
---

## Source: AUDIT-STAGE7.4.md

# Stage 7.4 source audit â€” Ready-state Ring-3 Migration & Affinity

Baseline: validated Stage 7.3 Pinned Ring-3 Foundation.

## Scope
- Normal `spawn_user_process()` remains CPU0-owned and non-migratable.
- Stage 7.3 `spawn_pinned_user_process_on()` remains hard-pinned.
- New `spawn_migratable_user_probe()` is the only new Ring-3 path that sets `migratable = true`.
- The probe starts Ready on CPU0 with the current online affinity mask.
- Existing `migrate_ready_task()` and `set_ready_task_affinity()` now accept an explicitly migratable Ring-3 task.
- Both operations still require `TaskState::Ready` and hold the global scheduler lock while changing ownership.
- Running, Switching, Blocked and Sleeping task migration remains rejected by the Ready-state check.
- General userspace placement, fork migration, filesystem/network/pager migration and concurrent service I/O are not enabled.

## Runtime proofs added
1. Explicit CPU0 -> highest-online-AP Ready-state ownership migration of `/bin/true`.
2. Single-bit hard-affinity ownership transfer of a Ready `/bin/true` probe to the highest online AP.
3. Exit(0) and parent reap after each transfer.
4. Stage 7.2 termination and Stage 7.3 AP Ring-3 markers remain mandatory.
5. Network regression remains mandatory on 1/2/4 CPUs.

## Local validation limitation
This packaging environment does not contain the Rust/Cargo/QEMU toolchain, so compile/Clippy/runtime acceptance must be established by `run-stage7-4-acceptance.ps1` on the development host.
---

## Source: AUDIT-STAGE7.5.md

# WovenHat OS Stage 7.5 Audit â€” Multicore Pager & File-I/O Ownership

## Validated baseline
Stage 7.4 is the only implementation baseline for this candidate. The earlier
combined "Stage 7 Complete" experiments are not used as a source tree.

## Problem being solved
`file_fault_io()` intentionally enables interrupts while a task performs an
I/O operation. Before Stage 7.5, `FILE_IO_DEPTH` was a single global counter
and `preempt_from_interrupt()` honored it only on CPU0. That matched the old
BSP-only file-fault execution model but is not correct once a faulting Ring-3
task runs on an AP: the AP itself could be preempted while holding an I/O lock,
while CPU0 could be unnecessarily prevented from scheduling.

## Correctness change
`FILE_IO_DEPTH` is now an array indexed by logical CPU. The function disables
local interrupts before capturing the current CPU index and incrementing the
local depth. Interrupts may then be enabled for the I/O operation. On return,
interrupts are disabled again and the same CPU's depth is decremented.

A task cannot be involuntarily migrated while this region is active because
local timer preemption checks the same per-CPU depth and returns without a
scheduler switch. This makes the CPU index stable for the enter/leave pair.
Other CPUs remain independently schedulable.

## Pager execution proof
The existing `/bin/mmaptest` workload is loaded exactly as before. On SMP, the
process is spawned through the already-validated Stage 7.3 pinned-user API on
the highest online AP. Its lazy file-backed page fault therefore originates on
an AP. The existing Stage 7.1.5 block/wake transaction marks the requester
Blocked and makes the BSP pager Ready under one scheduler critical section,
then issues the existing reschedule request to the pager CPU. The requester is
resumed on its pinned AP after completion and the original user instruction is
retried by `iretq`.

## Locking / ownership boundary
No process-table/cache lock is intentionally held across `file_fault_io()`.
The Stage 7.1 lock order (`SCHEDULER -> PROCESS_TABLE`) remains unchanged.
The pager remains one BSP-owned worker in Stage 7.5; this milestone changes the
location of the faulting userspace process, not pager-worker topology.

## Explicit non-goals
Stage 7.5 does not make normal userspace automatically migratable and does not
claim arbitrary concurrent filesystem/network service execution. Those are
separate follow-on milestones after this AP file-fault path is accepted.

## Acceptance gate
The gate preserves the validated Stage 7.2/7.3/7.4 markers, then requires the
new Stage 7.5 AP pager marker over repeated 2-CPU and 4-CPU QEMU runs, plus
network regressions at 1/2/4 CPUs.

Stage 7.5 is not considered frozen until the script prints:

`=== STAGE 7.5 ACCEPTANCE: PASS ===`
---

## Source: AUDIT-STAGE7.6-CANDIDATE.md

# Stage 7.6 Candidate Audit

Baseline: validated Stage 7.5.

Kernel source changes are intentionally concentrated in:

- `kernel/src/task.rs`: least-loaded audited userspace placement, Ready-only Ring-3 rebalance eligibility, userspace migration accounting, fork placement inheritance.
- `kernel/src/main.rs`: integrated Stage 7.6 boot proofs and AP mmap/fork execution through the multicore API.

Acceptance harness:

- `run-stage7-6-acceptance.ps1`

Safety invariants retained:

- `SCHEDULER -> PROCESS_TABLE` lock order.
- migration only when `TaskState::Ready`.
- owner CPU must be online and inside non-zero affinity.
- Switching remains physically running and cannot be migrated.
- legacy userspace stays pinned because `initialize_user()` itself is unchanged.
- file-fault I/O remains protected by the Stage 7.5 per-CPU depth guard.
- acknowledged global TLB shootdown remains unchanged.

Candidate is not called complete until the acceptance script passes all build, lint, host test, repeated 1/2/4 CPU memory regressions and 1/2/4 CPU network regressions.
---

## Source: AUDIT-STAGE7.6-PLACEMENT-SNAPSHOT-FIX.md

# Stage 7.6 â€” race-free multicore placement proof

## Evidence
An intermittent SMP run reached:
- `[S7.6] automatic Ready Ring-3 rebalancing: PASSED`
- `[S7.6] least-loaded Ring-3 placement: FAILED owner=1`

On a 2-CPU run, `owner=1` proves least-loaded AP placement succeeded. The old
acceptance branch could therefore only be failing because `task_affinity(task_id)`
returned something other than the expected mask.

## Root cause
`spawn_multicore_user_process()` installs the Ready TCB under the scheduler lock,
validates affinity invariants, releases the lock, sends a remote reschedule IPI,
and returns. The `/bin/true` probe can execute and retire on the AP before the
caller subsequently calls `task_affinity(task_id)`. Once the task is reaped, the
live-task lookup returns `None`. The test confused lifetime with placement.

## Correction
The spawn API now returns `MulticoreSpawnPlacement`, an immutable snapshot copied
from the installed TCB under the scheduler lock after affinity validation:
- `owner_cpu`
- `affinity_mask`

The Stage 7.6 acceptance test validates that snapshot rather than performing a
racy post-spawn live-task lookup. Scheduler policy, migration policy, affinity
semantics, and task lifetime are unchanged.

This does not weaken the test: it moves observation into the synchronization
domain where the property is established.
---

## Source: AUDIT-STAGE7.6-RACE-FIX.md

> SUPERSEDED by `AUDIT-STAGE7.6-SCHEDULER-SEMANTICS-FIX.md`. The `has_started` policy described below regressed the validated Stage 7.4 explicit affinity path.

# Stage 7.6 SMP race fix â€” first-dispatch migration boundary

## Failure observed
The original Stage 7.6 closure candidate passed build, Clippy, host tests, all 1-CPU preservation runs, and 24 consecutive 2-CPU stress runs before the 25th 2-CPU memory run timed out.

That pattern identifies a timing-sensitive scheduler ownership problem rather than a deterministic compiler, pager, VFS, or network failure.

## Root cause
Stage 7.4 validated Ready-state Ring-3 migration **before first dispatch**. The Stage 7.6 periodic rebalancer broadened this rule unintentionally: any migratable userspace task that later became `Ready` again after it had already executed could be moved to another CPU.

`Ready` proves that the task is not physically executing at that instant, but it does not by itself prove that WovenHat's post-dispatch userspace kernel-return/context history is portable between CPU ownership paths. That stronger protocol has not yet been implemented or validated.

## Fix
`TaskControlBlock` now carries `has_started`.

- Fresh tasks start with `has_started = false`.
- The bootstrap kernel task starts with `has_started = true` because it is already executing.
- `prepare_switch()` sets `has_started = true` under the scheduler lock at the same transition that publishes `Ready -> Running`.
- Kernel tasks retain the existing Ready migration rules.
- Ring-3 tasks may be moved only while `Ready && !has_started`.
- `rebalance_one()`, `migrate_ready_task()`, and `set_ready_task_affinity()` all enforce the same boundary.

The result preserves Stage 7.6's least-loaded initial placement and pre-first-dispatch automatic Ring-3 balancing while preventing the unproven post-dispatch migration class.

## What remains intentionally deferred
A later milestone may implement true post-dispatch userspace migration. That work must explicitly audit saved syscall/interrupt return frames, per-CPU kernel state, FPU/SIMD state, TLS, pending signals, and any per-CPU service ownership before relaxing this guard.
---

## Source: AUDIT-STAGE7.6-SCHEDULER-SEMANTICS-FIX.md

# Stage 7.6 Scheduler State Semantics Correction

## Regression observed

The first race fix introduced `has_started` and rejected Stage 7.4 explicit
Ready migration/affinity operations once that flag was set. The 2-CPU
preservation test then failed at the Stage 7.4 hard-affinity transfer.

## Root semantic error

`has_started` mixed two independent concepts:

1. **history** â€” whether the scheduler has ever dispatched the task; and
2. **permission** â€” whether a particular migration API is allowed to move it.

That made a Stage 7.6 automatic-balancing policy silently redefine Stage 7.4's
validated explicit API contract.

## Correction

`TaskControlBlock::ever_dispatched` is now pure execution-history metadata.
It is set under the scheduler lock at `Ready -> Running`.

Only `Scheduler::rebalance_one()` consumes this bit for Ring-3 tasks. Thus the
new Stage 7.6 automatic policy may move a Ring-3 task only before its first
scheduler dispatch.

`migrate_ready_task()` and `set_ready_task_affinity()` again use the Stage 7.4
contract only: live task, explicitly migratable, currently `Ready`, target
online, and affinity constraints satisfied.

## Preserved invariants

- No Running or Switching task is migrated.
- Scheduler ownership changes remain under the global scheduler lock.
- Affinity remains a hard constraint.
- Legacy CPU0-owned userspace remains unchanged.
- Pinned Stage 7.3 userspace remains pinned.
- Stage 7.6 automatic Ring-3 rebalancing remains bounded to never-dispatched
  tasks.

## Acceptance requirement

The full Stage 7.6 acceptance suite must pass before freeze/tag.
---

## Source: AUDIT-STAGE8.1-IPC-HANDLE-FOUNDATION.md

# WovenHat OS Stage 8.1 Audit â€” IPC Handle Foundation

## Baseline

Implemented on the Stage 7.6 placement-snapshot tree that passed the complete Stage 7.6 acceptance suite (build, strict Clippy, host tests, 1CPU x10, 2CPU x30, 4CPU x20, network 1/2/4).

## Changed production files

- `kernel/src/config.rs`
  - `MAX_IPC_OBJECTS = 64`
  - `MAX_IPC_HANDLES_PER_PROCESS = 16`
- `kernel/src/ipc.rs`
  - preserves legacy PID queue ABI,
  - adds endpoint objects, generation-tagged process handles, rights, lifecycle, tests.
- `kernel/src/main.rs`
  - adds local Stage 8.1 invariant gate,
  - adds real global-state Stage 8.1 post-SMP runtime probe marker.

No Stage 7 scheduler logic is modified.

## Key invariants

1. A registered process has both a legacy queue endpoint and handle space.
2. Endpoint object IDs are kernel-issued and never supplied by userspace.
3. A handle is meaningful only inside its owner process handle space.
4. Handle slot reuse changes generation; stale handles fail lookup.
5. Object allocation rolls back if handle installation fails.
6. Closing the last reference removes the endpoint object.
7. Process unregister closes all remaining handles before namespace removal.
8. Existing PID message queues continue to pass their original self-test.
9. Global runtime probe leaves zero synthetic resources behind.

## Locking

Stage 8.1 introduces no new cross-subsystem lock order. All object/handle metadata stays beneath the existing IPC `spin::Mutex<State>`.

## Validation status

Static source audit completed in the packaging environment. Rust/Cargo is not installed there, so compiler, strict Clippy and QEMU validation must be performed on Anthony's Windows WovenHat toolchain with `run-stage8-1-acceptance.ps1`.
---

## Source: AUDIT-STAGE8.1-NETWORK-TX-BACKPRESSURE-FIX.md

# Stage 8.1 â€” Network TX Backpressure Preservation Fix

## Failure evidence
The Stage 8.1 candidate completed the full 1-CPU, 2-CPU and 4-CPU memory/SMP preservation suite. In the final 4-CPU network preservation run, virtio-net initialized and DHCP completed, but DNS timed out.

## Root cause
`VirtioSmolDevice::transmit()` unconditionally returned a `WovenTxToken`, advertising transmit capacity to smoltcp even when the legacy polling virtio-net transport's single TX descriptor was still outstanding. `WovenTxToken::consume()` then ignored the boolean result from `virtio_net::transmit()`. If the descriptor was busy, the packet was dropped after smoltcp had already accepted the token as usable. Timing could therefore lose an outgoing DNS packet.

## Correction
- Added `virtio_net::tx_available()`.
- It takes the transport lock, reaps a completed TX descriptor, and returns true only when the transport is initialized and the descriptor is free.
- `VirtioSmolDevice::transmit()` now returns `Some(WovenTxToken)` only when `tx_available()` is true; otherwise it returns `None`, allowing smoltcp to defer/retry normally.

## Scope
No Stage 8.1 IPC semantics, scheduler state transitions, Ring-3 migration rules, timer behavior, DNS timeout, or DHCP policy were changed. This is a transport backpressure correctness fix in the pre-existing network stack.

## Validation requirement
Stage 8.1 remains unvalidated until `run-stage8-1-acceptance.ps1` completes and prints `=== STAGE 8.1 ACCEPTANCE: PASS ===`.
---

## Source: AUDIT-STAGE8.1-PUBLIC-API-LIVENESS-FIX.md

# Stage 8.1 Public API Liveness Fix

## Failure
The first Stage 8.1 candidate failed the project's strict `-D warnings` build because the new public endpoint/handle APIs were only exercised through private `State` methods inside the runtime probe. Rust therefore reported the public wrappers and ID accessors as dead code.

## Correction
`stage8_1_runtime_probe()` now exercises the production public API surface directly:
- `create_endpoint_object()`
- `handle_info()`
- `close_handle()`
- `handle_count()`
- `object_count()`
- `Handle::as_u32()`
- `EndpointObjectId::as_u64()`

The probe still verifies object creation, rights/ownership, handle accounting, close behavior, stale-handle rejection, generation change on slot reuse, and unregister-driven teardown.

## Non-changes
No `#[allow(dead_code)]` or `#[expect(dead_code)]` was added. No scheduler, SMP, process lifecycle, legacy PID IPC ABI, object allocation, handle generation, rights model, or teardown semantics were changed.

## Validation status
Statically audited and packaged. Full Rust/Clippy/QEMU validation must run on the user's configured Windows toolchain through `run-stage8-1-acceptance.ps1`.
---

## Source: AUDIT-STAGE8.1-STAGE7.4-READY-WINDOW-FIX.md

# Stage 8.1 preservation fix â€” Stage 7.4 Ready-window race

## Observed failure
The Stage 8.1 runtime probe passed, block I/O passed, Stage 7.3 passed, and the Stage 7.4 explicit migration probe passed. The next Stage 7.4 hard-affinity setup intermittently failed before execution.

## Root cause
`spawn_migratable_user_probe()` publishes a migratable `/bin/true` task in `Ready` state owned by CPU0. The Stage 7.4 proof then performed a second API call (`migrate_ready_task()` or `set_ready_task_affinity()`) after returning to interrupt-enabled BSP execution. A timer interrupt in that narrow interval can dispatch the tiny CPU0-owned probe. The second API then correctly returns `NotReady` (or `UnknownTask` after very fast retirement), producing a false preservation failure.

This is a test-observation/setup race. The scheduler's Ready-only semantics are correct and are not weakened.

## Fix
Both Stage 7.4 setup sequences now keep local BSP interrupts disabled across:

1. publishing the new CPU0-owned Ready probe;
2. inspecting its initial affinity where applicable; and
3. applying the explicit Ready migration or hard-affinity transfer.

The existing migration/affinity APIs still take the global scheduler lock and still reject non-Ready tasks. Once ownership has moved to an AP, the local interrupt boundary ends and normal SMP execution continues.

## Scope
Production scheduler, task states, IPC, SMP, IPI, process lifecycle, and affinity semantics are unchanged. Only the Stage 7.4 runtime proof in `kernel/src/main.rs` is hardened against local timer preemption during its intentionally two-step Ready-state setup.
---

## Source: AUDIT-STAGE8.2-CLIPPY-OP-REF-FIX.md

# Stage 8.2 Clippy op_ref Fix

## Failure
Strict `cargo clippy -D warnings` rejected one Stage 8.2 runtime-probe comparison in `kernel/src/ipc.rs` because the right-hand array operand was unnecessarily borrowed.

## Change
Changed:

```rust
message.payload() != &[index as u8]
```

to:

```rust
message.payload() != [index as u8]
```

This follows Clippy's `op_ref` guidance and relies on Rust's slice/array `PartialEq` implementation.

## Scope
No IPC object, handle, rights, queue, refcount, scheduler, SMP, network, or lifecycle semantics changed. This is compile/lint hygiene only.
---

## Source: AUDIT-STAGE8.2-HANDLE-MESSAGE-PASSING.md

# WovenHat OS Stage 8.2 Audit â€” Handle-Addressed Message Passing

## Baseline
Built directly on the validated Stage 8.1 IPC endpoint/handle foundation, including the validated Stage 7.4 Ready-window correction and virtio-net TX backpressure correction.

## Architectural change
Stage 8.1 established endpoint object identity, generation-tagged process-local handles, rights metadata and deterministic teardown. Stage 8.2 makes an endpoint object carry a bounded FIFO queue and introduces nonblocking handle-addressed message operations.

### Data path
`Process A -> SEND handle -> endpoint object FIFO -> RECEIVE handle -> Process B`

The handle is resolved inside the caller's handle space while holding the global IPC lock. The handle entry names the endpoint object and carries rights. SEND and RECEIVE are checked before touching the queue.

## New guarantees
- A SEND-only handle cannot receive.
- A RECEIVE-only handle cannot send.
- Rights granted to another process must be a subset of the source handle rights and require TRANSFER authority on the source.
- Queue overflow returns `QueueFull`; no packet/message is silently discarded.
- Empty receive returns `QueueEmpty`; Stage 8.2 never sleeps.
- FIFO ordering is preserved.
- Messages preserve sender process identity.
- Object references are incremented when peer handles are granted and decremented on close/process unregister.
- Stale generation-tagged handles remain invalid after close.
- Legacy PID-addressed IPC is preserved.

## Deliberate boundary
`grant_handle()` is kernel-mediated plumbing, not the final public userspace transfer ABI. General handle transfer is intentionally deferred to Stage 8.5 so the authority-transfer model can be designed and audited independently.

## Runtime proof
The Stage 8.2 runtime probe creates isolated synthetic SERVER and CLIENT namespaces, creates one endpoint, grants SEND to CLIENT and RECEIVE to SERVER, verifies denied reverse operations, FIFO order, sender identity, QueueEmpty, QueueFull, stale-handle rejection, and final object cleanup.

## Validation status
Static/source audit only in the packaging environment. The stage is not validated until `run-stage8-2-acceptance.ps1` passes on the user's Rust/QEMU environment for the preserved Stage 8.1 suite and Stage 8.2 markers on 1/2/4 CPU boots.
---

## Source: AUDIT-STAGE8.3-RACE-FREE-IPC-WAITS.md

# WovenHat OS Stage 8.3 Audit â€” Race-Free IPC Events / Waits

## Baseline
Stage 8.3 is based on the exact Stage 8.2 CLIPPY-FIX source tree that passed the
full Stage 8.2 acceptance gate.

## Problem being solved
A naive blocking receive has a classic lost-wakeup interval:

1. receiver checks endpoint: empty;
2. receiver prepares to sleep;
3. sender queues a message and attempts to wake receiver;
4. receiver is not Blocked yet, so a plain wake is ignored;
5. receiver finally marks itself Blocked and may sleep forever.

Holding the IPC lock while publishing scheduler Blocked state would close this
race but is rejected here because the codebase already contains scheduler -> IPC
registration paths. Adding IPC -> scheduler acquisition would create a global
lock-order cycle.

## Chosen protocol
Stage 8.3 introduces a scheduler-owned one-bit event permit in each TCB.

- `signal_event(id)`:
  - if target is Blocked: Blocked -> Ready and reschedule owner CPU;
  - otherwise: set `event_pending = true`.
- `wait_for_event()`:
  - under the scheduler lock, consume `event_pending` and continue; or
  - publish Running -> Blocked and context-switch.

Therefore a signal racing before the block is latched, while a signal racing
after the block sees Blocked. There is no unrepresented gap.

## IPC waiter ownership
Each endpoint carries bounded FIFO waiter arrays:
- readable waiters for empty receive;
- writable waiters for full send.

Each waiter records process owner, exact generation-tagged handle and TaskId.
This permits exact-handle cleanup on close and owner cleanup on unregister.

## Preserved contracts
- Stage 8.2 `send_handle` remains nonblocking and returns QueueFull.
- Stage 8.2 `receive_handle` remains nonblocking and returns QueueEmpty.
- SEND/RECEIVE rights checks are unchanged.
- endpoint queue FIFO semantics are unchanged.
- generation-tagged stale-handle rejection is unchanged.
- legacy PID-addressed IPC ABI is unchanged.
- no IPC lock is held while calling scheduler event signal/wait functions.

## Runtime proof
The Stage 8.3 production probe creates one endpoint and restricted SEND/RECEIVE
handles in synthetic namespaces.

Phase A â€” empty queue:
- receiver calls blocking receive;
- sender waits until a read waiter is visibly registered;
- sender queues the wake message;
- receiver resumes and verifies payload/sender.

Phase B â€” full queue:
- sender fills the bounded queue;
- sender calls blocking send for one extra message;
- receiver waits until a write waiter is visibly registered;
- receiver removes one message, releasing one slot and signaling sender;
- sender resumes and enqueues the final message;
- receiver verifies complete FIFO ordering.

On 2/4 CPU boots, sender is pinned to the highest AP to exercise a remote CPU
scheduler wake. The probe has a bounded timer deadline and cleans IPC state on
success.

## Static audit status
- No changes to paging, process table, filesystem, networking or userspace ABI.
- No Stage 8.2 semantic weakening.
- No timeout increase or polling substitution for the blocking primitive.
- No IPC -> scheduler nested lock acquisition.
- Public Stage 8.3 functions are exercised by the production runtime probe to
  remain live under strict `-D warnings` builds.

## Validation status
IMPLEMENTED / STATIC-AUDITED ONLY until `run-stage8-3-acceptance.ps1` reaches:

`=== STAGE 8.3 ACCEPTANCE: PASS ===`
---

## Source: AUDIT-STAGE8.4-BOOTSTRAP-STACK-FIX.md

# Stage 8.4 bootstrap-stack regression fix

## Observed failure
The corrected Stage 8.4 candidate compiled, passed strict Clippy and host tests, then the first 1-CPU QEMU boot double-faulted immediately after the pre-scheduler file-mmap regression marker. The exception reported a fault address immediately below the current bootstrap RSP, which is the characteristic signature of crossing the bootstrap stack guard.

## Root-cause class
`ipc::self_test()` and `ipc::handle_object_self_test()` historically created `let mut state = State::new()` on the kernel stack. Stage 8.4 enlarged `State` by adding bounded shared-memory objects, backing-frame arrays, and mapping registries. The bootloader kernel stack is intentionally bounded at 1 MiB. Keeping a complete production registry as a local self-test value therefore became an unsafe stack-footprint pattern; code-layout/inlining changes can move the overflow point earlier than the apparent call site.

## Fix
Both early IPC self-tests now exercise the real global `STATE` under its existing lock using reserved synthetic owners. They require the registry to be pristine before testing and clean both namespaces on every success/failure path. No second full `State` is created on the bootstrap stack.

## What was deliberately NOT changed
- kernel/boot stack size remains 1 MiB;
- no warning or fault was suppressed;
- no QEMU timeout was increased;
- Stage 8.4 shared-memory rights, mapping, object lifetime, page ownership, or IPC event semantics were changed;
- prior acceptance counts remain unchanged.

## Validation requirement
Run the unchanged full `run-stage8-4-acceptance.ps1`. Stage 8.4 remains unvalidated until the complete Stage 8.3 preservation chain and S8.4 markers pass on 1/2/4 CPUs.
---

## Source: AUDIT-STAGE8.4-PUBLIC-READ-LIVENESS-FIX.md

# Stage 8.4 Public Shared-Memory Read Liveness Fix

## Failure observed
The first Stage 8.4 acceptance build stopped under the existing strict `-D warnings` policy because `ipc::read_shared_memory()` was public but not exercised by production code, so rustc reported it as dead code.

## Fix
The Stage 8.4 production runtime probe now calls `read_shared_memory()` immediately after `write_shared_memory()` and verifies the returned bytes. The existing independent peer-address-space read remains in place.

This intentionally does **not** add `allow(dead_code)` / `expect(dead_code)` and does not weaken the build or Clippy policy.

## Added proof
The Stage 8.4 runtime probe now proves both:
1. kernel object-layer write -> kernel object-layer read, and
2. the same backing bytes -> peer user mapping visibility.

No scheduler, IPC wait, mapping, rights, timeout, or earlier-stage semantics were changed.
---

## Source: AUDIT-STAGE8.4-SHARED-MEMORY.md

# Stage 8.4 Audit â€” Shared Memory Foundation

## Objective

Introduce capability-governed shared physical memory without weakening the
validated Stage 8.1â€“8.3 endpoint/message/wait contracts.

## Object model

The existing generation-tagged `Handle` remains the sole process-local
capability token. `HandleEntry` now carries a private object reference with an
object kind (`Endpoint` or `SharedMemory`). This preserves stale-handle
protection and avoids parallel authority namespaces.

Endpoint IDs retain the public `EndpointObjectId` wrapper for ABI/source
compatibility. Internally the ID namespace is shared across object kinds.

## Rights model

Endpoint rights remain unchanged:

- SEND
- RECEIVE
- TRANSFER
- INSPECT

Shared memory adds:

- SHM_READ
- SHM_WRITE
- SHM_MAP

Shared owner authority is `TRANSFER | INSPECT | SHM_READ | SHM_WRITE |
SHM_MAP`. `grant_handle` first determines the source object kind, then rejects
rights outside that kind's allowed mask and still requires the requested rights
to be a subset of the source handle.

## Physical ownership

Each shared object allocates zeroed 4 KiB backing frames. The object owns the
initial physical reference. `paging::map_shared_frame` reuses the existing
bounded frame-reference table used by shared file pages/COW. Therefore:

- object only: refcount logically 1;
- first user mapping: physical refcount 2;
- second user mapping: physical refcount 3;
- unmap/destroy drops mapping references;
- final object release drops the original reference and frees the frame.

The runtime probe requires `memory::stats().allocated_frames` to return exactly
to its pre-probe baseline.

## Mapping transaction

`map_shared_memory` does not keep the IPC lock while modifying page tables.
Instead it:

1. validates object kind and rights under IPC lock;
2. takes a temporary object reference, pinning backing frames;
3. drops IPC lock;
4. maps each physical frame into the destination page table;
5. rolls back already-mapped pages on failure;
6. records the mapping under IPC lock; the temporary pin becomes the mapping
   lifetime reference.

`unmap_shared_memory` removes the mapping record transactionally, performs the
page-table unmap without the IPC lock, restores the record if paging fails, and
only then drops the mapping reference.

## Lifetime rules

Handle references and mapping references are independent. A process may close
all handles after mapping; the mapped pages remain valid. The object and backing
frames are destroyed only after the final handle *and* final mapping reference
are gone.

`unregister` rejects `MappingBusy` if the namespace still owns mappings. This
prevents the current Stage 8.4 kernel-only API from producing orphaned mapping
records. A future userspace VM syscall layer must unmap shared regions as part
of process-address-space teardown before unregistering IPC state.

## Runtime acceptance proof

The boot probe uses two fresh, independent user address spaces and a two-page
shared object. It checks:

- shared object creation and introspection;
- rights-reducing grant;
- rejection of endpoint SEND authority on a shared object;
- rejection of SHM_WRITE from a read/map-only peer;
- rejection of a writable peer mapping;
- RW+NX owner mapping and RO+NX peer mapping;
- physical-frame identity across both address spaces;
- cross-page data coherence;
- kernel-object write coherence;
- explicit unmap/remap;
- object survival after every handle closes;
- MappingBusy on premature unregister;
- exact frame-count recovery after address-space destruction.

## Locking

The Stage 8.3 `IPC -> scheduler` nested-lock hazard remains avoided. Stage 8.4
mapping operations pin objects and release the IPC state lock before page-table
work. No scheduler lock is acquired from the shared-memory object layer.

The final backing-frame release occurs from object reference release and enters
the paging/frame reference layer. No reverse `paging -> IPC` call path was added
in Stage 8.4.

## Non-goals

Stage 8.4 does not add the final userspace syscall ABI. This mirrors Stage 8.2's
kernel-first sequencing. Stage 8.5 will add general handle transfer, Stage 8.6
service discovery, and Stage 8.7 sustained SMP IPC/shared-memory stress.
---

## Source: AUDIT-STAGE8.5-CAPABILITY-HANDLE-TRANSFER.md

# Stage 8.5 audit - Capability handle transfer through IPC

## Baseline
Exact Stage 8.4 shared-memory stack-fix candidate that passed build, strict
Clippy, host tests, 1 CPU x10, 2 CPU x30, 4 CPU x20, and network 1/2/4.

## Security model
A sender never transmits its numeric handle value. A Stage 8.5 message can carry
an internal `TransferEscrow { object, rights }`. The source handle must include
`TRANSFER`; requested rights must be non-zero, object-kind-valid, and a subset
of source rights. The queue retain pins the object after sender closure.

At receive, allocation in the receiver's handle table occurs before dequeue.
If allocation fails (notably `HandleTableFull`), the queue entry and its escrow
remain unchanged. On success, the escrow reference is consumed by the newly
installed receiver handle without a redundant retain/release pair.

## Lifetime/rollback invariants
- Queue-full send rolls back the pre-enqueue object retain.
- Endpoint destruction walks queued messages and releases all transfer escrows.
- Sender closing its source handle after enqueue does not invalidate the queued
  capability.
- Receiver gets a fresh generation-tagged local handle.
- Closing the received handle releases the escrow-derived ownership normally.

## Endpoint cycle prevention
Owning escrow references can otherwise form refcount cycles (A queue -> B and B
queue -> A). Stage 8.5 performs a bounded graph walk over existing endpoint
transfer edges before enqueuing another endpoint capability. Any edge that would
create a cycle is rejected with `AccessDenied`. Shared-memory objects do not own
endpoint queues and therefore do not participate in this graph.

## Compatibility
`Message` gains an optional receiver-local transferred handle and a
`transferred_handle()` accessor. Existing ordinary messages retain the same
payload/sender semantics and return `None`. Stage 8.3 blocking receive uses the
same dequeue path and therefore inherits correct transfer installation.

## Production runtime proof
The Stage 8.5 probe validates:
1. rights amplification denial;
2. self/cycle edge denial;
3. shared-memory transfer with reduced rights;
4. sender-close survival through escrow;
5. receiver-local handle creation and read-only enforcement;
6. endpoint capability transfer;
7. abandoned-message escrow teardown;
8. queue-full retain rollback;
9. full receiver handle-table atomicity and retry;
10. final zero-object/zero-shared-memory cleanup.

## Acceptance rule
Stage 8.5 is not validated until the complete Stage 8.4 acceptance suite passes
unchanged and the Stage 8.5 marker is present in 1/2/4 CPU production logs.
---

## Source: AUDIT-STAGE8.6-SERVICE-DISCOVERY.md

# Stage 8.6 Audit â€” Named Service Registry + Capability Discovery

## Baseline
Built directly on the validated Stage 8.5 capability-handle-transfer tree.
No Stage 8.1â€“8.5 acceptance criteria are weakened.

## Problem
Stage 8.5 can securely pass endpoint and shared-memory capabilities, but a client
still needs an out-of-band way to learn which endpoint represents a system
service. Raw PIDs are not stable capability identities and would couple clients
to process placement/lifetime.

## Design
The IPC `State` now includes a bounded service registry. Each entry contains:

- bounded `ServiceName` (max 32 bytes),
- publisher/owner process ID,
- underlying endpoint `ObjectRef`,
- exact rights discoverers may receive.

Publishing retains one endpoint object reference. Discovery retains the object
again and allocates a fresh generation-tagged process-local handle. If handle
allocation fails, the retain is rolled back. Unpublishing or publisher teardown
releases the registry-held reference.

## New right
`HandleRights::PUBLISH_SERVICE` uses bit 7. Endpoint owners receive it with the
owner handle. A process holding only SEND/INSPECT cannot publish that endpoint.
Capability transfer may delegate PUBLISH_SERVICE only when the sender actually
possesses it and explicitly transfers that right; Stage 8.5 subset checks still
prevent rights amplification.

## Client rights ceiling
`HandleRights::SERVICE_CLIENT = SEND | TRANSFER | INSPECT`.
The registry rejects publication configurations containing RECEIVE or
PUBLISH_SERVICE. Discovery therefore cannot mint a second receiver/server or a
new publisher authority merely from a service lookup.

## Namespace semantics
Names must be non-empty, at most 32 bytes, and contain only ASCII alphanumeric,
'.', '-', or '_'. Registered names are globally unique. The registry is bounded
by `MAX_IPC_SERVICES = 32` and remains allocation-free.

## Process teardown
`State::unregister(owner)` removes all service entries published by `owner` and
releases their escrow references before draining ordinary process handles. This
prevents stale discoverable services after process exit.

## Runtime probe
The Stage 8.6 production probe verifies:

1. invalid/unsafe names are rejected;
2. RECEIVE cannot be delegated through service discovery;
3. a restricted endpoint lacking PUBLISH_SERVICE cannot be registered;
4. a valid service publishes successfully;
5. service metadata reports exact owner/object/client rights;
6. duplicate global names are rejected;
7. discovery returns a fresh client-local handle to the same endpoint object;
8. the discovered handle cannot receive;
9. a discovered client can send and the server receives the exact payload;
10. registry escrow keeps the service alive after the source handle closes;
11. non-owner unpublish is rejected;
12. unpublish removes discovery and releases its registry reference;
13. publisher process teardown automatically removes a service whose original
    endpoint handle was already closed;
14. final service/object counts return to their pre-probe baselines.

## Non-goals
Stage 8.6 does not yet add:

- a userspace syscall ABI for service registry operations;
- hierarchical namespaces or per-user namespaces;
- ACLs/policies beyond capability rights;
- service version negotiation;
- subscriptions to service arrival/departure;
- persistent service manifests;
- sustained concurrent SMP discovery stress (reserved for Stage 8.7).
---

## Source: AUDIT-STAGE8.7-DETERMINISTIC-SATURATION-FIX.md

# Stage 8.7 deterministic saturation fix

## Failure observed
The original Stage 8.7 stress harness passed 1 CPU but failed on the first 2-CPU run after Stages 8.1-8.6 had passed.

## Root cause
The receiver waited until a capability-bearing nonblocking send observed `QueueFull`. However, multiple workers were also issuing ordinary blocking sends before that condition was guaranteed. With two CPUs, the bounded endpoint could become full while both workers were blocked in ordinary sends. The receiver was still waiting for a later transfer attempt to report `QueueFull`, producing a circular wait in the test harness.

This was a Stage 8.7 orchestration bug, not evidence of an IPC kernel deadlock.

## Fix
Stage 8.7 now has two explicit phases:

1. **Deterministic saturation proof**: CPU0 alone discovers the service and uses only nonblocking capability-bearing sends until the endpoint reports `QueueFull`. The receiver intentionally does not drain until this proof exists.
2. **Mixed multicore stress**: the receiver drains and validates all prefill messages, then releases every online worker into the normal 32-round mixed blocking/nonblocking workload.

This guarantees the saturation proof without permitting ordinary blocking sends to create a circular wait first.

## Added diagnostics
On timeout or final invariant failure Stage 8.7 now prints:
- online/ready/done/success worker counts;
- CPU participation mask;
- QueueFull hits;
- prefill sent/drained counts;
- normal messages sent/received;
- capability transfers sent/received;
- receiver completion/failure code;
- per-CPU worker failure code, received count and round bitmap;
- waiter counts and IPC object/shared-memory/service/endpoint counts;
- individual final invariant booleans.

## Safety boundary
AP workers still do not enter paging, storage, networking, GUI, userspace-loader or heap paths. Shared-memory content validation remains on the receiver side.
---

## Source: AUDIT-STAGE8.7-SMP-IPC-STRESS.md

# Stage 8.7 Audit - SMP IPC Stress

## Scope

Stage 8.7 intentionally adds no new public userspace ABI. It is a production
runtime stress proof over the validated Stage 8.1-8.6 IPC stack.

## Concurrent topology

`stage8_7_runtime_probe()` creates one service endpoint, one shared-memory
object, one server receiver task, and one pinned producer on every online CPU.
All producers operate concurrently against the same global IPC state.

Each producer performs 32 rounds:

1. discover `woven.smp-stress` and obtain a fresh process-local service handle;
2. send an ordinary blocking message or, every fourth round, enqueue a
   reduced-rights shared-memory capability;
3. close the transient discovery handle;
4. yield periodically to increase scheduling interleavings;
5. unregister its IPC namespace immediately after its final send.

The receiver deliberately refuses to drain until a capability-bearing producer
has observed `QueueFull`. Because transfer rounds occur at 0, 4, 8, ... this
forces deterministic saturation of the 16-entry endpoint queue on 1/2/4 CPUs,
without relying on timer cadence or host speed.

## Safety and ownership properties checked

- **CPU participation:** a CPU mask must contain every online CPU.
- **Exact delivery:** each `(cpu, round)` tuple is accepted exactly once.
- **Blocking wakeups:** ordinary sends use the Stage 8.3 blocking path.
- **Queue rollback:** capability sends retry on QueueFull; Stage 8.5 must release
  the temporary retain on each failed enqueue.
- **Capability escrow:** worker namespaces are destroyed while capability-bearing
  messages may remain queued; the queue escrow must preserve object lifetime.
- **Rights reduction:** received transferred handles must be exactly
  `SHM_READ | INSPECT`.
- **Fresh receiver handles:** the receiver validates the installed server-local
  handle and closes it after inspection.
- **Shared-memory transfer/lifetime:** workers concurrently transfer reduced-rights
  references to one shared object; the CPU0 receiver validates the seeded bytes
  through each fresh receiver-local handle before closing it.
- **Service discovery concurrency:** every round performs fresh Stage 8.6 lookup
  and transient handle teardown.
- **Clean closure:** no waiter or queued message remains and all object/service
  counts return to baseline after final teardown.

## AP task contract

Pinned producer tasks use only atomics, the SMP-safe IPC lock, and scheduler
yield/wait/exit primitives. They do not enter the paging layer, manipulate page
tables, or call storage, network, GUI, userspace-loader, or heap services from
AP context.

## Acceptance

`run-stage8-7-acceptance.ps1` first reruns the entire validated Stage 8.6
acceptance chain, including strict `-D warnings`, host tests, 1 CPU x10,
2 CPU x30, 4 CPU x20, and network 1/2/4. It then requires the Stage 8.7 marker
in the final 1/2/4 CPU production serial logs.
---

## Source: AUDIT-STAGE9.1-WOVENGUARD-FOUNDATION.md

# Stage 9.1 Audit â€” WovenGuard Domain + Policy Foundation

## Goal

Establish the first central, enforceable WovenGuard policy layer without
changing the validated Stage 8 IPC ABI.

## Security model

A task now has two independent security properties:

1. `SecurityDomain` â€” maximum class of authority the task is eligible to hold.
2. `CapabilitySet` â€” authority the task currently holds.

This separation prevents a privileged actor from simply copying any capability
into an inappropriate target. The grant must pass the target domain ceiling.

## Domain assignments

- `Kernel`: bootstrap task only.
- `SystemService`: kernel workers/services created by the generic kernel-task path.
- `User`: Ring-3 processes.
- `Restricted`: idle/minimal infrastructure.

Fork preserves the parent's security domain as well as its capability set.

## Delegation checks

`task::grant()` now delegates authorization to `wovenguard::authorize_grant()`.
A successful grant requires all of:

- actor has `TaskControl`;
- actor already owns the delegated capability;
- target domain permits the capability;
- a non-Kernel actor cannot inject authority into the Kernel domain.

Denied grants do not mutate the target capability set.

## Audit behavior

Every known-target grant/revoke decision records a WovenGuard allow/deny audit
event followed by the existing capability grant/revoke audit event. This keeps
older audit invariants compatible while adding explicit policy-decision evidence.

## Regression-sensitive change

Generic kernel-task initialization now clears the capability set before assigning
`SystemService`. This prevents a reused TCB slot from retaining capabilities from
an earlier task.

## Runtime proof

Production boot requires:

`[S9.1] WovenGuard domains + least-privilege policy: PASSED`

The Stage 9.1 acceptance script first executes the complete Stage 8.7 acceptance
suite and then checks the Stage 9.1 marker on 1, 2 and 4 CPU boots.

## Validation status

IMPLEMENTED CANDIDATE â€” not validated until user/QEMU acceptance passes.
---

## Source: AUDIT-STAGE9.2A-CAPABILITY-LINEAGE.md

# Stage 9.2A Audit â€” Capability Lineage Foundation

## Security objective
Create a bounded, allocation-free ancestry primitive for authority delegation so
future revocation can invalidate descendants without guessing where authority
came from.

## Invariants
- A `LineageId` is generation-tagged; stale IDs fail after slot reuse.
- A derived token can never contain rights absent from its parent.
- Empty derived authority is rejected.
- Only the current token owner can derive from or release that token.
- A parent cannot be released while direct/indirect descendants remain because
  every live descendant keeps the ancestry chain connected through child counts.
- Lineage storage is fixed-capacity and protected by one `spin::Mutex`.
- No recursive revocation exists in this stage; release is leaf-only cleanup.

## Why revocation is deferred
Ancestry and revocation are separated on purpose. Stage 9.2A proves identity,
generation safety, subset rules and lifecycle bookkeeping. Stage 9.2B can then
add recursive invalidation without simultaneously debugging the lineage model.

## Compatibility
The Stage 9.1 task capability bitset remains authoritative in this stage. The new
lineage registry is a security primitive that is validated independently before
being bound into all task/IPC delegation paths. Therefore the Stage 8.7 and 9.1
behavioral baseline should remain unchanged.
---

## Source: AUDIT-STAGE9.2B-RECURSIVE-REVOCATION.md

# WovenHat OS Stage 9.2B Audit - Recursive Capability Revocation

## Scope
Stage 9.2B builds only on the Stage 9.2A generation-tagged lineage registry.
It does not yet bind every task capability or IPC handle to a lineage token;
that integration remains a later WovenGuard step. This milestone validates the
revocation primitive independently before widening its enforcement surface.

## Invariants
1. Revocation authorization follows ancestry ownership.
2. A target owner may revoke the target subtree.
3. An owner of a live ancestor may revoke authority descended from it.
4. An unrelated sibling owner receives PermissionDenied.
5. Subtree discovery and invalidation occur under one LINEAGES registry lock.
6. No descendant can remain valid after a successful subtree revocation.
7. Generation advancement makes pre-revocation LineageId values stale.
8. Sibling branches outside the subtree remain valid and usable.
9. Only the target branch is detached from a surviving parent.
10. Revocation is bounded and allocation-free.

## Algorithm
- Validate target generation/slot.
- Walk target -> ancestors (max 64) and authorize if actor owns any node.
- Snapshot a 64-element boolean revoke set before any mutation.
- Mark target plus every live descendant using bounded ancestry checks.
- Decrement the surviving boundary parent's direct-child count once.
- Clear every marked slot and advance its generation.
- Decrement registry count by the exact number revoked.
- Emit CapabilityLineageRevoke audit event.

## Runtime proof
The production boot creates two sibling branches and a three-node target
subtree. It checks unauthorized sibling revocation, ancestor-authorized subtree
revocation, stale descendant rejection, sibling preservation, post-revocation
derivation rejection, boundary child-count correctness, root teardown, slot
generation reuse, and zero metadata leakage.

## Acceptance policy
Stage 9.2B must preserve the complete Stage 9.2A acceptance chain and show its
new marker on 1/2/4 CPU production boots. Build, strict Clippy, unit tests,
1/2/4 CPU memory stress, and 1/2/4 CPU live networking therefore remain part
of the inherited acceptance gate.
---

## Source: AUDIT-STAGE9.2B-SMP-TLB-LIVENESS-FIX.md

# Stage 9.2B SMP TLB Liveness Fix

## Failure observed
The full Stage 9.2B acceptance passed 23 consecutive 2-CPU boots and timed out on run 24. The preserved serial log showed Stage 8.7, Stage 9.1, Stage 9.2A and Stage 9.2B had already passed. The final marker was `[PAGER-DIAG] load mmap ELF: BEGIN`; the matching `DONE` was never emitted.

## Root cause
The ELF loader builds a brand-new user address space before publishing it to the scheduler. `map_user_range_in` and `protect_user_range_in` nevertheless performed synchronous acknowledged global TLB shootdowns for every page-table edit. Likewise `destroy_user_address_space`, used after a scheduler-retired process is reaped or for unpublished rollback address spaces, performed global shootdowns while tearing down a CR3 that no CPU may legally be running.

Those shootdowns cannot invalidate useful translations for an unpublished/retired CR3. They only introduce a cross-CPU acknowledgement dependency into loader construction and reap-time teardown. Under SMP timing this created an intermittent liveness window, observed after the termination/reap test and during the immediately following second ELF load.

## Fix
- Preserve the existing conservative public mapping/protection APIs for potentially-live address spaces.
- Add `map_user_range_in_inactive` and `protect_user_range_in_inactive` for brand-new unpublished address spaces.
- Refactor mapping/protection through shared internal helpers with an explicit `synchronize_tlb` policy.
- Make the ELF loader use the inactive-address-space variants for executable segments and the initial stack.
- Remove global TLB shootdowns from `destroy_user_address_space`; document its retired/unpublished CR3 contract.
- Keep live `unmap_user_range_in`, COW, file mappings and other runtime paths unchanged.

## Why this is safe
Before first dispatch, the new CR3 has never executed on any CPU, so no CPU can have TLB entries tagged to that address space. After scheduler-owned retirement, a reaped process is off every CPU before destruction. PTE writes are therefore visible when that CR3 is later installed, and teardown needs no cross-CPU invalidation.

## Validation requirement
Do not weaken timeouts or reduce the established acceptance matrix. Re-run the existing Stage 9.2B acceptance unchanged. The fix is accepted only if build, strict Clippy, tests, 1CPU x10, 2CPU x30, 4CPU x20, network 1/2/4, and all inherited security markers pass.
---

## Source: AUDIT-STAGE9.2C-SMP-IRQ-LINEAGE-LOCK-FIX.md

# Stage 9.2C SMP IRQ-Safe Lineage Lock Fix

## Failure observed
During full Stage 9.2C acceptance, build, strict Clippy, host tests, 1-CPU x10, 2-CPU x30, and the first ten 4-CPU memory runs passed. 4-CPU run 11 timed out. The preserved serial log stopped immediately after:

`[S9.2A] WovenGuard capability lineage foundation: PASSED`

with no Stage 9.2B marker.

## Root cause
Stage 9.2C made the WovenGuard lineage registry reachable from scheduler lifecycle code through `Scheduler::reap_dead()` -> `wovenguard::revoke_all_owned_lineages()`.

The lineage registry used a plain `spin::Mutex`. Stage 9.2A/9.2B operations can hold that lock in normal kernel context while timer interrupts and scheduling are live. If the same CPU is interrupted while it owns `LINEAGES`, the interrupt-driven scheduler can enter dead-task reaping and try to acquire `LINEAGES` again. A spin mutex is not re-entrant, so that CPU waits forever for a lock it already owns.

This is a same-CPU interrupt re-entrancy deadlock. Its timing sensitivity explains why 1 CPU, all 2-CPU runs, and the first ten 4-CPU runs passed before the 4-CPU run 11 timeout.

## Fix
`kernel/src/wovenguard.rs` now centralizes every lineage-registry access through:

```rust
fn with_lineages<R>(operation: impl FnOnce(&mut LineageRegistry) -> R) -> R {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut registry = LINEAGES.lock();
        operation(&mut registry)
    })
}
```

This guarantees that local interrupts cannot re-enter lineage code while the CPU owns the registry lock. Other CPUs still synchronize through the spin mutex, so SMP mutual exclusion is preserved.

`revoke_all_owned_lineages()` was also changed to hold one IRQ-safe registry critical section for its complete bounded cleanup rather than repeatedly releasing and reacquiring the registry lock.

## What was deliberately not changed
- No timeout was increased.
- No acceptance count was reduced.
- No warning or Clippy lint was suppressed.
- Stage 9.2B recursive-revocation semantics were not weakened.
- Stage 9.2C task-capability enforcement semantics were not weakened.
- The scheduler, paging, IPC, and networking acceptance criteria remain unchanged.

## Validation requirement
Run the unchanged full Stage 9.2C acceptance script. Stage 9.2C is not considered fully validated until the complete 1/2/4 CPU and network matrix passes.
---

## Source: AUDIT-STAGE9.2C-SMP-MIGRATION-STRESS-FIX.md

# Stage 9.2C SMP migration-stress deterministic coverage fix

## Failure evidence
The preserved 2-CPU run-8 serial log did not hang. It reached a kernel panic in `kernel/src/smp.rs` at the final migration-stress CPU-mask assertion:

- observed mask: `0b10` (`2`)
- expected mask: `0b11` (`3`)
- panic: `migration stress did not exercise every CPU`

All WovenGuard markers S9.1, S9.2A, S9.2B and S9.2C had already passed in that boot. The pager and integrated Stage 7.6 userspace tests had also passed.

## Root cause
The old migration stress test spawned every migratable probe Ready on CPU 0, then immediately created migration/rebalancing pressure. A valid scheduler execution can migrate every Ready probe from CPU 0 to CPU 1 before any probe executes its first instruction. The test then sees only CPU 1 in `STRESS_MASK` and panics even though migration itself worked. This is a timing-dependent test oracle, not evidence that WovenGuard failed.

## Correction
The stress test now separates two properties that were previously conflated:

1. deterministic execution coverage: one short pinned atomic-only probe is spawned on every online CPU and the test waits for every CPU to execute;
2. migration pressure: the original `online_count * 4` migratable jobs still start on CPU 0, run 64 migration/rebalance rounds, require at least `jobs` successful ownership transfers, and must all complete.

The stress counts, migration threshold, timeout budgets, strict assertions, and production scheduler behavior are not weakened. The migration phase receives its own original 500-tick budget after coverage completes.

## Scope
Changed file: `kernel/src/smp.rs`.
The earlier WovenGuard IRQ-safe lineage registry hardening remains intact; the new preserved failure demonstrates that the reported run-8 failure itself was the SMP test oracle panic described above.
---

## Source: AUDIT-STAGE9.2C-TASK-AUTHORITY-ENFORCEMENT.md

# Stage 9.2C Audit - Real Task Authority Enforcement

## Security objective
Turn WovenGuard lineage from metadata into an enforcement dependency for real task capabilities.

## Enforcement model
A task capability is effective only when:
- its raw capability bit is present, and
- if a lineage binding exists, that generation-tagged lineage is still live and contains that capability.

This means recursive lineage revocation takes effect at the next capability check without requiring a mutable sweep through every task.

## Locking
The existing global order remains scheduler -> WovenGuard lineage registry.
Lineage registry operations never acquire the scheduler lock, so the new effective-authority checks do not introduce a reverse lock edge.

## Delegation
`task::grant()` still passes Stage 9.1 domain/TaskControl/no-amplification policy first. A successful delegation then derives a one-capability child lineage and stores it beside the target capability bit.

## Revocation
`task::revoke()` first passes TaskControl policy, then revokes a bound lineage subtree and clears local state. Empty temporary roots are released to preserve the fixed 64-entry lineage budget.

## Fork
Fork inherits capability lineage bindings with capability state. This prevents delegated authority from becoming intrinsic merely because a process forked.

## Lifecycle
`Scheduler::reap_dead()` recalls all lineage nodes owned by a retiring task before recycling its TCB. Descendants are invalidated by the same recursive semantics proved in Stage 9.2B.

## Scope intentionally deferred
- IPC handle lineage binding
- service discovery lineage binding
- shared-memory handle lineage binding
- SMP stress specifically targeted at simultaneous capability use/revocation

These should be closed in the next enforcement/stress stage rather than mixed into the first task-authority boundary.

## Build-cleanup correction â€” strict `-D warnings`

The Stage 9.2B inactive-address-space TLB liveness refactor introduced
`protect_user_range_in_inactive()` for ELF construction. After that migration,
the former public `protect_user_range_in()` wrapper had no remaining caller in
the kernel binary and therefore tripped the project's strict dead-code policy.

The obsolete wrapper has been removed. The shared
`protect_user_range_in_impl(...)` implementation and the inactive loader path
remain unchanged. This is an API/liveness cleanup only; it does not weaken TLB
synchronization for any live mapping path.

## Strict-Clippy follow-up fix

Focused validation exposed two `-D warnings` Clippy findings in `kernel/src/task.rs` after the Stage 9.2C enforcement changes:

1. `initialize_fork` exceeded the seven-argument Clippy threshold after capability lineage and security-domain inheritance were added. The security fields are now carried by `ForkSecurityContext`, keeping fork security provenance as one coherent value rather than suppressing `clippy::too_many_arguments`.
2. The intrinsic-lineage cleanup path used a nested `if` that triggered `clippy::collapsible_if`. The condition was flattened with identical semantics; no authorization or cleanup check was removed.

No lint allowances were added. Runtime policy and revocation semantics are unchanged.

## SMP IRQ-safe lineage registry correction
Full acceptance exposed an intermittent 4-CPU timeout after the Stage 9.2A marker. The cause was same-CPU interrupt re-entrancy on the plain `LINEAGES` spin mutex after Stage 9.2C made lineage cleanup reachable from scheduler dead-task reaping. All lineage-registry access is now wrapped in `x86_64::instructions::interrupts::without_interrupts`, and retiring-task lineage cleanup executes as one bounded IRQ-safe registry transaction. See `AUDIT-STAGE9.2C-SMP-IRQ-LINEAGE-LOCK-FIX.md`.
---

## Source: AUDIT-STAGE9.2D-IPC-OBJECT-CAPABILITY-LINEAGE.md

# Stage 9.2D â€” IPC/Object Capability Lineage Audit

## Security invariant
A process-local numeric handle is not sufficient authority when it was delegated. The handle must
also carry a live WovenGuard IPC lineage. Revocation therefore invalidates authority without a mutable
sweep through every process handle table.

## Design
Creator handles are intrinsic (`lineage = None`). The first delegation of an endpoint or shared-memory
object creates a WovenGuard root containing only `Capability::Ipc`. Delegated handles, queued transfer
escrows and service-discovery handles carry that lineage. Object-specific `u8` rights still control
SEND/RECEIVE/TRANSFER/SHM_* operations; WovenGuard lineage controls whether the delegation epoch is live.

A single object delegation epoch is intentionally shared transitively by all downstream transfers in
Stage 9.2D. This makes recall immediate and bounded: revoking the epoch invalidates every delegated copy,
including copies that crossed a queue. It also avoids allocating one global lineage slot per transfer.
After recall, the intrinsic object owner can establish a fresh epoch by delegating again.

## Lifecycle
Revoked handles remain closable through generation-checked raw removal, preventing security revocation
from becoming an object-reference leak. Destroying an object releases a live delegation root. A revoked
root is already absent from the lineage registry, so destruction tolerates stale metadata.

## Runtime proof
`stage9_2d_runtime_probe()` proves:
1. direct endpoint grant works before recall and fails immediately after recall;
2. creator intrinsic authority survives recall;
3. queued shared-memory transfer installs a lineage-backed fresh handle;
4. recall invalidates the transferred handle without invalidating the creator handle;
5. service discovery creates lineage-backed clients;
6. recall blocks both existing service clients and future discovery;
7. cleanup returns the lineage registry to its baseline count.

## Scope boundary
Already-installed shared-memory page mappings are not synchronously unmapped in 9.2D. Doing so safely
requires paging/TLB work outside the IPC lock and a mapping-specific revocation protocol. 9.2D gates all
new handle-mediated shared-memory I/O/mapping operations; active mapping recall is reserved for the later
resource/memory capability-gate stage.
---

## Source: AUDIT-STAGE9.2E-CLIPPY-DOC-COMMENT-FIX.md

# Stage 9.2E Clippy Documentation Placement Fix

## Failure
Strict `cargo clippy -- -D warnings` rejected `kernel/src/ipc.rs` with
`clippy::empty_line_after_doc_comments`.

## Root cause
The Stage 9.2D documentation comment was left above the newly inserted Stage
9.2E constants. Rust therefore attached that `///` documentation to
`STAGE9_2E_OWNER` instead of `stage9_2d_runtime_probe()`.

## Fix
The Stage 9.2D documentation block is now immediately adjacent to
`stage9_2d_runtime_probe()`. No lint suppression was added and no runtime,
security, stress, timeout, or acceptance semantics were changed.

## Validation policy
Run the unchanged `run-stage9-2e-acceptance.ps1`. Stage 9.2E remains pending
until that complete acceptance suite passes.
---

## Source: AUDIT-STAGE9.2E-SMP-REVOCATION-CLOSURE.md

# Stage 9.2E â€” SMP Revocation Stress and Lineage Closure

Stage 9.2E does not add a second revocation mechanism. It stress-proves the Stage 9.2Bâ€“9.2D production mechanism at the multicore boundary.

## Linearization contract

`revoke_object_delegations()` takes the IPC state lock, resolves the object's delegation root, and invokes WovenGuard's atomic subtree revocation. When the call returns successfully, that lineage generation is no longer live. A later handle lookup checks `lineage_authorizes()` and must reject it.

Operations that began before the revocation linearization point may complete. Operations initiated after return may not regain delegated authority.

## SMP proof

One worker is pinned to every online CPU. Each worker receives a lineaged shared-memory handle and must successfully use it before recall. The coordinator then revokes the shared object's delegation tree. After the revocation returns, each worker performs repeated read and inspection attempts and requires `AccessDenied` every time.

## Teardown and generation proof

Workers close revoked handles and unregister their IPC namespaces. The owner then repeatedly delegates to itself, revokes, closes, and recreates the delegated handle. The previous handle value must become `InvalidHandle` after slot reuse while the fresh generation works only until its own delegation epoch is revoked.

## Closure invariant

The probe succeeds only when IPC objects, shared-memory objects, services, and WovenGuard lineage count exactly match their pre-test baseline.
---

## Source: AUDIT-STAGE9.3-SECURITY-LEDGER.md

# Stage 9.3 â€” WovenGuard Security Audit Ledger

## Purpose
Convert the pre-existing diagnostic audit ring into a kernel security ledger
that can become the data source for a later graphical WovenHat Security Center.

## Architecture
The ledger remains fixed-size and allocation-free. Writers from syscalls,
WovenGuard lineage code, task policy and fault paths commit under a single SMP
spin mutex with local interrupts disabled, preventing same-CPU interrupt
re-entry deadlock while preserving global event ordering across CPUs.

Each retained event records a monotonic sequence, timer tick, CPU, actor, action,
target, action-specific detail and explicit allowed/denied outcome.

## Retention semantics
Capacity is 128 events. Once full, the oldest event is overwritten. The ledger
tracks the number of overwritten records so consumers can distinguish a complete
window from a truncated history. `recent_into` copies events oldest-to-newest
without allocating.

## Stage 9.3 proof
The local self-test proves wraparound, ordering and overwrite accounting without
mutating production history. The production runtime probe writes two sentinel
WovenGuard decisions through the real global path and verifies sequence order,
CPU attribution, structured fields, outcomes and bounded retention.

## Non-goals
Persistent storage, cryptographic sealing, user-space export policy and the GUI
Security Center are intentionally deferred. Stage 9.3 establishes the trusted
kernel event substrate first.
---

## Source: AUDIT-STAGE9.4A-DEAD-CODE-FIX.md

# Stage 9.4A build fix

The first acceptance run stopped during `cargo build` because `-D warnings` rejected two unused Stage 9.4A helpers: `SandboxProfile::ceiling()` and `default_sandbox()`.

Fix:
- Removed the unnecessary public `ceiling()` getter; policy continues to use `allows()` and `is_valid_for()`, avoiding exposure of a raw ceiling accessor that had no production caller.
- Wired `default_sandbox(domain)` into real task/scheduler initialization so the canonical domain-to-default-profile mapping is used rather than duplicated constants.
- No lint suppression, acceptance weakening, capability-policy change, or runtime timeout change.

Re-run the unchanged `run-stage9-4a-acceptance.ps1`.
---

## Source: AUDIT-STAGE9.4A-SANDBOX-PROFILE-FOUNDATION.md

# Stage 9.4A â€” Sandbox Profile & Capability-Ceiling Foundation

## Objective
Introduce a per-task WovenGuard sandbox profile below the Stage 9.1 SecurityDomain ceiling.

## Security model
Effective authority now requires all three conditions:

1. the task owns the raw capability bit;
2. any delegated lineage is still live; and
3. the bound sandbox profile permits that capability.

A sandbox profile is valid only when its capability ceiling is a subset of the task's SecurityDomain ceiling.

## Binding semantics
A TaskControl-authorized controller may bind a valid profile. Tightening is destructive: capabilities and lineage bindings outside the new ceiling are removed/revoked so later profile changes cannot resurrect hidden authority. Bind and deny outcomes are emitted to the Stage 9.3 security ledger.

## Fork semantics
ForkSecurityContext copies the SandboxProfile together with capabilities, lineage bindings and SecurityDomain. A fork child therefore cannot escape its parent's sandbox ceiling.

## Runtime proof
The production boot probe temporarily binds the idle task to an empty Restricted-valid profile, verifies TimerRead delegation is denied, verifies an invalid broader User profile is rejected for the Restricted domain, restores the Restricted profile, proves TimerRead delegation works again, revokes it, and checks the baseline profile is restored.

## Validation
Run `run-stage9-4a-acceptance.ps1`. Stage 9.4A is not frozen until the full inherited Stage 9.3 suite and the 1/2/4 CPU Stage 9.4A marker pass.
---

## Source: AUDIT-STAGE9.4B-IPC-SERVICE-EXPOSURE.md

# WovenHat OS Stage 9.4B â€” IPC / Service Exposure Policy

## Goal
Stage 9.4B extends the validated Stage 9.4A sandbox profile from a capability ceiling into a named-service exposure policy. A task can hold generic `Capability::Ipc` authority while still being denied access to classes of services it does not need.

## Security model
Effective service use now requires both generic IPC authority at the task/syscall layer and an allowed service class in the task's current sandbox profile. Service classes are allocation-free policy buckets: Core, Network, Storage, UI, AI, Application, and Test.

Existing Stage 9.4A profiles default to `ServicePolicy::ALL`, preserving all validated Stage 8/9 behavior. New application/service profiles can narrow publication and discovery independently without changing the global IPC ABI.

## Enforcement points
- `publish_service`: validates the publisher's live TCB sandbox service-publish mask before registry mutation.
- `discover_service`: validates the client's live service-discovery mask before a handle is returned.
- service-derived handles carry `ServiceClass` metadata.
- direct handle grants preserve that metadata and reject grants into a real task whose current profile denies the class.
- queued capability transfer preserves the service-class tag.
- `send_handle`, `send_handle_with_transfer`, and `send_handle_blocking` re-check the live sandbox profile before use of a service-derived handle.
- `close_handle` and `unpublish_service` remain teardown-safe even after a profile is tightened.

This recheck is intentional: a profile change must take effect immediately. WovenGuard does not rely only on the policy that existed when a service handle was first discovered.

## Audit ledger
Stage 9.3 is extended with:
- `SandboxServicePublish`
- `SandboxServiceDiscover`
- `SandboxServiceUse`

The ledger target is a stable FNV-1a hash of the service name for publish/discovery events. The detail field packs the sandbox profile ID and service class. Service-handle use records the handle value plus the same profile/class detail.

## Production runtime proof
The Stage 9.4B boot probe uses the real idle TCB and real IPC service registry, then restores the Stage 9.4A baseline. It proves:
1. a Network-only service policy may publish a Network service;
2. the same profile cannot publish an AI service;
3. it can discover and send through the allowed Network service;
4. it cannot discover an AI service;
5. after tightening the live profile to `ServicePolicy::NONE`, the already-open Network service handle immediately returns `AccessDenied` on send;
6. the deny is visible as the latest `SandboxServiceUse` ledger event;
7. the sandbox profile, service count, IPC namespace, and object count return to baseline.

Production marker:
`[S9.4B] WovenGuard IPC/service exposure policy: PASSED`

## Acceptance
`run-stage9-4b-acceptance.ps1` first executes the full Stage 9.4A acceptance chain, then requires the Stage 9.4B production marker in 1-, 2-, and 4-CPU memory regression logs.

## Scope boundary
Stage 9.4B governs named service exposure and service-derived IPC handles. Filesystem/resource path policy remains Stage 9.4C. Device-resource capability gates remain Stage 9.5.
---

## Source: AUDIT-STAGE9.4C-FILESYSTEM-RESOURCE-SANDBOX.md

# Stage 9.4C â€” Filesystem / Resource Sandbox

## Objective
Make `FileRead` and `FileWrite` coarse capability gates rather than global filesystem authority. A TCB-bound `SandboxProfile` now carries an allocation-free `FilePolicy` that determines which filesystem scopes may be read or written.

## Policy model
Effective filesystem operation requires:

`capability authority âˆ© SecurityDomain ceiling âˆ© SandboxProfile capability ceiling âˆ© FilePolicy scope permission`

Stable scope buckets are Root, System, Temporary, Mounted, Home, and Other. Classification occurs only after task-side path resolution/normalization.

## Open-descriptor closure
Checking only `open()` would be insufficient: a process could open a resource, then retain access after its sandbox is tightened. Stage 9.4C therefore stores `FileScope` in `FdKind::File`; dup/dup2/fork preserve it, while read/write/seek re-check the current sandbox profile. Close remains teardown-safe.

## File mappings
The process table now retains an optional FileScope alongside each memory mapping. New file-backed mmap operations check current policy and `msync` re-checks current write scope before flushing. Anonymous mappings carry no file scope. Unmap clears both mapping and scope metadata.

## Path operations
Scope gates are applied to open, stat, readdir, chdir, mkdir, unlink, and both source/destination paths of rename. O_CREAT additionally requires write permission for that scope.

## Audit ledger
`SandboxFileRead` and `SandboxFileWrite` records capture actor, scope, a stable path hash, and allow/deny outcome without storing path strings or allocating in the ledger.

## Runtime proof
The Stage 9.4C marker binds a restricted idle TCB to a temporary profile that allows System/Temporary reads and only Temporary writes, verifies permitted/denied scope decisions, tightens to `FilePolicy::NONE`, proves denial, checks the WovenGuard ledger, restores the original profile, and validates representative path classification.

## Scope boundary
Stage 9.4C gates new filesystem operations, descriptor use, new file-backed mappings, and `msync`. It does not synchronously tear down already resident mapped pages when policy changes; active mapping recall belongs with later resource/memory capability enforcement where TLB and mapping revocation can be coordinated safely.
---

## Source: AUDIT-STAGE9.4C-RUNNING-CODE-FIX.md

# Stage 9.4C running-code correction

This package supersedes the prior Stage 9.4C BUILD-FIX archive.

Corrections:
- Verified the `FdKind::File` match arms in `dup_current` and `dup2_current` end with commas.
- Added explicit commas to the following `PipeRead` and `PipeWrite` block arms in both matches, eliminating parser ambiguity.
- Preserved Stage 9.4C filesystem/resource sandbox semantics and all prior acceptance scripts.
- Added `RUN-STAGE9.4C.ps1`, a clean one-command launcher that sets the working directory and `CARGO_NET_OFFLINE` automatically.

Important: PowerShell prompt text (`PS C:\...>`) and prior command output must not be pasted as commands. Run only the launcher command shown with this package.
---

## Source: AUDIT-STAGE9.4C-SMP-FILE-AUTHORITY-FIX.md

# Stage 9.4C SMP File-Authority Fix

Observed acceptance evidence: build/clippy/host tests passed; 1 CPU x10 passed; 2 CPU passed runs 1-19, then run 20 timed out.

Correction:
- file descriptor read/write/seek now evaluate effective FileRead/FileWrite authority and the current FilePolicy under one SCHEDULER lock acquisition;
- file-backed mmap snapshots effective read/write authority and SandboxProfile atomically before taking PROCESS_TABLE;
- msync uses the same atomic file-authority snapshot;
- no timeout, stress-count, warning, or acceptance requirement was weakened;
- filesystem scope semantics and the Stage 9.4C production marker are unchanged.

This closes a TOCTOU window between capability and sandbox-profile checks and reduces scheduler-lock contention in SMP file-I/O paths.
---

## Source: AUDIT-STAGE9.4D-DOMAIN-CEILING-FIX.md

# Stage 9.4D Domain-Ceiling Runtime Fix

## Failure
The first 1-CPU QEMU preservation run timed out after build, clippy, and host tests passed.

## Root cause
Stage 9.4D workers are created by `spawn_on`, whose TCB initialization assigns `SecurityDomain::SystemService`. The final non-resurrection phase attempted to bind `SandboxProfile::KERNEL_TRUSTED`. WovenGuard correctly rejects that profile because it exceeds the SystemService domain ceiling. The Stage 9.4D probe therefore returned false and the boot failure path halted, which appeared externally as a QEMU timeout.

## Fix
The final re-broadening phase now binds `SandboxProfile::SYSTEM_SERVICE`, the broadest default profile valid for the worker's actual domain. Workers expect that same profile. The security property remains unchanged: `DeviceIo` was destructively removed during the first tightening and must remain absent after re-broadening.

No timeout, stress count, warning policy, or acceptance criterion was weakened.
---

## Source: AUDIT-STAGE9.4D-SANDBOX-LIFECYCLE-SMP-CLOSURE.md

# Stage 9.4D â€” Sandbox Lifecycle + SMP Closure

## Goal
Close Stage 9.4 by proving that a live TCB-bound sandbox can be tightened while tasks execute on every online CPU, and that the scheduler lock is the policy linearization point seen by all readers.

## Production proof
One pinned kernel worker is created per online CPU. The coordinator applies two progressively restrictive profiles while workers are live. Workers verify the exact profile after each phase, verify `DeviceIo` is stripped while `TimerRead` remains, and then verify that rebinding the broad `KERNEL_TRUSTED` profile does not resurrect the destructively removed `DeviceIo` authority.

The second tightening also changes service exposure to `ServicePolicy::NONE` and filesystem exposure to `FilePolicy::NONE`, exercising the combined Stage 9.4A/9.4B/9.4C profile as one coherent object.

## SMP contract
`bind_sandbox_profile()` and sandbox/capability readers serialize on the global scheduler lock. The profile replacement and destructive capability stripping therefore form one linearization point. Atomic phase publication occurs only after all target TCBs have been updated.

## Acceptance
The full Stage 9.4C acceptance chain is rerun unchanged. Stage 9.4D then requires the production marker on the final 1-, 2-, and 4-CPU serial logs.
---

## Source: AUDIT-STAGE9.5-RESOURCE-DEVICE-CAPABILITY-GATES.md

# Stage 9.5 â€” Resource / Device Capability Gates

## Security objective
Move WovenHat away from one broad `DeviceIo` permission toward explicit,
auditable resource authority before driver isolation and graphics/input work.

## Capability decomposition
- `NetworkIo`: socket/DNS/DHCP/ping resource access; included in normal userspace.
- `StorageIo`: direct storage-device resource access; SystemService/Kernel only.
- `DisplayIo`: direct display resource access; SystemService/Kernel only.
- `InputIo`: direct input-device resource access; SystemService/Kernel only.
- `DeviceIo`: retained as the legacy/generic raw-device class during migration.

## Sandbox device policy
`SandboxProfile` now carries an allocation-free `DevicePolicy` bitmask. A
resource operation succeeds only when the caller has the class capability AND
the current sandbox allows the device class.

## Real enforcement
Network-facing syscalls call `task::authorize_current_device(Network)` before
resource use. `socket_close` intentionally remains teardown-safe so policy
tightening cannot trap a resource handle that must be released.

## Audit
Each runtime gate emits `SandboxDeviceAccess` with the device class as target,
the required capability as detail, and explicit allow/deny outcome.

## Compatibility
The User domain and `CapabilitySet::userspace()` include `NetworkIo`, preserving
existing validated userspace networking. Direct storage/display/input resource
caps are restricted to SystemService and Kernel domains.

## Acceptance
The Stage 9.5 acceptance recursively preserves the full Stage 9.4D chain, then
requires the Stage 9.5 production marker on 1/2/4 CPU boots. No stress count,
timeout or prior acceptance criterion is weakened.

## Build fix â€” 2026-09-13
The first Stage 9.5 candidate failed strict `-D warnings` because the new
`DeviceClass::Display` and `DeviceClass::Input` variants were defined and
mapped to capabilities but were not constructed by the production proof.
The proof now explicitly verifies that a SystemService profile can authorize
both DisplayIo and InputIo. This removes the dead-code failure by exercising
the intended policy paths; no lint suppression or acceptance weakening was
added.
---

## Source: docs/audit-stage10-7-2026-09-16.md

# Stage 10.7 audit â€” 2026-09-16

## Audit scope

Repository inventory, build/acceptance scripts, Stage 10 documentation, async
network/completion code, socket lifecycle, relevant syscall and scheduler paths,
transport locks and Ring-3 TCP probes were inspected. This is a focused Stage 10.7
audit, not a claim that every line of every historical subsystem is formally verified.

## Findings and corrections

1. **High: blocked requests starved later requests.** The worker selected the
   first pending slot on every attempt and stopped its pass on `WouldBlock`.
   A host regression running the actual worker failed with no completion for a
   ready second socket. The corrected worker scans each slot once per event pass.
   The host regression passes; the Ring-3 probe now posts receive before send.
2. **High: plain spin locks allowed same-CPU preemption deadlock.** Kernel workers
   start with interrupts enabled. A timer could preempt a network/completion lock
   holder and schedule a syscall or teardown path needing that lock. Interrupt-safe
   guards now cover the network queue, generic operation table, socket runtime and
   VirtIO transport. A pre-guard one-CPU TCP run timed out after its entry marker;
   the strengthened post-guard suite passed on 1/2/4 CPUs. The halted instruction
   pointer was not captured, so that timeout alone does not identify its exact lock.
3. **High: the readiness bridge used two inconsistent observations.** A reader
   could see no Pending request, then the worker could transition InProgress ->
   Pending and clear its atomic bridge, then the reader could see the cleared
   bridge. One queue-locked predicate now includes both states. A host regression
   injects progress while the only request is InProgress.
4. **Medium: host exchange completion was not an explicit acceptance condition.**
   Stopping the server suppressed its incomplete-exchange error. Startup used a
   fixed delay. The harness now has listener-ready and exchange-finished events,
   requires both exchanges, and tests incomplete/corrupt/bind-failure cases.
5. **Validation infrastructure:** stress startup deleted prior artifacts and killed
   all QEMU processes by name. It now retains per-run evidence and relies on each
   harness to manage only its own QEMU process. The stage launcher records a
   transcript and snapshots regression logs. Memory boots also save QEMU output.
6. **Build input tracking:** the image build script lacked explicit rerun inputs.
   It now tracks `build.rs` and the kernel artifact rather than treating changing
   acceptance evidence as an image input.

## Security, ABI and concurrency

No syscall numbers, permissions or WovenGuard policies were broadened. Validated
copies, owner checks and generation-pinned in-flight sockets are preserved. The
new interrupt guard contains no unsafe Rust. It protects bounded critical sections
and releases locks before restoring IF. Queue -> runtime and runtime -> transport
ordering is retained; notification occurs after runtime unlock. This is still one
CPU0-owned worker and a serialized network runtime, not parallel NIC processing.

## Files

Added: `AGENTS.md`, `kernel/src/irq_lock.rs`, `tests/async_network.rs`, `tests/irq_lock.rs`,
`tests/test_tcp_harness.py`, this report, `architecture-and-codebase-guide.md`,
`stage-status.md`, and the supplied `master-development-roadmap.md`.

Modified: `async_network.rs`, `async_op.rs`, `network.rs`, `virtio_net.rs`,
`userspace.rs`, `main.rs`, `build.rs`, `.gitignore`, the Stage 10.7 launcher/gate,
the Stage 7.6 stress runner, and the TCP/memory Python harnesses.

## Acceptance results

The final full QEMU gate passed on 2026-09-16 (75 boots): memory preservation
10/30/20 boots on 1/2/4 CPUs, plus live networking and isolated block/file/UDP/TCP
on each CPU configuration. The chain emitted `=== STAGE 10.7 ACCEPTANCE: PASS ===`
and returned status 0. Final recorded checks also passed: `cargo build`,
`cargo clippy -- -D warnings`, freestanding-kernel Clippy with `-D warnings`,
13 Rust host tests and 4 Python harness tests.

Failure evidence before interrupt guards is under
`audit-artifacts/before-irq-guards/`; the reproduced starvation failure and
preserved pre-existing stress logs are under `audit-artifacts/baseline-*`.

Successful stress serial logs are in `stage7-6-artifacts/20260916-081211-645/`.
The stage launcher snapshots the final suite logs in `audit-artifacts/acceptance-*`.
The observed terminal tail is saved as `audit-artifacts/acceptance-observed-tail.txt`.
The initial transcript mechanism omitted native child output; the launcher now
explicitly tees that output, with stdout/stderr and nonzero-exit capture verified
by a controlled host probe. The earlier stress boots still have individual serial
evidence. Final static checks are recorded under `audit-artifacts/final-static/`.

Known limitations and next-stage prerequisites are recorded in the architecture
guide and stage status. These are debug QEMU acceptance results, not physical
hardware qualification or a production-completeness claim for the entire OS.
---

## Source: docs/audit-stage10-7-acceptance-2026-10-17.md

# Stage 10.7 acceptance evidence â€” 2026-10-17

The user-provided transcript from `RUN-STAGE10.7.ps1` ends with:

`=== STAGE 10.7 ACCEPTANCE: PASS ===`

The transcript reports host TCP regressions (33 tests), host lint, Cargo
build/Clippy/tests, preservation through Stages 7.6â€“10.6, and dedicated
async-TCP boots on 1, 2, and 4 CPUs. Each CPU count passed the host TCP
exchange: connect, two transmissions, receive, and FIN/EOF, with QEMU exit
33 and retained serial logs under the project-local `target` tree.

This records the supplied run only. It does not replace the full release
validation chain, physical USB/xHCI evidence, or AX201 hardware qualification.
---

## Source: docs/audit-stage10-7-release-rerun-2026-10-21.md

# Stage 10.7 release-chain revalidation â€” 2026-10-21

## Evidence

The authoritative `RUN-STAGE10.7.ps1` chain completed its host regressions,
warning-denying host Clippy, host tests, Stage 7.5/7.6 preservation loops,
1/2/4-CPU networking, and preserved Stage 8.1 through Stage 10.6 markers.

The chain's first isolated Stage 10.7 image build failed with Rust/LLVM
`no space on device` after the build volume reached zero free space. This was
an infrastructure failure; no test assertion or source diagnostic failed.

After task-owned QEMU cleanup and removal of disposable `.cargo-build`
output, the isolated TCP gate was rerun directly:

- `python scripts/test-stage10-7-tcp.py --cpus 1`: PASS
- `python scripts/test-stage10-7-tcp.py --cpus 2`: PASS
- `python scripts/test-stage10-7-tcp.py --cpus 4`: PASS

Each run verified two host-to-guest TCP transmissions, guest response, and
remote FIN/EOF completion. Serial evidence is preserved in
`target/stage10-7-regression-{1,2,4}-debug/serial.log`.

## Scope and limits

This revalidation made no source changes and does not replace the physical
TPM, USB, or real-hardware qualification gates. The shared agent's unrelated
uncommitted persistent-account provisioning edit was preserved.
---

## Source: docs/audit-stage10-8-2026-09-16.md

# Stage 10.8 audit and validation

Status: accepted. The full preservation gate passed with host exit status 0.

Added bounded per-process completion ports, one-time operation association,
batch poll/wait, timeout-aware scheduler waits, cancellation notifications,
generation retirement and teardown. The public contract and limits are specified
in [completion-port ABI](completion-port-abi.md).

Host tests include the production queue, operation and port modules. They cover
completion before association, duplicate association, wrong owner, exclusive
consumer claims, copyout retry, independent result lifetime, cancellation,
stale producers, four concurrent producers, saturation, batch draining, stale
port handles and teardown. Queue tests additionally force sequence exhaustion.
The host lock/scheduler doubles do not prove interrupt or context-switch safety.

The Ring-3 probe covers the actual syscalls, invalid destination retry, batching,
cancellation, timeout, stale port rejection and process exit with pending work.
Its finite wait checks elapsed kernel ticks. A kernel probe completes distinct
operations from tasks on every online CPU and validates their cookies and values.
A second task registers a waiter before another CPU publishes its completion,
checking the registration-to-sleep path through the actual scheduler.
Focused QEMU boots passed on 1, 2 and 4 CPUs before the elapsed-time assertion was
added. The full gate reruns those boots with that assertion.

Review covered operation/port lock ordering, local IRQ exclusion, notification
after unlocking, reservation backpressure, copying outside locks, generation
reuse and scheduler-locked process teardown. TCB initialization resets the new
deadline field. Existing service-only completion authority and class-specific
data collectors remain enforced.

Validation command: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
.\RUN-STAGE10.8.ps1`. This includes build, host tests, warning-denying kernel and
probe Clippy, the complete Stage 10.7 gate, and three completion-port QEMU boots.

Timers/events and asynchronous IPC remain later stages. The port mechanism can
route existing generic operation classes, but this stage does not certify mixed
real-I/O workloads or a production multithreaded runtime.

Final evidence: `audit-artifacts/acceptance-20260916-085314-575/acceptance-output.txt`.
All 78 QEMU boots passed, including the measured timeout and producer wakeup
probes on 1/2/4 CPUs. Build and warning-denying host/kernel/probe Clippy passed.
All 18 Rust and four Python tests passed. Final Rust output is retained at
`audit-artifacts/stage10-8-final-host.txt`; the last host tests add port quota,
cross-owner close rejection and registered-waiter notification checks.
An additional `cargo clippy --all-targets -- -D warnings` check found three
pre-existing nested-if style warnings in the host inclusion of `async_network.rs`
and `fat32.rs`. The required default host and kernel lint gates passed. These
expanded host-target warnings remain to be corrected with the next source batch;
the expanded command is not recorded as passing.
---

## Source: docs/audit-stage10-9-2026-09-16.md

# Stage 10.9 audit â€” 2026-09-16

Stage 10.9 adds bounded process-owned one-shot and periodic timers, monotonic
deadlines, manual-reset event objects, cancellation, sleep-until scheduling,
and timer/event completion classes (syscalls 87â€“94). Handles carry generations,
teardown releases owner state, and the worker pumps timers without busy waiting.

Validation completed:

- `cargo build` passed.
- Kernel Clippy with `-D warnings` passed, including the Stage 10.9 feature.
- 18 Rust regression tests passed.
- QEMU Stage 10.9 passed on 1, 2, and 4 CPUs (exit 33), with serial evidence in
  `audit-artifacts/stage10.9-*`.

The kernel structural probe covers event completion, timer completion,
deadline advancement, cancellation, and owner teardown. The syscall surface is
wired for application use; a follow-up runtime stage will exercise all async
classes through one completion architecture.
---

## Source: docs/audit-stage11-1-2026-09-16.md

# Stage 11.1 audit â€” 2026-09-16

The process subsystem provides PID allocation, parent/child relationships,
exit status, wait semantics, process groups, isolated address spaces, per
process file and IPC handles, capabilities, and deterministic teardown. A
dedicated acceptance feature and SMP gate now keep those guarantees explicit
while later stages add threads and notifications.

Build, warning-denying freestanding Clippy, host regressions, and QEMU boots on
1, 2, and 4 CPUs passed. Serial evidence is retained under
`audit-artifacts/stage11.1-*`.
---

## Source: docs/audit-stage11-2-2026-09-16.md

# Stage 11.2 audit â€” 2026-09-16

Added the bounded process-thread registry with generation-safe thread IDs,
owner-checked join, termination status, per-thread TLS storage, and thread-local
errno state. The structural probe verifies running-to-exited transitions,
join-before-exit behavior, owner cleanup, and stale-state retirement.

Build, freestanding Clippy with `-D warnings`, host regressions, and QEMU
acceptance passed on 1, 2, and 4 CPUs. Evidence is retained under
`audit-artifacts/stage11.2-*`.
---

## Source: docs/audit-stage11-2-scheduler-threads-2026-10-14.md

# Stage 11.2 scheduler-backed thread increment â€” 2026-10-14

This audit records the production-safety increment built on the earlier
generation-safe thread registry. It does not close the complete Stage 11
roadmap: shared libraries, symbol resolution, dynamic TLS, and hardware
qualification remain open.

## Implemented boundary

- Scheduler task control blocks now carry stable process ownership and an
  optional generation-safe thread ID.
- `ThreadCreate` validates the entry against an executable mapping, creates a
  private NX stack, copies the process's bounded initial-exec TLS image, and
  schedules the new task in the owner's address space.
- `ThreadExit` latches the exit status in the TCB and publishes registry
  termination only after the scheduler has switched away from the exiting
  thread. This prevents a joiner from unmapping a stack that is still in use.
- `ThreadJoin` is owner-checked and retains exited records/resources until the
  owner joins them; join then unmaps the private stack/TLS mappings before the
  TCB and generation slot are recycled.
- `ThreadSelf`, `ThreadSetTls`, and `ThreadGetTls` expose the bounded ABI
  required by the current initial-exec TLS model.
- Kernel-task identity remains compatible with existing sandbox and scheduler
  self-tests; user-task identity is process-stable for shared-address-space
  lookup.
- A joiner now registers with the thread record before blocking. Thread exit
  wakes that task through the scheduler's latched event path after safe
  hand-off, eliminating userspace polling and closing the registration/exit
  race.
- Process-wide termination now retires ready/blocked siblings immediately,
  requests scheduler-safe termination for siblings executing on another CPU,
  releases per-task async ownership, and permits all resulting TCBs to be
  reclaimed after the address space remains alive through parent reaping.

## Validation

The following gates passed from the repository root with project-local Cargo
build directories:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-2-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test   (all host integration/unit suites in the Stage 11.2 launcher)
```

The launcher also passed the Ring-3 create/exit/join acceptance on all target
CPU counts, with exit code 33 and retained serial logs:

```text
audit-artifacts/stage11.2-1cpu-1791975128090915200
audit-artifacts/stage11.2-2cpu-1791975197249865000
audit-artifacts/stage11.2-4cpu-1791975247102041500

The event-driven join follow-up reran the same gate and retained these newer
serial artifacts:

```text
audit-artifacts/stage11.2-1cpu-1791975427938656800
audit-artifacts/stage11.2-2cpu-1791975491088108800
audit-artifacts/stage11.2-4cpu-1791975537755343700

The process-teardown follow-up reran the same gate and retained these newer
serial artifacts:

```text
audit-artifacts/stage11.2-1cpu-1791975759656374100
audit-artifacts/stage11.2-2cpu-1791975829411200100
audit-artifacts/stage11.2-4cpu-1791975876876280500

The registry self-test now directly covers waiter registration, termination,
waiter hand-off, and join status. The resulting regression gate retained:

```text
audit-artifacts/stage11.2-1cpu-1791976051779252200
audit-artifacts/stage11.2-2cpu-1791976117458262700
audit-artifacts/stage11.2-4cpu-1791976165640693800
```
```
```
```

The image emitted the existing lower-stage markers and the new
`[S11.2] scheduler-backed thread: PASSED` marker in each serial log.

## Explicit remaining limits

This increment is not a claim that all Stage 11 requirements are complete.
Join currently exposes a bounded nonblocking retry result rather than a
fully event-driven wait queue. Process-wide sibling cancellation and teardown
of unjoined thread records still need a dedicated lifecycle pass. Shared
libraries, symbol-based relocations, dynamic TLS/module IDs, and physical
hardware qualification remain fail-closed or unqualified.

## 2026-10-15 sibling-cancellation follow-up

The scheduler-backed thread boundary now includes append-only syscall 136,
`ThreadCancel`. A process may cancel an owned sibling with an explicit exit
status. Ready, blocked, and sleeping targets are retired while the scheduler
lock protects selection metadata; running and switching targets carry the
request to the existing post-switch finalizer. Both paths transition the
generation-tagged registry exactly once, wake the registered join waiter, and
leave the thread record/resources available for `ThreadJoin` to unmap and
reclaim. Canceling the current thread, a foreign handle, an exited handle, or
an invalid generation is rejected.

The Stage 11.2 Ring-3 image now exercises normal create/exit/join followed by
create/cancel/join with status 23. The focused gate passed on 1, 2, and 4
CPUs; serial evidence is retained at:

```text
audit-artifacts/stage11.2-1cpu-1792053325191002600
audit-artifacts/stage11.2-2cpu-1792053407390931300
audit-artifacts/stage11.2-4cpu-1792053457047808100
```

Additional validation passed:

```text
cargo check -p wovenhat-os --all-targets
cargo clippy -p wovenhat-os --all-targets -- -D warnings
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-2-test -- -D warnings
cargo test -p wovenhat-os --tests --no-fail-fast
```

This closes sibling cancellation for the bounded scheduler-backed thread
increment. It does not claim process-wide teardown of unjoined thread records,
shared-library loading, cross-image symbol resolution, dynamic TLS/module IDs,
or physical hardware qualification.

## 2026-10-15 process-teardown follow-up

The process exit path now retires unjoined scheduler-backed siblings before
publishing the process zombie. Ready/blocked/sleeping siblings are removed
from scheduling and their thread records are terminated after releasing the
scheduler lock. Running siblings receive the internal thread-exit request and
complete through the existing post-switch finalizer; the termination-request
safe point distinguishes that sentinel from a process signal. The exited
process retains its address space and mappings until `wait_process` observes
that no sibling task is live, then reclaims thread records and destroys the
address space after releasing the process-table lock.

This closes the previously open unjoined-sibling teardown requirement for the
bounded scheduler-backed thread model. The Ring-3 image explicitly creates a
third sibling and exits the process without joining it. Final focused evidence:

```text
audit-artifacts/stage11.2-1cpu-1792057473308506500
audit-artifacts/stage11.2-2cpu-1792057580969821900
audit-artifacts/stage11.2-4cpu-1792057667538341900
```

The earlier failed runs remain preserved as diagnostic evidence: one
identified lower-ranked registry cleanup under the scheduler lock, and the
other identified a privileged `HLT` in the Ring-3 fixture. Neither is counted
as acceptance evidence. Host tests, host all-targets Clippy, freestanding
Stage 11.2 Clippy, and the final 1/2/4-CPU gates passed after both fixes.
---

## Source: docs/audit-stage11-3-5-2026-09-16.md

# Stages 11.3â€“11.5 audit â€” 2026-09-16

Stage 11.3 adds bounded structured lifecycle notifications for termination,
suspend/resume, child exit, exceptions and user events. Stage 11.4 adds the
`libwoven` no-std API boundary covering process, thread, filesystem, network,
IPC, async I/O, security, time and graphics namespaces. Stage 11.5 retains the
loader's ELF validation, W^X enforcement, stack guards and segment permission
checks as the hardened loading baseline.

Each module has an individual feature and marker. Build, Clippy, host tests and
QEMU passed for every module on 1, 2 and 4 CPUs. Evidence is retained in the
timestamped `audit-artifacts/stage11.3-*`, `stage11.4-*`, and `stage11.5-*`
directories.
---

## Source: docs/audit-stage11-aslr-geometry-2026-10-15.md

# Production-ASLR geometry â€” bounded correction accepted

## Current decision (2026-10-20)

The arena/guard overlap correction described by the original investigation is
implemented and accepted as a bounded Stage 11.2-ASLR increment. The prior
failure evidence below is retained as historical defect evidence; it is not the
current implementation status. This audit does not claim full Stage 11
closure, broader entropy qualification, or physical hardware validation.

## CPU reproduction â€” 2026-10-16

The bounded compiler local-exec TLS prerequisite is accepted in `8b5d7150`.
A new independently guarded `stage11-aslr-test` uses the same production graph
loader with explicit private geometry, not a separate test loader. It forces
the supported stack draw 16 / mmap draw 0 pair. Its Ring-3 program calls the
ordinary Mmap syscall sixteen times for writable 64 KiB allocations and checks
every returned address against the expected arena slots. Only then does it
write the intended guard, checked unmapped before spawning. Allocation failure
exits 98; a successful guard write exits 99; the regression requires fault
status -11 and exact frame/process/endpoint/thread teardown.

Session 43891 exited 1, QEMU 35, as expected for this unfixed defect. Serial:
`audit-artifacts/stage11.2-aslr-1cpu-1792103210808207600/serial.log`.
It records guard `0x4000001df000`, process status `Some(99)` and exact frames
3488/3488. Thus actual CPU execution confirms removal of the intended guard,
not merely a source overlap or a rejected allocation. Existing failure
assertions and timeouts are unchanged. This is retained failing evidence,
not acceptance. The private geometry seam preserves normal production draws;
no layout correction is implemented in this checkpoint.

The harness additionally requires the original Stage 11.2 scheduler-backed
thread and overall markers; host negative tests reject each missing marker.
The new gate deliberately remains red until the joint planner is corrected.

Regression-source checks pass: all 168 root Rust host tests, twenty-two Python
harness tests, warning-denying host all-targets Clippy, and both original
`stage11-2-test` / new `stage11-aslr-test` kernel feature Clippy. Logs are in
`audit-artifacts/aslr-geometry-20261016/`. These establish that the regression
integrates cleanly; they do not convert its failed CPU security assertion into
acceptance. Full inherited preservation must run after the actual correction.

This read-only review ran during the frozen compiler-TLS chain (session
77593). It changes no kernel source, executes no second build/QEMU chain and
does not certify a planner. Source baseline: `e58b803b`.

## Authoritative source relationships

`userspace::aslr_stack_base` subtracts a draw of 0â€“63 pages from offset
`0x1f0000`. `aslr_mmap_base` adds a draw of 0â€“79 pages to `0xe0000`.
`entropy::random_range` has an exclusive upper bound. Config preserves sixteen
anonymous slots, each with a 64 KiB stride/maximum. `UserStack` reserves a
4 KiB guard immediately below its base and an 8 KiB stack above it.

The two draws are independent. A direct enumeration of all 5120 stack/mmap
pairs, without compiling or changing source, gives:

| Geometry | Count |
| --- | ---: |
| Arena ends at or before the guard | 136 |
| Arena end exceeds the guard start | 4984 |
| Arena ends exactly at the stack base | 17 |

These are domain counts, not measured production probabilities or QEMU proof.
For stack draw `s` and mmap draw `m`, the arena overlaps the guard whenever
`s + m > 15`. Template/TCB reservations need additional disjoint space.

## Concrete guard counterexample

Use stack draw 16 and mmap draw 0. Offsets from USER_REGION_START are:

- stack base: `0x1e0000`;
- intended guard: `[0x1df000, 0x1e0000)`;
- mmap slot 15: `[0x1d0000, 0x1e0000)` for a full 64 KiB allocation.

`task::mmap_current` chooses a free mapping slot and delegates to
`userspace::map_anonymous`. That function bounds slot/size/alignment but does
not exclude the stack guard. `paging::map_user_range_in_impl` refuses an
already-mapped page; an intentionally unmapped guard does not satisfy that
check. In this counterexample the allocation ends before the first mapped
stack page. Source analysis therefore indicates that allocating the full last
slot can remove the intended guard, rather than merely returning AlreadyMapped.
A dedicated forced-draw CPU test must establish the actual runtime failure
before implementing and accepting the correction.

QEMU feature builds deliberately use zero draws. Their existing success is
valid for the deterministic bounded contract, not for randomized geometry.
The new TLS loader rejects a reservation crossing the mmap bound, safely
failing load; that does not repair the inherited geometry for ordinary images.

## Required correction gate

After the current TLS prerequisite passes full acceptance, build a validated
joint address-space plan, preserving fixed ET_EXEC placement, nonzero PIE
relocation, the established sixteen-slot/64 KiB capacities, W^X and unmapped
guards. Keep all regions disjoint: main ELF, full anonymous arena, immutable
TLS template, mutable TLS/TCB, guard and stack, plus the bounded library windows.
Audit maximum image/reservation limits rather than silently widening them.

Do not disable ASLR, replace real draws with zero, reduce slot capacity or
accept random load failure as the fix. Exercise every supported draw in pure
host geometry tests, then actual nonzero/extreme-draw CPU execution, full arena
mapping and guard-fault behavior on 1/2/4 CPUs. Preserve old zero-draw behavior
and all inherited gates. Review overflow, malformed image/range inputs, fork,
thread allocation and teardown. Commit source and acceptance separately.

Until that evidence exists, randomized placement and full Stage 11/1â€“19
production readiness remain open regardless of the bounded TLS outcome.
---

## Source: docs/audit-stage11-compiler-pie-2026-10-15.md

# Compiler-produced signed shared PIE â€” accepted bounded correction

## Full acceptance decision

Source `da6d16e6` passed complete serialized preservation. Session 73596 exited
**0** at 2026-10-15 22:42:56 local with:
`=== COMPILER SHARED PIE FULL ACCEPTANCE: PASS ===`.
Independent post-run inspection verified all 505 frozen source/fixture inputs
with zero mismatches and every retained result below. No compiler or QEMU
process remained at inspection. Earlier pending/focused notes are historical.

- Immutable manifest and runner: `audit-artifacts/compiler-pie-20261015/`.
- Full QA transcript/results: `attempt-20261015-220041-866/` below that path.
- Three fresh signed-status/placement runs: 1 CPU `1792094442058959300`,
  2 CPUs `1792094492270433100`, 4 CPUs `1792094573402180300`, under their
  `audit-artifacts/stage11.2-status-*cpu-*` directories. Original thread,
  signed-status/authority/resource and fixed/nonzero-PIE markers pass.
- Three fresh compiler shared-PIE runs: 1 CPU `1792094635152162500`,
  2 CPUs `1792094689206155400`, 4 CPUs `1792094738901937300`, under their
  `audit-artifacts/stage11.5-compiler-*cpu-*` directories. Each executes the
  unpatched signed compiler ELF at zero and `0x30000` bias, returns 42, restores
  exact resources and then passes every original Stage 11.5 assertion.
- Build, fixture release Clippy, host all-targets Clippy, production kernel
  Clippy and all sixteen feature lints complete with warnings denied.
  Root host tests: 162 passed; libwoven: nineteen; Python: twenty.
- Exact full launcher: `audit-artifacts/acceptance-20261015-221058-179/`.
  All sixty logs in `stage7-6-artifacts/20261015-221104-415/` contain the
  success marker: 1 CPU x10, 2 CPUs x30 and 4 CPUs x20. Live network and
  isolated async block, VFS, UDP and TCP preservation pass on all three CPUs.
- Twenty-one existing Stage 10.8, 10.9 and 11.1â€“11.5 runtime boots pass on
  1/2/4 CPUs, each QEMU exit 33. All six focused boots also exit 33.

This accepts explicit inert-debug metadata handling and real compiler-produced
signed shared-PIE execution. It does not qualify compiler TLS, general dynamic
linking, full Stage 11, the combined Claude/Codex tree, a new release or hardware.

## Defect and explicit metadata policy

The retained real Rust/LLD executable contains an inert `DT_DEBUG = 0` entry.
The accepted graph loader rejected the whole program because the parser had no
explicit policy for that tag. The [ELF gABI dynamic-section specification](https://gabi.xinuos.com/elf/08-dynamic.html)
describes DT_DEBUG as debugger metadata, optional for executables, with no
ABI-specified contents. This correction accepts a single zero-valued placeholder
without interpreting, relocating, writing or following a debugger address.
Nonzero values remain Unsupported; duplicate entries are InvalidSegment.
This does not implement or promise a debugger rendezvous ABI.

Baseline evidence was captured before changing production parsing:

- `audit-artifacts/compiler-pie-20261015/host-before.txt`: the new positive
  null-debug test fails, native exit 101; nineteen other ELF tests pass.
- `audit-artifacts/stage11.5-compiler-1cpu-1792093997660030600/`: the actual
  unmodified signed executable fails to load; QEMU exits 35. The harness
  rejects the missing compiler and inherited loader markers. Build, command,
  compiler, source/output hashes, ELF reports and serial output are retained.

After the explicit parser branch, all twenty ELF host tests pass, including
nonzero/duplicate rejection and existing malformed/mutation/placement/scope
tests. The first real 1-CPU signed execution also passes, QEMU exit 33, at
`stage11.5-compiler-1cpu-1792094111691517900/`.

## Durable execution gate and invariants

The new `stage11-compiler-test` feature deliberately includes Stage 11.5. Its
separately guarded block runs first, then the unchanged loader gate must pass
before boot exits. The harness stage `11.5-compiler` builds exact committed
Rust fixture sources with warnings denied using the existing provenance
builder. Mandatory include paths have no fallback, and neither ELF is patched
after linking. TLS fixtures are also compiled by that builder but are not
executed or certified by this shared-PIE increment.

The test signs both payloads with the existing test-only RFC key. It installs
only its owned read-only `/lib/libwoven-demo.so`, verifies the main WAPP through
the normal signed loader, executes the real imported function in Ring 3 and
requires Exit(42). An expanded run also verifies the same signed payload and
offers the production graph loader a nonzero `0x30000` bias, checking the
relocated entry and actual shared call. Each execution must restore exact
frame, process, endpoint and owner-thread baselines. Owned VFS fixtures and
any test-created `/lib` directory are cleaned on both ordinary success and
load failure; unrelated paths are never removed.

Every old Stage 11.5 signature/path/SONAME, cross-image relocation,
W^X/RELRO/fork and rollback assertion remains mandatory. The host harness
regression requires their complete marker set plus both compiler execution
markers, and rejects every individually omitted marker even for QEMU exit 33.
The Python suite now has twenty tests.

Production changes are confined to explicit parsing of inert metadata: no
new unsafe code, object, pointer lifetime, capability, lock order, scheduler
transition or relocation operation is introduced. The accepted graph's copied
signature-verified library ownership, bounds, BFS scope, mapping permissions
and transactional rollback remain unchanged.

## Acceptance boundary and next requirements

The expanded focused matrix passed, QEMU exit 33 on all three CPU counts:
1 CPU `stage11.5-compiler-1cpu-1792094192264200000/`, 2 CPUs
`stage11.5-compiler-2cpu-1792094242727185500/`, 4 CPUs
`stage11.5-compiler-4cpu-1792094296734632600/`, under `audit-artifacts/`.
Session 66821 exited 0 with its focused-matrix pass marker. The full chain
will repeat these checks on the final formatting-cleaned frozen inputs.
At that checkpoint these were focused results, not acceptance. Full host/kernel/sixteen-feature warning-denying
lint/tests, signed-status/placement preservation, exact Stage 10.7 and Stage
10.8â€“11.5 preservation were required; the full decision above now supplies them.

Compiler-compatible TLS/TCB and fresh child initialization, virtual-address
congruence, dynamic-library TLS, general REL/PLT tables, symbol/version/weak
semantics, constructors/destructors and runtime loading remain open. See the
compiler-TLS follow-up audit. No combined-tree, release, physical-hardware or
Stage 1â€“19 production qualification is claimed.
---

## Source: docs/audit-stage11-compiler-tls-followup-2026-10-15.md

# Compiler TLS follow-up â€” source review, not acceptance

**Later bounded acceptance (2026-10-16):** session 37505 passed full
preservation on 515 unchanged inputs. Main-image compiler local-exec TLS,
fresh immutable-template initialization and tested lifecycle are now accepted;
see the implementation audit for evidence and exact limits. This document
retains the earlier review, not a current blanket TLS rejection. Broader TLS,
pure-BSS runtime and production ASLR geometry remain open.

**Prerequisite update:** compiler shared-PIE/null-DT_DEBUG correction source
`da6d16e6` is accepted after session 73596 exited 0 with full preservation and
all 505 inputs unchanged. See the compiler-PIE audit. That resolves the separate
debug-metadata prerequisite mentioned historically below, not any TLS contract.

This review was performed while executable-placement session 62865 tests a
frozen source tree. No compiler fixture or kernel source was edited during
that gate. Main-image compiler TLS, library TLS and full Stage 11 remain open.

## Fixture prerequisite

`tools/loader-fixtures/tls-threads.rs` currently calls ThreadCreate (130) with
an 8192-byte stack. `task::create_user_thread` rounds that to 8192 and rejects
anything below `MIN_STACK = 16 * 1024`. Its stack request therefore cannot
exercise fresh child TLS even after the loader layout is corrected. The
fixture must request a supported stack size, retaining its initialization,
zero-fill, alignment, private-write, sequential-join and parent-state checks.
The kernel minimum must not be reduced to accommodate the test. This is a
source-level defect diagnosis, not a new QEMU result.

## Runtime contracts that must be proved

The current loader page-rounds PT_TLS memory size, copies initialized bytes at
the allocation start, and sets FS to the allocation end next to an unmapped
stack guard. The actual compiler fixture reads FS:0 as a self pointer and then
uses negative offsets. It needs a mapped TCB, correctly aligned template and
zero-fill layout, and checked arithmetic across small, aligned and page-edge
cases. Existing handwritten probes do not prove that compiler contract.

`clone_initial_tls` reads the live main-thread allocation. New threads instead
need the immutable original initialized bytes plus fresh zero-fill, their own
TCB/self pointer and private writable storage. Merely copying the live TCB or
changing the returned FS value would not fix fresh initialization. The task
creation path currently independently derives FS from the mapping end; it
must use the same validated layout as the loader.

Address-space destruction, partial loader/thread rollback and fork cloning
must account for every new template/TCB allocation. Fork preserves the calling
thread's state at the same virtual addresses; fresh-thread creation must not
inherit that mutated state. Audit owner exit, failed publication, cancellation,
join and capacity exhaustion, preserving lock order and exact resource counts.
The existing address-space metadata is Copy and intentionally compact after a
previous kernel-stack overflow; avoid introducing unbounded ownership or large
inline template buffers into those copies.

After the executable-placement prerequisite passes full acceptance, integrate
the unmodified compiler-produced fixtures, preserve their baseline failures,
correct the TLS contract and run actual Ring-3 tests on 1/2/4 CPUs plus all
required inherited gates. The separate null DT_DEBUG rejection in the shared
PIE also needs an explicit parser policy and signed execution proof; neither
that fix nor dynamic-library TLS is certified by a static TLS fixture.

## Primary ABI references checked during the frozen gate

The [x86-64 psABI thread-pointer clarification](https://gitlab.com/x86-psABIs/x86-64-ABI/-/merge_requests/14)
specifies FS as the thread-pointer segment and a linear self pointer at offset
zero. This independently supports the disassembly finding above.

Drepper's [ELF TLS specification](https://www.akkadia.org/drepper/tls.pdf),
section 2 and table 3, describes a retained initialization image and a fresh
copy for each thread, with PT_TLS file size, total memory size and alignment.
Section 4.4.6 describes x86-64 local-exec access through FS:0 and a linked
thread-pointer-relative offset. These are implementation requirements, not
evidence that WovenHat currently meets the contract. The integration must
preserve the initialized template separately from mutable per-thread storage.

LLVM's [LLD TLS offset implementation](https://raw.githubusercontent.com/llvm/llvm-project/main/lld/ELF/InputSection.cpp)
(`getTlsTpOffset`, x86-64 Variant 2) also accounts for PT_TLS virtual-address
congruence, not merely rounding memory size. With aligned TP, padding must
make `TP - memory_size - padding` congruent to `p_vaddr` modulo `p_align`.
The current `TlsImage` metadata retains alignment and sizes but discards
PT_TLS `p_vaddr`; simply rounding memory size would miss nonzero congruence.
Preserve the necessary validated congruence and use checked allocation/layout
arithmetic. Require both zero- and nonzero-congruence host and compiler/runtime
cases, including a template spanning a page boundary. The upstream source is
design evidence, not a claim about the exact bundled LLD version; compare its
actual linked offsets when integrating the retained toolchain fixtures.

## Concrete source/lifetime integration review after placement acceptance

This additional review runs while shared-PIE session 73596 freezes the kernel;
it changes no tested source and is not TLS implementation or acceptance.

`AddressSpace` currently stores only one TLS `(start, size)` range. A fresh
immutable initialization image needs distinct lifetime accounting from the
writable main-thread TLS/TCB. A candidate compatible with compact Copy metadata
is a process-owned RO/NX template mapping plus explicit scalar layout metadata,
not a large inline kernel buffer or an unowned borrowed ELF slice. This design
still needs implementation review and tests; it is not an allocated object yet.

Audit these exact paths together:

- `linker::Pending::drop`: record only successfully mapped ranges, including
  a template if one is added, and unwind every partially initialized layout.
- `linker::load_with_candidate_bias`: validate the template's source and size,
  retain initialization independently of mutable TLS, construct a mapped self
  pointer, and preserve image/mmap/stack-guard disjointness and RW/NX policy.
- `clone_initial_tls` and `task::create_user_thread`: initialize fresh private
  bytes from the retained template and use the validated thread-pointer offset;
  do not continue deriving FS as the end of a page-rounded anonymous mapping.
- `destroy`, `clone_address_space` and its rollback journal: account for the
  template separately, sharing it only read-only and reclaiming it on all
  failure/owner-exit paths. Their fixed range-array capacities must include
  every additional range; do not overwrite or omit an entry at maximum load.
- `AddressSpace::is_logically_writable`: include mutable TLS/TCB but exclude
  the template even after fork/COW. New thread mappings retain existing slot
  ownership, NX permissions, cancellation, join and teardown behavior.

The retained `tls-threads-demo` report has PT_TLS file size 128, memory size
136, alignment 64 and zero virtual-address congruence. Its static TLS span
before TP must therefore be 192 bytes, not a page-rounded 4096. A template
whose memory size is exactly a page additionally needs mapped storage for the
TCB beyond that span; leaving TP at the following guard page is invalid.
Nonzero-congruence and cross-page cases remain required independent tests.

The mmap implementation has an existing 64 KiB stride/maximum, and anonymous
mapping metadata tracks sixteen resident pages. Any proposed TLS layout must
explicitly check this capacity rather than overflowing a slot or silently
changing the mmap ABI. If broader TLS allocation is needed, design and qualify
that allocator as well; do not claim general TLS from fixtures that fit.

Finally, initialized TLS bytes can contain dynamic relocations. Validate their
relationship to the loaded ELF image and capture the correct post-relocation
initialization state, rather than assuming a raw pre-relocation file copy is
sufficient. The existing synthetic parser-only TLS header is not proof of a
mapped compiler template. Preserve its bounds tests and add real initializer,
fresh-thread, fork/COW, malformed-header, capacity and rollback evidence.
---

## Source: docs/audit-stage11-compiler-tls-implementation-2026-10-15.md

# Compiler static TLS/TCB implementation â€” bounded acceptance 2026-10-16

## Acceptance decision

Session 37505 exited 0 with `COMPILER TLS FULL ACCEPTANCE: PASS` at
2026-10-16 00:17:15 local. Sources `a33dea86`, `e58b803b` and `c366cfaa`
are accepted for bounded main-image compiler local-exec TLS, not general TLS
or full-stage production readiness. Independent inspection verified all 515
frozen inputs unchanged, nine focused boots, 28 TLS execution cases across
every online CPU in 1/2/4-CPU configurations, twenty-one inherited runtime
boots, seventeen feature lints and sixty individual stress logs.

Build, warning-denying host/production/affected-feature and fixture Clippy,
168 root host Rust tests, nineteen libwoven tests and twenty-one Python tests
passed. The exact full Stage 10.7 launcher passed, followed by Stage
10.8â€“11.5 on 1/2/4 CPUs. Existing assertions and capacities were preserved.

The immutable manifest/runner are in
`audit-artifacts/compiler-tls-20261015/revision2/`; final QA is in
`attempt-20261015-233729-275/` beneath it. Exact inherited launcher evidence:
`audit-artifacts/acceptance-20261015-234437-300/`; sixty stress logs:
`stage7-6-artifacts/20261015-234440-216/`. The first failed full attempt and
its separate original manifest remain historical evidence, not a passing gate.

This qualifies initialized/mixed-zero main-image local-exec TLS, relocated PIE
initializer pointers, fresh threads, fork state, cancellation, capacity, stale
joins, owner exit, rollback and exact resource reclamation. Pure-BSS TLS has
parser/layout coverage but no dedicated all-BSS runtime compiler fixture.
Initial-exec relocation models, library/dynamic TLS, general dynamic linking,
production ASLR geometry, combined-tree, release and physical qualification
remain open. Earlier pending statements below describe historical checkpoints;
this decision supersedes them only for the stated bounded contract.

## Preserved failure and initial correction

The real unmodified static Rust TLS fixture faulted reading FS:0 at
`0x4000001ef000`, the unmapped stack guard. RIP was `0x4000000011d7`, process
status -11, QEMU exit 35, and frame counts returned to 3488/3488. Evidence:
`audit-artifacts/stage11.2-tls-1cpu-1792097890108521900/`; the full captured
baseline is also in `audit-artifacts/compiler-tls-20261015/qemu-before.txt`.
The first CLI-edit indentation error is separately retained as a harness
setup failure; no QEMU baseline ran until all twenty-one Python tests passed.

The TLS-thread compiler fixture now requests 16 KiB, the kernel's existing
minimum, instead of its invalid 8 KiB request. No kernel bound was relaxed.

The production changes add a checked x86-64 Variant-II layout: preserve PT_TLS
virtual-address congruence, align TP, put initialized/zero-filled bytes before
TP at their linked offset, and map the eight-byte TP self pointer. A page-sized
TLS span needs another mapped page for that TCB. Layouts exceeding the existing
64 KiB anonymous-slot capacity fail closed; the mmap ABI and page bitmap stay
unchanged.

PT_TLS initializer bytes must match the exact file-backed PT_LOAD source.
Their source virtual address follows image relocation. The loader copies the
post-relocation bytes into a separate process-owned RO/NX template and uses
that immutable template for both main-thread and fresh child initialization.
Each child gets private RW/NX storage and a new self pointer; mutable parent
values are not inherited. There is no new unsafe code, lock, syscall, capability
or asynchronous user-pointer lifetime.

## Lifetime and rollback review

`InitialTls` is compact Copy metadata, not an inline template buffer or a
borrowed ELF lifetime. The loader records a successfully mapped template in
its Pending guard before copying/protecting it, and records mutable TLS only
after its mapping succeeds. Either range is included on rollback even if the
subsequent operation fails. Normal destruction includes both ranges. Fixed
range arrays/journals have one additional entry for the template.

Fork shares the immutable template RO/NX and COW-shares writable TLS at the
same virtual addresses, preserving the calling thread's pointer/state. Fresh
thread creation instead initializes from the template and returns the validated
new pointer to the scheduler. Failed child initialization unmaps its private
range before returning. Existing join/cancel/owner teardown owns the same
anonymous mapping records; allocator bounds, lock order and scheduler blocking
are unchanged. `is_logically_writable` includes mutable TLS but not the template.

The previously accepted user-ancestor/COW policy remains essential when a TLS
clone is destroyed and the remaining main allocation faults on a later write.
Do not treat a leaf-only mapping query as proof of CPU writability.

## Evidence so far â€” not full acceptance

- Twenty-three actual-parser ELF tests pass, including backing mismatch,
  retained congruence/relocated source, malformed bounds/alignment and duplicate
  TLS headers; prior scope/debug/placement/mutation tests remain unchanged.
- Three pure-layout tests pass, checking every congruence for each supported
  power-of-two alignment through 4096 and several size/page-edge cases, plus
  overflow, capacity and real compiler layouts.
- Warning-denying all-target host Clippy, production-kernel Clippy and the
  dedicated TLS feature Clippy pass. The complete affected-feature matrix is
  still required; focused lint does not substitute for it.
- Initial real static/fresh-child execution passed at
  `stage11.2-tls-1cpu-1792098127245619500/`.
- Expanded 1-CPU execution passed at
  `stage11.2-tls-1cpu-1792098402326047700/`: real compiler initialized/aligned/
  zero-fill/private-child state, independent CPU probes for nonzero congruence
  and a page-edge TCB, RO/NX template and RW/NX TLS/guard assertions, fork
  mapping/self-pointer preservation, late argv-failure rollback, exact frame/
  process/endpoint/thread baselines and the unchanged original thread gate.
- The formatting-cleaned focused matrix passed on 1/2/4 CPUs in session 43180,
  exit 0, with `COMPILER TLS FOCUSED MATRIX: PASS`:
  `stage11.2-tls-1cpu-1792098483487762400/`,
  `stage11.2-tls-2cpu-1792098524417062400/`,
  `stage11.2-tls-4cpu-1792098567320766300/`. These are focused, not acceptance.

At that checkpoint, further compiler lifecycle and relocated-initializer
coverage was required. The expansion below now implements those probes.
Before acceptance, freeze the final inputs and run
all host/build/warning-denying affected-feature checks, prior signed-status and
compiler shared-PIE matrices, exact full Stage 10.7 and Stage 10.8â€“11.5 gates.
Retain failed and final serial evidence and commit acceptance separately.

Dynamic-library TLS, general dynamic-linker interoperability and full Stage 11
remain open. Neither these focused tests nor source integration qualify the
combined tree, a release, physical hardware, or Stage 1â€“19 production readiness.

## Expanded compiler lifecycle and every-CPU gate

The first full attempt, session 77593, stopped with exit 1 at TLS-feature
Clippy: its constant-size `chunks_exact(24)` test helper must use `as_chunks`.
Nine focused boots, 168 host Rust tests, nineteen libwoven tests, twenty-one
Python tests, host/production lint and preceding feature lints passed, but
full inherited runtime preservation had not started. Evidence and the original
515-input freeze remain in
`audit-artifacts/compiler-tls-20261015/attempt-20261015-232423-500/` and its
parent. The helper is corrected without a lint suppression, assertion change,
capability expansion or timeout increase. A complete fresh chain is required.

The compiler TLS-thread fixture is now an unmodified LLD PIE, still with
compiler-generated local-exec access. A volatile TLS pointer to a separate
global adds an actual R_X86_64_RELATIVE relocation inside the PT_TLS initializer.
The gate requires the PIE kind, a relative relocation within that initializer,
and a nonzero `0x30000` load bias before accepting real pointer equality and
dereference. The first passing bundle at
`stage11.2-tls-1cpu-1792098947990990300/` records PT_TLS virtual start
`0x400000005080` and a relative relocation targeting that exact address. This
exercises post-relocation template capture rather than raw-file initialization.

The same compiled program checks mutable parent TLS through fork, independent
child changes and a freshly initialized descendant thread. It then joins two
fresh threads, rejects stale joins, fills all eight available stack/TLS pairs
in the established sixteen-slot arena, verifies the ninth creation fails,
cancels each initialized sleeping worker with signed status -2 and reaps each
exactly once. Parent state stays unchanged. Finally it exits with an initialized
sleeping sibling deliberately unjoined; the kernel wait and exact resource
baselines prove owner cleanup rather than treating a user report as cleanup.
Readiness waits use the existing timer sleep, not spinning. No timeout or
capacity is increased.

The first strengthened compiler lifecycle passed on 1 CPU. A subsequent gate
runs all four execution cases separately on every online CPU through the
existing production multicore spawn API, with one allowed CPU at a time and
ordinary User capabilities. It requires an individual lifecycle marker for
every CPU. Thus 2/4-CPU boots exercise real AP FS switching, fork/thread calls
and remote retirement, not merely a BSP-only program under an SMP kernel.
The all-CPU matrix passed in session 4739, exit 0, with its explicit focused
pass marker: 1 CPU `stage11.2-tls-1cpu-1792099168828370800/`, 2 CPUs
`stage11.2-tls-2cpu-1792099208530646200/`, 4 CPUs
`stage11.2-tls-4cpu-1792099254231710700/`. Every individual online-CPU marker
and all earlier assertions are present. Full acceptance is still pending.

## Separate inherited production-ASLR geometry finding

Read-only source review found that stack and mmap draws are independent. With
the current constants, the lowest stack base is `USER_REGION_START + 0x1b1000`
and its guard starts at `+0x1b0000`. The highest mmap arena ends at
`USER_REGION_START + 0x22f000` (16 slots x64 KiB after base `+0x12f000`).
These advertised ranges can overlap. QEMU features deliberately draw zero,
so ordinary green matrices do not qualify randomized arena geometry.

The TLS loader rejects a reservation crossing its mmap bound, which is safe
failure rather than a layout fix. Do not disable ASLR or reduce slot capacity
to make a test pass. A separate validated address-space planner and explicit
nonzero/extreme-draw production-layout gates are required before ASLR/stage
closure. No planner change is hidden in this TLS increment.
---

## Source: docs/audit-stage11-dynamic-relocations-2026-10-14.md

# Stage 11 dynamic relocation audit â€” 2026-10-14

## Scope

The ELF loader now accepts a deliberately narrow ET_DYN/PT_DYNAMIC contract.
It validates the dynamic table and allows bounded RELA entries whose targets
lie in non-executable writable load segments. Relative entries and locally
defined symbol entries (`R_X86_64_64`, `R_X86_64_GLOB_DAT`, and
`R_X86_64_JUMP_SLOT`) are applied after all image bytes are copied and before
segment permissions and RELRO are finalized. SysV hash metadata provides a
bounded symbol count; section, string-table, image-range, and signed-addend
checks prevent the loader from guessing at an invalid symbol scope. The load
bias is carried through ASLR, so values and targets are relocated together.

The loader rejects PT_INTERP, DT_NEEDED, undefined symbols, legacy REL tables,
PLT relocation tables, text-relocation requests, duplicate dynamic headers,
malformed/truncated tables, unsupported dynamic tags, and relocation targets
outside writable data. Shared-library dependency mapping, cross-image symbol
resolution, versioned symbols, lazy binding, and dynamic TLS remain
unsupported and fail closed.

## Validation

- Loader self-tests cover a valid relative-relocation table and malformed
  dynamic/TLS/RELRO cases.
- Kernel and host warning-denying Clippy pass.
- Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs:
  - audit-artifacts/stage11.5-1cpu-1791973749450962200
  - audit-artifacts/stage11.5-2cpu-1791973828064459400
  - audit-artifacts/stage11.5-4cpu-1791973879025768100

The local-symbol follow-up reran the inherited Stage 11.3â€“11.5 matrix on the
current source and retained:

```text
audit-artifacts/stage11.3-1cpu-1791977053721810200
audit-artifacts/stage11.3-2cpu-1791977120029898500
audit-artifacts/stage11.3-4cpu-1791977170903077000
audit-artifacts/stage11.4-1cpu-1791977221235626900
audit-artifacts/stage11.4-2cpu-1791977286654426900
audit-artifacts/stage11.4-4cpu-1791977331645912200
audit-artifacts/stage11.5-1cpu-1791977377816478600
audit-artifacts/stage11.5-2cpu-1791977444546806600
audit-artifacts/stage11.5-4cpu-1791977498582992000
```

This is a production safety boundary for self-contained PIE images, not a
claim of a complete dynamic linker or shared-library runtime.
---

## Source: docs/audit-stage11-executable-placement-2026-10-15.md

# Executable placement in the signed graph loader â€” accepted bounded correction

## Full acceptance decision

Source `a8acbaaf` passed the complete serialized chain. Session 62865 exited
**0**, with `EXECUTABLE PLACEMENT FULL ACCEPTANCE: PASS`, at 2026-10-15
21:48:20 local. Independent post-run inspection confirmed every result below
and all 500 frozen source/fixture hashes, with zero mismatches. No competing
compiler or QEMU process remained at inspection. Earlier failed and focused
evidence below is retained historical context, not the acceptance decision.

- Runner and immutable manifest: `audit-artifacts/elf-placement-20261015/`.
- Complete QA: `attempt-20261015-211007-631/` under that directory.
- Fresh placement/status fixture: 1 CPU `1792091407873704600`, 2 CPUs
  `1792091444732574900`, 4 CPUs `1792091486188735200`, each under the
  corresponding `audit-artifacts/stage11.2-status-*cpu-*` directory.
  Both placement markers, signed-status/ownership/resource assertions and
  original thread markers passed, QEMU exit 33.
- Build, fixture release Clippy, host all-targets Clippy, production kernel
  Clippy and fifteen affected feature configurations completed; all lint
  invocations deny warnings. Root host tests: 159 passed, including seventeen
  ELF tests; libwoven: nineteen passed; Python: nineteen passed.
- Exact `RUN-STAGE10.7.ps1`: `audit-artifacts/acceptance-20261015-211629-382/`.
  All sixty individual stress logs in
  `stage7-6-artifacts/20261015-211632-692/` contain their success marker:
  1 CPU x10, 2 CPUs x30, 4 CPUs x20. Live network and isolated async block,
  VFS, UDP and TCP gates passed on all three CPU counts.
- All twenty-one Stage 10.8, 10.9, 11.1, 11.2, 11.3, 11.4 and 11.5 runtime
  results passed on 1/2/4 CPUs, QEMU exit 33. Their result and serial logs are
  retained alongside the full transcript.

This accepts the fixed-ET_EXEC/nonzero-PIE placement correction in the signed
graph loader. It does not close compiler TLS, general dynamic linking, full
Stage 11, combined-tree integration, a new release or hardware qualification.
The next compiler-runtime prerequisites are detailed in
`audit-stage11-compiler-tls-followup-2026-10-15.md`.

## Finding and peer integration

Claude's `6f02202d` correctly stopped sliding ET_EXEC images in the old loader.
The accepted signed graph loader replaced that loader body and still applied
`aslr_image_delta()` unconditionally to the main image. This increment adapts
Claude's ELF-kind distinction to the graph implementation; it does not restore
the old dependency rejection or merge unqualified later-stage source.

Ordinary QEMU gates draw zero image bias and therefore could not expose this
failure. A test-only caller now offers the **same production graph loader** a
nonzero `0x30000` candidate without changing other boot-probe placements.
The actual compiled libwoven fixture loaded at `0x400000030000` instead of its
linked `0x400000000000`, then instruction-fetch faulted at absolute address
`0x400000000151`. QEMU exited 35 and the process exited -11; exact resource
baselines were restored but execution failed. Evidence:
`audit-artifacts/stage11.2-status-1cpu-1792090711011358100/`.
The initial harness compile error from an inappropriate `expect` on the
non-Debug ELF error type is also retained in its separate build log; an explicit
parse-failure branch corrected that harness defect without changing the parser
error type, suppressing a warning or weakening an assertion.

## Integrated policy and tests

The validated ELF header records ET_DYN as position-independent.
`Image::relocated_for_execution` ignores the candidate for ET_EXEC and retains
it for ET_DYN. The graph main-image preparation uses this common policy;
shared-object placement still uses the existing bounded library windows and
generic relocation path after `parse_shared` enforces ET_DYN. W^X/RELRO,
signatures, copied input ownership, dependency bounds/order, capability sets,
stack/mmap randomness, rollback and teardown are unchanged. No unsafe code,
kernel object, lock, syscall or asynchronous pointer lifetime was added.

Two new parser-backed host cases bring the ELF suite to seventeen tests:
fixed static executables remain at their linked address even for a maximal
candidate; PIE entry/segment/bias fields move by a nonzero candidate. Static
ET_EXEC remains forbidden as a shared object. Existing scope/relocation,
malformed-input and mutation tests still pass.

The real static libwoven fixture now executes with the nonzero candidate while
retaining its linked entry and all prior status/ownership/resource assertions.
A separate position-independent scalar Exit(37) probe must execute at
`0x400000030000`, retaining PIE ASLR rather than disabling it globally. Both
markers and the original thread gate are mandatory in the runtime harness;
host harness tests explicitly require those markers and reject their omission.
Nineteen Python tests pass. The revised focused matrix passed, QEMU exit 33:

- `stage11.2-status-1cpu-1792090906487855600/`
- `stage11.2-status-2cpu-1792090950173088300/`
- `stage11.2-status-4cpu-1792090989123605700/`

At this earlier checkpoint these were focused results, **not acceptance**. Full warning-denying host,
fixture, production and affected-feature checks, all host tests, exact full
Stage 10.7 and Stage 10.8â€“11.5 1/2/4-CPU preservation were still required;
the subsequent full acceptance decision above supplies that evidence.
Compiler-produced shared-PIE interoperability, null DT_DEBUG handling,
compiler TLS/TCB and fresh child templates, library TLS and broader dynamic
linking remain separate open requirements. The minimal PIE probe does not
qualify those contracts. Neither a new release nor combined-tree/hardware
production readiness is claimed.
---

## Source: docs/audit-stage11-lazy-binding-2026-10-20.md

# Stage 11 lazy PLT binding audit â€” 2026-10-20

## Scope

This increment defers writable PLT `R_X86_64_JUMP_SLOT` binding until the
first Ring-3 call. It does not claim library TLS or full Stage 11 closure.

## Implementation

- The loader retains validated slot/target pairs in an eight-entry bounded
  process table and emits per-slot RX stubs in the loader-owned resolver page.
- Each stub supplies its GOT slot in `R11`, invokes the internal lazy-bind
  syscall, and jumps to the returned target.
- The kernel accepts only slots registered in the current process address
  space, requires an executable target, and patches through the active-user
  copy path so forked address spaces break COW before mutation.
- PLT slots covered by RELRO remain eager; no writable alias is created to
  bypass RELRO. Exceeding the bound fails closed.

## Validation

Host ELF scope tests: 33 passed. Warning-denying freestanding Stage 11.5 and
host all-target Clippy passed, as did the 11 Python harness tests. Stage 11.5
QEMU acceptance passed on 1/2/4 CPUs. The dedicated fixture verifies the
writable PLT slot contains the resolver stub before first call and that the
Ring-3 call resolves, executes, and tears down with status 42.

Evidence: `audit-artifacts/stage11.5-1cpu-1792508068498107100`,
`stage11.5-2cpu-1792508101002664400` and
`stage11.5-4cpu-1792508116869852100`.

## Limit and next prerequisite

The process-local lazy table is bounded to eight slots to preserve the
kernel-stack copy budget. RELRO PLT slots are intentionally eager. Library TLS
is the next unresolved Stage 11 prerequisite.
---

## Source: docs/audit-stage11-library-tls-2026-10-20.md

# Stage 11 shared-library TLS implementation audit (2026-10-20)

## Scope

The bounded ELF loader now plans one combined TLS block across the main image
and shared libraries, copies immutable PT_TLS initializers into a read-only
template, creates a fresh mutable TLS/TCB per process, and resolves the
toolchain's shared-object TLS relocation forms. The exact Rust/LLD fixture
contains a shared object with PT_TLS, a TLS initializer, an exported function,
and a TPOFF64 relocation.

## Validation

- `cargo test --workspace`: passed.
- Freestanding Stage 11.5 and host warning-denying Clippy: passed.
- Stage 11.5 QEMU acceptance passed on 1, 2, and 4 CPUs. Evidence:
  `audit-artifacts/stage11.5-1cpu-1792510954965696000`,
  `audit-artifacts/stage11.5-2cpu-1792510968884579900`,
  `audit-artifacts/stage11.5-4cpu-1792510982932216800`.
- The exact compiler-fixture gate passed on 1, 2, and 4 CPUs. Evidence:
  `audit-artifacts/stage11.5-compiler-1cpu-1792510865272847800`,
  `audit-artifacts/stage11.5-compiler-2cpu-1792510895456748200`,
  `audit-artifacts/stage11.5-compiler-4cpu-1792510921851793300`.
- `cargo test --workspace`, host Clippy, and freestanding compiler-feature
  Clippy with the generated fixture paths all passed.

The compiler gate initially exposed a fixed-capacity kernel stack defect and
an incorrect ELF TLS-symbol discriminator. The stack reservation is now 256
KiB, and TLS symbols are recognized by `STT_TLS` with a non-undefined section
index. No assertion or timeout was weakened.

## Acceptance result

Stage 11 shared-library TLS is accepted for the current roadmap gate. Broader
physical-hardware qualification and later production-release requirements
remain governed by their later roadmap stages.
---

## Source: docs/audit-stage11-lifecycle-2026-10-20.md

# Stage 11 lifecycle callback audit â€” 2026-10-20

## Scope

This increment implements bounded ELF lifecycle callbacks for dynamically
linked user images: `DT_INIT`, `DT_FINI`, `DT_INIT_ARRAY`, and `DT_FINI_ARRAY`.
It does not claim lazy binding, library TLS, or full Stage 11.5 closure.

## Implementation

- ELF parsing validates callback addresses against executable load segments,
  rejects malformed array byte sizes, and bounds callback counts.
- The graph linker schedules constructors in dependency order and destructors
  in reverse order.
- A tracked RX trampoline re-enters the normal exit syscall with the original
  status in callee-saved `R12`.
- The loader prebuilds the bounded destructor return chain in a reserved
  writable stack region. Exit handling validates the syscall origin and stack
  range, snapshots paging state before lock-sensitive work, and rechecks the
  process owner before marking teardown pending.

## Validation

Host ELF scope tests: 33 passed. Stage 11.5 QEMU on 1/2/4 CPUs passed the complete
prior loader suite plus:

- constructors scheduled before Ring-3 entry;
- Ring-3 constructor execution;
- Ring-3 destructor execution and teardown;
- unresolved-import and argv rollback after lifecycle execution.

Evidence: `audit-artifacts/stage11.5-1cpu-1792507060371452000`,
`stage11.5-2cpu-1792507334555453100` and
`stage11.5-4cpu-1792507351184297200`.

## Limits and next gate

The callback bound is four functions per image, with the existing four-image
scope bound. Lazy binding and library TLS remain the next Stage 11
implementation items.
---

## Source: docs/audit-stage11-loader-boundary-2026-10-05.md

# Stage 11 loader boundary audit - 2026-10-05

This pass closes a production-safety hole in the Stage 11 loader boundary without
claiming full dynamic linking support.

The ELF parser now rejects `PT_INTERP`, `PT_DYNAMIC`, `PT_TLS`, and
`PT_GNU_RELRO` program headers with `Unsupported` instead of silently ignoring
them. A boot self-test mutates a known-good executable to include each header and
requires all four variants to fail closed.

This preserves the existing static executable ABI and W^X checks. It does not
implement shared libraries, dynamic relocations, TLS image allocation, or RELRO
page-protection transitions; those remain Stage 11 production work. The security
improvement is that binaries requiring those semantics can no longer be accepted
as if they were correctly loaded.

Validation:

- `cargo build`
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings`
- `cargo test`

The broader 1/2/4-CPU QEMU acceptance gate must pass before this audit entry is
treated as accepted source.
---

## Source: docs/audit-stage11-needed-metadata-2026-10-14.md

# Stage 11 dynamic dependency metadata audit â€” 2026-10-14

## Scope

This increment hardens the ELF dynamic metadata boundary before implementing
shared-library mapping. The parser now retains at most four `DT_NEEDED` string
offsets, checks every offset against the validated string table, and rejects
an over-capacity dependency list. The current loader explicitly rejects an
image with dependencies because it does not yet provide dependency namespace,
cross-image symbol scope, dynamic TLS, or shared-image teardown.

This is intentionally fail-closed. It does not claim shared-library support.

## Validation

- Freestanding kernel Clippy for `stage11-5-test`, `-D warnings`: passed.
- Host all-targets Clippy, `-D warnings`: passed.
- Complete `wovenhat-os` host test suite: passed.
- Stage 11.3, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.3-1cpu-1791979019677103300`
  - `audit-artifacts/stage11.3-2cpu-1791979074468240000`
  - `audit-artifacts/stage11.3-4cpu-1791979113345985100`
- Stage 11.4, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.4-1cpu-1791979244427396000`
  - `audit-artifacts/stage11.4-2cpu-1791979348256155100`
  - `audit-artifacts/stage11.4-4cpu-1791979401924884100`
- Stage 11.5, 1/2/4 CPUs: passed, exit 33.
  - `audit-artifacts/stage11.5-1cpu-1791978975094941300`
  - `audit-artifacts/stage11.5-2cpu-1791979454404979000`
  - `audit-artifacts/stage11.5-4cpu-1791979520620604600`

One earlier Stage 11.4 1-CPU attempt timed out during early boot before the
acceptance markers; its artifact remains at
`audit-artifacts/stage11.4-1cpu-1791979155491013700`. The same exact run was
rerun without changing assertions or timeouts and passed at the artifact listed
above.

## Remaining prerequisite

The next implementation slice must add a bounded `/lib` dependency namespace,
separate W^X mappings, exact-name cross-image symbol resolution, and ordered
teardown. Versioned symbols, lazy binding, and dynamic TLS remain unsupported
until separately designed and tested.
---

## Source: docs/audit-stage11-plt-rel-2026-10-20.md

# Stage 11 eager PLT REL increment â€” 2026-10-20

## Scope

The ELF loader now accepts a bounded `DT_JMPREL` table encoded as legacy
`DT_REL`. It validates `DT_PLTRELSZ`, `DT_PLTREL == 17`, the fixed 16-byte
entry format, file-backed placement and writable, non-executable relocation
targets. The loader reads each signed 64-bit addend from the mapped private
target and applies the existing checked relocation and symbol-scope rules
before W^X and RELRO protection.

The Stage 11.5 acceptance fixture signs and loads a DT_JMPREL/DT_REL main
image, verifies the relocated cross-image address, executes the import in
Ring 3, and checks process-owner teardown. The harness requires both runtime
markers.

## Validation

- `cargo test --test elf_scope`: 28 passed, including valid PLT REL metadata
  and bounded target validation.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 11.5 QEMU passed with exit 33 on 1/2/4 CPUs:
  `audit-artifacts/stage11.5-1cpu-1792503297814943600`,
  `stage11.5-2cpu-1792503342880325200` and
  `stage11.5-4cpu-1792503373152819300`.

## Limits

Symbol versioning/weak binding, lazy binding, constructors/destructors,
library TLS and the complete dynamic linker ABI remain open. This is an
accepted bounded Stage 11 increment, not full Stage 11 or production
readiness.
---

## Source: docs/audit-stage11-plt-rela-2026-10-20.md

# Stage 11 eager PLT RELA increment â€” 2026-10-20

## Scope

The ELF loader now accepts a bounded `DT_JMPREL` table when it is explicitly
encoded as `DT_RELA` with `DT_PLTRELSZ`, `DT_PLTREL` and `DT_RELAENT`. The table
must be file-backed by a load segment, contain no more than the existing
relocation bound, and target writable, non-executable mapped storage. Its
entries use the same validated eager relocation set as the ordinary RELA
table, including `R_X86_64_JUMP_SLOT`; no lazy binding, text relocations or
runtime symbol-table mutation was added.

The loader applies ordinary and PLT RELA entries transactionally before final
W^X and RELRO protection. Malformed encoding, missing metadata, out-of-file
tables, unsupported relocation types and read-only targets fail closed.

## Validation

- `cargo test --test elf_scope`: 25 passed, including valid PLT metadata,
  wrong `DT_PLTREL` encoding and read-only target rejection.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python scripts/test-stage10-runtime.py --stage 11.5 --cpus 1`: passed,
  exit 33, evidence under `audit-artifacts/stage11.5-1cpu-1792501620049883600`.
- The same Stage 11.5 gate passed on 2 CPUs and 4 CPUs, exit 33, with evidence
  under `stage11.5-2cpu-1792501673567079100` and
  `stage11.5-4cpu-1792501694886543100`.

## Limits

Legacy `DT_JMPREL`/`DT_REL` is covered by the separate
`audit-stage11-plt-rel-2026-10-20.md` increment. Versioned/weak symbol
semantics, lazy binding, constructors/destructors, library TLS and the
complete dynamic linker ABI remain outside this audit. This is an accepted
bounded increment, not full Stage 11 or production readiness.
---

## Source: docs/audit-stage11-rel-2026-10-20.md

# Stage 11 bounded DT_REL increment â€” 2026-10-20

## Scope

The ELF parser and graph loader now recognize a separate `DT_REL` table with
`DT_RELSZ` and `DT_RELENT`. The table is bounded by the existing relocation
limit, must be file-backed, and every target must be writable and
non-executable. Only the existing eager relocation types are accepted.

Because REL stores its addend at the relocation target, the loader reads the
already-mapped target bytes from the private inactive address space, interprets
the eight-byte value as a signed addend, and writes the checked result before
final W^X/RELRO protection. No userspace pointer is retained and no lazy
binding is introduced.

## Validation

- `cargo test --test elf_scope`: 27 passed, including valid REL metadata,
  wrong entry size and read-only target rejection.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Inherited Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs, exit 33, with
  evidence under `audit-artifacts/stage11.5-1cpu-1792501845221185500`,
  `stage11.5-2cpu-1792501867464427000` and
  `stage11.5-4cpu-1792501878849660600`.

The Stage 11.5 QEMU harness now includes a dedicated signed DT_REL executable
fixture. It verifies the relocated target, executes the imported function in
Ring 3, and checks owner teardown. The required markers passed on 1/2/4 CPUs:

- `[S11.5] shared libraries DT_REL runtime relocation: PASSED`
- `[S11.5] shared libraries DT_REL Ring-3 execution and teardown: PASSED`

Evidence is retained under `audit-artifacts/stage11.5-1cpu-1792502825772958300`,
`stage11.5-2cpu-1792502960229582900` and
`stage11.5-4cpu-1792502998392183900`.

## Limits

PLT RELA is supported by the preceding increment. PLT REL, symbol
versioning/weak binding, constructors/destructors, lazy binding and library
TLS remain open. Full inherited release-chain preservation is also required
before Stage 11 closure.
---

## Source: docs/audit-stage11-relro-2026-10-14.md

# Stage 11 loader RELRO audit â€” 2026-10-14

## Scope

The ELF loader now accepts a single `PT_GNU_RELRO` range only when it is
page-aligned into a non-executable writable load segment. After the image is
copied and before the process is published, the loader changes that range to
read-only. Address-space write-fault bookkeeping carries the same range, so a
fork/COW fault cannot re-enable writes to RELRO pages. Relocation by the
production ASLR delta updates the RELRO range with the load segments.

`PT_INTERP` remains rejected. Self-contained `ET_DYN` images with only bounded
relative `RELA` relocations are now supported by the dynamic-relocation path
documented in the dynamic relocation audit. Static `PT_TLS` is supported by
the separate initial-exec TLS path documented in the
[static TLS audit](audit-stage11-static-tls-2026-10-14.md). Dynamic symbol
resolution, shared-library ownership, TLS module IDs, and symbol-based
relocation sequencing remain outside the loader contract.

## Validation

- Valid and invalid RELRO parser cases are covered by the loader self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings` â€” passed.
- `cargo clippy --all-targets -- -D warnings` â€” passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` â€” passed.
- Release build and `git diff --check` â€” passed.
- Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs, exit 33:
  `audit-artifacts/stage11.5-1cpu-1791971762993756000`,
  `audit-artifacts/stage11.5-2cpu-1791971862705300300`,
  `audit-artifacts/stage11.5-4cpu-1791971961472267000`.

## Limits

The acceptance image uses the existing static ELF fixtures; it does not claim
dynamic linking, shared libraries, or physical hardware enforcement of page
permissions. Those remain explicit follow-up requirements.
---

## Source: docs/audit-stage11-shared-libraries-2026-10-15.md

# Stage 11.5 signed shared-library loading â€” 2026-10-15

**Later compiler-interoperability follow-up:** source `da6d16e6` resolves the
initial null-DT_DEBUG rejection recorded below. Session 73596 passed full
preservation and actual signed Rust/LLD shared imports at zero/nonzero bias on
1/2/4 CPUs, with all 505 inputs unchanged. See the compiler-PIE audit for the
accepted bounded correction. The original probes remain historical evidence;
main-image compiler local-exec TLS was subsequently accepted by full session
37505 on 515 unchanged inputs (2026-10-16); see its implementation audit.
Broader/library TLS and general dynamic linking remain open.

## Implementation and authority

`userspace::load_elf` and `load_elf_with_argv` now use the same transactional
loader in `kernel/src/userspace/linker.rs`. Before allocating an address space,
it copies and verifies each dependency's WAPP payload using the configured
application trust root. Lookup uses `/lib/<basename>` only; names are bounded
ASCII components, with no path separators, empty names, `.` or `..`. Writable
files are refused at lookup. The signature covers the exact copied payload, so
later file changes do not alter a loaded image. A declared SONAME must match
the requested basename. No capability is granted by loading a library.

The executable and up to four distinct libraries form one bounded graph.
Discovery preserves breadth-first `DT_NEEDED` order, rejects duplicate entries
within an image, reuses previously discovered dependencies, and supports
cycles. The symbol scope validates reachability and order before resolution.
Default-visible globals follow executable-first scope; private/protected
definitions bind locally. Undefined imports require a matching global export.

Each library receives a separate 16 MiB placement window beginning 256 MiB
above the user image base, with page-granular random placement inside it.
There are at most forty image mappings (five images Ã— eight segments). Image
mapping records use checked page offsets/counts in eight bytes each, avoiding
large repeated full-address copies in the fork syscall. These fields cover
the complete loader placement windows; supported capacity was not reduced.

All images are copied into RW/NX pages before relocation. Final segment W^X
and each image's RELRO permissions are applied before publication. Every
RELRO range is carried through address-space cloning and logical write-fault
authorization. The process owns the mappings until its scheduler-safe parent
reap; image mappings are released in reverse discovery order.

## Rollback, teardown and concurrency

The `Pending` guard owns an unpublished root and every successfully mapped
range. Every mapping, relocation, TLS or argv failure rolls that root back;
custom argv failure previously leaked an already-loaded address space.
File handles close during snapshot reads, and no borrowed image/file bytes
survive the call. Mapping/relocation/permission work on the unpublished root
uses the existing inactive paging APIs without synchronous TLB shootdowns.
No new lock, asynchronous registration, kernel handle or syscall is added.

Page-table teardown now validates private root branches before unmapping and
walks all empty private P3/P2/P1 tables. It preserves copied kernel branches,
rejects active/kernel roots and rejects omitted live mappings. The paging
guard serializes validation and reclamation; existing allocator/COW lock order
is preserved. The caller still must retire the root on all CPUs before
destruction, as required by the established process wait contract.

## Defects exposed during integration

- `stage11.5-1cpu-1792067119033534300`: the expanded full-address mapping
  table overflowed the existing syscall/fork stack budget and tripped the
  kernel stack canary. Compact checked page metadata fixed the overhead;
  the canary and stack size were preserved.
- `stage11.5-1cpu-1792067369377159600` and
  `stage11.5-1cpu-1792067479153948500`: the library function returned 42,
  but four physical frames remained. The old destructor freed only one
  lowest-level table per address space; separate library windows left two
  extra tables in both parent and clone. Complete private-table reclamation
  restored the exact frame baseline.
- The intervening diagnostic-build failure is retained at
  `stage11.5-1cpu-1792067458989384900`; it attempted to print an error enum
  without a `Debug` implementation and was corrected locally.

These directories are under `audit-artifacts/`; no earlier evidence was deleted.

## Verification contract

Host parser tests cover shared-object entry/type rules, transitive cycles,
ordered lookup, interposition/protected binding and version-tag rejection,
alongside the earlier ASLR/visibility/range tests. The Stage 11.5 gate signs
two real ELF shared objects into test-only WAPP packages, publishes them in
the VFS and loads an executable importing a library function. A Ring-3 indirect
call crosses into the mapped library and returns 42 through the ordinary exit
syscall. The gate checks W^X, RELRO, cloning, exact frame reclamation, unsigned
and writable dependency rejection, signature tampering, SONAME mismatch, path
traversal, unresolved imports and argv rollback. The harness requires explicit
markers for these checks, with host regressions rejecting missing evidence.

The final focused chain passed build, all host Rust tests (including twelve
ELF tests), sixteen `libwoven` tests, twelve Python harness tests, host
all-targets Clippy, production kernel Clippy and feature-specific kernel
Clippy for Stage 10.7, 11.2, 11.3, 11.4, 11.5 and 15.4, all warning-denying.
Commands, exit codes and complete logs are retained under
`audit-artifacts/shared-library-integration-20261015/`. The source hash manifest
pins 134 kernel/configuration files for the preservation run.

| Gate | 1 CPU artifact suffix | 2 CPU artifact suffix | 4 CPU artifact suffix |
| --- | --- | --- | --- |
| 10.8 | `1792072643048060800` | `1792072687006649200` | `1792072705336038000` |
| 10.9 | `1792072740941425600` | `1792072781945775800` | `1792072802438888400` |
| 11.1 | `1792072828709855200` | `1792072965584723200` | `1792073006840612900` |
| 11.2 | `1792067999973212900` | `1792068104726483800` | `1792068252051317700` |
| 11.3 | `1792068369419958000` | `1792068449380064700` | `1792068536934310400` |
| 11.4 | `1792068581779676200` | `1792068617911028200` | `1792068636127855200` |
| 11.5, including final security negatives | `1792069025545868100` | `1792069089887453700` | `1792069149340660400` |

Each directory is `audit-artifacts/stage<GATE>-<CPU>cpu-<SUFFIX>`. All listed
runs returned QEMU exit 33 and required their explicit serial markers. The
full `RUN-STAGE10.7.ps1` preservation chain returned exit 0 and its final pass
marker after Claude's live release-validation process finished. Its retained
output is under `audit-artifacts/acceptance-20261015-151448-207/`. The
overlapped phase was repeated successfully as described below.
Integration and focused runs do not close Stage 11.5.

### Preservation provenance correction

While adding host malformed-input coverage, `rustfmt tests/elf_scope.rs`
followed its `#[path]` module and changed formatting in `kernel/src/elf.rs`.
The frozen-manifest check caught the change. Only whitespace was involved;
the exact pre-format bytes were restored with SHA-256
`a1bf5af5a7896438e3e07933614f3c3b9f92ba2083f71622875383a76391e180`
at 2026-10-15 15:35:26 local time, and all 134 frozen hashes match again.
The three added host tests pass with warnings denied (15 ELF tests total),
covering every truncated fixture prefix, overflowing file/dynamic metadata,
and 1,086 deterministic metadata mutations through parser/resolver boundaries.
Their log is `shared-library-integration-20261015/elf-malformed-host-tests.log`.

The formatter window overlapped the final 4-CPU stress/network phase, so those
results cannot stand alone as exact frozen-source evidence. After the current
chain exits, rerun the original Stage 7.6 `Run-MemoryStress 4 20` and
`Run-Network` on 1/2/4 CPUs, preserving all original assertions and deadlines.
Do not count the affected phase as accepted until that repeat passes. Avoid
recursive module formatting under a frozen gate (`skip_children=true` is
needed for focused host-file formatting).

The restored-source repeat returned exit 0 with its final pass marker at
`audit-artifacts/restored-baseline-repeat-20261015-154630-461/repeat.txt`;
per-run serial/QEMU logs are under
`stage7-6-artifacts/20261015-154630-577/`. All twenty 4-CPU stress boots and
all three live-network CPU configurations passed, followed by warning-denying
host all-targets Clippy and all fifteen ELF host tests. The repeat directory
also retains network serial/build/QEMU logs, the frozen manifest and the
unaltered Stage 7.6 harness hash. All 134 source hashes matched before and
after the repeat. This closes the formatter-window provenance gap.

The subsequent Stage 10.8, 10.9 and 11.1 matrices passed on 1/2/4 CPUs, with
their feature-specific warning-denying kernel Clippy. The chain returned exit
0 and its final pass marker; commands and results are retained in
`audit-artifacts/loader-prior-runtime-20261015-155714-747/`. All 134 frozen
hashes matched before and after it. These prior runtime gates are not covered by
the Stage 10.7 preservation launcher or the already completed Stage 11.2â€“11.5
matrix, so their fresh evidence is recorded explicitly above. The bounded
signed-graph/transactional-teardown increment is accepted with these limits;
Stage 11.5 as a whole and the release image are not certified by this decision.

## Remaining loader and production limits

### Fixed executable placement integration requirement

Read-only review of Claude's pending branch found commit `6f02202d` (followed
by session commit `4586c3f2`): its ELF parser records whether the main image is
`ET_DYN`, and its old loader applies image ASLR only to that kind. The commit
records a production WovenShell instruction-fetch fault caused by relocating
an `ET_EXEC` image; its new acceptance chain is still running. That report is
not combined-tree acceptance.

The accepted bounded graph loader in this checkout still unconditionally calls
`relocated(aslr_image_delta())` in `linker::prepare`. Its replacement of the old
`load_elf` body means a mechanical merge must not silently discard Claude's
placement correction or restore the obsolete dependency-rejecting loader.
Port the parser's image-kind knowledge into the graph loader explicitly, retain
fixed `ET_EXEC` placement, and qualify randomized `ET_DYN` placement separately.
Current `qemu-test` builds force the ASLR draw to zero, so their passing loader
markers are not evidence for this production nonzero-bias case. Require a
nonzero-bias/production-path executable regression and preserve graph loading,
W^X/RELRO and transactional teardown tests. No frozen source changed during
this review; integration follows the current generation/earlier IPC gates.

A separate real-toolchain probe is retained in
`audit-artifacts/shared-library-integration-20261015/toolchain-probe/`.
Nightly Rust emitted PIC objects and its bundled LLD linked a SysV-hash shared
object plus a PIE executable importing `woven_answer` through `GLOB_DAT`.
The production parser accepts the unmodified entry-zero library. It rejects
the executable's LLD-generated null `DT_DEBUG` tag as `Unsupported`. An
in-memory diagnostic changing only that tag to inert zero-valued `DT_FLAGS`
makes parsing succeed; neither ELF artifact nor kernel was changed. This
is a concrete interoperability blocker, not a passing execution test. The
probe's parser helper compiled with warnings denied. Accepting this metadata
safely and booting a signed, unmodified toolchain-produced executable is the
next dedicated loader gate; it must not be inferred from the synthetic fixture.

The same probe now includes `tls.rs`, linked as an unmodified static executable.
Its PT_TLS has file/memory size 8 and alignment 8; the production parser accepts
it. Disassembly of the compiler-generated local-exec access first reads `FS:0`
as the thread pointer, then reads at offset -8. The current loader instead sets
FS to the end of a page-rounded allocation, immediately before the stack guard,
with no mapped self-pointer/TCB, and copies the template at the allocation start.
It therefore cannot satisfy this compiler ABI. The page-sized synthetic TLS
parse test does not exercise this access. This is a source/disassembly diagnosis,
not a QEMU runtime result; `tls-elf-report.txt`, `tls-disassembly.txt` and
`tls-parser-result.txt` retain the evidence. Compiler TLS execution is an open
correctness gate even for the main image, not merely for shared libraries.
Further, `clone_initial_tls` currently copies the main thread's live TLS bytes;
new threads need fresh initialized template state rather than inheriting
mutated main-thread variables. The follow-up must cover a mapped TCB/self
pointer, alignment-aware template placement, fresh per-thread initialization,
clone/fork self-pointer rebasing and complete teardown before claiming general
initial-exec/local-exec compatibility.

The probe is reproducible from committed sources via
`python scripts/build-loader-fixtures.py` (`2cfd5f8f`, separately committed and
pushed while the kernel remained frozen). A successful final invocation wrote
`target/loader-fixtures/build-1792070703733127200/manifest.json`; it pins input,
builder, compiler/linker tools, command log and ELF output hashes. Generation
uses direct compiler commands, not a competing Cargo/QEMU acceptance chain.
All fifteen Python harness tests passed after adding the builder's output
collision, failed-command retention and missing-tool tests. The fixture files
remain unmodified after linking and still require kernel runtime acceptance.
Follow-up fixture commit `44850832` adds `tls-threads-demo`: initialized and
zero-filled compiler TLS, 64-byte alignment, two sequential event-joined child
threads, private child writes and unchanged mutated parent state. The complete
warning-denying build wrote
`target/loader-fixtures/build-1792070954551903600/manifest.json`, and all fifteen
host Python tests pass. This is compiled test infrastructure, not runtime proof.

This is startup loading of a bounded signed graph. Legacy REL/PLT relocation
tables, versioned/weak symbols, constructors/destructors, dynamic library TLS,
lazy binding and runtime `dlopen`/`dlclose` remain unsupported. The existing
main-image TLS mapping contract is preserved, but compiler TLS execution has
the correctness gap described above. Libraries are copied into
process-owned mappings; cross-process executable-page caching is future work.
The existing COW/frame, VFS file-size and process resource ceilings still
apply. Symbol lookup is bounded linear scanning; adversarial-scale performance
and broader toolchain interoperability need further qualification. Deployment
keys, persistent library installation/updates and hardware qualification remain
open. This increment does not certify Stage 1â€“19 production readiness.
---

## Source: docs/audit-stage11-static-tls-2026-10-14.md

# Stage 11 static TLS audit â€” 2026-10-14

## Scope

The ELF loader now accepts one validated `PT_TLS` image in an otherwise
static executable. It allocates a page-aligned, writable initial TLS block
below the user stack guard, zero-fills it, copies the file image, and exposes
the block end as the task's FS base. The FS base is installed on first entry,
exec, and every scheduler context switch. Fork shares the TLS pages through
the existing copy-on-write address-space path, and TLS writes are treated as
writable for COW fault bookkeeping.

The parser rejects duplicate or malformed TLS headers, including zero-sized,
file-larger-than-memory, non-power-of-two, over-page alignment, and truncated
file ranges. `PT_INTERP` remains rejected. Self-contained relative ET_DYN
relocations are covered by the separate dynamic-relocation audit. Shared-library
TLS, TLS module IDs, and multiple kernel-managed user threads remain outside
this static initial-exec contract.

## Validation

- Static and invalid TLS parser fixtures pass the loader self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings` â€” passed.
- `cargo clippy --all-targets -- -D warnings` â€” passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` â€” passed.
- Stage 11.5 QEMU acceptance passed on 1, 2, and 4 CPUs:
  - `audit-artifacts/stage11.5-1cpu-1791972922171894100`
  - `audit-artifacts/stage11.5-2cpu-1791972980542087100`
  - `audit-artifacts/stage11.5-4cpu-1791973031110997200`

These gates validate the loader and scheduler contract in QEMU; they do not
claim physical hardware TLS qualification or dynamic-linker support.
---

## Source: docs/audit-stage11-symbol-scope-2026-10-15.md

# Stage 11 bounded cross-image symbol scope â€” 2026-10-15

## Scope

This increment adds the first safe building block for shared-library loading in
`kernel/src/elf.rs`:

- `SymbolScope` borrows one main image and at most four direct dependencies;
- dependency names are exact, non-empty, NUL-free byte strings;
- duplicate names and capacity overflow are rejected;
- `validate_dependencies` requires a one-to-one match with `DT_NEEDED`;
- resolution searches the main image first, then dependencies in declaration
  order, and exports only defined `STB_GLOBAL` symbols inside load segments;
- all image bytes are borrowed for the scope lifetime, so no unchecked buffer
  can be retained for asynchronous use.

Weak/versioned symbols, lazy binding, dynamic TLS, dependency file lookup,
mapping, relocation application and shared-image teardown remain unsupported.
`userspace::load_elf` therefore continues to reject dependency-bearing images;
this increment does not claim shared-library support.

The boot fixture now exercises positive exact-name resolution and negative
unknown-name, duplicate-name, empty-name and capacity checks. The resolver is
heap-backed in that fixture because the existing loader self-test already has
a large stack footprint; production user stacks are not enlarged to hide test
allocation pressure.

## Validation

- Host `cargo test -p wovenhat-os`: passed.
- Host all-targets Clippy with warnings denied: passed.
- Freestanding kernel Clippy for `stage11-5-test`, warnings denied: passed.
- Stage 11.3 final regression: 1/2/4 CPUs passed.
  - `audit-artifacts/stage11.3-1cpu-1792063124447040900`
  - `audit-artifacts/stage11.3-2cpu-1792063199536084600`
  - `audit-artifacts/stage11.3-4cpu-1792063255560697600`
- Stage 11.4 final regression: 1/2/4 CPUs passed.
  - `audit-artifacts/stage11.4-1cpu-1792063310702602200`
  - `audit-artifacts/stage11.4-2cpu-1792063384179067000`
  - `audit-artifacts/stage11.4-4cpu-1792063442296250500`
- Stage 11.5 before the final negative-check fixture: 1/2/4 CPUs passed.
  - `audit-artifacts/stage11.5-1cpu-1792062928518294300`
  - `audit-artifacts/stage11.5-2cpu-1792063003256375700`
  - `audit-artifacts/stage11.5-4cpu-1792063062215317400`
- With the final negative checks, Stage 11.5 passed on 1/2/4 CPUs.
  - `audit-artifacts/stage11.5-1cpu-1792063534960218100`
  - `audit-artifacts/stage11.5-2cpu-1792063857524788700`
  - `audit-artifacts/stage11.5-4cpu-1792063914115084100`
  Two earlier timeout artifacts remain preserved; one reached the required
  marker while another worktree was running a QEMU gate. They are diagnostic
  failures, not substituted for the final passing rerun.

## Next gate

Wire this scope into a bounded `/lib` namespace that reads only verified,
read-only dependency files, maps each image with separate W^X ranges, applies
cross-image relocations before RELRO, and tears images down in reverse
dependency order. Keep the current fail-closed path until that complete gate
passes.

## Review follow-up: address and namespace correctness

Review of `28db2929` found four reproducible defects in the new scope:

- nonzero load bias made valid symbol addresses fail containment checks;
- hidden/internal symbols, TLS/IFUNC types and reserved sections were treated
  as ordinary exports;
- dependency indices above `needed_count` returned unused zero offsets;
- unordered membership checks accepted reordered and duplicate `DT_NEEDED`
  declarations, and lookup could use an incomplete scope.

Four parser-backed host tests failed on the old source. The preserved failure
log is `audit-artifacts/symbol-scope-review-20261015/before.log`. The expanded
eight-test suite also covers main-image precedence, complete segment bounds,
overflow, truncated input, ordered lookup and private image-local definitions.
The QEMU fixture now uses a nonzero dependency bias.

The same address bug existed in the production local-relocation path. Both
paths now use `Image::defined_symbol_value`, applying the bias once before
checking the complete symbol range. The loader self-test also now derives its
expected bias from the loaded entry instead of drawing new entropy for each
comparison. Repeated entropy draws had made its expected addresses unrelated
to the image selected by the production loader; deterministic QEMU tests hid
that error.

No syscall ABI, object ownership, locks, asynchronous registrations or teardown
paths are added by this correction. Scope data borrows the image bytes and
resolution validates the complete dependency order on every call. Dynamic
library file lookup and mapping remain prerequisites.

The first review QEMU attempt failed with exit 35 at
`audit-artifacts/stage11.5-1cpu-1792064806382906400`. The existing fixture
overlaid metadata on copied instructions but initialized only selected symbol
fields; type and size could still contain instruction bytes. Both complete
symbol records are now initialized before filling fields. After that correction
the same 60-second Stage 11.5 command passed on 1 CPU at
`audit-artifacts/stage11.5-1cpu-1792064913412567100`.

Earlier timeout causes remain unproven: passing after a stack-layout change
does not establish stack exhaustion, and concurrent QEMU activity does not by
itself establish the cause of a timeout. All those failure artifacts remain
retained; none is used as acceptance evidence.

### Timeout investigation

The review 2-CPU gate timed out at
`audit-artifacts/stage11.5-2cpu-1792065038849912700`. A diagnostic boot using
the same image and the same 60-second bound captured QMP CPU snapshots at
`audit-artifacts/scope-diag-1792065231481947500`. CPU 0 was executing at
`0x10000038492c`, within `sha2::sha256::soft::sha256_digest_round_x2` in the
matching ELF; the last serial marker was the SHA primitive self-test, before
the full kernel measurement completed. CPU 1 was still parked by firmware.
This snapshot establishes the bottleneck in that diagnostic run, without
attributing every historical timeout to the same cause.

The development profile now compiles the `sha2` dependency at optimization
level 3. Kernel measurement still hashes the complete ELF, and the known-answer
self-test and fixed acceptance deadlines remain in place. Release profile
behavior is unchanged.

### Final review Stage 11 matrix

All runs below use the corrected fixture and development SHA profile, the
unchanged 60-second deadline, required serial markers, and QEMU exit 33.

| Gate | 1 CPU artifact suffix | 2 CPU artifact suffix | 4 CPU artifact suffix |
| --- | --- | --- | --- |
| 11.3 | `1792065607716118200` | `1792065676487457300` | `1792065694456222600` |
| 11.4 | `1792065740610343800` | `1792065825816156100` | `1792065845142600000` |
| 11.5 | `1792065571962600000` | `1792065422228678500` | `1792065588767558800` |

Each directory is `audit-artifacts/stage<GATE>-<CPU>cpu-<SUFFIX>` and retains
the build, serial and QEMU logs. Full shared-library acceptance, release-chain
revalidation and hardware qualification remain open.

The first follow-up Stage 15.6 build failed before QEMU because Cargo's
incremental dependency graph/cache paths disappeared; retained build output is
`audit-artifacts/stage15.6-1cpu-1792065902904271700/build.log`. The rerun uses
the repository's documented `CARGO_INCREMENTAL=0` setting without deleting
cache trees or evidence. This build failure is distinct from the captured
SHA-compression timeout.

Stage 15.6 then passed on 1/2/4 CPUs with exit 33, including the known-answer
SHA test and full kernel measurement before the stage marker:

- `audit-artifacts/stage15.6-1cpu-1792065974170811800`
- `audit-artifacts/stage15.6-2cpu-1792066056110025200`
- `audit-artifacts/stage15.6-4cpu-1792066073924753600`

Final checks, with project-local build/target directories and
`CARGO_INCREMENTAL=0`, all returned exit 0:

- `cargo build`
- `cargo test -p wovenhat-os` (includes all eight new ELF regressions)
- `cargo test -p libwoven` (16 tests)
- `cargo clippy --all-targets -- -D warnings`
- production freestanding kernel Clippy and individual feature Clippy for
  `stage11-3-test`, `stage11-4-test`, `stage11-5-test`, `stage15-6-test`, all
  targeting `x86_64-unknown-none` with `-D warnings`.

Their complete logs are in `audit-artifacts/symbol-scope-review-20261015/`.
This accepts the resolver correction and measurement-build fix as an increment;
it does not close Stage 11.5 or the Stage 1â€“19 production objective.
---

## Source: docs/audit-stage11-symbol-versioning-2026-10-20.md

# Stage 11 versioned symbol tables audit â€” 2026-10-20

## Scope

This increment implements bounded SysV ELF symbol-version metadata for the
Stage 11 graph loader: `DT_VERSYM`, `DT_VERDEF`, and `DT_VERNEED`. Definitions
and imports use exact requested version names; malformed or truncated records
fail closed.

## Implementation

- `kernel/src/elf.rs` records and validates file-backed version tables,
  bounded auxiliary chains, version indices, names, and hidden-version bits.
- `Image::relocated` shifts dynamic metadata addresses together with the
  relocated image, preserving version-table walks for shared objects.
- `SymbolScope` resolves strong and weak definitions by name and requested
  version, preserving existing visibility, bounds, relocation, W^X, and
  transactional teardown rules.
- `kernel/src/userspace/linker/acceptance.rs` adds signed versioned main and
  library fixtures, relocation verification, Ring-3 execution, and teardown.

## Validation

- `cargo test --workspace`: passed.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p wovenhat-os --test elf_scope`: 31 passed.
- Stage 11.5 QEMU passed on 1 CPU, evidence in
  `audit-artifacts/stage11.5-1cpu-1792505125207763300`.
- Stage 11.5 QEMU passed on 2 CPUs, evidence in
  `audit-artifacts/stage11.5-2cpu-1792505299339871100`.
- Stage 11.5 QEMU passed on 4 CPUs, evidence in
  `audit-artifacts/stage11.5-4cpu-1792505368683688700`.

## Limits and next prerequisites

This is not full dynamic-linker production completion. Constructors and
destructors, lazy binding, and library TLS remain outside the accepted scope;
the inherited Stage 11 and later-stage gates remain required.
---

## Source: docs/audit-stage11-thread-exit-encoding-2026-10-15.md

# Process/thread exit-result encoding â€” bounded correction accepted

## Final acceptance â€” 2026-10-15

Following COW acceptance `79b2f5a` and wait-ownership acceptance `6c57a17`,
the signed-status correction in `ad11e1f1` is accepted. Session 94658 exited 0
with full combined acceptance PASS; all 500 frozen inputs matched at completion
and independent inspection. The expanded actual-libwoven fixture and original
thread gate passed on 1/2/4 CPUs. All six process statuses and twelve natural/
cancelled thread cases, stale/double joins, child ownership and genuine errors,
signal termination and exact resource baselines are covered. Full Stage 10.7,
all twenty-one Stage 10.8â€“11.5 runtime boots, all host tests, nineteen libwoven
and nineteen Python tests, build and warning-denying host/fixture/production/
fifteen-feature Clippy passed. See [combined evidence](audit-exit-status-combined-acceptance-2026-10-15.md)
for authoritative paths and independent audit checks.

The public wait type, syscall numbers and full-width errors are unchanged;
successful signed exit statuses now have an unambiguous low-32-bit payload.
This accepts the bounded encoding correction, not full Stage 11, combined-tree
integration, a new release or hardware readiness. Compiler TLS, fixed-ET_EXEC
placement and general loader interoperability are the next open prerequisites.
Pending/read-only statements below are the retained investigation timeline,
not the final acceptance decision. No original failing evidence was removed.

## Current executable correction â€” 2026-10-15

The original casts were extracted without changing behavior into the production
`exit_status::encode` helper, and both syscall branches were wired through it.
Actual-source host tests reproduced the kernel payload collision and both
libwoven decoder failures (exit 101):
`audit-artifacts/exit-status-20261015/kernel-before.txt` and `libwoven-before.txt`.
The helper now zero-extends through `u32`. Kernel payload tests and nineteen
libwoven tests pass (`kernel-after.txt`, `libwoven-after.txt`). The actual
thread decoder is shared with its test rather than copied. Syscall IDs, full-width
error constants, `Join` variants, and the public process wait return type remain
unchanged. A successful wait retains the status's low 32 bits; `as i32` recovers
the signed value. No unsafe code, object, capability, lock or wait was added
for the encoding correction.

`tools/exit-status-fixture` is a separate freestanding crate linked against the
actual libwoven library, using the established static ELF linker contract.
Its only assembly establishes CPU entry-stack alignment. It exercises six
signed process exits, six natural thread exits, six cancelled sibling statuses,
double reap/stale joins, cross-process authority, real WouldBlock and invalid
errors, and signal-terminated child cleanup. Workers park on timer events;
nonblocking process waits sleep between retries. It uses existing test process
capabilities, not an expanded capability set or unsigned production image path.

The independent `stage11-exit-status-test` subgate deliberately includes the
existing Stage 11.2 feature. Its guarded block runs first and then falls through
to the unchanged original thread assertions and exit. The runtime harness's
`11.2-status` choice builds the fixture, supplies its mandatory compile-time ELF
input, and requires every new family marker plus the original thread markers.
It cannot pass merely by reaching an earlier exit. Linting that new feature
requires `WOVENHAT_EXIT_STATUS_ELF`; missing input is not a pass.

Real execution exposed earlier COW ancestor-permission and wait-ownership
classification defects. They are documented separately in the Stage 7 COW and
process-wait ownership audits; all failing serial logs and source snapshots
are preserved. After their corrections, the full one-CPU fixture and original
Stage 11.2 gate passed in `stage11.2-status-1cpu-1792087223023180400/` with exact
resource baselines. A final revised 1/2/4-CPU matrix is running. Full lint,
host, inherited runtime and combined-source preservation are still required;
none of these integrated corrections is accepted yet. The original read-only
findings below remain investigation history.

### Final revised focused matrix and current full gate

All three revised focused boots passed, exit 33, with every libwoven family,
resource-baseline and original Stage 11.2 marker required by the harness:

- `stage11.2-status-1cpu-1792087580234175000/`
- `stage11.2-status-2cpu-1792087612734709000/`
- `stage11.2-status-4cpu-1792087650972914800/`

All nineteen Python harness/builder regressions pass. The builder uses a locked
dependency graph and shared project-local Cargo caches, records compiler and
input/output hashes, refuses existing evidence directories, and refuses to
publish a manifest if any input changes during compilation. Missing markers,
wrong exit, panic and failure strings remain fatal in the runtime harness.

Session 94658 is running the complete combined acceptance from
`audit-artifacts/exit-status-20261015/run-acceptance.ps1`, with the immutable
500-file manifest alongside it. This freeze also includes the ELF and its
published manifest supplied to new-feature Clippy. It is separate from all
historical thread/IPC manifests. Full build, host and fixture Clippy, all tests,
fifteen affected kernel-feature lint configurations, full Stage 10.7 and
Stage 10.8â€“11.5 runtime preservation remain required before acceptance.

Read-only review during the serialized runtime wait. No kernel/runtime input
was changed and no competing test chain was started for this finding.

Reviewed SHA-256:

- `kernel/src/syscall.rs`: `da08696e03b020409cb876aac4406c58b5ce57a2eaceb1d74e0b1bdbb6848af1`
- `libwoven/src/lib.rs`: `67a4f819ca3988f17ce225b5dbd9e9907d06fdfd95a3f2718d397267ac0e335b`

## Static counterexample

Thread exit and cancellation accept an `i32` status. The registry and task join
return that signed status successfully; the `ThreadJoin` syscall branch encodes
`Ok(exit)` as `exit as u64`. `libwoven::syscall::thread_join` reserves the exact
64-bit `ERROR` and `WOULD_BLOCK` values before decoding other results as `i32`.

| Successful thread status | Kernel's current result | Userspace interpretation |
| --- | --- | --- |
| `-1` | `u64::MAX` | `Join::Invalid`, not `Exited(-1)` |
| `-2` | `u64::MAX - 1` | `Join::Running`, not `Exited(-2)` |
| `i32::MIN` | Sign-extended 64-bit value, not either sentinel | `Exited(i32::MIN)` |
| `0`, positive `i32` | Nonnegative 64-bit value | `Exited(status)` |

The syscall blocks for a running thread, so a completed `-2` status is not an
actual would-block response. Registry join can already have reclaimed the
record/resources before userspace misclassifies the result. Cancellation has
the same return contract because its supplied `i32` status is joined normally.
This counterexample follows the inspected casts/decoder; it is not yet an
executed host or QEMU regression.

A separate concern about high-bit thread IDs was checked and not substantiated:
the ABI uses exact sentinels, not a generic signed-negative check. Valid thread
slots are 0â€“63 in the low 32 bits, so generation-tagged IDs cannot equal either
all-ones sentinel. Do not unnecessarily reduce the generation domain or alter
the handle format to fix a nonexistent sign-bit restriction.

## Required correction and evidence

After the current generation correction and earlier IPC gate, make successful
32-bit exit payloads unambiguous from 64-bit error sentinels. Zero-extending the
status's `u32` representation is a candidate: the existing `as i32` decoder can
recover the original signed status while `u64::MAX`/`MAX-1` stay distinct. Review
all raw syscall consumers before selecting that encoding. Preserve syscall
numbers, owner checks, blocking semantics, ordinary statuses and teardown.

Require actual encode/decode host regressions and Ring-3/libwoven join tests for
`-1`, `-2`, `i32::MIN`, `i32::MAX`, zero and an ordinary positive status, including
both natural thread exit and requested sibling cancellation. Invalid/stale or
foreign-owner joins must still fail, and joins must reclaim stack/TLS/registry
resources exactly once. Run warning-denying affected checks, Stage 11.2 and
inherited 1/2/4-CPU preservation without weakening assertions or timeouts.

This finding does not change the frozen generation correction or certify
Stage 11.2/Stages 1â€“19. The ABI correction remains a separate unaccepted gate.

## Earlier process wait path has the same collision â€” source review

Read-only follow-up during IPC session 63244; no new test chain or source
edit. `task::wait_process` returns `Result<i32, WaitError>`, and the `Waitpid`
syscall encodes `Ok(exit_code)` with the same sign-extending `as u64` cast.
Its explicit errors are `u64::MAX - 1` for `StillRunning` and `u64::MAX` for
`NoSuchChild`. `libwoven::wait` passes the result through `check`, which maps
those exact values to `Error::WouldBlock` and `Error::Failed` respectively.
Successful child statuses `-2` and `-1` are therefore indistinguishable from
those errors even after the kernel has consumed the zombie and reclaimed its
resources. This is a source counterexample, not executed process regression
evidence. The reviewed syscall/libwoven hashes above still match; task source
SHA-256 is `11b2cd9d1c61903f2624fae10e0eb2b7e670bdd57530f555ccc3c474d1c16a3c`.

The current shell's `reap` treats `WouldBlock` as still running; it can thus
temporarily retain a completed `-2` child until a subsequent wait reports no
child. That does not prove a kernel resource leak or indefinite shell retention.
The reviewed shell SHA-256 is
`59514e1e23ae695a0db9774fd7c5ae4a3cec4b35d9a631f4262fbd4ea0a90127`.
Existing boot wait/reap tests exercise successful status zero and double-wait
rejection; those assertions do not cover this signed boundary.

Resolve this earlier process-wait encoding together with the thread-status
prerequisite after IPC acceptance, before advancing loader/TLS work. Preserve
syscall 5, the existing public `Result<u64, Error>` wait signature, actual-running
and no-child errors, parent ownership, zombie/resource reclamation and all
ordinary positive statuses. Review raw consumers and explicitly document the
32-bit status payload representation before sharing an encoding helper; do not
change the type or invent a new wait ABI silently. Require host production
encode/decode tests and real Ring-3 parent/child waits for `-1`, `-2`, `i32::MIN`,
`i32::MAX`, zero and positive status, actual-running/no-child/foreign-child
errors, double reaping and exact resource baselines. Original thread natural
exit/cancellation coverage remains required as well.
---

## Source: docs/audit-stage11-thread-generation-exhaustion-2026-10-15.md

# Stage 11 thread generation exhaustion â€” bounded correction accepted

## Final serialized acceptance â€” 2026-10-15

Session 79248 completed with exit 0 and
`=== THREAD GENERATION RUNTIME AND PRESERVATION: PASS ===`. The fresh attempt
`audit-artifacts/thread-generation-20261015/runtime-attempt-20261015-180920-929/`
passed Stage 11.2 on 1/2/4 CPUs, the exact full `RUN-STAGE10.7.ps1` launcher,
and Stage 10.8, 10.9, 11.1, 11.3, 11.4 and 11.5 on 1/2/4 CPUs. The launcher
evidence is `audit-artifacts/acceptance-20261015-181015-071/`; inherited stress
includes 10 one-CPU, 30 two-CPU and 20 four-CPU memory boots plus live network
and isolated asynchronous I/O gates. Host QA ends with its explicit pass marker.
All 135 original frozen source hashes matched at completion and independent
post-run inspection. No competing foreign test/compiler/QEMU process was found
at that inspection. Earlier interrupted attempts remain historical evidence,
not the basis for acceptance.

This accepts the permanent thread-record retirement correction committed in
`92959b3d`, including its actual-source host boundary regressions and inherited
runtime preservation. It does not resolve the separately documented negative
exit-result ABI collision, IPC rollover, compiler TLS, loader interoperability,
combined-tree integration, or hardware qualification. No release image is
newly qualified by this increment. The pending statements below describe the
retained investigation timeline, not the final acceptance decision.

## Finding

At the reviewed baseline, `thread::create` selected every `State::Free` slot.
Join, abort and owner reaping
advance its generation with checked addition but saturate at `u32::MAX`.
Once that generation is issued and retired, the next allocation can issue the
same slot/generation handle again. Stale handles can then address a later
thread. This contradicts the generation-safe lifecycle contract even though
ordinary low-generation teardown tests pass.

The reviewed source has SHA-256
`976c13aa42c7511c5979c97aca93b9e67770618253a8043b0e762c0c6489eab0`.
This is an existing Stage 11.2 defect, independent of signed library loading.

## Reproduction and candidate evidence

Artifacts are in `audit-artifacts/shared-library-integration-20261015/`:

- `thread-generation-repro.rs` copies the production registry and adds private
  host tests. A standard host mutex replaces only the IRQ-lock environment,
  and a minimal mapping type stands in for opaque resource payloads. These
  tests make no claim about hardware locking, paging or scheduler behavior.
- The tests seed all slots at generation `u32::MAX - 1`, allocate and retire
  through join, abort or owner reaping, and reject any reissued stale handle.
  All three fail against the copied production logic (exit 101), retained in
  `thread-generation-repro-all.log`. The earlier single-case failure is also
  preserved in `thread-generation-repro.log`.
- `thread-generation-candidate.rs` changes allocation to skip saturated
  generations. All three tests then pass, including complete exhausted-table
  refusal (`thread-generation-candidate.log`). Both host binaries compile
  with warnings denied.

No production kernel source changed for that experiment. The loader increment
has since passed its preservation gates and is committed as `454f1ff2`.

## Current correction

`Record::reusable` reserves `u32::MAX` as a permanent retirement sentinel,
and allocation applies that predicate under the existing rank-10 registry
guard. The last issued generation is `u32::MAX - 1`; each release path already
advances that to the sentinel. Saturated free records remain unavailable rather
than recycling stale authority. The 64-slot ordinary capacity, thread-handle
encoding, syscall numbers, ownership checks and full-table refusal ABI remain
unchanged. No new object, lock, allocation, unsafe code, wait or cross-CPU
publication path is introduced.

`tests/thread_registry.rs` now imports the actual production registry, not a
copied implementation. Its test-only IRQ-lock and opaque mapping doubles make
the registry state machine executable on the host. Before the fix, all three
exhaustion cases fail while ordinary owner/waiter reuse passes
(`thread-registry-before.log`, exit 101). After integration, those cases plus
full 64-slot capacity/reuse all pass (`thread-registry-after.log`). The earlier
copied-source stale-alias reproducer remains preserved separately.

The Stage 11.2 structural fixture also checks the retirement boundary and now
reclaims its final replacement record rather than leaving a test-owned live
entry behind. All existing assertions remain; the new cleanup requires both
a distinct replacement and a successful terminate/join.

Direct warning-denying host compilation, Cargo's five registry tests, and
test-target Clippy pass. Complete build, host all-targets Clippy, all host Rust
tests, sixteen `libwoven` tests, fifteen Python tests, production-kernel Clippy
and kernel feature Clippy for Stage 10.7, 10.8, 10.9 and 11.1â€“11.5 all pass.
Commands/results are retained under `audit-artifacts/thread-generation-20261015/`;
`host-qa.txt` ends with the explicit pass marker. Its manifest freezes 135
kernel/build/harness files, and all still match.

The runtime chain initially queued behind confirmed live Claude test processes
(PIDs 20996 and 24984 observed at launch). Its required sequence is Stage 11.2 on 1/2/4 CPUs,
the full `RUN-STAGE10.7.ps1` preservation chain, then Stage 10.8, 10.9, 11.1,
11.3, 11.4 and 11.5 on 1/2/4 CPUs. Failure stops the chain with its logs retained;
no assertions or timeouts were weakened. Runtime preservation remains pending.

Claude's original runner later recorded `release: PASS` and `DONE`; its release
report has 102 passing checks and no failures. Codex's first attempt began after
that runner exited, but Claude launched a new `run2015.py` acceptance chain
(PID 20780, start 2026-10-15 17:04:46). Codex stopped only its own verified
process tree rooted at PID 19632 rather than continue overlapping acceptance.
Session 22900 ended with exit 1 from that deliberate interruption; it is not
runtime acceptance. `runtime-chain.txt` and
`audit-artifacts/stage11.2-1cpu-1792076665295818400/` remain preserved; the
interrupted attempt's serial and QEMU logs are empty and prove no pass.

The stricter user-selected all-checkout rule and Codex's next-slot reservation
were synchronized into all three Claude worktree coordination documents.
The new runner `run-runtime-preservation.ps1` waits for the identified live
Claude chain, then for any remaining foreign compiler/test or QEMU process,
checks the original 135 hashes, and repeats the entire required sequence.
Every attempt uses a unique directory, so it cannot overwrite the interrupted
evidence. No foreign process/source was changed.

### Second handoff interruption â€” acceptance still open

Claude's `run2015.py` later ended after a 102-check release sub-chain pass;
its earlier inherited desktop installation/Quick Settings failures are retained
separately and prevent accepting the enclosing change. Codex's next attempt
started in `runtime-attempt-20261015-175941-447/`. Stage 11.2 reported passes
on 1/2/4 CPUs (exit 33), with evidence directories ending
`1792079981704042900`, `1792079993977286800`, `1792080011582643200`.
Full Stage 10.7 preservation then began.

A new Claude `run2015b.py` chain (PID 30304, start 17:59:57 local) overlapped
that handoff despite the written all-checkout reservation. Codex again stopped
only its own verified process tree, rooted at PID 29444 (start 17:10:16), and
session 4650 ended with exit 1. The partial attempt's logs remain intact; this
does not establish full runtime acceptance, and the complete sequence must be
repeated under actual serialization. No assertion, timeout or source hash was
changed to accommodate the overlap.

At that interruption no automatic restart remained queued. The user was asked to obtain Claude's
acknowledgement of the reserved slot because written notes have not prevented
two overlaps. Preserve the current source freeze until that coordination is
confirmed. This is a scheduling interruption, not a newly diagnosed kernel-test
failure or proof that the unaccepted correction is production-ready.

The user subsequently replied "Yes" to confirmation that Claude has agreed to
hold new acceptance chains. A fresh repeat now waits for the confirmed live
`run2015b.py` PID 30304 (start 17:59:57) and remaining foreign compiler/test/QEMU
processes. It checks the original source manifest and reruns the entire sequence
in another unique attempt directory. The overlapped attempt is not reused as
acceptance. No source or assertion was changed to resume the gate.

Fresh serialized session 79248 started
`runtime-attempt-20261015-180920-929/` after PID 30304 exited. Its Stage 11.2
1/2/4-CPU runs all passed with exit 33; retained evidence directories end in
`1792080561132871900`, `1792080580109722800`, `1792080595486127500`.
The subsequent live-process check found no foreign test/QEMU process. Full
Stage 10.7 preservation is now running, followed by the remaining matrices.
This is focused progress only: complete runtime acceptance is still pending.

During that wait, a byte-level check found `thread.rs` converted from LF to
CRLF (writer not established). Normalizing only line endings reproduced the
original frozen SHA-256 exactly. Explicit Unix-newline formatting restored
those exact bytes before any own runtime gate started; all 135 manifest entries
matched again. `line-ending-restoration.txt` retains both hashes and the
restoration command. The freeze was not replaced or relaxed.
The conversion recurred before runtime started. Git's system configuration
reports `core.autocrlf=true`; the actual writer remains unestablished. A narrowly
scoped `.gitattributes` rule now requests LF for `kernel/src/thread.rs` without
changing any user/system Git configuration or normalizing other frozen files.
The same exact original source hash and all 135 manifest entries match after
restoration; `git check-attr` confirms `text=set`, `eol=lf`.

### Build-space maintenance

Free space fell below 1 GB while other worktrees also compiled. The prescribed
routine's dry run found little idle incremental space, and its broad path
substring guard conservatively treated the main checkout as busy for work in
`C:\WovenHat-OS-wt*`. After this checkout's own host chain exited, explicit
process/path checks found no compiler/QEMU reference inside `C:\WovenHat-OS\`.
The literal `.cargo-build` target and all descendants were checked for reparse
points; only that own regenerable cache was removed (4,355,631,770 measured
file bytes). Sources, audit/gate evidence and `target/releases/0.8.0` were not
removed. Cargo will rebuild the cache. The preserved release image still hashes
to `6195a4721219a80b4e7775531a3657c7ce7e50a78648da53ca0b31b53ae03544`.
No foreign worktree cache or live QEMU process was modified.

Thirteen additional own `apps/<name>/target` caches were later measured at
about 2.2 GB and removed after validating every literal resolved target,
all parent/descendant reparse attributes and absence of own application builds.
These are the same regenerable per-app targets selected by the prescribed
routine's deep mode; sources, signed packages, release archives and gate/audit
evidence were preserved. The runtime chain was still waiting on Claude's live
test process. The packager's explicit per-app target policy explains repeated
dependency-cache growth; changing that policy requires separate build/output
qualification and was not mixed into the frozen kernel correction.

Before the queued runtime chain started, free space again fell below 3 GB.
The literal own `target/debug` and `target/x86_64-unknown-none` compiler-cache
directories were resolved and checked recursively for reparse points. No own
cargo/rustc/Clippy/QEMU process referenced this checkout, while the original
foreign runner PID and start time were revalidated. Only those two idle caches
were removed (2,087,889,718 and 715,611,196 measured file bytes). They are
regenerable, not test evidence. Free space rose to 5,555,716,096 bytes; the
retained release image again matched `6195a472â€¦`. No gate, audit or release
directory was removed, and no foreign worktree was changed.

Later, with free space near 3 GB, the prescribed deep routine's dry run found
idle caches in `C:\WovenHat-OS-wt2`. Its literal root, `.cargo-build` and twelve
existing `apps/<name>/target` paths were resolved within that named worktree;
root/parent/descendant reparse checks passed. No compiler/Clippy/QEMU process
referenced it, and the routine independently rechecked that it was idle. Running
`python scripts/reclaim-build-space.py --min-free-gb 5 --deep C:\WovenHat-OS-wt2`
removed only those 13 regenerable compiler targets. The retained transcript is
`idle-wt2-cache-cleanup.log`; free space rose from about 3.0 to 6.5 GiB
(6,975,184,896 bytes observed). Unlike the earlier maintenance, this deliberately
reclaimed a different idle worktree's caches; it did not change its source,
audit/gate/release evidence or any live process. Its only tracked modification
remained the synchronized coordination document. The main retained release
image again matched `6195a472â€¦`. No kernel freeze or test requirement changed.

## Required integration gate

Run warning-denying host/kernel Clippy, all host tests, Stage 11.2 on
1/2/4 CPUs and inherited acceptance before accepting the correction.

The bounded correction has passed the required integration gate above. Full
Stage 11 readiness remains open: negative exit-result encoding, compiler TLS,
loader interoperability, combined-tree integration and hardware qualification
remain separate prerequisites.
---

## Source: docs/audit-stage11-weak-symbols-2026-10-20.md

# Stage 11 weak symbol binding increment â€” 2026-10-20

## Scope

The bounded ELF symbol scope now recognizes `STB_WEAK` definitions. Strong
definitions are preferred over weak definitions during main-image-first
interposition; a weak definition is retained as the fallback when no strong
definition exists. An undefined, default-visible weak relocation resolves to
zero when the bounded dependency scope has no matching definition. Existing
symbol visibility, section, address-range, relocation-target, W^X/RELRO and
transactional owner-teardown checks remain in force.

Version-table metadata is still rejected before resolution. This increment
does not claim GNU symbol-version semantics, lazy binding, constructors or
destructors, library TLS, or a complete dynamic-linker ABI.

## Validation

- `cargo test --test elf_scope`: 30 passed, including strong-over-weak
  interposition and undefined-weak-zero behavior.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 11.5 QEMU passed with exit 33 on 1/2/4 CPUs:
  `audit-artifacts/stage11.5-1cpu-1792504013870714100`,
  `stage11.5-2cpu-1792504112883675500` and
  `stage11.5-4cpu-1792504158087902300`.

The required runtime markers are weak relocation, Ring-3 execution and owner
teardown; all passed on each CPU configuration.
---

## Source: docs/audit-stage12-1-2026-09-16.md

# Stage 12.1 audit â€” 2026-09-16

Added the `FileSystem` VFS 2.0 boundary and `SystemVfs` adapter over the
existing in-memory and disk-backed implementations. The boundary exposes typed
metadata, open, directory creation, and removal while preserving existing
generation and open-description rules. The structural probe verifies write,
stat, open/read, close, and unlink behavior.

Build, freestanding Clippy, and QEMU acceptance passed on 1, 2, and 4 CPUs.
Evidence is retained under `audit-artifacts/stage12.1-*`.
---

## Source: docs/audit-stage12-2-5-2026-09-16.md

# Stages 12.2â€“12.5 audit â€” 2026-09-16

Added bounded WovenFS metadata records with timestamps, modes and content
checksums; a volume-integrity envelope primitive; copy-on-write snapshot
catalog records; and a mount/partition inventory boundary for fixed and
removable media. Existing FAT32, GPT and MBR parsers remain the device-facing
validation layer. Stage 12.3's authenticated-encryption and bounded key-vault
follow-up is recorded separately in `audit-volume-crypto-2026-09-16.md`.

Each module has an individual feature and QEMU marker. All stages passed build,
freestanding Clippy, and QEMU on 1, 2, and 4 CPUs. Evidence is retained in
`audit-artifacts/stage12.2-*` through `stage12.5-*`. The integrity primitive
was an ABI boundary; authenticated encryption and key lifecycle now have their
own RFC-vector-backed implementation and acceptance gate. Measured provisioning,
Historical boundary note: at the time of this audit, measured provisioning,
persistent key storage, and mount integration were still open. Those kernel
boundary requirements were subsequently implemented and accepted in
`audit-volume-crypto-2026-09-16.md` (persistent authenticated wrapped-key
records, measured provisioning evidence, successor-generation rotation, and
encrypted mount-record I/O). TPM/firmware-backed measurement, physical
power-loss qualification, and deployment key-slot policy remain open.
---

## Source: docs/audit-stage12-acceptance-2026-10-13.md

# Stage 12 acceptance revalidation â€” 2026-10-13

## Evidence

`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-stage12-acceptance.ps1`
completed with:

- Stage 12.1: 1/2/4 CPUs passed, exit 33.
- Stage 12.2: 1/2/4 CPUs passed, exit 33.
- Stage 12.3: 1/2/4 CPUs passed, exit 33.
- Stage 12.4: 1/2/4 CPUs passed, exit 33.
- Stage 12.5: 1/2/4 CPUs passed, exit 33.
- Final marker: `=== STAGE 12 ACCEPTANCE: PASS ===`.

Evidence directories are retained under `audit-artifacts/` with the
`stage12.{1,2,3,4,5}-{1,2,4}cpu-179188...` prefixes created by this run.

## Limits

This revalidates the existing bounded Stage 12 contracts. It does not claim
that the native WovenFS implementation is the mounted system filesystem,
write-back from the VFS is complete, or that physical power-loss and native
DMA qualification have been performed.
---

## Source: docs/audit-stage12-metadata-rollback-2026-09-16.md

# Stage 12 metadata and rollback follow-up â€” 2026-09-16

WovenFS metadata now carries permission mode and four bounded extended-
attribute slots. Snapshot records expose restore state (generation and root
checksum) before deletion, enabling a rollback coordinator to restore a
validated generation. These additions are independent of the existing VFS
open-file lifetime rules.

Focused Clippy and QEMU tests passed on 1, 2, and 4 CPUs for WovenFS metadata
and snapshots. Native on-disk WovenFS replay and full data rollback remain
separate gaps in the production completion audit.
---

## Source: docs/AUDIT-STAGE13.10J-BLUETOOTH-ATT-GATT-ACCEPTANCE.md

# Stage 13.10J Bluetooth ATT/GATT Acceptance Audit

Date: 2026-10-01

## Accepted implementation

- Branch: `stage13.10j-bluetooth-att-gatt`
- Accepted implementation SHA: `48bdd085e8d8918bb058a59de06d42ce8ad7108a`
- Acceptance run: `36815325948`
- Result: PASS
- Release-validation artifact: `11141696246`
- Artifact digest: `sha256:9247004a999b8470e5431a2396d1d1237bd1cd278f43abdf9ebf9460ed0a3dca`

## Accepted contract

Stage 13.10J adds a bounded ATT/GATT foundation above live Bluetooth LE link ownership:

- fixed-capacity ATT attribute storage with nonzero handles and bounded values;
- ATT Read Request/Response and Write Request/Response with fail-closed permission enforcement;
- ATT Error Response for invalid handles, permissions and malformed/unsupported requests;
- stale LE connection handles rejected before ATT/GATT authority is granted;
- 16-bit GATT Primary Service and Characteristic Declaration/value representation;
- characteristic properties and value handles represented in the ATT database;
- bounded primary-service and characteristic discovery by handle range;
- per-LE-link, per-value-handle notification/indication subscription state;
- bounded Handle Value Notification/Indication construction;
- unsubscribe and LE disconnect revoke usable subscription authority.

## Validation

GitHub Actions run `36815325948` completed successfully at the exact accepted implementation SHA. The dedicated Stage 13.10J 1/2/4-core acceptance step passed and validation evidence was preserved.

Required runtime marker at the accepted implementation boundary:

`[S13.10J] Bluetooth GATT subscriptions: PASSED`

## Security and ownership properties

ATT/GATT does not create ambient authority from attribute handles. Operations recheck a live `LeLinkState` handle. Attribute permissions remain enforced at the ATT layer. Subscription authority is scoped to the LE connection handle and characteristic value handle. Disconnect causes subsequent transactions, discovery and subscription emission to fail closed.

## Explicitly outside this acceptance boundary

- physical BLE controller/radio ATT/GATT interoperability;
- ATT MTU exchange/negotiation;
- 128-bit UUID attribute representation;
- complete wire-level ATT discovery request/response procedures;
- indication confirmation tracking and retransmission policy;
- BLE SMP pairing, bonding and Long-Term Key lifecycle;
- persistent protected BLE key storage;
- LE privacy/resolving-list qualification;
- production timing, recovery and hardware interoperability.

## Next stage

Stage 13.10K should add BLE profiles and services above this accepted ATT/GATT authority boundary without weakening LE ownership or ATT permission checks.
---

## Source: docs/AUDIT-STAGE13.10K-BLE-PROFILES-SERVICES-ACCEPTANCE.md

# Stage 13.10K BLE Profiles and Services Acceptance Audit

Date: 2026-10-01

## Acceptance candidate
- Branch: `stage13.10k-ble-profiles-services`
- Last accepted functional checkpoint: `c5f3c6194c58ad16e75c0f44cf90c6d336393c40`
- Accepted checkpoint run: `36819330823`
- Checkpoint result: PASS
- Preserved artifact: `11142569874`
- Artifact digest: `sha256:677f88e58b878de73553225ec533f76c4d1b0e74864b1a69aaa30367caef50db`

## Accepted contract
- Device Information Service with bounded read-only Manufacturer Name and Model Number.
- Battery Service with read-only Battery Level bounded to 0..=100.
- Battery notification updates require the exact live LE handle plus exact subscribed value handle.
- Failed notification authorization does not mutate Battery Service state.
- Bounded WovenHat OS custom service with read-only status and write-only command characteristics.
- ATT/GATT permissions and live LE ownership remain authoritative.
- Characteristic creation preflights capacity for declaration and value attributes to avoid partial mutation.

## Validation
The final closure head must pass the dedicated Stage 13.10K runtime marker on 1/2/4 CPU validation and preserve evidence before merge.

Required marker:
`[S13.10K] WovenHat BLE service API: PASSED`

## Outside this boundary
- physical BLE radio/profile interoperability and certification;
- BLE SMP, bonding and LTK lifecycle;
- protected persistent BLE key storage;
- full standardized profile catalog;
- production controller reset/recovery and reconnect policy;
- hardware timing and transport-failure qualification.

## Next
Stage 13.10L Bluetooth lifecycle and hardening.
---

## Source: docs/AUDIT-STAGE13.10L-BLUETOOTH-LIFECYCLE-HARDENING-ACCEPTANCE.md

# Stage 13.10L Bluetooth Lifecycle and Hardening Acceptance Audit

Date: 2026-10-01

## Acceptance candidate
- Branch: `stage13.10l-bluetooth-lifecycle-hardening`
- Accepted stress checkpoint: `4c0b0e5e0c8d3b9e00b47dc58868a154c9c970ef`
- Stress run: `36825376710`
- Result: PASS
- Preserved artifact: `11145129234`
- Artifact digest: `sha256:67cc6f35203061ef8a79dfba3a8fef739db667bf7fb907b1eb989659c104f9e0`

## Accepted contract
- Valid LE disconnect removes the owned link and revokes all subscriptions for that handle.
- Malformed or stale/unknown disconnects fail before lifecycle mutation.
- Controller reset clears all LE link and GATT subscription authority.
- Successful disconnect/reset advances the lifecycle generation exactly once.
- Reconnect does not inherit prior subscription authority.
- Explicit re-subscription is required before notifications can resume.
- 32 bounded connect/subscribe/notify/teardown cycles leave no link or subscription leakage.
- Stale notification authority is rejected after every disconnect/reset.

## Required final runtime marker
`[S13.10L] Bluetooth lifecycle stress: PASSED`

The final closure head must pass the dedicated 1/2/4 CPU validation and preserve release evidence before merge.

## Outside this acceptance boundary
- physical Bluetooth controller or radio recovery;
- USB transport fault injection and hardware timing qualification;
- BLE SMP pairing, bonding and LTK lifecycle;
- persistent protected BLE key storage;
- physical BLE interoperability and standards certification;
- full ATT/GATT standards hardening such as MTU negotiation, fixed ATT CID integration, real CCCD attributes, UUID128 and indication confirmation tracking.

## Roadmap transition
After merge, roadmap Bluetooth Stage 13.10A-L software foundation is closed at its stated deterministic boundary. BLE security and physical hardware qualification remain explicit follow-on work before production Bluetooth claims.
---

## Source: docs/AUDIT-STAGE13.10R-BLE-SECURE-SESSION-IDENTITY.md

# Stage 13.10R â€” BLE secure-session identity binding

Date: 2026-10-01

## Scope

The LE security-session authority is bound to both the controller connection
handle and the peer address identity. ATT/GATT reads, writes, notifications and
indications now reject a stale secured session after a controller reuses a
connection handle for a different peer.

The failure mode is fail-closed: a handle-only session is not sufficient to
authorize a protected operation. The live `LeLinkState` entry must match the
session address type and address. Existing disconnect and controller-reset
teardown still revoke transient sessions, while this identity check protects
against missed cleanup and handle reuse.

## Verification

The exact source tip passed:

```text
cargo build --features stage13-10-test
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none \
  --features stage13-10-test -- -D warnings
python -m unittest discover -s tests -p 'test_*.py'
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 1 ...
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 2 ...
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 4 ...
```

The host suite ran 11 tests successfully. The QEMU gate exited 33 (the
harness success code) on all three CPU counts and emitted
`[S13.10R] Bluetooth LE secure-session identity binding: PASSED`, together
with all earlier Stage 13.10 markers. Evidence was retained under
`audit-artifacts/stage13.10-*`.

This is deterministic software acceptance. Physical BLE controller/radio
interoperability, transport fault injection, SMP/LTK provisioning and hardware
qualification remain open.
---

## Source: docs/AUDIT-STAGE13.10S-BLE-OUTBOUND-KEY-SIZE.md

# Stage 13.10S closure â€” BLE minimum key size on outbound GATT

Date: 2026-10-05

The Stage 13.10S minimum encryption-key-size policy now applies consistently
to protected ATT reads/writes and to secured notification/indication emission.
A live authenticated session with a key below the policy threshold is rejected
before an outbound value is encoded. A session at the configured threshold is
allowed only after the identity-bound session lookup succeeds.

Verification on the exact source tip:

```text
cargo build --features stage13-10-test
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none \
  --features stage13-10-test -- -D warnings
python -m unittest discover -s tests -p 'test_*.py'  # 11 passed
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 1 ...  # PASS
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 2 ...  # PASS
python scripts/test-stage10-runtime.py --stage 13.10 --cpus 4 ...  # PASS
```

The QEMU harness exited with its expected success code 33 and retained serial
evidence under `audit-artifacts/stage13.10-*`. This remains deterministic
software validation; physical BLE controller/radio interoperability, SMP/LTK
provisioning and hardware qualification remain open.

---

## Source: docs/AUDIT-STAGE13.10T-BLE-OUTBOUND-KEY-SIZE-GATE.md

# Stage 13.10T â€” outbound BLE key-size authorization gate

Date: 2026-10-05

The Stage 13.10 runtime acceptance now requires
`[S13.10T] Bluetooth LE notification minimum key size: PASSED`. A dedicated
self-test rejects an authenticated 12-byte session when the protected
subscription requires 16 bytes, then accepts a 16-byte session and verifies
the encoded notification.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
the Stage 13.10 QEMU gate on 1/2/4 CPUs. The QEMU harness exited with its
expected success code 33 and retained serial evidence under
`audit-artifacts/stage13.10-*`. Physical BLE interoperability, SMP/LTK
provisioning and hardware qualification remain open.

---

## Source: docs/AUDIT-STAGE13.11A-BLE-SMP-FOUNDATION.md

# Stage 13.11A â€” BLE SMP pairing and fixed-channel foundation

Date: 2026-10-05

The BLE Security Manager Protocol foundation is bounded above live LE link and
L2CAP ownership. It validates Pairing Request and Pairing Response fields,
enforces the supported key-size range, rejects unsupported authentication and
key-distribution bits, and accepts SMP traffic only on fixed L2CAP CID `0x0006`
for a live connection handle. Pairing state is bound to that handle and
advances only from validated request to negotiated parameters.

The dedicated Stage 13.11 gate passed on 1/2/4 CPUs with exit code 33 and
retained evidence under `audit-artifacts/stage13.11-*`. This remains a
deterministic foundation; cryptographic key derivation, user interaction,
bonding persistence, controller transport and hardware qualification remain
open.

---

## Source: docs/AUDIT-STAGE13.11B-BLE-SMP-LTK-DISTRIBUTION.md

# Stage 13.11B â€” BLE SMP LTK distribution

Date: 2026-10-05

The SMP foundation now parses legacy Encryption Information and Master
Identification PDUs, validates their lengths and key material, and stores the
result in the canonical address/type-keyed `LeBondStore`. Distribution is
accepted only for the live pairing handle and negotiated key size; malformed,
stale, duplicate or mismatched identity inputs fail closed.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
the dedicated Stage 13.11 1/2/4-CPU QEMU gate with exit code 33. Evidence is
retained under `audit-artifacts/stage13.11-*`. Cryptographic derivation,
user-confirmed pairing, controller transport and hardware qualification remain
open.

Closure hardening binds pending distribution to the live peer address type and
address. After handle reuse for another peer, stale Encryption Information
cannot be completed into a bond; duplicate pending distribution is rejected
and sensitive pending material is cleared on abort.
---

## Source: docs/AUDIT-STAGE13.11C-BLE-SMP-CONFIRM-RANDOM.md

# Stage 13.11C â€” BLE SMP confirm/random verification boundary

Date: 2026-10-05

The SMP security boundary validates legacy Pairing Confirm and Pairing Random
PDUs against negotiated pairing state, peer address context and a temporary
key through the `c1` confirm function. The `s1` STK derivation boundary is also
exercised. Confirmation state is handle-bound and transitions to `Failed` on a
mismatch; malformed, stale or out-of-order inputs fail closed.

The exact tip passed warning-denying kernel Clippy, 11 host tests, and the
dedicated Stage 13.11 1/2/4-CPU QEMU gate with exit code 33. Evidence is
retained under `audit-artifacts/stage13.11-*`. Production pairing UX,
authenticated key-derivation integration, bonding persistence, controller
transport and hardware qualification remain open.

---

## Source: docs/AUDIT-STAGE13.11F-BLE-ENCRYPTION-AUTHORITY.md

# Stage 13.11F â€” BLE controller encryption/session authority

Date: 2026-10-05

The SMP flow owns the bounded LE Start Encryption command, preserves the live
peer identity and negotiated key size in a pending-encryption record, and
accepts controller Encryption Change only when the completion event matches
that record. Failure, disabled encryption, stale handles and peer identity
changes revoke authority instead of creating a secured session.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
QEMU 1/2/4 CPU acceptance with exit code 33. Physical controller transport,
SMP interoperability, bond persistence and hardware qualification remain open.

---

## Source: docs/AUDIT-STAGE13.11G-BLE-IDENTITY-PRIVACY.md

# Stage 13.11G â€” BLE identity/privacy foundation

Date: 2026-10-05

The BLE SMP identity foundation validates Identity Information and Identity
Address Information distribution, retains bounded IRK-backed identities, and
resolves private addresses only against stored identity material. Duplicate
identity replacement is deterministic and malformed or stale distribution is
rejected. The pending LTK distribution remains peer-bound, so privacy
resolution cannot bypass connection ownership.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
QEMU 1/2/4 CPU acceptance with exit code 33. Physical privacy-list hardware
behavior, SMP interoperability, persistent protected keys and qualification
remain open.

---

## Source: docs/audit-stage13-10ac-2026-09-23.md

# Stage 13.10AC handoff audit â€” 2026-09-23

Historical handoff findings and results; see the [subsequent remediation](audit-stage13-10ac-remediation-2026-09-23.md) for current status.

Baseline: `0563d0bb21700f8f9104ef3d97089b78a8a119ca`, branch
`stage13.9-wifi`, verified against origin at the start of this audit.

## Scope and acceptance boundary

The historical commits use 13.10Aâ€“AC for AX200 Wi-Fi development. These are
extensions of roadmap Stage 13.9; the master roadmap's Stage 13.10 remains
Bluetooth. This audit does not authorize a later roadmap stage or claim that
either production Wi-Fi or Bluetooth is complete.

The AC implementation joins the global IRQ work flag to the existing Intel
CSR interrupt controller. It is a bounded synthetic service boundary, not a
running hardware receive driver. `wifi_hw` is compiled only with
`stage13-9-test`, and the only service callers are its self-test. No scheduler
worker, wake registration, RX-ring draining, or ALIVE delivery is connected
to this function. The self-test runs before AP startup: 1/2/4-CPU boot success
does not establish concurrent AX200 interrupt-service qualification.

## Findings

1. The original Python acceptance checker required only the old 13.9H marker
   for the Wi-Fi suite. The launcher's 13.10AC PASS message therefore did not
   prove execution of AC or its intervening prerequisites.
2. The handoff commit contained no stage-status, architecture, or audit update.
   This checkout contained no retained `audit-artifacts` directory. Prior test
   results cannot be reconstructed from the commit title.
3. A single global coalescing flag has no device identity or generation. The
   current test is single-consumer; multi-device ownership, detach, cancellation,
   rebind, and stale interrupt rejection are not implemented by this boundary.
4. The hard IRQ publishes work and performs LAPIC EOI without allocation or
   locking. The deferred function atomically claims work before CSR service.
   A later publication remains pending; multiple earlier publications coalesce.
   Error results consume the claimed publication and propagate to the caller;
   fatal CSR errors leave delivery masked. Runtime recovery remains open.
5. The synthetic page verifies values written for acknowledgment; ordinary RAM
   does not model hardware write-one-to-clear behavior or DMA ordering. No
   physical AX200 was supplied to this audit.
6. The full Windows preservation launcher failed at host lint because the
   unconditional `wifi_session`/`wifi_smol` modules and network adapter imported
   feature-gated Wi-Fi dependencies. Correcting this configuration exposed 36
   warning-denying default-build errors in inherited code (unused entropy,
   PCI/MSI, interrupt, input, and audio items). These are acceptance blockers;
   feature-enabled success does not resolve them.
7. `IntelRxInterruptController::disable` cleared the CSR mask but retained the
   configured RX mask. Deferred work claimed after disable could restore RX
   delivery. The fix clears the saved mask after a successful hardware-mask
   write; a queued-work test requires the disabled mask and cause to remain
   unchanged, with no RX work reported. Explicit `enable_rx` is still available.
8. `cargo test --features stage13-9-test` also fails: the host-included heap
   module compiles its `qemu-test` runtime-growth helper, which calls
   `maintain_capacity` even though that function is excluded under `cfg(test)`.
   This is a host/QEMU configuration mismatch, separate from AC. Default
   `cargo test` is already blocked by the default kernel build errors.

## Audit changes

- The runtime harness now pins all 54 existing Wi-Fi success markers, requires
  the requested CPU count and exit 33, and rejects failure/panic/timeout markers.
  Five host regressions reject incomplete or contradictory evidence, including
  the former 13.9H-only false-positive case.
- AC's self-test verifies untouched idle CSR state, publication coalescing,
  acknowledgment/mask writes, disable with pending work, and propagation of
  hardware and firmware errors.
  It does not claim concurrent producer coverage or hardware W1C simulation.
- `main.rs` and `network.rs` now consistently feature-gate the experimental
  Wi-Fi adapter and its session dependencies. Other configurations use the
  existing VirtIO device directly. No Wi-Fi test or existing assertion was
  disabled. An unused `Box` import was removed.
- The focused launcher uses the project-local Cargo directories, runs host
  harness regressions, and labels its result as bounded synthetic acceptance.
  Status and architecture documentation record the scope and outstanding gates.

Files added: this audit and `tests/test_runtime_harness.py`. Files modified:
`kernel/src/main.rs`, `kernel/src/network.rs`, `kernel/src/wifi_hw.rs`,
`scripts/test-stage10-runtime.py`, `run-stage13-9-acceptance.ps1`,
`docs/architecture-and-codebase-guide.md`, and `docs/stage-status.md`.

## Security and concurrency review

AC adds no syscall, capability, userspace pointer, queue, or asynchronous user
buffer. WovenGuard policy and authority checks are unchanged. The synthetic
page is kernel-owned and retained through service. The
service takes an exclusive controller borrow, adds no lock nesting, allocation,
blocking, or task switch. Its unsafe MMIO construction follows the existing
page-owned test pattern. Atomic flag ordering is not proof of device DMA
visibility. Production integration must establish device lifetime, exclusive
consumer ownership, interrupt quiescence on teardown, event-driven wakeup,
bounded RX draining, and fatal-error recovery before enabling hardware traffic.

## Validation

Full acceptance is blocked. The final-source focused gate passed build,
warning-denying kernel probe Clippy, nine Python tests, and all 54 Wi-Fi markers
on 1/2/4 CPUs with exit 33. Host Clippy with `stage13-9-test` also passed. This
does not repair the default-build failures: the probe configuration already
allows unused imports/dead code in `main.rs`; this audit added no warning
suppression.

Baseline output is retained in `audit-artifacts/stage13-10ac-baseline.txt`.
Final focused output and static failure diagnostics are in
`audit-artifacts/stage13-10ac-review-1790155254234949000/`; `results.json` records
commands and exit codes. Per-boot evidence is retained in timestamped runtime
harness directories. All 21 selected prior-stage boots passed on 1/2/4 CPUs:
memory/boot, storage, live DHCP/DNS/ICMP/UDP/TCP, Stage 10.8 completion ports,
Stage 10.9 timers/events, Stage 13.1 driver framework, and Stage 13.2 PCI.
Together with the three final AC boots this is **24/24 QEMU boots, exit 33**.

All eight standalone Rust host suites compiled and passed (24 tests), using
the release harness's `rustc --test` method and the project-local
`.cargo-build/debug/deps` dependency directory. Commands, outputs, and results
are in `audit-artifacts/stage13-10ac-host-1790156328744714100/`. This establishes
host-unit results separately; neither failed Cargo test command is counted as
passing. `git diff --check` passed.

| Gate | Result |
| --- | --- |
| Focused Wi-Fi build, host/kernel probe Clippy | Pass |
| Python harness regressions | 9 passed |
| Standalone Rust host suites | 24 passed |
| Final AC QEMU, 1/2/4 CPUs | 3 passed |
| Selected prior QEMU probes, 1/2/4 CPUs | 21 passed |
| Default build and default host/kernel lint | Failed: inherited unused-code errors |
| Default `cargo test` | Failed during kernel build |
| `cargo test --features stage13-9-test` | Failed: host/QEMU heap configuration mismatch |
| Full Windows Stage 10.7 preservation | Failed at host lint, before its QEMU chain |
| Complete release and remaining prior-stage matrices | Not completed; full acceptance blocked |

Final AC boot evidence:

- `audit-artifacts/stage13.9-1cpu-1790155303182005100/`
- `audit-artifacts/stage13.9-2cpu-1790155354456442600/`
- `audit-artifacts/stage13.9-4cpu-1790155402860336900/`

## Next prerequisites

Repair default-build integration without silencing warnings, repair the
host/QEMU test configuration mismatch, and pass the complete required
preservation/release matrix. Runtime AX200 integration additionally requires
the lifecycle, wakeup, RX, and physical qualification work described above.
No later stage is accepted by this audit, and there is no accepted-stage commit
for these local audit changes.
---

## Source: docs/audit-stage13-10ac-continuation-2026-09-23.md

# Stage 13.10AC continuation audit â€” 2026-09-23

Baseline: `0563d0bb21700f8f9104ef3d97089b78a8a119ca` (clean working tree).
No retained acceptance artifacts were present at the start of this checkout.

## Final result on 2026-09-24

Preservation and bounded candidate acceptance passed: 144 QEMU boots,
24 Rust host tests, nine Python harness tests, ordinary build, normal release
build, host/freestanding-kernel Clippy, and per-feature freestanding Clippy.
This is a validated continuation, not production completion of Stage 13.

| Evidence | Result |
| --- | --- |
| `audit-artifacts/acceptance-20260924-021102-008/acceptance-output.txt` | Full Stage 10.7 gate: PASS, 75 boots, exit 0 |
| `audit-artifacts/continuation-matrix-20260924-114937/results.json` | 54 runtime boots through 13.5 and corresponding lint passed; original 13.6 lint failure retained |
| `audit-artifacts/continuation-matrix-20260924-120822/results.json` | Corrected 13.6 through 13.10AC: 12 runtime boots and lint passed; 2 hotplug boots passed; interrupted release-build attempt retained |
| `audit-artifacts/release-shell-20260924-155123/` | Normal release shell, PS/2 IOAPIC and SMP smoke: PASS; serial/QEMU logs and exit 0 retained |

The only kernel change after the full preservation pass removed the
misplaced HDA check inside `stage13-6-test`; unaffected configurations were
not redundantly rerun. The affected HID feature passed lint and 1/2/4-CPU
boots afterward, as did input, audio and Wi-Fi. Wi-Fi acceptance required all
54 markers and exit code 33 on each CPU count. Earlier failed logs were not
deleted or overwritten.

Files added: this audit and `tests/test_runtime_harness.py`. Files modified:
`main.rs` and `network.rs` for Wi-Fi feature consistency/HID error reporting;
`entropy.rs`, `hal/pci.rs`, `interrupts.rs`, `smp.rs`, `device.rs`, `shell.rs`,
and `woven_input.rs` for candidate boundaries; the Stage 13.9 launcher and
runtime/shell harnesses; architecture and status documents.

No syscall or external ABI change, new unsafe code, capability grant,
WovenGuard policy change, lock, or kernel object is introduced. Existing
ownership, generation, capacity, cancellation, teardown, local-preemption
and cross-CPU synchronization paths are preserved. Candidate input variants
have no ordinary-build producers and no exposed userspace struct ABI.

Remaining prerequisites: a production AX200 worker and lifecycle owner,
secure entropy provisioning, real firmware startup/interrupt delivery/DMA
quiescence/RF qualification, and the earlier roadmap production gaps. The
synthetic CSR test does not emulate hardware write-one-to-clear semantics.
The existing QEMU feature's broad lint allowances were not expanded; a
warning-denying invocation is not evidence that those inherited allowances
have been eliminated. No later stage is authorized by this continuation.

## Findings and changes

The Stage 13.10AC launcher delegated to the shared Stage 13.9 harness, which
required only the old 13.9H Wi-Fi marker. A successful QEMU exit with that
older marker could therefore pass without evidence of the newer tests.
The harness now explicitly requires all 54 existing Wi-Fi success markers
through 13.10AC, reports missing markers, and retains exit-code validation.
The marker list is static; it is not generated from the kernel during testing.

New host regressions exercise the actual harness entry point with mocked
build/QEMU processes and temporary serial files. They reject every individual
missing Wi-Fi marker, the old marker-only evidence set, and a bad QEMU exit.
The focused PowerShell launcher now reuses project-local Cargo directories.

The first preservation run failed host Clippy while building the kernel:
`wifi_session` was included in ordinary builds while four of its required
modules were compiled only with `stage13-9-test`. The network module also
imported an unused `Box`. Inspection found the Wi-Fi transport constructor
used only by its acceptance probe. `wifi_session`, `wifi_smol`, and the Wi-Fi
transport wrapper now share the backend's existing feature boundary. Ordinary
builds use `VirtioSmolDevice` directly; the candidate selector remains available
to its existing Wi-Fi acceptance tests. The unused import is removed.

No kernel ABI, syscall, capability, WovenGuard policy, lock, or kernel object
is added in this continuation. The architecture guide records the
feature-gated source and missing runtime-worker/lifecycle integration. The
existing synthetic CSR test cannot qualify real AX200 interrupt delivery,
firmware startup, DMA quiescence, or RF operation.

## Initial validation on 2026-09-23

The nine Python harness regressions passed, including rejection of each
missing Wi-Fi marker. The initial full preservation attempt failed at host
lint; its transcript is retained under
`audit-artifacts/acceptance-20260923-211805-671/acceptance-output.txt`.
The next preservation run passed all nine Python regressions and resolved the
import errors, but failed at host lint with 36 warning-denied unused-code
diagnostics. Evidence:
`audit-artifacts/acceptance-20260923-212851-951/acceptance-output.txt`.
The remaining errors concern candidate audio, secure entropy, PCI/MSI, IRQ,
and input APIs with no ordinary-build consumers. They are not suppressed.
The chain stopped before Rust host tests and preservation QEMU boots.

The incomplete initial focused Wi-Fi validation was recorded in
`audit-artifacts/wifi-continuation-20260923-213321-837/acceptance-output.txt`.
Full stage acceptance has not been established.
Do not treat this harness correction as completion of Stage 13.10AC or as
authorization to skip earlier unfulfilled roadmap requirements.

## Continuation on 2026-09-24

The 36 ordinary-build unused-code diagnostics were traced to candidate-only
APIs outside their callers' feature boundaries. Secure entropy/MSI candidate
modules, driver-specific PCI enablement, audio registration, and extended
input event candidates now follow those boundaries. Two unused private PCI
write wrappers were removed. Ordinary VirtIO traffic, keyboard bytes, the
reserved interrupt handler, and candidate test behavior are preserved.
No warning suppressions, capability grants, or timeout changes were added.

Host Clippy with `-D warnings` passed. The full preservation rerun is in
`audit-artifacts/acceptance-20260924-021102-008/acceptance-output.txt`;
it passed completely with exit code 0 and `STAGE 10.7 ACCEPTANCE: PASS`.
Results: build; host/kernel warning-denying lint; 24 Rust and nine Python
host tests; 60 memory/SMP boots (10/30/20 on 1/2/4 CPUs); three live network
boots; and 12 isolated async block/file/UDP/TCP boots. All 75 QEMU boots
passed. The later-stage results follow below.


The subsequent matrix passed stages 1-5, 10.8-10.9, 11.1-11.5,
12.1-12.5, and 13.1-13.5 (per-feature freestanding Clippy and 1/2/4-CPU
QEMU), then failed Stage 13.6 lint. A duplicated HDA topology check had been
inserted in the USB HID report-error branch, referencing the disabled audio
module. The misplaced duplicate was removed; the original audio topology
check remains in Stage 13.8, and HID errors still fail with their original
error report. The change is inside `stage13-6-test` and does not affect
previously tested feature configurations. Failure evidence is retained in
`audit-artifacts/continuation-matrix-20260924-114937/13.6-clippy.log`.
Validation resumed at Stage 13.6 after repeating normal kernel lint.


The resumed matrix passed Stage 13.6-13.9 Clippy and all 1/2/4-CPU boots,
including every Wi-Fi marker, followed by 2/4-CPU hotplug. Evidence is in
`audit-artifacts/continuation-matrix-20260924-120822/`. The release shell
build then blocked in the bootloader dependency's nested `cargo install`.
After identifying the exact installer PID and parent, only that installer
was stopped; no QEMU process was stopped. Its captured stderr identified
an artifact-lock wait on `target/release/.cargo-artifact-lock`, held by the
outer release build. The interrupted build is retained as a failed attempt;
no shell boot had started.

`scripts/test-shell-qemu.py` now passes the main target/build directories as
Cargo options and gives nested installer processes a separate project-local
`target/bootloader` artifact directory. The parent's explicit `--target-dir`
is necessary: `--config build.target-dir` loses to `CARGO_TARGET_DIR` and
was rejected by the first retry. Build intermediates remain project-local. This
avoids recursive release-lock acquisition without changing dependencies,
profiles, kernel code, test assertions, or timeouts. The first retry is preserved in
`audit-artifacts/release-shell-20260924-154928/`. The corrected invocation
passed in `audit-artifacts/release-shell-20260924-155123/` (exit code 0),
using cached locked dependencies with `CARGO_NET_OFFLINE=true`. It built the
normal release image and passed PS/2 IOAPIC input plus the shell SMP test.
The offline setting was local to this validation invocation, not committed
as a dependency or network-policy change.
---

## Source: docs/audit-stage13-10ac-firmware-parser.md

# Stage 13.10AC firmware parser follow-up

Baseline: `42f5f11`, branch `stage13.9-wifi`, 2026-09-23. This closes additional
software defects found after the deferred-worker acceptance. Historical AC
remains part of Wi-Fi roadmap Stage 13.9; no later stage is started.

## Defects and changes

The original parser interpreted TLV 32 as an addressed data section, rejected
valid four-byte paging metadata, skipped secure runtime/init types 24/25, and
could expose separator words as executable payloads. Its 16-section generic
image limit also rejected the selected upstream AX200 container's 48 sections.

`kernel/src/wifi_firmware_tlv.rs` now holds the allocation-free parser used by
both kernel probes and host tests. Paging metadata has an exact four-byte
length, 4-KiB alignment and 1-MiB bound; duplicate declarations fail closed.
Secure and plain runtime/init sections share parsing. CPU and paging separators
advance independent per-image LMAC/UMAC/paging groups and are never returned as
data sections. Malformed, repeated, empty-group and dangling delimiters fail.
The reviewed delimiter profile accepts an offset-only marker or one zero word.

Whole-image size, section size, address overflow, truncated records, padding,
empty containers and section capacity are checked before exposing a result.
The Intel parser's 64-entry limit matches the existing owned DMA buffer limit;
the generic `FirmwareImage` contract remains 16. No DMA allocation limit,
hardware identity, capability, or automatic device activation is expanded.
Parsing a container is not authorization to load it: device/firmware selection,
command ABI validation and signature verification remain loader requirements.

The unchanged upstream binary in `tests/fixtures/iwlwifi` is pinned by SHA-256,
with its redistribution license and provenance. Tests inspect its documented
container structure; no instruction decoding or firmware modification occurs.
The aggregate source manifest now hashes test fixtures as well as source.

## Ownership and concurrency

The parser uses only safe Rust, fixed arrays and immutable slices borrowed from
the caller. It performs no asynchronous work, takes no locks and creates no
kernel handle or DMA object. Rust lifetimes prevent these borrowed sections
from outliving their input. A future asynchronous loader must copy them into
owned DMA storage and quiesce the device before teardown. Existing worker
epochs and shutdown limitations are unchanged.

## Evidence and limits

Seven new host tests cover the upstream image, secure/plain interleaving,
paging metadata, delimiters, capacity and input/range bounds. The kernel's
existing Stage 13.10L probe now also checks secure sections, paging metadata
and group transitions. A Python test verifies fixture size and hash.
The final-source aggregate is blocked by host memory exhaustion; this follow-up
is not full stage acceptance.

The first follow-up aggregate, `audit-artifacts/ac-full-1790188788583138000`,
passed build, strict lint, both host-test configurations and Python tests, then
failed before the first Stage 10.7 preservation guest boot. QEMU could not
allocate a 1,026,486,272-byte JIT buffer under host commit pressure (Windows
reported its paging file was too small). The complete failure is retained.
All eight QEMU test launchers now explicitly bound the TCG translation cache
at 128 MiB using `-accel tcg,tb-size=128`, as documented in
[QEMU's accelerator options](https://www.qemu.org/docs/master/system/invocation.html).
Guest RAM, SMP matrices, TCG threading defaults, workloads, assertions and
timeouts are unchanged. This changes host resource use, not the acceptance
criteria. No unrelated application was terminated or host paging setting changed.

A four-CPU memory/boot check then passed with the bounded cache. The next
aggregate, `audit-artifacts/ac-full-1790189175727349400`, again passed its first
six checks but failed while creating a nested PowerShell process (`Thread
failed to start`), before any preservation guest boot. The prior-stage chain
retained 27 nested PowerShell processes. Its calls now invoke the same child
scripts through `& "$PSScriptRoot\..."` in separate script scopes within one
process. Exceptions and native exit-code checks still propagate failure;
test bodies, stage ordering and repetition counts are unchanged. PowerShell
syntax validation passed. Both unsuccessful aggregate runs remain evidence,
not accepted gates.

The final attempt, `audit-artifacts/ac-full-1790189677915188900`, passed default
build, warning-denying host/kernel Clippy, all 34 Rust host tests in each of the
default/Wi-Fi configurations, and all 10 Python tests. Its Stage 10.7 chain
reached the first memory boot in one PowerShell process, but QEMU failed with
`cannot set up guest memory 'pc.ram'`. The guest never started. This failure
propagated correctly through the revised chain. All three aggregate attempts
stopped at the failed gate; no later-stage pass is inferred.

Focused Wi-Fi acceptance passed on 1/2/4 CPUs before the launcher resource
changes (`audit-artifacts/firmware-parser/focused-1.txt`), and the four-CPU
memory/boot suite passed after the TCG cache bound was applied. These focused
passes are not a substitute for the blocked final aggregate. Kernel/parser
source was unchanged between that focused run and the final aggregate.

Resume prerequisite: free host memory/commit capacity, then rerun
`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\RUN-STAGE13.10AC.ps1`
as the only acceptance chain, using the project-local Cargo directories.
Do not shrink guest RAM, change assertions or increase timeouts to work around
the host shortage. Physical AX200 integration and qualification remain separate
requirements even after the software aggregate succeeds.

This is container-parser acceptance only. Physical RX descriptor integration,
device-written ALIVE, firmware ABI policy, DMA shutdown and actual AX200 MSI/DMA
qualification remain open. The available host is AX201; adding its PCI ID would
not establish AX201 support. No successful physical firmware boot is claimed.

## Format references

Reviewed against Linux v6.12's documented Intel container interface:

- [TLV definitions](https://github.com/torvalds/linux/blob/v6.12/drivers/net/wireless/intel/iwlwifi/fw/file.h)
- [Paging and secure-section parsing](https://github.com/torvalds/linux/blob/v6.12/drivers/net/wireless/intel/iwlwifi/iwl-drv.c)
- [Paging limits](https://github.com/torvalds/linux/blob/v6.12/drivers/net/wireless/intel/iwlwifi/fw/img.h)
- [Context-info section groups and DMA lifetime](https://github.com/torvalds/linux/blob/v6.12/drivers/net/wireless/intel/iwlwifi/pcie/ctxt-info.c)

The implementation is local safe Rust; those references establish format facts.
---

## Source: docs/audit-stage13-10ac-remediation-2026-09-23.md

# Stage 13.10AC software remediation â€” 2026-09-23

Baseline: `0563d0b` on `stage13.9-wifi`. This report follows the
[handoff audit](audit-stage13-10ac-2026-09-23.md). The historical AX200 labels
13.10Aâ€“AC extend Wi-Fi roadmap Stage 13.9; Bluetooth Stage 13.10 is unaffected.

## Scope and implementation

The default build previously included half of several experimental subsystems.
Their compile boundaries now match their consumers: the secure-entropy
prototype and MSI programming belong to the Wi-Fi probe, non-keyboard input
variants belong to Stage 13.7, and audio registry support belongs to Stage 13.8.
The existing default VirtIO and keyboard paths remain available. PCI MMIO/DMA
enablement is compiled with its NVMe/AHCI/USB/HDA/Wi-Fi consumers. No warning
allowance, test assertion, or existing acceptance timeout was weakened.

The heap runtime-growth boot probe requires the kernel global allocator and
real paging. It is now excluded from host `cfg(test)` compilation, while all
host HeapState tests and QEMU growth assertions remain active. Host Clippy
across all targets also required an equivalent `Option::filter` formulation
of one allocation condition (compatible with kernel Rust 2021 and host Rust
2024) and relocating the host lock-order shim after non-test items.

The existing CSR controller now clears its saved RX mask on disable. Deferred
work cannot restore a disabled mask. Its self-test checks untouched idle
registers, coalescing, pending work at disable, acknowledgment writes, and both
fatal-error results.

`wifi_runtime.rs` adds an event-driven kernel worker to the opt-in Wi-Fi build.
The real vector-0xd0 handler publishes work, performs LAPIC EOI, then signals
the worker through the scheduler's event latch. The worker owns one bounded
subscription and one coalescing completion, services the existing CSR helper,
and notifies the owner after dropping the slot guard. It sleeps without any
lock held. A fatal error retires the subscription and disables the controller;
further work cannot re-enable it.

`irq_mailbox.rs` packs the active epoch and pending bit into one atomic word.
The owner allocates monotonically increasing epochs and refuses exhaustion.
Claiming consumes only that epoch's pending bit. Retirement invalidates queued
work; producers holding an old epoch cannot publish into a replacement slot.
Host tests exercise four concurrent producers racing claim, retirement, and
rebinding. This is a software subscription protocol, not a claim that an
untagged hardware MSI contains a generation.

The post-SMP QEMU probe owns a synthetic CSR page, starts the service worker,
and invokes the installed interrupt handler from a task pinned to the last
online CPU. It checks that CPU identity, receive-cause delivery, fatal-error
masking, stale epochs, bounded attachment, and owner-exit reclamation work
through real scheduler blocking and wakeup. Early synthetic tests remain.

## Ownership, security, and concurrency

The rank-20 slot lock serializes CSR access and retirement. No controller or
page reference escapes service. The backing frame is released after the slot
guard is dropped. Kernel-task exit and both normal/forced process teardown
paths call owner cleanup. Scheduler notifications occur after the slot unlock,
so scheduler rank 10 may enter cleanup without an inverse device-to-scheduler
lock path. The IRQ handler takes no device lock. IRQ mutex holders mask local
interrupts; the sole IRQ-enabled preemption mutex is the rank-10 terminal
guard, which permits the rank-10 scheduler wakeup path.

The worker and owner retain no CPU-local mutable state across blocking. The
coalescing completion retains observed status/acknowledgment bits and sticky
errors; it does not buffer packets or claim DMA completion. No syscall, user
pointer, userspace ABI, capability grant, or WovenGuard policy changed. The
only new assembly invokes an initialized kernel interrupt vector for testing.

Discovery no longer enables PCI bus mastering, and the early boot contract
probe no longer programs physical MSI before RX ownership and the service
owner exist. The explicit binding helpers remain available to the opt-in
transport. Hardware activation must follow a reviewed DMA-ready attachment.

## Acceptance tooling

The focused checker requires all 55 Wi-Fi markers, including the worker probe,
the requested CPU count, and exit 33. Missing markers and contradictory failure
logs are tested by host regressions. `RUN-STAGE13.10AC.ps1` runs a serial full
software chain with project-local Cargo directories, an OS-released lock,
unique evidence directories, source hashes, commands, return codes, and copies
of logs from legacy fixed output paths. It includes the required full Stage
10.7 launcher, release matrix, 2/4-CPU hotplug, journal, completion/timer,
process/thread/runtime, storage, and driver gates through AC. Original stage
launchers retain their existing timeouts and assertions.

The release harness now delegates host-test dependency resolution to Cargo;
it no longer guesses an rlib path under `target/debug/deps`, which is wrong
when the project-local Cargo build directory is enabled.

The release gate exposed a separate nested-Cargo deadlock: bootloader 0.11.17's
UEFI build script runs `cargo install` while inheriting the parent's target
directory. Its child waits for `target/release/.cargo-artifact-lock`, held by
the parent waiting for that child. The initial registry connection was not
the whole cause; an offline preflight reproduced the lock wait. Both attempts
were stopped only by terminating their identified installer child, allowing
the harness to record failure and retain diagnostics.

`vendor/bootloader` retains the published 0.11.17 package build inputs and
licenses. Its sole code patch assigns the nested UEFI installer separate target
and intermediate directories under `OUT_DIR`; no runtime or image-generation
code changes. The root Cargo patch selects this dependency, and the acceptance
manifest hashes its source and manifests. See its `WOVENHAT-PATCH.md` for the
upstream commit and crate checksum. The final rerun uses cached dependencies
with `CARGO_NET_OFFLINE=true`; this does not change acceptance assertions.

## Files and interface changes

Added: `kernel/src/irq_mailbox.rs`, `kernel/src/wifi_runtime.rs`,
`kernel/src/hal/pci/msi.rs`, `tests/irq_mailbox.rs`,
`tests/test_runtime_harness.py`, `scripts/test-stage13-10ac.py`,
`RUN-STAGE13.10AC.ps1`, and both AC audit documents.
Also added: the narrowly patched `vendor/bootloader` dependency with provenance.

Modified: feature-owned declarations in device, entropy, PCI, SMP, interrupt,
input, shell, and network modules; kernel initialization and task teardown;
heap host/boot configuration and lock shim placement; Wi-Fi CSR tests and
disable/discovery behavior; runtime/release harnesses, the focused launcher,
and architecture/status documentation. MSI code was moved without changing
its programming protocol. There are no new syscall numbers or capabilities.
Root Cargo configuration and lockfile select the same bootloader version with
the build-directory patch.

## Preservation failure: queued output lost at close

The subsequent complete run `ac-full-1790160662768434100` passed the 60 memory
stress boots and preservation through Stage 10.6, then failed the four-CPU
Stage 10.7 host receive with a socket timeout. Its one/two-CPU cases passed.
A focused four-CPU retry passed, which did not explain or resolve the failure.

Review found that `socket_close`, owner exit, and the last async unpin could
remove a smoltcp socket while its send buffer still held data. Completion of
the async send means the bytes were queued; it does not mean a network poll
has transmitted them. The original close-before-completion test could lose
its first payload if descriptor consumption/unpin beat network polling.

Closing entries now retain their existing bounded slot and generation until
TCP output is acknowledged (or the connection closes), or queued UDP output
is dispatched. TCP close starts only after the final async pin is released.
An unresponsive peer has a 30-second drain bound; regular polling reaps the
entry. Closing descriptors reject new operations, and all mutations remain
under the existing rank-20 runtime lock. No sleep, scheduling, new allocation,
slot expansion, or capability change was added. Socket statistics continue to
count draining entries; teardown assertions are unchanged. The probes now
wait for both requests and sockets to drain within the original 200-tick
cleanup deadline.

The Stage 10.7 build deliberately withholds polling the first queued TCP
payload until its descriptor is closed and the final pin is released. The
acceptance predicate requires observing that queued close. Thus the existing
host-verified two-connection exchange exercises the failing order on every
run, rather than relying on a lucky scheduling race. Other builds have no such
polling interlock.

## Preservation failure: USB error path referenced HDA

Run `ac-full-1790162261267779600` then passed the complete Stage 10.7 gate,
release matrix, hotplug, runtime/storage gates and drivers through Stage 13.5.
Stage 13.6 failed to compile: its HID report-error branch contained a copied
`hda::discover_topology` block, even though HDA is only enabled for Stage 13.8.
That unrelated block also existed in baseline `0563d0b`. It was removed from
the USB error branch; the original USB failure report and failure exit remain,
and the actual Stage 13.8 topology probe remains in its audio branch. The
remaining driver gates are checked before another complete final-source run.

The focused continuation passed Stage 13.6, then caught a remediation error:
the PCI memory/bus-master helper's new feature guard omitted Stage 13.7, which
also compiles xHCI. Stage 13.7 was added to that guard to match the existing
consumer; no new device authority was granted. The first failure is retained
in `ac-driver-preflight-13-7.txt`, and the corrected driver continuation uses
separate `ac-driver-final-*` logs.

Those focused logs report all Stage 13.6/13.7/13.8/13.9 CPU cases passing.
Before the final aggregate rerun, disk space fell to about 1.2 GB. Only the
two verified project-local debug `incremental` cache directories were removed,
recovering about 7 GB; historical logs and validated images were retained.
The final aggregate additionally uses `CARGO_INCREMENTAL=0` to bound compiler
cache growth. This changes build caching, not kernel behavior or test coverage.

## Results

Final software acceptance: **PASS**. Evidence:
`audit-artifacts/ac-full-1790166196507145600`, launched by
`RUN-STAGE13.10AC.ps1` with project-local Cargo directories,
`CARGO_NET_OFFLINE=true`, and `CARGO_INCREMENTAL=0`.

All 40 aggregate checks returned 0 (2,951.78 seconds of child-check runtime).
The aggregate verified its source hashes were unchanged and printed
`STAGE 13.10AC SOFTWARE ACCEPTANCE: PASS`. The matrix includes 157 QEMU boots:
75 in full Stage 10.7 preservation, 14 in the release matrix, 2 hotplug,
6 Stage 1â€“5/10.8, 3 Stage 10.9, 15 Stage 11, 15 Stage 12, and 27 Stage 13.
Default build, warning-denying host/kernel and feature Clippy, 27 Rust host
tests in each of the default/Wi-Fi configurations, and all 9 Python harness
tests passed. Commands, exit codes, timing, source hashes and serial snapshots
are retained alongside `results.json`.

Earlier failures remain evidence, not accepted gates: the initial all-target
Clippy diagnostics; the interrupted run in `ac-full-1790157588437175600`;
the bootloader build-lock failure in `ac-full-1790158698498588700`; the TCP
receive failure in `ac-full-1790160662768434100`; and the USB feature build
failure in `ac-full-1790162261267779600`. Focused fixes were followed by the
complete final-source aggregate above. The historical Stage 10.7 pass in
`ac-full-1790158698498588700` alone was not used to certify the final source.

## Remaining physical integration requirements

The host inventory reports Intel AX201 (`8086:a0f0`), while the reviewed device
is AX200 (`8086:2723`). No AX200 passthrough target or firmware image is supplied.
The worker probe binds owned synthetic CSR storage. Real firmware loading,
RX-ring programming/draining, ALIVE delivery from DMA, hardware MSI/W1C
behavior, and DMA visibility still require integration and hardware validation.
The default build therefore does not activate this experimental Wi-Fi driver.
Physical detach/rebind must quiesce the device and drain its interrupt vector;
software epochs alone cannot reject a late untagged MSI after vector reuse.

The next integration requires a device-specific owner for the firmware image,
staged DMA sections, context-info block, command queue, free/used RX descriptor
rings, and status page. The existing `IntelRxDmaQueue::complete_from_device`
copies synthetic bytes and marks a software slot complete; it does not read a
hardware completion ring. Likewise, the startup publication interfaces are
exercised with mock implementations. Connecting these pieces requires real
register access, descriptor-index validation, bounded draining and replenishment,
and a shutdown barrier that stops DMA before any backing allocation is freed.
ALIVE acceptance must come from a device-written notification and match the
selected firmware ABI. A simulated ALIVE is insufficient for hardware acceptance.

A read-only comparison with the
[upstream Intel firmware parser](https://github.com/torvalds/linux/blob/master/drivers/net/wireless/intel/iwlwifi/iwl-drv.c)
also identifies prerequisites for real-image loading: TLV 32 is a four-byte
paging-size declaration, while the current prototype treats it as an addressed
payload and rejects a four-byte value as empty. Secure runtime/init section
types 24/25 are not collected by the prototype. These are open firmware-loader
compatibility defects, not covered by the bounded CSR worker acceptance.
Correcting them requires real-image fixtures, separator/group handling and
loader-policy tests before any claim of firmware compatibility.

Update: the [firmware parser follow-up](audit-stage13-10ac-firmware-parser.md)
addresses those container parsing defects and adds an upstream binary fixture.
Its separate evidence applies to the follow-up source; hardware loading and
firmware ABI policy remain outstanding.

AX201 must not be enabled by adding its PCI ID to AX200 classification. Intel
documents AX201's [CNVio2 system interface](https://www.intel.com/content/www/us/en/products/sku/130293/intel-wifi-6-ax201-gig/specifications.html)
and AX200's [PCIe interface](https://www.intel.com/content/www/us/en/support/articles/000057689/wireless.html).
The chosen adapter/platform and firmware revision must be recorded before
physical bring-up. Firmware provenance is available through the
[upstream iwlwifi documentation](https://wireless.docs.kernel.org/en/latest/en/users/drivers/iwlwifi.html).

Passing this software gate closes the recorded build/test defects and validates
the bounded deferred-worker mechanism. It does not complete production Wi-Fi,
Bluetooth, or physical AX200 qualification, and does not authorize a later
roadmap stage while those requirements remain open.
---

## Source: docs/audit-stage13-1-2026-09-16.md

# Stage 13.1 audit â€” 2026-09-16

Added a bounded WovenDriver manager with device-kind matching, duplicate-safe
registration, binding, suspend/resume lifecycle transitions, and explicit
ownership of driver state. The existing device registry remains the discovery
source, preserving current PCI, timer, keyboard, block, and network behavior.

Build, freestanding Clippy, and QEMU acceptance passed on 1, 2, and 4 CPUs.
Evidence is retained under `audit-artifacts/stage13.1-*`.
---

## Source: docs/audit-stage13-12-driver-bring-up-2026-10-14.md

# Stage 13.12 boot-time driver bring-up audit â€” 2026-10-14

## Finding that opened this stage

Every Stage 13 driver was compiled only into its own test feature
(`#[cfg(feature = "stage13-N-test")] mod nvme;` and so on). A normal boot
contained no driver manager, NVMe, AHCI, xHCI or HDA code. Each driver had
passing QEMU gates, but none was ever bound by a running system.

## What changed

**Bring-up (`kernel/src/drivers.rs`).** It runs in every normal boot, after
storage and networking.
1. It registers a PCI class matcher with the WovenDriver manager for each
   driver:
   - NVMe: 01/08/02
   - AHCI: 01/06/01
   - xHCI: 0c/03/30
   - HDA: 04/03
2. For each driver, it takes the first PCI function the manager matches to
   it.
3. It binds that function through `driver::bind_pci`. Binding claims the
   function and every programmed MMIO BAR for the boot owner token.
4. It calls the driver's new `init_at(device)` with the function re-read
   after admission, which may move BARs.

Initialization failure runs `driver::unbind_pci_resources`, so a failed
controller keeps no ownership. Ready drivers are entered in the device
registry and listed by the shell's `devices` command.

**Drivers.** `nvme`, `ahci`, `xhci` and `hda` gain `init_at`. `init()` keeps
finding the first controller itself.

**Driver manager.** `driver.rs` gains:
- `pci_binding(name)`: the function a driver is bound to;
- `pci_bar_leases(name)`: how many BAR leases it holds;
- `PciMatch::matches`, now crate-visible.

**Build gating.** The driver modules, the driver manager and the PCI
helpers that binding uses now compile in production (no `qemu-test`) and
in `stage13-12-test`.
- Production compiles only what it uses. The PCI hotplug functions stay
  gated to the Stage 13 tests.
- Each driver module has one `expect(dead_code, reason = â€¦)`, active only
  in those two configurations. The reason names the APIs still waiting for
  production callers.
- Shared PCI, MSI, MSI-X and paging items that the live binding path now
  uses had their dead-code expectations narrowed so they do not apply in
  those builds.

## Gate

`[S13.12] driver bring-up: PASSED`. The gate runs the same `bring_up()` a
normal boot runs. It does not trust bring-up's report on its own; it checks
each driver against the driver manager and the device registry.
- **Ready:**
  - the bound function's class matches the driver's matcher;
  - `pci_binding` equals the reported address;
  - at least one BAR lease is held;
  - the registry lists the device with the right kind.
- **Absent, InitFailed, BindFailed:** no binding, and no registry entry.
- **Block I/O:** NVMe and AHCI each write, flush, read back and restore the
  last sector through the bound controller.
- **Idempotence:** a second bring-up binds nothing twice. Ready drivers
  report `AlreadyBound` at the same address, and the rest repeat their
  outcome.

The harness, `scripts/test-stage13-12-drivers.py`, fixes the expected
outcome of every driver per machine:
- **`devices`:** q35 with NVMe, a disk on the chipset AHCI controller, xHCI
  with a USB keyboard, and HDA. All four drivers must be Ready.
- **`nodisk`:** q35 with no disk. AHCI must be InitFailed, so the rollback
  path runs for real, and the others absent.
- **`absent`:** i440FX `pc`, which has none of these controllers. All four
  must be absent.
- **`production`:** a kernel built with no test feature, on the `devices`
  machine. All four drivers must be Ready, and the boot must reach
  `[BOOT] shell-first runtime ready`.

## Evidence (`audit-artifacts/`)

Stage 13.12, every mode on 1/2/4 CPUs, all passing:
- `devices`: `stage13.12-devices-1cpu-1791991503667577800`, `stage13.12-devices-2cpu-1791990748265435800`, `stage13.12-devices-4cpu-1791990800577141600`
- `nodisk`: `stage13.12-nodisk-1cpu-1791991302563673200`, `stage13.12-nodisk-2cpu-1791990899979693700`, `stage13.12-nodisk-4cpu-1791990949150002000`
- `absent`: `stage13.12-absent-1cpu-1791990999411980500`, `stage13.12-absent-2cpu-1791991051625832800`, `stage13.12-absent-4cpu-1791991105619152400`
- `production`: `stage13.12-production-1cpu-1791993293840764300`, `stage13.12-production-2cpu-1791991237755855900`, `stage13.12-production-4cpu-1791991270219159200`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each one failed the gate as intended:
- **Init failure keeps its binding** (`nodisk`): fails at "ahci0 InitFailed
  matches the driver manager and registry".
- **Init without binding** (`devices`): fails at "nvme0 Ready matches the
  driver manager and registry".
- **Registry not updated** (`devices`): fails at the same check.
- **Production boot compiles bring-up but never runs it**
  (`production`): fails with all four drivers missing.
  - The first version of this control removed the call. That only failed
    to compile, because of dead code under warning denial, so it proved
    nothing and was redone.

After the restore, `devices` passes again on 1 CPU.

Prior Stage 13 gates, rerun because their modules' gating changed, all
passing on 1/2/4 CPUs with exit 33:
- 13.1: `stage13.1-1cpu-1791991564231310000`, `stage13.1-2cpu-1791991637059693200`, `stage13.1-4cpu-1791991689376412700`
- 13.2: `stage13.2-1cpu-1791991735805966200`, `stage13.2-2cpu-1791991803606816100`, `stage13.2-4cpu-1791991852038745100`
- 13.3: `stage13.3-1cpu-1791991910277480900`, `stage13.3-2cpu-1791991983189973600`, `stage13.3-4cpu-1791992033063998500`
- 13.4: `stage13.4-1cpu-1791992082266661900`, `stage13.4-2cpu-1791992144276219400`, `stage13.4-4cpu-1791992185591944800`
- 13.5: `stage13.5-1cpu-1791992228265296100`, `stage13.5-2cpu-1791992296143722300`, `stage13.5-4cpu-1791992343240542000`
- 13.6: `stage13.6-1cpu-1791992388773827400`, `stage13.6-2cpu-1791992458686005800`, `stage13.6-4cpu-1791992505974391000`
- 13.7: `stage13.7-1cpu-1791992553411053400`, `stage13.7-2cpu-1791992629052095800`, `stage13.7-4cpu-1791992684607507200`
- 13.8: `stage13.8-1cpu-1791992736166240000`, `stage13.8-2cpu-1791992809983741000`, `stage13.8-4cpu-1791992857002094700`
- 13.9: `stage13.9-1cpu-1791992900875924000`, `stage13.9-2cpu-1791992988846889200`, `stage13.9-4cpu-1791993044100267500`
- 13.10: `stage13.10-1cpu-1791993100935930400`, `stage13.10-2cpu-1791993172706277700`, `stage13.10-4cpu-1791993226844159800`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-12-test`;
- each of `stage13-1-test` through `stage13-10-test`;
- `stage12-1-test`, `stage14-5d-test` and `stage1-5-test`;
- host `--all-targets`.

## Limits

- **One function per driver.** A second controller of the same class
  stays unbound.
- **xHCI needs a connected device.** Its initialization requires a
  connected root port, so a controller with nothing plugged in reports
  InitFailed.
- **Initialization still polls.** The controllers poll synchronously
  under their own locks during initialization. Interrupt-driven
  completion is the next step for xHCI.
- **Not yet consumed by anything else.** Storage is not mounted from NVMe
  or AHCI, USB keyboards and mice are not yet read, and audio is not
  exposed to userspace. These are the remaining Stage 13 closure steps.
- **QEMU only.** None of this is physical-hardware qualification.
---

## Source: docs/audit-stage13-13-usb-input-2026-10-14.md

# Stage 13.13 USB keyboards and mice audit â€” 2026-10-14

## What changed

**xHCI input (`kernel/src/xhci.rs`).** `start_input` runs from bring-up
once `xhci0` is ready.
- **Enumeration.** It enumerates every connected root port, each with its
  own reset and slot. Each keyboard or mouse is kept as an `InputDevice`
  with its own slot, EP0 ring, contexts, interrupt ring and report buffer,
  up to four devices.
- **Report mode.** Each device is switched to report-on-change
  (`SET_IDLE 0`) and has one interrupt-IN transfer armed.
- **Interrupt binding.** An MSI-X or MSI vector is bound through the
  driver manager, the `usb-input` worker is registered as the vector's
  waiter, and the interrupter is enabled with 1 ms moderation.

**The worker.** Under the controller lock it acknowledges IMAN, drains the
event ring without waiting, copies each report and re-arms its transfer.
After unlocking it decodes and publishes into WovenInput: key-down edges
with shift (held keys do not repeat, rollover reports are ignored), and
mouse motion and button edges. It then sleeps in `wait_for_event`.

**Interrupt wakeups (`kernel/src/interrupts.rs`).** Each PCI vector
(0x90â€“0xcf) can have one waiter task. The generic handler signals it
through the scheduler event latch after publishing the work flag.

**MSI-X delivery fix (`kernel/src/hal/pci/msix.rs`).** `enable_owned`
asserted Function Mask while programming the table entry and never cleared
it. With Function Mask set, every vector is masked, so no MSI-X interrupt
was ever delivered on any device. The Stage 13.2 gate checked programming,
not delivery. Activation now clears Function Mask, and verifies the write,
after unmasking the entry. A failure there keeps the lease for ordered
teardown, like the other late activation failures.

**Mouse axis fix.** `decode_boot_mouse` negated Y. HID boot mice report Y
positive towards the user (down the screen), which is WovenInput's
orientation. The decoder and its self-test are corrected, and the Stage
13.13 gate now checks the axis with real injected motion.

## Gate

`[S13.13] USB input: PASSED`. Harness:
`scripts/test-stage13-13-usb-input.py`.
- **Machine:** q35 with QEMU's xHCI, a USB keyboard and a USB mouse.
- **Decoder self-test:** presses arrive once, held keys do not repeat,
  shift applies, and rollover reports are ignored.
- **Discovery:** bring-up reports xhci0 Ready, with one keyboard and one
  mouse on a PCI device vector.
- **Injection:** after the kernel's ready line, the harness sends QEMU
  input over QMP, on a free local port. QEMU routes it to the USB devices,
  which were added after the chipset PS/2 devices:
  - typing `w`, `h`, `1`, shift+`a`, Enter must arrive as exactly `'w' 'h'
    '1' 'A' Enter`;
  - moving right 40, down 25, then a left click must arrive as motion
    (40, 25), a press and a release.
- **Delivery:**
  - the vector's interrupt count rises and at least five USB reports are
    copied, so input that took the PS/2 path would fail;
  - worker passes are at most interrupts + 2, so the worker sleeps between
    interrupts rather than polling.

In the 1-CPU run, 15 interrupts carried 15 reports in 17 worker passes.

## Evidence (`audit-artifacts/`)

Stage 13.13 on 1/2/4 CPUs, all passing: `stage13.13-1cpu-1791997347904187000`, `stage13.13-2cpu-1791996407064891200`, `stage13.13-4cpu-1791996488426811900`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each one failed the gate as intended:
- **Transfer not re-armed:** only the first report per device arrives
  (`w`, and the first motion).
- **Function Mask left set** (the original bug): nothing arrives.
- **Worker polls instead of sleeping:** every event arrives, but the gate
  fails at "the worker sleeps between them".
- **Held keys repeat:** the decoder check fails.
- **Mouse Y inverted again:** motion arrives as (40, âˆ’25).
- **No input injected:** nothing arrives.

After the restore, the gate passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- 13.12 `devices`: `stage13.12-devices-1cpu-1791997427047816800`, `stage13.12-devices-2cpu-1791997514470681600`, `stage13.12-devices-4cpu-1791997591909102600`
- 13.12 `nodisk`: `stage13.12-nodisk-1cpu-1791997665910623600`, `stage13.12-nodisk-2cpu-1791997736519754300`, `stage13.12-nodisk-4cpu-1791997807852208900`
- 13.12 `absent`: `stage13.12-absent-1cpu-1791997878042457400`, `stage13.12-absent-2cpu-1791997944166498300`, `stage13.12-absent-4cpu-1791998009777679000`
- 13.12 `production`: `stage13.12-production-1cpu-1791998078111124800`, `stage13.12-production-2cpu-1791998128345580800`, `stage13.12-production-4cpu-1791998163544483500`
- 13.2: `stage13.2-1cpu-1791998197713407000`, `stage13.2-2cpu-1791998288138623900`, `stage13.2-4cpu-1791998348411565000`
- 13.5: `stage13.5-1cpu-1791998399483751100`, `stage13.5-2cpu-1791998488400660100`, `stage13.5-4cpu-1791998553506569200`
- 13.6: `stage13.6-1cpu-1791998614369898400`, `stage13.6-2cpu-1791998697837326700`, `stage13.6-4cpu-1791998767761400400`
- 13.7: `stage13.7-1cpu-1791998831173952000`, `stage13.7-2cpu-1791998909034208900`, `stage13.7-4cpu-1791998971158633900`
- 13.9: `stage13.9-1cpu-1791999032502021800`, `stage13.9-2cpu-1791999133577684100`, `stage13.9-4cpu-1791999212910642800`
- 13.10: `stage13.10-1cpu-1791999292801794200`, `stage13.10-2cpu-1791999362484614300`, `stage13.10-4cpu-1791999416399032400`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-13-test` and `stage13-12-test`;
- `stage13-2-test`, `stage13-5-test`, `stage13-6-test`, `stage13-7-test`,
  `stage13-9-test` and `stage13-10-test`;
- `stage14-5d-test`;
- host `--all-targets`.

## Limits

- **Boot protocol only.** Report-protocol HID devices such as touchpads
  and multi-button mice need the report descriptor parser; that work is
  part of Stage 13.16.
- **No hotplug.** Devices are enumerated at bring-up only. Port Status
  Change events are drained but not acted on.
- **Limited keymap.** The keyboard map covers the keys WovenInput can
  represent today: printable US layout, Enter, Backspace, Tab and F1.
- **QEMU only.** Physical USB controllers and devices are not qualified.
---

## Source: docs/audit-stage13-14-storage-2026-10-14.md

# Stage 13.14 storage on NVMe and AHCI audit â€” 2026-10-14

## What changed

- **Selectable root device.** `block_io` has a root device, `RootDevice::
  {Ata, Nvme, Ahci}`, and `with_root` runs an operation on it. The
  worker's `direct_primary`, the `PrimaryAta` handle (sector count,
  read-only state, presence), and `storage::mount_ata_root` all use
  `with_root`. `storage.rs` no longer uses the ATA driver. The default
  root is ATA, so existing configurations are unchanged.
- **Root selection.** `drivers::storage_root` returns the first ready
  `nvme0` or `ahci0`.
- **Production boot.** A normal boot with no ATA volume mounted selects
  that device after bring-up and mounts it at `/mnt`. It logs
  `[FS] FAT32 mounted at /mnt from nvme0` (or `ahci0`), or that the
  device has no mountable volume.

## Gate

`[S13.14] NVMe/AHCI storage: PASSED`. Harness:
`scripts/test-stage13-14-storage.py`.

The machine is q35, which has no ATA, with a blank FAT32 image from
`create-fat32.py` on NVMe or on the chipset AHCI controller. The kernel
checks:
- no ATA disk is present;
- bring-up yields a storage root;
- the volume mounts at `/mnt` from the selected device;
- `/mnt/WOVEN.TXT` is written and persisted;
- after `remount_mnt`, reading the file directly from the disk returns
  exactly what was written.

After QEMU exits, the harness requires both the file's bytes and its
`WOVEN   TXT` directory entry to be in the image.

In production mode, a kernel with no test features boots from the NVMe
image. It must report the mount and reach the shell.

## Evidence (`audit-artifacts/`)

Stage 13.14 on 1/2/4 CPUs, all passing:
- `nvme`: `stage13.14-nvme-1cpu-1792001143000348500`, `stage13.14-nvme-2cpu-1792000265099941200`, `stage13.14-nvme-4cpu-1792000348894914400`
- `ahci`: `stage13.14-ahci-1cpu-1792000984066447000`, `stage13.14-ahci-2cpu-1792000521713763700`, `stage13.14-ahci-4cpu-1792000608351884900`
- `production`: `stage13.14-production-1cpu-1792001085352292800`, `stage13.14-production-2cpu-1792000729769511300`, `stage13.14-production-4cpu-1792000768143619800`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Root never selected:** the volume does not mount.
- **File not persisted:** the remount readback fails, and the bytes are
  not in the image.
- **AHCI selected but I/O still sent to ATA:** the mount fails.
- **Production boot skips the mount:** "production boot did not mount the
  NVMe volume".

After the restore, `nvme` passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- ATA storage regression (`test-storage-qemu.py`, live FAT32 mutation on
  the ATA path);
- the Stage 15.2 WovenFS device gate;
- Stage 13.12 `devices` and `production`, and Stage 13.13, on 2 CPUs;
- 1-5: `stage1-5-1cpu-1792001766046240900`, `stage1-5-2cpu-1792001853927339600`, `stage1-5-4cpu-1792001925379426100`
- 10.8: `stage10.8-1cpu-1792001998368022200`, `stage10.8-2cpu-1792002087490436000`, `stage10.8-4cpu-1792002159983687200`
- 12.1: `stage12.1-1cpu-1792002226367299500`, `stage12.1-2cpu-1792002302790727900`, `stage12.1-4cpu-1792002362112628600`
- 13.3: `stage13.3-1cpu-1792002421343947600`, `stage13.3-2cpu-1792002506875713100`, `stage13.3-4cpu-1792002563690624400`
- 13.4: `stage13.4-1cpu-1792002620083867300`, `stage13.4-2cpu-1792002684797238800`, `stage13.4-4cpu-1792002732988226200`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-14-test`, `stage13-13-test`, `stage13-12-test`,
  `stage13-3-test` and `stage13-4-test`;
- `stage1-5-test`, `stage10-8-test`, `stage12-1-test`, `stage15-2-test`
  and `qemu-test`;
- host `--all-targets`.

## Limits

- **Only `/mnt`.** One FAT32 volume is mounted, from the first ready
  storage driver. Multiple volumes, and a WovenFS root on NVMe or AHCI,
  are not handled (`wfs.rs` still uses the ATA primary master).
- **ATA takes precedence.** The selection is made once at boot; a machine
  with an ATA volume keeps it as the root.
- **Polled I/O.** NVMe and AHCI I/O is still synchronous and polled, now
  behind the block-I/O worker.
- **QEMU only.** Physical NVMe and SATA devices are not qualified.
---

## Source: docs/audit-stage13-15-audio-2026-10-14.md

# Stage 13.15 WovenAudio playback for userspace audit â€” 2026-10-14

## What changed

**Capability.** `AudioPlayback` is capability 24, so `Capability::COUNT` is
now 25. It is in the system-service ceiling and in the user ceiling. The
permission broker maps the new `Permission::Audio` (16) to it, so a user
application gets it only when the broker grants it. Recording, not yet
implemented, will use the existing broker-gated `Microphone` capability.

**HDA output (`kernel/src/hda.rs`).** `start_output`, called from bring-up:
1. finds the output route (converter, pin, amplifier) and enables the pin;
2. sets the amplifier to 75%;
3. allocates the playback stream's buffer descriptor list and 16 DMA pages
   (64 KiB) once;
4. binds the controller's MSI-X or MSI vector through the driver manager.

**`play`.** It accepts 48 kHz 16-bit stereo, whole frames, up to 64 KiB.
1. Only one playback runs at a time. The caller registers as the vector's
   waiter (`interrupts::set_pci_vector_waiter`, released with the new
   `clear_pci_vector_waiter`).
2. It fills the pages and the descriptor list, with interrupt-on-completion
   on the last entry only.
3. It starts the stream with IOCE and stream interrupts enabled.
4. It sleeps in `wait_for_event_until` until BCIS shows completion, or the
   deadline passes. The deadline is twice the playing time plus half a
   second.
5. It stops the stream and clears status and the interrupt enable.

`set_volume` sets the amplifier gain; 0 mutes.

**Syscalls (`kernel/src/syscall.rs`).**
- **203 `AudioInfo`:** returns the output format.
- **204 `AudioPlay`:** checks `AudioPlayback`. It copies the PCM into
  kernel memory before taking the controller lock, so a user page fault
  never happens under the IRQ lock. Every attempt is recorded in the
  security ledger as `Action::AudioPlay`, allowed or denied.
- **205 `AudioSetVolume`:** checks `AudioPlayback`.

**libwoven.** `audio_info`, `audio_play` and `audio_set_volume`.

**hello-gui `--audio`.** Plays a quarter second of a 500 Hz square wave.
The wave is filled at run time in `.bss`, so it is neither on the 8 KiB
stack nor in the 64 KiB-limited image.

## Gate

`[S13.15] WovenAudio playback: PASSED`. Harness:
`scripts/test-stage13-15-audio.py`.

The machine is q35 with an Intel HDA controller and a `hda-duplex` codec.
QEMU records the codec's output to a WAV file at 48 kHz 16-bit stereo.
The gate checks:
- hda0 is ready, with output on a PCI device vector;
- a client with `AudioPlayback` plays the wave and exits 0, with exactly one
  completed playback;
- the completion arrives as a device interrupt and wakes the player before
  its deadline (counted by `AUDIO_ON_TIME`);
- the same client without the capability exits 3 (refused), and nothing
  more is played.

The harness then requires the recording to hold the wave: at least
three-quarters of its 12,000 frames loud, and the sign changing every
48 Â± 2 frames. QEMU leaves the WAV sizes unset when the guest exits through
`isa-debug-exit`, so the data chunk is read to the end of the file.

## Evidence (`audit-artifacts/`)

Stage 13.15 on 1/2/4 CPUs, all passing: `stage13.15-1cpu-1792005061603763000`, `stage13.15-2cpu-1792004414605407100`, `stage13.15-4cpu-1792004486814112100`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All five built, and each failed the gate as intended:
- **Capability not checked:** the client without `AudioPlayback` is not
  refused.
- **No completion interrupt:** no interrupt, and playback times out.
- **Player not registered as the waiter:** interrupts arrive, but the
  player only notices at its deadline, so the on-time check fails.
- **Silence copied instead of the PCM:** the recording has no loud frames.
- **Output muted:** the recording has no loud frames.

After the restore, the gate passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- the HDA driver (13.8), the permission broker (15.3), signed applications
  (15.4) and the security ledger (15.8), all affected by the new capability
  and audit action:
- 13.8: `stage13.8-1cpu-1792005158794826400`, `stage13.8-2cpu-1792005254650444900`, `stage13.8-4cpu-1792005324219174200`
- 15.3: `stage15.3-1cpu-1792005390267631200`, `stage15.3-2cpu-1792005476337633700`, `stage15.3-4cpu-1792005542090239400`
- 15.4: `stage15.4-1cpu-1792005607690301900`, `stage15.4-2cpu-1792005694779679700`, `stage15.4-4cpu-1792005762415378500`
- 15.8: `stage15.8-1cpu-1792005829865106300`, `stage15.8-2cpu-1792005917530759900`, `stage15.8-4cpu-1792005984911482100`
- Stage 13.12 `devices` and `production`, 13.13 and 13.14 `nvme` on 2 CPUs;
  Stages 20.11 and 20.12 on 1 CPU.

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-12-test` through `stage13-15-test`, and `stage13-8-test`;
- `stage15-3-test`, `stage15-4-test` and `stage15-8-test`;
- `stage20-12-test`;
- host `--all-targets`.

## Limits

- **One stream, one buffer at a time.** Playback is a single blocking
  buffer of up to 64 KiB, which is about a third of a second. There is no
  queue for gapless streaming and no mixing of several clients.
- **Fixed format.** 48 kHz 16-bit stereo only, with no resampling.
- **No recording.** Capture behind `Microphone` is the next audio step.
- **QEMU only.** Physical codecs are not qualified.
---

## Source: docs/audit-stage13-16-hid-reports-2026-10-14.md

# Stage 13.16 report-protocol HID input audit â€” 2026-10-14

## What changed

**`kernel/src/hid.rs` (pure; host-tested).**
- `parse` reads a report descriptor into a bounded `Layout`: the top-level
  application and up to 48 input fields (usage, report ID, bit offset and
  size, logical range, relative).
- It supports report IDs, usage ranges, push and pop, constant padding and
  long items. It refuses malformed descriptors, descriptors that are too
  large, and applications it does not decode (keyboards stay on the boot
  path).
- `Decoder` reports changes only:
  - relative and absolute pointer motion, wheel and button edges;
  - pen position, pressure and contact (hover included, nothing out of
    range);
  - per-contact touch, with each lift reported once;
  - game-controller axes, hat and buttons.
- Positions are scaled to 0..=32767, pressure to 0..=1023, and controller
  axes to -32768..=32767.

**xHCI.** A HID interface that is not a boot keyboard or mouse has its
report descriptor fetched (its length comes from the HID class descriptor)
and parsed. Only a decodable layout becomes an input device, with reports
up to 64 bytes. The worker keeps one `Decoder` per report device and
publishes outside the controller lock.

**WovenInput and the userspace ABI.**
- `Scroll`, `PointerAbsolute`, `Touch`, `Pen` and `GameController` are now
  production events. Before this stage they existed only in the Stage 13.7
  probe build.
- The input syscall encodes them as kinds 9â€“13, and libwoven's `InputEvent`
  decodes them.

## Gates

**Host tests (`tests/hid_reports.rs`), against real descriptors.**
- QEMU's usb-tablet: absolute position, wheel with sign extension, and
  button edges.
- A 14-button game pad: hat null state, four axes, buttons, and changes
  only.
- A digitizer pen with report ID 2: position, pressure, hover, out of range,
  and foreign report IDs.
- A two-contact touch screen: per-contact reports and lifts.
- Malformed, unbalanced, keyboard-only and zero-size descriptors, and
  short reports.

The game-pad test caught a real bug: a fully deflected axis scales to
`i16::MIN`, which had been used as the "no value yet" marker, so it was
never reported. The marker is now `Option<i16>`.

**QEMU: `[S13.16] report-protocol HID input: PASSED`.** Harness:
`scripts/test-stage13-16-hid-reports.py`.
- The machine is q35 with QEMU's xHCI and a USB tablet.
- Bring-up must parse the tablet's report descriptor as a pointer device.
- QMP then injects absolute (8192, 24576) and (24576, 8192), a left press,
  a move while the button is held, a release, and a wheel step up and down.
- They must arrive in order:
  - the positions exactly, X before Y;
  - one press and one release;
  - `Scroll` +1 and then -1;
  - all of it carried by device interrupts.

## Evidence (`audit-artifacts/`)

Stage 13.16 on 1/2/4 CPUs, all passing, with the held-button injection:
`stage13.16-1cpu-1792009812938824900`, `stage13.16-2cpu-1792009909416253200`, `stage13.16-4cpu-1792009987932861000`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Report descriptor never fetched:** the tablet is not an input device.
- **X and Y swapped:** the positions arrive mirrored and out of order.
- **Held buttons reported again as presses:** the click check fails.
  - The first run of this control was not caught: the injection never
    produced a report while a button was held.
  - The harness now moves the pointer during the press, and the control is
    caught.
- **Wheel not sign-extended:** the step down arrives as +127.

After the restore, the gate passes again.

Regression, all passing:
- Stage 13.13 on 1/2/4 CPUs.
- 13.5: `stage13.5-1cpu-1792008823436264500`, `stage13.5-2cpu-1792008905908146600`, `stage13.5-4cpu-1792008969277639000`
- 13.6: `stage13.6-1cpu-1792009030435929400`, `stage13.6-2cpu-1792009113797248000`, `stage13.6-4cpu-1792009183415151900`
- 13.7: `stage13.7-1cpu-1792009248467170400`, `stage13.7-2cpu-1792009339592498600`, `stage13.7-4cpu-1792009411593361400`
- Stage 19.12 (pointer input) on 1/2/4 CPUs, through its own harness.
- 13.12 `devices` and 13.15 on 2 CPUs.

**Lint and tests.** Warning-denying Clippy passes for:
- production;
- `stage13-6-test`, `stage13-7-test`, and `stage13-12-test` through
  `stage13-16-test`;
- `stage19-12-test`;
- host `--all-targets`.

The host HID tests and the libwoven tests pass.

## Limits

- **Single application per device.** A device whose report descriptor has
  several top-level applications (for example a keyboard with a touchpad)
  is decoded for its first decodable application only.
- **Gate coverage.** QEMU provides only the tablet as a report-protocol
  device, so pens, touch screens and game pads are covered by the host
  tests on real descriptors, not by a QEMU gate.
- **No virtio-input.** Touch input through virtio-input (`virtio-tablet`,
  `virtio-multitouch`) needs a virtio-input driver.
- **QEMU only.** Physical digitizers and controllers are not qualified.
---

## Source: docs/audit-stage13-17-to-19-live-hardware-testing-2026-10-15.md

# Stages 13.17â€“13.19: live hardware testing audit â€” 2026-10-15

Three stages that make a WovenHat USB stick safe to boot on someone else's
machine, and make that boot report back.
- **13.17:** disks WovenHat does not own are never written.
- **13.18:** USB mass storage.
- **13.19:** the boot's log kept on the stick.

They extend the Stage 13 driver closure (roadmap 13.5, USB transfers) and
prepare roadmap Stage 29, real hardware qualification.

## Stage 13.17 â€” disks WovenHat does not own are never written

**The defect.** Without an ATA disk, a normal boot mounted the first
ready NVMe or AHCI disk's FAT32 volume at `/mnt`, writable.
- On a Windows PC that volume is the EFI system partition.
- Any file written, changed or deleted under `/mnt` went through to the
  other system's boot partition.

**The rule.**
- A root device is writable only when the FAT32 volume found on it is
  labelled `WOVENHAT` (WovenHat's data disks) or `WOVENLOG` (the
  live-test log partition).
- `storage::mount_ata_root` reads the label (`fat32_label`) before
  mounting and records ownership (`block_io::set_root_owned`).
- `block_io::with_root`, the path of every root writer, wraps the device
  in a guard:
  - `write_sector` returns `Error::ReadOnly`;
  - `is_read_only` is true;
  - `flush` writes nothing.
- This covers file write-back, journal recovery, swap and raw block
  syscalls alike, and a foreign volume imports read-only.
- Test boots keep the ATA primary master writable, because several gates
  write raw sectors on scratch disks that hold no file system.
- WovenFS writes only volumes carrying a valid WovenFS superblock, which
  only WovenHat creates.

**AHCI takeover from firmware (bug found by this gate).**
- With a firmware variable store, OVMF uses the AHCI controller during
  boot and stops the port. The disk then reads signature `0xFFFFFFFF`
  with an active link, and the driver skipped it (`NoSataPort`).
- Real firmware always uses the controller, so every real SATA machine
  was likely affected.
- The driver now takes each port over in this order:
  1. stop the port;
  2. install its own buffers;
  3. start FIS receive;
  4. COMRESET if the link is not active or the signature is not ATA;
  5. wake a link left in a power-saving state.
- In QEMU, steps 1â€“3 are enough: once FIS receive runs, the disk reports a
  fresh signature. The gate therefore does not exercise COMRESET or the
  link wake; they remain fallbacks for hardware that needs them.

**Diagnostics.** Driver bring-up now logs every failing driver's specific
error (`[DRIVERS] <name> init error: â€¦`). The outcome alone gives a field
tester nothing to act on.

**Gate** (`scripts/test-stage13-17-foreign-disk.py`). The data disk is a
FAT32 volume formatted as another system would format it: OEM
`MSDOS5.0`, label `NO NAME`, one file.
- Modes:
  - `nvme` and `ahci` (stage13-17-test kernel): the volume mounts and its
    file reads. Changing the file, creating a file, a raw sector write
    and a flush are then attempted, and every write must be refused.
  - `production`: a normal kernel mounts the disk and reaches the shell.
- The host requires the image to be byte-for-byte unchanged.
- OVMF gets its own variable store. The first run showed that without
  one the firmware writes `NvVars` to the first FAT volume it finds.

## Stage 13.18 â€” USB mass storage

**Protocol (`kernel/src/usb_msc.rs`, host tests `tests/usb_msc.rs`).**
- Bulk-Only SCSI interface discovery.
- Command Block Wrappers, and Command Status Wrapper validation:
  signature, tag, residue and status.
- SCSI commands: INQUIRY, TEST UNIT READY, REQUEST SENSE, READ CAPACITY
  (10), READ/WRITE (10) and SYNCHRONIZE CACHE.

**Transport (`kernel/src/xhci.rs`).**
- Enumeration reads each device's configuration once; a mass-storage
  interface becomes the USB disk.
- Attach sequence:
  1. SET_CONFIGURATION, then bulk endpoint contexts.
  2. INQUIRY.
  3. TEST UNIT READY until ready, with REQUEST SENSE between attempts. A
     reader reporting no medium stops early.
  4. READ CAPACITY. Only 512-byte blocks are accepted.
- Stalls are cleared with Reset Endpoint, Set TR Dequeue Pointer and
  CLEAR_FEATURE(ENDPOINT_HALT).
- `UsbDisk` implements `BlockDevice`. Each block takes the controller lock
  for one command, so interrupts are never masked for longer than one
  block.

**Shared event ring.**
- Storage transfers and the USB input worker share the controller's event
  ring.
- While a storage command waits, input reports it drains are routed: the
  device is re-armed and the report stashed. The input worker delivers
  them on its next pass.

**Ring wrap fix.**
- `Ring::push` used to flip its cycle bit before rewriting the link TRB,
  so the link carried the new cycle.
- A controller that had not yet fetched the link would stop at it, which
  could freeze a USB keyboard after its ring wrapped (every 255 reports).
- QEMU prefetches the link, so the gates never showed it.
- The link is now written with the consumer's cycle first.

**Gate** (`scripts/test-stage13-18-usb-storage.py`). A patterned 32 MiB
image is attached as `usb-storage` beside a `usb-kbd`. The kernel must:
1. identify QEMU's disk and its capacity;
2. read the pattern back, first and last blocks;
3. write 64 blocks, flush, and read them back;
4. refuse a block past the end;
5. verify 300 or more further reads, wrapping both bulk rings, while
   "usb" plus Enter typed over QMP arrives in order.

The host requires exactly the 64 written sectors to have changed, each
holding the kernel's record.

## Stage 13.19 â€” the boot's log on the WovenHat USB stick

**Kernel log.** `serial.rs` keeps every byte written to COM1 in a 256 KiB
lock-free ring (`log_position`, `read_log`).

**`kernel/src/livelog.rs`.**
- It finds the FAT32 partition labelled `WOVENLOG` on the USB disk
  (`gpt::fat_partitions`). That volume is the only one it writes.
- It creates `LOGS/BOOTnnnn.TXT` per boot, preallocated to 512 KiB:
  - Updates rewrite data sectors in place, with no FAT or directory
    changes after creation.
  - The first 4 KiB is a summary rewritten on every flush: boot number,
    machine clock, kernel version, and every log line reporting a
    failure, up to 40.
  - The kernel log follows, padded with spaces.
- It writes a hardware inventory: CPU vendor and brand string, logical
  CPUs, memory, display mode, every PCI function, and the USB disk.
- A worker flushes every 3 s. The panic handler flushes once more if the
  log and the controller lock are free.

**Kernel shell commands.**
- `note <text>` records a tester's observation and saves it at once.
- `savelog` flushes on demand.

**The stick.** `scripts/write-live-usb.ps1` writes the release-validated
image, verifies it by read-back SHA-256, then:
1. creates the `WOVENLOG` FAT32 partition in the rest of the stick;
2. checks the label in its boot sector;
3. adds a README.

**Gate** (`scripts/test-stage13-19-livelog.py`). It builds a stick as the
writer leaves a real one: GPT, an EFI partition, then the FAT32
`WOVENLOG` partition. It reads the log back with its own FAT32 reader.
- Modes:
  - `test`: a tester's note, a reported failure that must reach the
    summary, and a 200-line burst.
  - `production`: a normal boot reaches the shell, and the log holds the
    inventory and the boot messages within one flush period.
  - `unlabelled`: the partition is labelled `NO NAME`, and the stick must
    not change at all.
- In every mode, everything before the log partition must be unchanged.

## Evidence (`audit-artifacts/`)

All gates on 1/2/4 CPUs:
- **Stage 13.17:** `stage13.17-nvme-1cpu-1792065092555576200`, `stage13.17-nvme-2cpu-1792064222063722500`, `stage13.17-nvme-4cpu-1792064675450888300`, `stage13.17-ahci-1cpu-1792070090765846700`, `stage13.17-ahci-2cpu-1792064293120539100`, `stage13.17-ahci-4cpu-1792064755571914200`, `stage13.17-production-1cpu-1792063844090949600`, `stage13.17-production-2cpu-1792064338888488100`, `stage13.17-production-4cpu-1792064804937379700`
- **Stage 13.18:** `stage13.18-1cpu-1792065576766775400`, `stage13.18-2cpu-1792064417807942400`, `stage13.18-4cpu-1792064863140471100`
- **Stage 13.19:** `stage13.19-test-1cpu-1792065740723695600`, `stage13.19-test-2cpu-1792064505542364800`, `stage13.19-test-4cpu-1792064929004817200`, `stage13.19-production-1cpu-1792064101415762000`, `stage13.19-production-2cpu-1792064590889875700`, `stage13.19-production-4cpu-1792064994848484100`, `stage13.19-unlabelled-1cpu-1792065674471097500`, `stage13.19-unlabelled-2cpu-1792064637008146600`, `stage13.19-unlabelled-4cpu-1792065039369906000`

Chain results (lint, host tests, gates, negative controls, regression):

```
lint-all-features: exit 0 []
host tests: ok
libwoven tests: ok
13.17 nvme 1cpu: PASS (72s)
13.17 ahci 1cpu: PASS (39s)
13.17 production 1cpu: PASS (44s)
13.18 1cpu: PASS (85s)
13.19 test 1cpu: PASS (117s)
13.19 production 1cpu: PASS (69s)
13.19 unlabelled 1cpu: PASS (42s)
13.17 nvme 2cpu: PASS (66s)
13.17 ahci 2cpu: PASS (43s)
13.17 production 2cpu: PASS (75s)
13.18 2cpu: PASS (80s)
13.19 test 2cpu: PASS (82s)
13.19 production 2cpu: PASS (44s)
13.19 unlabelled 2cpu: PASS (35s)
13.17 nvme 4cpu: PASS (75s)
13.17 ahci 4cpu: PASS (46s)
13.17 production 4cpu: PASS (56s)
13.18 4cpu: PASS (61s)
13.19 test 4cpu: PASS (63s)
13.19 production 4cpu: PASS (42s)
13.19 unlabelled 4cpu: PASS (51s)
negative control 13.17 guard lets writes through: caught ['the foreign disk image changed: ccf8de5dcf0f4c3043f781a1ce1e5bdb1842f0ed8c9250850f4ca7a467d6675a -> ebf83bc6351cc0570b65b5e9037c8053898500f9104e2fefb1f8f45672c24b01', '[DRIVERS] ahci0 init error: NoSataPort', '[S13.17] changing its file is refused FAIL', '[S13.17] creating a file is refused FAIL']
negative control 13.17 AHCI port taken only if already active with an ATA signature: NOT CAUGHT ['Stage 13.17 ahci: PASS (1 CPU); disk unchanged. Evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.17-ahci-1cpu-1792065159614502500']
negative control 13.18 no keys typed: caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] verified 105386 reads; keys []', '[S13.18] typed keys arrive once each, in order, during storage I/O FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.18 input events dropped during storage transfers: caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] verified 95120 reads; keys []', '[S13.18] typed keys arrive once each, in order, during storage I/O FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.18 writes land one block late: caught ['changed sectors [4097, 4098, 4099, 4100, 4101, 4102, 4103, 4104]... (64), expected 4096..4159', '[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] 64 blocks written, flushed and read back FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.19 any FAT32 label accepted: caught ['the stick changed although its partition is not labelled WOVENLOG', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-unlabelled-1cpu-1792065674471097500']
negative control 13.19 issues never reach the summary: caught ['[S13.19] simulated driver FAILURE for the summary', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-test-1cpu-1792065740723695600']
13.14 nvme 1cpu: PASS (77s)
13.14 ahci 1cpu: PASS (58s)
13.14 production 1cpu: PASS (62s)
13.12 production 1cpu: PASS (23s)
13.13 1cpu: PASS (83s)
13.16 1cpu: PASS (106s)
13.3 1cpu: PASS (66s)
13.4 1cpu: PASS (51s)
13.5 1cpu: PASS (54s)
13.6 1cpu: PASS (54s)
storage 1cpu: PASS (73s)
13.14 nvme 2cpu: PASS (54s)
13.14 ahci 2cpu: PASS (50s)
13.14 production 2cpu: PASS (28s)
13.12 production 2cpu: PASS (22s)
13.13 2cpu: PASS (50s)
13.16 2cpu: PASS (52s)
13.3 2cpu: PASS (39s)
13.4 2cpu: PASS (40s)
13.5 2cpu: PASS (48s)
13.6 2cpu: PASS (43s)
storage 2cpu: PASS (57s)
13.14 nvme 4cpu: PASS (76s)
13.14 ahci 4cpu: PASS (47s)
13.14 production 4cpu: PASS (28s)
13.12 production 4cpu: PASS (23s)
13.13 4cpu: PASS (52s)
13.16 4cpu: PASS (59s)
13.3 4cpu: PASS (44s)
13.4 4cpu: PASS (44s)
13.5 4cpu: PASS (52s)
13.6 4cpu: PASS (38s)
storage 4cpu: PASS (57s)
20.1 1cpu: PASS (61s)
20.14b 1cpu: PASS (93s)
release: PASS (2278s)
DONE
negative control 13.17 AHCI judged before takeover (old order): caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.17] a storage driver is ready FAIL']
```

The release gate passed 100/100 (`audit-artifacts/release-validation-1792070090/`); its validated image is the one written to the live-test stick.

**Negative controls.** Each ran on 1 CPU with the source restored byte for
byte afterwards:
- **The guard lets writes through:** the host saw the image change, and
  three kernel steps failed.
- **AHCI port judged before the takeover** (the old order): `NoSataPort`.
  A first version of this control removed only the COMRESET, and it was
  not caught, for the reason given under Stage 13.17. The corrected
  control was run after the chain.
- **No keys typed during storage I/O.**
- **Input events dropped during storage transfers:** the keys never
  arrived.
- **Writes landing one block late:** the host sector check failed.
- **Any FAT32 label accepted:** the unlabelled stick changed.
- **Issues never reaching the summary.**

## Limits

- **13.17.**
  - Ownership is by volume label. A foreign volume that happens to be
    labelled WOVENHAT or WOVENLOG is treated as WovenHat's.
  - Test boots exempt the ATA primary master.
- **13.18.**
  - One disk, one LUN, on a root port. Hubs and UAS are not supported.
  - Only 512-byte blocks addressable by 10-byte commands, up to 2 TiB.
  - Polled completion, one block per command, so transfers are slow.
  - No Bulk-Only reset recovery after a phase error.
- **13.19.**
  - The kernel log ring holds 256 KiB, and earlier bytes are lost if the
    first flush comes later.
  - The file holds 508 KiB of log per boot.
  - A panic flush is skipped if the panicking code holds the USB or log
    lock.
  - Logs are written only when a USB disk is attached at boot.
---

## Source: docs/audit-stage13-20-power-off-and-reset-2026-10-15.md

# Stage 13.20: ACPI power-off and reset audit â€” 2026-10-15

The kernel could neither turn a machine off nor restart it. This stage
does both, for live hardware testing, where a tester must be able to shut
down and the stick's log must get a last flush. It also delivers the first
two items of roadmap Stage 25, power management.

## What changed

**ACPI (`hal/acpi.rs`, published as `acpi::power()`).**
- The FADT's PM1a and PM1b control blocks. The extended (GAS) addresses
  are preferred when they are system I/O.
- The SMI command port and ACPI_ENABLE.
- The reset register, when RESET_REG_SUP is set.
- The `\_S5` sleep types, from the DSDT or else an SSDT:
  `Name (_S5, Package)` with Zero, One, byte, word or dword elements.

**`power.rs`.**
- `power_off()`:
  1. hands the machine to ACPI if the firmware left it in legacy mode;
  2. writes `SLP_TYP | SLP_EN` to PM1b, then PM1a;
  3. returns only if the machine is still running, with the reason.
- `reboot()` tries each reset method in turn:
  1. the FADT reset register (system I/O);
  2. the 8042 reset line;
  3. the 0xCF9 reset control;
  4. a triple fault.
- Both give the live-test log a last flush first.

**Kernel shell.** `poweroff`/`shutdown` and `reboot`/`restart`.

**Live-test log.** The `WOVENLOG` label is now also accepted from the
FAT32 root directory, where Windows' format writes it. A 13.19 gate mode,
`windows-label`, covers it.

## Gate

`scripts/test-stage13-20-power.py`:
- **poweroff:** the stage13-20-test kernel enters S5, and QEMU must exit
  by itself with status 0.
- **reboot:** the stage13-20b-test kernel resets, and QEMU (`-no-reboot`)
  must exit likewise.

No failure line may follow the request. In QEMU, the FADT reset register
(port 0xCF9) reset the machine on the first attempt. `\_S5` read as
(0, 0), with PM1a at port 0x604.

## Evidence (`audit-artifacts/`)

- **13.20 power-off, 1/2/4 CPUs:** `stage13.20-poweroff-1cpu-1792074113568239300`, `stage13.20-poweroff-2cpu-1792073744690430900`, `stage13.20-poweroff-4cpu-1792073853280290100`.
- **13.20 reset, 1/2/4 CPUs:** `stage13.20-reboot-1cpu-1792074383549659200`, `stage13.20-reboot-2cpu-1792073765647322300`, `stage13.20-reboot-4cpu-1792073894760101800`.
- **13.19 windows-label, 1/2/4 CPUs:** `stage13.19-windows-label-1cpu-1792074649172553700`, `stage13.19-windows-label-2cpu-1792073802466461400`, `stage13.19-windows-label-4cpu-1792073918705552400`.
- **Release gate:** `release-validation-1792076670/`.

Chain results:

```
lint-all-features: exit 0 []
host tests: ok
13.20 poweroff 1cpu: PASS (57s)
13.20 reboot 1cpu: PASS (34s)
13.19 windows-label 1cpu: PASS (37s)
13.20 poweroff 2cpu: PASS (18s)
13.20 reboot 2cpu: PASS (29s)
13.19 windows-label 2cpu: PASS (47s)
13.20 poweroff 4cpu: PASS (30s)
13.20 reboot 4cpu: PASS (20s)
13.19 windows-label 4cpu: PASS (32s)
13.19 test 1cpu: PASS (99s)
13.19 production 1cpu: PASS (30s)
13.19 unlabelled 1cpu: PASS (24s)
negative control 13.20 S5 sleep type ignored (suspend instead of off): caught ['timeout: the machine neither turned off nor reset', 'QEMU exit status 1, expected 0 (stopped by the guest)', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.20-poweroff-1cpu-1792074113568239300']
negative control 13.20 every reset method skipped: caught ['timeout: the machine neither turned off nor reset', 'QEMU exit status 1, expected 0 (stopped by the guest)', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.20-reboot-1cpu-1792074383549659200']
negative control 13.19 root-directory label ignored: caught ['FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-windows-label-1cpu-1792074649172553700']
13.12 production 1cpu: PASS (21s)
13.2 1cpu: PASS (27s)
13.12 production 2cpu: PASS (6s)
13.2 2cpu: PASS (13s)
13.12 production 4cpu: PASS (6s)
13.2 4cpu: PASS (13s)
13.17 ahci 1cpu: PASS (37s)
13.18 1cpu: PASS (34s)
20.1 1cpu: PASS (27s)
release: PASS (1753s)
DONE
```

**Negative controls**, each caught, on 1 CPU with the source restored byte
for byte:
- **Sleep type altered:** QEMU suspends instead of turning off.
- **Every reset method skipped.**
- **Root-directory label ignored:** the windows-label stick is not
  logged.

## Limits

- **Reset registers.** Only system-I/O reset registers are used directly;
  memory and PCI-configuration ones fall through to the 8042 and 0xCF9.
- **Not yet done (roadmap Stage 25):** sleep, wake, CPU idle and frequency
  management, battery, thermal and lid.
---

## Source: docs/audit-stage13-2-admission-routing-2026-10-13.md

# Stage 13.2 admission routing audit â€” 2026-10-13

PCI boot discovery now has an explicit discover-and-route admission phase.
`hal::init` publishes a discovery generation through `discover_and_route`; a
bridge-bearing generation is not driver-admissible until the complete
rollback-capable bridge transaction succeeds. A bridge-free generation is
admitted after discovery. Any discovery truncation or routing failure leaves
the admission flag false, and `bind_pci_with_bars` rejects claims fail-closed.

The existing hotplug path continues to route after ordered removal teardown,
so both initial admission and rescan admission use the same bridge transaction
boundary. This avoids exposing a device behind an unconfigured bridge window
to a driver.

Validation completed:

- warning-denying host and freestanding kernel Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791886751085795900`,
  `audit-artifacts/stage13.2-2cpu-1791886761254233800`, and
  `audit-artifacts/stage13.2-4cpu-1791886770270981900`.

QEMU does not expose a valid mappable physical BAR/bridge topology for a
positive hardware routing qualification. This increment proves admission
ordering and fail-closed behavior; physical PCIe bridge-window interoperability
and hotplug remain hardware-dependent.
---

## Source: docs/audit-stage13-2-bar-admission-2026-10-13.md

# Stage 13.2 automatic BAR admission audit â€” 2026-10-13

Normal `bind_pci_with_bars` now invokes the persistent BAR admission path after
claiming the function. If all required BARs are unassigned, the path reserves
and programs the complete BAR set transactionally, refreshes the published
device view, and only then leases the resulting MMIO authority. A mixed
assigned/unassigned layout is rejected fail-closed because moving only part of
a function requires device-specific quiescence.

The normal-bind path reroutes bridge forwarding after a successful rebalance.
On teardown, command decoding is disabled, every assigned BAR register is
cleared and verified, the published inventory is refreshed, bridge forwarding
is recomputed, and only then are allocator reservations and function authority
released. BAR-clear or routing failures retain retryable ownership state.

The binding records whether it owns a persistent rebalance. Ordered unbind
first quiesces interrupts and revokes owner mappings, releases ordinary BAR
leases, disables command-space decoding, releases the persistent BAR
reservations, and only then releases the PCI function claim. Any failed phase
leaves the binding in `Unbinding` with the remaining authority checkpointed for
retry.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791890690873217500`,
  `audit-artifacts/stage13.2-2cpu-1791890699703532100`, and
  `audit-artifacts/stage13.2-4cpu-1791890709466970800`.

The QEMU device set does not provide a valid unassigned physical BAR plus
firmware aperture for positive hardware allocation qualification. Mixed-layout
and physical BAR interoperability remain explicit hardware-dependent limits.
---

## Source: docs/audit-stage13-2-bar-resources-2026-10-13.md

# Stage 13.2 persistent BAR resource audit â€” 2026-10-13

The explicit firmware BAR rebalance path now owns its reservations in a
bounded persistent PCI resource state. A successful `rebalance_bars_from_firmware`
transaction retains the allocator reservations and records the function and
all six BAR assignment slots. Duplicate function admission and allocation
table exhaustion are rejected before hardware programming. The corresponding
`release_bars_from_firmware` path disables PCI command-space decoding, clears
and verifies every assigned BAR register, refreshes the published inventory,
then releases the exact reservations and removes the record only after every
step succeeds; a failed disable, clear, refresh, or release therefore leaves
the record authoritative for retry.

The existing transactional planner still reserves all BARs before config-space
programming and rolls back provisional reservations on probe, encoding,
verification, or restoration failure. The persistent state closes the prior
stack-lifetime defect where a successful rebalance returned assignments but
dropped the allocator that owned them.

Validation completed:

- freestanding kernel check with `stage13-2-test`;
- warning-denying freestanding kernel Clippy with `stage13-2-test`.
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, with exit 33 and
  evidence in `audit-artifacts/stage13.2-1cpu-1791886909736158900`,
  `audit-artifacts/stage13.2-2cpu-1791886920671967900`, and
  `audit-artifacts/stage13.2-4cpu-1791886932096798900`.

The BAR-clear, inventory-refresh, and reroute follow-up was revalidated by the
latest 1/2/4-CPU preservation gate:
`audit-artifacts/stage13.2-1cpu-1791890690873217500`,
`audit-artifacts/stage13.2-2cpu-1791890699703532100`, and
`audit-artifacts/stage13.2-4cpu-1791890709466970800`.

The full Stage 13.2 gate remains open: this increment is an explicit resource
admission primitive, not proof of automatic normal-bind BAR rebalance. Bridge
window routing, generic physical MSI/MSI-X activation, physical PCIe
hotplug, and valid physical BAR qualification remain outstanding.
---

## Source: docs/audit-stage13-2-driver-lifecycle-2026-10-13.md

# Stage 13.2 PCI/WovenDriver lifecycle integration audit â€” 2026-10-13

## Scope

This increment connects the existing generation-safe PCI topology and BAR
leases to the WovenDriver ownership table. The Stage 13.2 acceptance probe
selects one discovered PCI function, claims it for a bounded synthetic driver
owner, retains every currently programmed MMIO BAR that can be validated, and
tears the binding down in dependency order.

The probe is feature-gated and does not activate arbitrary hardware during a
normal production boot. It proves lifecycle integration without treating PCI
presence as driver compatibility.

## Invariants checked

- The function claim is published before the driver binding.
- BAR leases are retained only after generation and owner validation.
- Partial admission rolls back the outer function claim.
- Teardown quiesces subordinate resources before releasing function ownership.
- A failed or completed probe cannot leave the synthetic binding table entry.
- Existing unsupported devices remain unbound.

## Evidence

`run-stage13-2-preservation.ps1` passed:

- focused PCI host tests;
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings`;
- runtime harness regressions;
- QEMU Stage 13.2 boots on 1, 2, and 4 CPUs, exit 33.

The owner-scoped MMIO follow-up was also revalidated by this matrix. The
discovered QEMU firmware BAR at `0x10011af410011af0` is outside the kernel
direct-map range, so `map_mmio_owned` rejects it fail-closed; the driver still
completes ordered unbind, and the gate proves mapping, revoke, and same-page
remap on a reserved test page. This is not physical device-BAR qualification.
Current serial evidence is retained in `audit-artifacts/stage13.2-1cpu-1791884820933030900`,
`audit-artifacts/stage13.2-2cpu-1791884835369436700`, and
`audit-artifacts/stage13.2-4cpu-1791884857633831800`.

The latest probe also completes driver-aware no-removal rescan and bridge-route
reconciliation after teardown. Current evidence is retained in
`audit-artifacts/stage13.2-1cpu-1791885536550950300`,
`audit-artifacts/stage13.2-2cpu-1791885544104217200`, and
`audit-artifacts/stage13.2-4cpu-1791885553144637100`.

The rescan-specific follow-up passed with evidence in
`audit-artifacts/stage13.2-1cpu-1791885536550950300`,
`audit-artifacts/stage13.2-2cpu-1791885544104217200`, and
`audit-artifacts/stage13.2-4cpu-1791885553144637100`.

## Remaining Stage 13.2 limits

This closes the driver-binding integration and owner-scoped MMIO revoke
primitive items. Full Stage 13.2 remains open for live BAR rebalance/admission
policy, bridge-window routing in normal device admission, generic physical
MSI/MSI-X activation and quiescence, physical PCIe hotplug qualification, and
valid-BAR hardware qualification.
---

## Source: docs/audit-stage13-2-ecam-offset-2026-10-13.md

# Stage 13.2 ECAM offset audit â€” 2026-10-13

## Finding

The ECAM path passed the physical address of a configuration dword to the
legacy page mapper but used the returned page base without adding the dword's
offset within that page. Consequently, live QEMU PCI discovery read the
vendor/device dword for every ECAM offset, and ECAM writes could target the
page base instead of the requested register.

## Repair

`read_config_unlocked` and `write_config_unlocked` now add the physical page
offset after mapping and use checked arithmetic. Legacy I/O-port configuration
access is unchanged. This preserves the public PCI ABI while restoring correct
class-code, capability, BAR, command, and MSI configuration access.

## Evidence

- Warning-denying freestanding kernel Clippy passed for `stage13-2-test`.
- All-target warning-denying Clippy passed.
- The complete Stage 13.2 preservation chain passed on 1/2/4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791893707568086100`,
  `audit-artifacts/stage13.2-2cpu-1791893716313293800`, and
  `audit-artifacts/stage13.2-4cpu-1791893724978189100`.
- Serial evidence shows correct live class/capability decoding and generic MSI
  activation followed by ordered teardown on the QEMU SATA controller.

## Limits

This qualifies ECAM access and generic MSI lifecycle behavior in QEMU. It does
not qualify physical PCIe bridge interoperability, MSI-X table delivery,
hotplug, reset/recovery, or arbitrary physical hardware.
---

## Source: docs/audit-stage13-2-high-mmio-2026-10-13.md

# Stage 13.2 bounded high-BAR MMIO audit â€” 2026-10-13

## Scope

The previous MMIO mapper formed a virtual address by adding the physical BAR
address to `0xffff_fe00_0000_0000`. That arithmetic rejects high 64-bit BARs
and made owner-scoped MSI-X table mappings dependent on the BAR being in the
low alias range.

The mapper now retains the actual virtual address in every bounded mapping
entry. Low physical pages preserve the historical fixed alias; pages whose
physical address cannot fit that alias use a separate 256-slot high-MMIO
window. Release and owner revocation unmap the retained virtual address, so
teardown does not reconstruct or guess it.

## Evidence

- Warning-denying freestanding kernel Clippy passed for `stage13-2-test`.
- The Stage 13.2 probe activated and tore down a real QEMU MSI-X lease.
- The probe mapped physical `0x2000_0000_0000` to a high-window virtual page
  and released it successfully (`high-mmio ... released=true`).
- The complete 1/2/4-CPU preservation gate passed, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The high-MMIO window is deliberately bounded at 256 pages and is not a
general virtual-memory allocator. BARs larger than the supported mapping
capacity, bridge-window routing, physical hotplug, reset/recovery, and
hardware-specific device protocols remain separate acceptance work.
---

## Source: docs/audit-stage13-2-hotplug-insertion-2026-10-13.md

# Stage 13.2 hotplug insertion audit â€” 2026-10-13

PCI rescan now has a bounded insertion path in addition to removal detection.
`rescan_added` scans configuration space without publishing, compares
generation-owned topology addresses and full driver-relevant PCI identity, and returns
newly present or same-slot replacement functions with their probed device
descriptors. Reconciliation still occurs transactionally;
the driver-facing `hotplug_rescan_report` returns additions only after the new
topology is published and bridge routing succeeds. The existing summary-only
`hotplug_rescan` API is preserved as a compatibility wrapper.

If a replacement is detected at an existing BDF before the old owner has been
quiesced, the coordinator aborts the whole batch before issuing teardown
configuration writes. This fail-closed boundary avoids directing old-device
BAR/command cleanup at the replacement; platform hotplug notification must
establish quiescence before that case can be completed.

Newly detected functions are deliberately reported but not auto-bound. Driver
matching and initialization must supply an explicit compatible driver before
claiming a function; unknown hardware remains unowned. Removal now also
releases any persistent BAR rebalance reservation for an unbound function
before invalidating its topology generation, while preserving retryable error
state on disable/release failure.

Validation completed:

- host and freestanding warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791890128143342200`,
  `audit-artifacts/stage13.2-2cpu-1791890136688207600`, and
  `audit-artifacts/stage13.2-4cpu-1791890146247574800`.

QEMU does not provide a physical insertion/removal fixture, so live PCIe
hotplug signaling, link state, power control, and hardware removal races
remain unqualified.
---

## Source: docs/audit-stage13-2-interrupt-selection-2026-10-13.md

# Stage 13.2 generic interrupt selection audit â€” 2026-10-13

Bound PCI drivers now have a generic interrupt activation boundary. The
policy checks the published capability and retained BAR authority, attempts
MSI-X when it is admissible, and falls back to MSI only after a clean MSI-X
failure. If MSI-X reports `ActivationRetained`, the generic path returns that
live lease obligation without attempting fallback. Existing individual MSI,
MSI-X, attach, and ordered teardown APIs remain available and unchanged.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33, with
  disposable QEMU NVMe and e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.
- The QEMU serial log reports correct PCI class/capability decoding for the
  virtio fixture, `interrupt-selection=active`,
  `interrupt-selection teardown=true`, and successful high-MMIO release.

The probe attempts generic interrupt activation only when discovery reports a
capability and the required BAR is admissible. Generic MSI-X activation and
teardown are now qualified by the QEMU virtio fixture. Physical vector
delivery, PCIe link/power hotplug, reset/recovery, and physical hardware
qualification remain open.
---

## Source: docs/audit-stage13-2-msi-rollback-2026-10-13.md

# Stage 13.2 MSI programming rollback â€” 2026-10-13

## Scope

The generic owner-bound MSI path now snapshots the original capability header,
message address, optional 64-bit high address, and data dword before disabling
MSI. Every message/header write is read back before the next activation step,
including the initial disabled-header write; a failed initial write is repaired
before the error is returned.
If any later configuration write or verification fails, it restores those
fields with MSI disabled during the repair and restores the original header
last. A failed repair returns `RestoreFailed`; the caller therefore retains the
interrupt activation obligation instead of recycling a vector against
uncertain device state.

The existing vector and interrupt-mode ownership checkpoints remain ordered:
software ownership is acquired before programming, and vector reuse is allowed
only after successful hardware disable and mode release.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` â€” passed.
- `cargo clippy -p wovenhat-os --tests -- -D warnings` â€” passed.
- `run-stage13-2-preservation.ps1` â€” passed on 1/2/4 CPUs, exit 33.
- Latest preservation evidence after the initial-header rollback hardening,
  with disposable QEMU NVMe/e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The QEMU preservation device did not provide a valid directly mappable BAR for
physical MSI/MSI-X activation. This change proves rollback behavior and keeps
physical interrupt-controller qualification open.
---

## Source: docs/audit-stage13-2-msix-mapping-2026-10-13.md

# Stage 13.2 MSI-X owner-scoped table mappings â€” 2026-10-13

## Scope

MSI-X activation now asserts Function Mask and keeps MSI-X disabled before any
table read or write. Table reads, masked programming, restoration, and entry
unmasking use owner-scoped page mappings instead of the legacy shared MMIO
mapper. Pages are released immediately after each bounded volatile operation.
If a page cannot be released, the operation fails with `MmioMap`; if table
programming has already made the device-visible entry live, the activation
returns a retained `MsixLease` so the driver can quiesce it during ordered
teardown.

This preserves the existing public MSI-X capability and lease structures while
closing the vector-loss and unowned-mapping failure paths.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` â€” passed.
- `cargo clippy -p wovenhat-os --tests -- -D warnings` â€” passed.
- `run-stage13-2-preservation.ps1` â€” passed on 1/2/4 CPUs, exit 33.
- Latest evidence with disposable QEMU NVMe and e1000 PCI fixtures present:
  `audit-artifacts/stage13.2-1cpu-1791894430987033100`,
  `audit-artifacts/stage13.2-2cpu-1791894442432591200`, and
  `audit-artifacts/stage13.2-4cpu-1791894453903648700`.

## Limits

The QEMU virtio fixture exposed a valid MSI-X table BAR; the preservation probe
qualified Function Mask ordering, table writes, owner-scoped mappings, vector
ownership, and ordered teardown. Physical interrupt delivery, PCIe hotplug,
reset/recovery, and arbitrary hardware qualification remain open.
---

## Source: docs/audit-stage13-2-suspend-resume-2026-10-13.md

# Stage 13.2 PCI suspend/resume audit â€” 2026-10-13

Owned PCI suspend now validates the generation owner, disables I/O, memory,
and bus-master command decoding, and verifies the result. When firmware exposes
the PCI Power Management capability, it also checkpoints PMCSR and enters
D3hot with read-back verification. Resume validates ownership, restores D0,
then restores the exact command checkpoint with read-back verification. Any
failed transition remains fail-closed and attempts the safe rollback state.
PMCSR writes preserve the upper half of the containing configuration dword so
adjacent PM capability fields are not overwritten.
The driver manager uses this path only for PCI bindings; the established
non-PCI state transition is preserved. Missing checkpoints fail closed instead
of treating a PCI binding as a generic driver.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791889555695876300`,
  `audit-artifacts/stage13.2-2cpu-1791889566832554400`, and
  `audit-artifacts/stage13.2-4cpu-1791889577521462400`.

This validates the software lifecycle and the PMCSR transition when a valid
capability is present. Device-specific reset, D3cold, platform wake signaling,
and physical suspend/resume behavior remain hardware and driver dependent.
---

## Source: docs/audit-stage13-2-topology-2026-09-27.md

# Stage 13.2 PCIe topology/ownership audit â€” 2026-09-27

## Status

Bounded topology/ownership increment implemented. Full Stage 13.2 acceptance is **not claimed**.

Stage 13.3/NVMe is outside this change and must not be advanced by this audit.

## Implemented

- Allocation-free topology bounded at 64 PCIe functions and 96 logical MMIO leases.
- Generation-tagged function handles prevent stale slot reuse.
- Single-owner function claims reject owner zero and competing owners.
- Generation-tagged BAR leases require the current function owner.
- Function teardown invalidates every logical BAR lease before the function generation advances or its slot is reusable.
- Type-1 bridge primary/secondary/subordinate bus windows are recorded.
- Parent selection chooses the narrowest matching bridge window in the same segment.
- Removing a bridge reparents surviving children from current topology.
- Discovery builds replacement topology off-lock and publishes no partial topology after insertion failure.
- PCI rank-10 inventory/configuration locks are not nested inside the rank-20 topology lock.
- Runtime acceptance now requires the topology lifecycle marker; the former inventory-only marker cannot certify Stage 13.2.
- Focused host tests cover nested bridges, ownership rejection, stale handle/lease invalidation, duplicate rejection and bounded capacity.
- Dedicated preservation gate runs host topology tests, build, warning-denying kernel Clippy, runtime-harness tests and QEMU on 1/2/4 CPUs.

## MMIO lifetime boundary

The current topology lease is an authorization token. Teardown invalidates that logical authority before reuse.

The paging layer currently exposes MMIO mapping but no reviewed physical unmap/revoke primitive. Therefore this increment does **not** claim that page-table mappings are revoked during topology teardown. A physical mapping lifetime primitive is required before that stronger guarantee can be made.

## Full Stage 13.2 blockers

Full acceptance remains blocked until all of these are implemented and tested:

1. BAR size probing plus bounded I/O, 32-bit MMIO and 64-bit/prefetchable MMIO allocation/rebalance with rollback.
2. PCI-to-PCI bridge I/O, memory and prefetchable window routing/programming with subordinate resource validation.
3. Generic MSI and MSI-X vector allocation, programming, masking/quiescence and teardown; the existing AX200-specific MSI path is not sufficient.
4. PCIe hotplug/rescan with generation-safe insertion/removal and teardown ordering.
5. WovenDriver binding/unbinding integrated with PCIe generation ownership so removal cannot leave a bound driver or live lease.
6. Warning-denying Rust validation and the dedicated 1/2/4 CPU QEMU preservation matrix for the final implementation.

## Acceptance rule

Do not change Stage 13.2 to accepted, do not restore the inventory-only marker, and do not begin Stage 13.3 as a consequence of this increment until the blockers above are closed with evidence.
---

## Source: docs/audit-stage13-3-nvme-2026-10-13.md

# Stage 13.3 NVMe audit â€” 2026-10-13

## Result

The bounded Stage 13.3 NVMe acceptance gate passed on 1, 2, and 4 CPUs after
the controller-failure lifecycle repair:

- 1 CPU: `audit-artifacts/stage13.3-1cpu-1791894897482681000`
- 2 CPUs: `audit-artifacts/stage13.3-2cpu-1791894909297351100`
- 4 CPUs: `audit-artifacts/stage13.3-4cpu-1791894920160288000`
- marker: `=== STAGE 13.3 ACCEPTANCE: PASS ===`

The gate boots the QEMU NVMe fixture, identifies a namespace, creates admin
and I/O queues, writes and reads a disposable sector, and flushes it. The
feature-specific freestanding Clippy gate and `cargo build --features
stage13-3-test` also pass.

## Lifecycle repair

`DmaPage` now owns its physical frame and returns it to the frame allocator on
drop. Initialization records the original PCI command register and installs a
rollback guard immediately after enabling memory decoding and bus mastering.
Every later failure attempts bounded controller disable/readiness polling and
restores the exact PCI command value, preventing a failed probe from leaving
DMA enabled or leaking its queue/identify buffers.

## Acceptance boundary

This is a production-quality emulated NVMe foundation, not a claim of complete
physical NVMe support. The current implementation still uses bounded polling,
one namespace, one synchronous I/O queue, legacy per-access MMIO aliases, and
no interrupt-backed completion, reset/recovery workflow, hotplug removal, or
multi-namespace/multipath policy. Those are explicit prerequisites before a
physical NVMe device can be called production-qualified. The inserted USB
flash disk was not modified; it remains outside this QEMU-only acceptance.
---

## Source: docs/audit-stage13-4-7-validation-2026-10-14.md

# Stage 13.4â€“13.7 validation audit â€” 2026-10-14

The current tree revalidated the software-backed storage, USB-core, USB-HID,
and unified-input gates in order. Each harness exited through the debug-exit
success path on 1, 2, and 4 CPUs.

## Evidence

- Stage 13.4 AHCI/SATA DMA block I/O:
  `stage13.4-1cpu-1791981605010453300`,
  `stage13.4-2cpu-1791981675679929300`,
  `stage13.4-4cpu-1791981716077680700`.
- Stage 13.5 xHCI USB core:
  `stage13.5-1cpu-1791981759297896800`,
  `stage13.5-2cpu-1791981820766639900`,
  `stage13.5-4cpu-1791981864694609800`.
- Stage 13.6 USB HID:
  `stage13.6-1cpu-1791981914079760100`,
  `stage13.6-2cpu-1791981975767952400`,
  `stage13.6-4cpu-1791982020550370400`.
- Stage 13.7 WovenInput:
  `stage13.7-1cpu-1791982061927761800`,
  `stage13.7-2cpu-1791982127301158800`,
  `stage13.7-4cpu-1791982174067000900`.

These are QEMU software/contract gates. They do not prove native AHCI,
xHCI, USB-HID, or input-device interoperability on the inserted physical
machine. The current Stage 13.6 fixture exercises the bounded HID keyboard
path; USB mouse report decoding remains a separate production gap documented
by the stage status.
---

## Source: docs/audit-stage13-6-usb-mouse-decoder-2026-10-14.md

# Stage 13.6 USB HID mouse report decoder â€” 2026-10-14

`xhci::decode_boot_mouse` decodes the fixed USB HID boot-protocol report
without borrowing DMA memory or allocating. It bounds the report to the
three button bits, converts signed X/Y deltas to the WovenInput coordinate
convention, emits at most one movement event plus three button-edge events,
and returns the new button state for the next report. Publication remains the
caller's responsibility after releasing the controller lock.

The decoder's motion, press, and release paths are included in the xHCI
self-test. Host Clippy, freestanding Stage 13.6 Clippy, all host tests, and
the Stage 13.6 QEMU gate passed on 1/2/4 CPUs. Evidence:

- `stage13.6-1cpu-1791982285425284200`
- `stage13.6-2cpu-1791982343461199100`
- `stage13.6-4cpu-1791982390647822300`

This closes report decoding, not the whole device path. A production xHCI
input worker still needs to poll/submit mouse reports and publish them into
WovenInput, and native USB mouse interoperability remains hardware-dependent.
---

## Source: docs/audit-stage13-8-hda-mmio-2026-10-13.md

# Stage 13.8 HDA MMIO audit â€” 2026-10-13

The first Stage 13.8 run failed at HDA controller reset with
`Controller(ResetAssertTimeout)`. The cause was confirmed in the HDA MMIO
helpers: they mapped the containing page but discarded the register's page
offset, so all non-zero-offset registers accessed the BAR base.

The 8-, 16-, and 32-bit helpers now split every physical register address into
page and offset, map the page, and perform the volatile access at `mapped +
offset`. After the repair, the feature build, freestanding warning-denying
Clippy, and the complete QEMU matrix passed:

- 1 CPU: `audit-artifacts/stage13.8-1cpu-1791895760922139400`
- 2 CPUs: `audit-artifacts/stage13.8-2cpu-1791895771085454400`
- 4 CPUs: `audit-artifacts/stage13.8-4cpu-1791895781435410400`
- marker: `=== STAGE 13.8 ACCEPTANCE: PASS ===`

The gate covers the QEMU Intel HDA codec, topology, playback/capture stream
and mixer/API boundary. Physical codec interoperability, interrupt/DMA fault
recovery, and hardware qualification remain outside this deterministic gate.
---

## Source: docs/audit-stage13-driver-matching-2026-10-13.md

# Stage 13 driver matching audit â€” 2026-10-13

The bounded WovenDriver table now supports explicit PCI identity matchers
(vendor, device, class, subclass, and programming interface). An empty
matcher is rejected, matching is deterministic by registration order, and
`bind_matching_pci` separates discovery/matching from the existing
owner-token binding transaction. Unknown functions remain unclaimed.

The existing generic `register` and explicit `bind_pci` APIs remain unchanged.
Insertion reports can now be passed to the matcher without inventing a driver
for unsupported hardware.

Validation completed:

- freestanding and host warning-denying Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791887987650651800`,
  `audit-artifacts/stage13.2-2cpu-1791888007030937500`, and
  `audit-artifacts/stage13.2-4cpu-1791888027621533400`.

The Stage 13.2 runtime lifecycle probe also registers a vendor/device matcher,
verifies it selects the synthetic driver, and binds through
`bind_matching_pci` before exercising MMIO and teardown.

This adds matching policy, not device-specific drivers or physical PCIe
interoperability qualification.
---

## Source: docs/audit-stage14-2b-ipv6-neighbor-foundation-2026-10-06.md

# Stage 14.2B IPv6 neighbor foundation audit â€” 2026-10-06

## Scope

This increment adds production-reusable, allocation-free IPv6 address and
prefix primitives: prefix containment, link-local/multicast classification,
and RFC 4291 solicited-node multicast derivation. The acceptance self-test
exercises the exact primitives used by the future Neighbor Discovery path and
fails closed on malformed prefix lengths.

This is not yet IPv6 packet transport, ICMPv6 Neighbor Solicitation/Advertisement
handling, Duplicate Address Detection, Router Solicitation/Advertisement,
DHCPv6, or physical IPv6 qualification.

## Verification

The following checks are required before this commit is accepted:

- `cargo build`
- warning-denying freestanding kernel Clippy
- host tests
- `cargo build --features stage14-2-test`
- warning-denying feature Clippy
- `scripts/test-network-qemu.py --feature stage14-2-test` on 1, 2, and 4 CPUs
- complete release validation

The resulting serial evidence must contain both `[S14.2A]` and `[S14.2B]`
success markers for each CPU configuration.

## Limits and next prerequisite

No physical NIC or IPv6 wire claim is made. The next prerequisite is a bounded
ICMPv6 Neighbor Discovery message/parser boundary backed by owned packet data,
before enabling live IPv6 interface configuration.
---

## Source: docs/audit-stage14-2c-icmpv6-neighbor-parser-2026-10-06.md

# Stage 14.2C ICMPv6 Neighbor Discovery parser audit â€” 2026-10-06

## Scope

This increment adds a bounded parser for ICMPv6 Router Solicitation,
Router Advertisement, Neighbor Solicitation, and Neighbor Advertisement
envelopes. It validates type/code, fixed-body length, non-multicast neighbor
targets, and complete non-zero-length options, with a 1 KiB input bound. The
parser borrows only the caller-owned option slice and does not retain packet
state for asynchronous use.

The parser deliberately does not verify the IPv6 pseudo-header checksum or
perform stateful Neighbor Discovery actions. Duplicate Address Detection,
router lifetime/prefix processing, DHCPv6, live IPv6 sockets, and physical
qualification remain outside this increment.

## Verification

- `cargo build --features stage14-2-test`
- warning-denying feature Clippy
- `scripts/test-network-qemu.py --feature stage14-2-test` on 1/2/4 CPUs
- complete release validation

Each network serial log must contain `[S14.2A]`, `[S14.2B]`, and `[S14.2C]`
success markers.

---

## Source: docs/audit-stage14-2e-icmpv6-checksum-2026-10-06.md

# Stage 14.2E ICMPv6 checksum boundary audit â€” 2026-10-06

## Scope

This increment adds ICMPv6 pseudo-header checksum calculation and verification
for caller-owned packets, plus a checked Neighbor Discovery parser entry point.
The boundary authenticates source and destination IPv6 addresses, payload
length, next-header value 58, and the packet checksum before returning a
parsed message. Packets remain bounded to 1 KiB and no packet data is retained
for asynchronous use.

The implementation does not yet maintain Neighbor Discovery state, process
router lifetimes or prefixes, perform Duplicate Address Detection, implement
DHCPv6, expose live IPv6 sockets, or qualify physical hardware.

## Verification

- `cargo build --features stage14-2-test`
- warning-denying feature Clippy
- `scripts/test-network-qemu.py --feature stage14-2-test` on 1/2/4 CPUs
- complete release validation

Each focused serial log must contain `[S14.2A]` through `[S14.2E]` success
markers.
---

## Source: docs/audit-stage14-2f-neighbor-state-2026-10-06.md

# Stage 14.2F Neighbor cache and DAD state audit â€” 2026-10-06

This increment adds an eight-entry expiring neighbor cache, checksum-validated
Neighbor Advertisement application, and explicit Duplicate Address Detection
state transitions. Invalid message types, checksums, option shapes, and table
capacity fail closed. State is bounded and does not retain packet slices.

The implementation does not yet transmit Neighbor Solicitations, integrate a
live IPv6 interface, implement DHCPv6, or qualify physical IPv6 hardware.

Verification requires feature build and Clippy, the complete release matrix,
and Stage 14.2 focused QEMU acceptance on 1/2/4 CPUs with markers Aâ€“F,
including `[S14.2F] WovenNet neighbor state and DAD: PASSED`.
---

## Source: docs/audit-stage14-2j-ipv6-interface-ingress-2026-10-06.md

# Stage 14.2J IPv6 interface ingress audit â€” 2026-10-06

This increment adds a bounded ingress coordinator for caller-owned ICMPv6
packets. It verifies the IPv6 pseudo-header checksum, parses the envelope,
dispatches Router Advertisement and Neighbor messages into existing bounded
state, and emits explicit ingress events. Tampered packets are rejected before
state mutation; packet slices are not retained.

This remains a software interface boundary. IPv6 address configuration,
controlled packet egress, live sockets, and physical NIC/radio qualification
remain open. DHCPv6 protocol framing is covered by the preceding Stage 14.2H
foundation but live lease exchange is not.

Verification requires feature build and warning-denying Clippy, the complete
release matrix, and focused Stage 14.2 QEMU acceptance on 1/2/4 CPUs with
markers Aâ€“J.

Verification on the exact source tip:

- `cargo build --features stage14-2-test` â€” PASS.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage14-2-test -- -D warnings` â€” PASS.
- The focused QEMU gate passed on 1, 2, and 4 CPUs.
---

## Source: docs/audit-stage14-3-socket-api-2026-10-06.md

# Stage 14.3 WovenNet socket API audit â€” 2026-10-06

Stage 14.3 hardens the existing socket ABI boundary without changing the
accepted descriptor, owner, generation, or asynchronous completion model.
Socket connect endpoints now fail closed for zero ports, unspecified IPv4
addresses, and IPv4 multicast destinations. Packed IPv4 endpoint decoding is
validated before it reaches smoltcp, and valid endpoint packing round-trips
are covered by a deterministic self-test.

This is bounded software validation. The current transport is IPv4-only;
live IPv6 sockets, routing-table policy, and physical NIC qualification remain
outside this stage.

Verification on the exact source tip:

- `cargo build --features stage14-3-test` â€” PASS.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage14-3-test -- -D warnings` â€” PASS.
- `scripts/test-network-qemu.py --feature stage14-3-test --cpus 1` â€” PASS.
- The same QEMU gate on 2 and 4 CPUs â€” PASS.
---

## Source: docs/audit-stage14-4-routing-table-2026-10-06.md

# Stage 14.4 WovenNet routing-table audit â€” 2026-10-06

Stage 14.4 adds a bounded routing-table foundation with eight route slots.
Routes carry a generation handle, prefix length, gateway, and metric. Lookup
selects the longest matching prefix, then the lowest metric, then the oldest
generation. Invalid prefix lengths and zero gateways fail closed; removal
requires the exact generation-tagged route value.

This is deterministic software validation. It does not yet mutate the live
smoltcp route set, provide policy/routing daemons, or qualify physical NICs.

Verification requires the Stage 14.3 regression chain, build, warning-denying
kernel Clippy, and the focused QEMU network gate on 1/2/4 CPUs.
---

## Source: docs/audit-stage14-5d-revalidation-2026-10-14.md

# Stage 14.5D network revalidation â€” 2026-10-14

The current tree passed the live Stage 14.5D network suite on 1, 2, and 4
CPUs. Each run completed DHCP, DNS, ICMP, host-verified UDP and TCP
round-trips, SMP/TLB checks, and the firewall transport/path marker with exit
33. The harness also verifies the host-forwarded UDP and TCP payloads rather
than accepting only an in-guest marker.

Evidence is retained in the harness output directories:

- `target/network-regression-1-debug/serial.log`
- `target/network-regression-2-debug/serial.log`
- `target/network-regression-4-debug/serial.log`

This is a QEMU virtio-network qualification. Native NIC/radio drivers,
physical link behavior, and hardware-specific offload/interrupt behavior
remain outside this evidence.
---

## Source: docs/audit-stage14-feature-gates-2026-10-13.md

# Stage 14 feature-gate audit â€” 2026-10-13

Stage 14.1 through 14.5D were exercised locally with the repositoryâ€™s live
network harness. Each completed the feature build, warning-denying freestanding
kernel Clippy, DHCP/DNS/ICMP, host-verified UDP/TCP, and final SMP/TLB markers
on 1, 2, and 4 CPUs. The strongest final gate, `stage14-5d-test`, passed all
three CPU counts.

During Stage 14.1 linting, PCI persistent BAR admission types had
`expect(dead_code)` guards that stopped at Stage 13.9. The guards now include
the later Stage 14â€“20 feature family, so shared PCI code is warning-clean when
compiled through later gates.

This validates the bounded software/network contracts. It does not qualify
physical NICs, radio hardware, firmware interoperability, or unrestricted
production deployment.
---

## Source: docs/audit-stage15-3-permission-broker-2026-10-13.md

# Stage 15.3 permission broker audit â€” 2026-10-13

Stage 15.3 now has a bounded runtime permission broker. Requests carry a
generation-tagged handle, owner task, requested authority, and reason digest;
the state machine distinguishes pending, resolving, granted, denied, and
revoked. Capacity is fixed at 32 entries and stale handles fail closed.

Decisions require `TaskControl`, and successful grants/revokes still traverse
the existing WovenGuard lineage and sandbox checks. Request, decision, and
revoke events are recorded in the security ledger. The userspace ABI exposes
request, poll, decide, and revoke operations as syscall numbers 122â€“125, with
typed `libwoven` wrappers.

Validation:

- freestanding Stage 15.3 Clippy: passed with `-D warnings`;
- host/all-target Clippy: passed with `-D warnings`;
- 1 CPU: `audit-artifacts/stage15.3-1cpu-1791899076247234100`;
- 2 CPUs: `audit-artifacts/stage15.3-2cpu-1791899112419732700`;
- 4 CPUs: `audit-artifacts/stage15.3-4cpu-1791899123374724100`;
- marker: `[S15.3] permission broker: PASSED`.

This closes the kernel broker and ABI boundary. A graphical permission UI,
application identity/signature policy, and persistent user consent database
remain later Stage 15 requirements.
---

## Source: docs/audit-stage15-4-signed-applications-2026-10-13.md

# Stage 15.4 signed applications â€” cryptographic boundary accepted (2026-10-13)

The signed-image cryptographic boundary is accepted on 1/2/4 CPUs. The kernel
now uses the pure-Rust, `no_std` `ed25519-compact` implementation and validates
a bounded versioned `WAPP` envelope before exposing its immutable payload.
Malformed envelopes and payload tampering fail closed.

## Investigation

The first Ed25519 attempt used `ed25519-dalek`; its freestanding compile
reached `curve25519-dalek`, which selected its x86_64 SIMD backend and failed
in the pinned nightly LLVM with:

`rustc-LLVM ERROR: Do not know how to split the result of this operator!`

That dependency was removed. The replacement has no C/assembly build step and
passes warning-denying freestanding Clippy. The 1/2/4-CPU runtime evidence is:

- `audit-artifacts/stage15.4-1cpu-1791903175601544700`
- `audit-artifacts/stage15.4-2cpu-1791903335400954700`
- `audit-artifacts/stage15.4-4cpu-1791903444701949100`

After application trust-root provisioning was added, the final revalidation
also passed:

- `audit-artifacts/stage15.4-1cpu-1791957273567583900`
- `audit-artifacts/stage15.4-2cpu-1791957346171841400`
- `audit-artifacts/stage15.4-4cpu-1791957408773610900`

After signed external-application loading was tightened, the prior Stage 15.4
gate was rerun on the final loader boundary:

- `audit-artifacts/stage15.4-1cpu-1791961845238807400`
- `audit-artifacts/stage15.4-2cpu-1791961914527257900`
- `audit-artifacts/stage15.4-4cpu-1791961962399446600`

All three exited 33 with the required marker.

## Remaining acceptance requirements

- immutable firmware/installer-owned trust-root provisioning and
  rotation/revocation policy;
- enforcement before every executable load, including all internal launch
  paths, `exec`, and shell launch;
- rollback metadata, key rotation, and production signed-package tooling;
- refreshed 1/2/4-CPU evidence for the final loader integration.

Historical boundary note: the WAPP boundary defaulted to the development
application trust root for test images, while release builds now require the compile-time
`WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` provisioning value. The RFC self-test is
kept independent of the release key. Firmware/installer-owned key slots,
package rollback metadata, and release package tooling remain open.

The external-application pipeline now includes `tools/wapp-packager`, which
signs the Stage 16.1 ELF before the kernel artifact is compiled. The Stage 16.1
1/2/4-CPU gate verifies that package before ELF parsing and again after the VFS
round trip. This closes the previously missing signed-package integration for
that acceptance path. User-triggered loads reject raw external ELF except for
the finite kernel-owned system-image allowlist. The ten-application catalog,
rollback-safe RAM publication, release packaging, and production system-app
boot are now covered by the current Stage 19 release chain.
Installer/firmware-owned key slots, deployment-key rotation/revocation, and
physical rollback qualification remain open.
Kernel-internal system images remain explicitly trusted system paths;
user-controlled `exec`, command, and spawn loads use the signed-package
boundary.
---

## Source: docs/audit-stage15-5-secure-boot-2026-10-13.md

# Stage 15.5 secure boot â€” signed handoff accepted (2026-10-13)

The kernel now performs a fail-closed signed handoff check before mutable
allocator, paging, driver, or userspace initialization. The manifest signs a
canonical executable PT_LOAD representation sourced from the bootloader's
immutable kernel ELF range. The boot trust key is separate from the Stage
15.4 application key. Release builds now require both
`WOVENHAT_BOOT_SIGNING_SEED_HEX` and the matching
`WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX`; the image builder rejects a mismatch.
The negative provisioning check was exercised on 2026-10-14 with the final
public-key byte changed; Cargo failed in `build.rs` with
`boot signing seed/public key mismatch` before producing an image.

Acceptance passed with the exact current source and required CPU counts:

- `audit-artifacts/stage15.5-1cpu-1791910199679516200`
- `audit-artifacts/stage15.5-2cpu-1791910271627070600`
- `audit-artifacts/stage15.5-4cpu-1791910414693161200`

After release-key provisioning was added, the final revalidation also passed:

- `audit-artifacts/stage15.5-1cpu-1791911220669252100`
- `audit-artifacts/stage15.5-2cpu-1791911441869901500`
- `audit-artifacts/stage15.5-4cpu-1791911631937944200`

The current bootloader API exposes the loaded kernel range and memory map, but
does not expose UEFI Secure Boot state, authenticated variable access, or a
TPM/event-log interface to the kernel. Firmware enforcement, signed-initrd
policy, revocation, and rollback protection remain platform requirements.

This accepts the software signed-handoff boundary, not full platform Secure
Boot. The current bootloader API does not expose UEFI Secure Boot state,
authenticated variable access, TPM/event-log interfaces, release-key
provisioning, signed-initrd policy, revocation, rollback protection, or
negative tamper/rollback evidence. Those firmware, provisioning, and hardware
qualification requirements remain open.

Current status: the software signed-handoff gate is accepted; the title's
platform-level â€œsecure bootâ€ wording remains intentionally conservative.
# Stage 15.5 secure boot â€” signed handoff accepted (2026-10-13)

---

## Source: docs/audit-stage15-6-software-measurement-2026-10-13.md

# Stage 15.6 measured boot â€” software measurement accepted (2026-10-13)

The kernel now performs a bounded SHA-256 measurement of the bootloader-loaded
kernel ELF range after the physical-memory mapping is established and before
driver/userspace activation. The measured range is limited to 64 MiB and is
read only through the bootloader-provided physical address and length. A
known-answer SHA-256 self-test and a serial measurement prefix are included.

Warning-denying freestanding Clippy passed. The runtime gate passed with exit
33 on all required CPU counts:

- `audit-artifacts/stage15.6-1cpu-1791902683781855300`
- `audit-artifacts/stage15.6-2cpu-1791902842148564400`
- `audit-artifacts/stage15.6-4cpu-1791902986405576100`

This is not TPM-backed measured boot. The current bootloader API does not pass
TPM event-log access or UEFI Secure Boot state to the kernel. Firmware PCR
extension, event-log publication, signed kernel/initrd enforcement, and
hardware qualification remain open Stage 15.5/15.6 requirements.
---

## Source: docs/audit-stage15-7-wovenvault-handles-2026-10-13.md

# Stage 15.7 WovenVault handle audit â€” 2026-10-13

The WovenVault ABI now has a bounded volatile secret store. Secrets are copied
into 16 fixed 256-byte kernel slots, addressed by generation-tagged handles,
and are never exposed as kernel pointers. Reads require the owning task and
copy through the validated userspace-memory helpers; revoke and owner teardown
wipe slot bytes with volatile stores. Stale generations, foreign owners,
capacity, empty values, and oversized values fail closed.

The new `Secrets` capability is absent from ordinary userspace defaults but is
inside the userspace sandbox ceiling so a future permission decision can grant
it. Syscalls 127â€“129 expose store/read/revoke, and `libwoven` provides typed
wrappers.

Validation:

- freestanding kernel Clippy with `-D warnings`: passed;
- 1 CPU: `audit-artifacts/stage15.7-1cpu-1791901065186624800`;
- 2 CPUs: `audit-artifacts/stage15.7-2cpu-1791901104773292200`;
- 4 CPUs: `audit-artifacts/stage15.7-4cpu-1791901119604868100`;
- marker: `[S15.7] WovenVault handles: PASSED`.

This closes the volatile handle and teardown boundary. The later kernel
security pass added authenticated persistent wrapped-key records, encrypted
mount-record I/O, and crash-safe successor-generation rotation; those are
covered by `audit-volume-crypto-2026-09-16.md`. TPM/firmware-backed key
provisioning, deployment key custody/revocation, physical power-loss
qualification, and a graphical secrets UI remain required for a complete
WovenVault product.
---

## Source: docs/audit-stage15-8-security-ledger-2026-10-13.md

# Stage 15.8 security-ledger ABI audit â€” 2026-10-13

Stage 15.8 now exposes a bounded, newest-relative security-ledger read ABI.
The kernel authorizes reads with `TaskInspect`, validates the userspace output
buffer, and returns a fixed 48-byte record without exposing internal ledger
layout. `libwoven` decodes the record into a typed `AuditEvent`; unknown action
values fail closed instead of being silently reinterpreted.

The read path is newest-relative and bounded by the existing ledger capacity.
Sequence, tick, CPU, action, allow/deny, actor, target, and detail fields are
copied only after the event is read under the ledger lock. The gate also checks
that a fresh permission-decision event can be observed through the ABI.

Validation:

- `cargo build --features stage15-8-test`: passed;
- freestanding kernel Clippy with `-D warnings`: passed;
- host/all-target Clippy and host tests were previously green before this
  gate; the shared checks are rerun after the Stage 15.8 documentation update;
- 1 CPU: `audit-artifacts/stage15.8-1cpu-1791899464254186200`;
- 2 CPUs: `audit-artifacts/stage15.8-2cpu-1791899481574683100`;
- 4 CPUs: `audit-artifacts/stage15.8-4cpu-1791899497818349600`;
- marker: `[S15.8] security ledger ABI: PASSED`.

This closes the kernel and userspace ledger-read boundary. A full security
center UI, durable export/retention policy, signed identity model, and secure
or measured boot remain product-hardening work; this gate does not claim those
features.
---

## Source: docs/audit-stage15-9-exploit-mitigations-2026-10-13.md

# Stage 15.9 exploit-mitigation audit â€” 2026-10-13

The Stage 15.9 gate verifies the existing production user-space mitigations
and adds a scheduler-checked canary to every static kernel task stack. The
canary is installed whenever a task stack is initialized or recycled and is
checked before a scheduler hand-off; the boot self-test also proves that a
reserved stack's sentinel corruption is detected and restored.

The gate rechecks the ELF loader's W^X and unsupported-program-header rejection
paths. Existing boot checks continue to cover unmapped user-stack guard pages,
non-executable anonymous mappings, and randomized production ELF/stack/mmap
bases (the deterministic `qemu-test` configuration keeps acceptance addresses
stable).

Validation:

- freestanding kernel Clippy with `-D warnings`: passed;
- 1 CPU: `audit-artifacts/stage15.9-1cpu-1791900119875786500`;
- 2 CPUs: `audit-artifacts/stage15.9-2cpu-1791900214713058600`;
- 4 CPUs: `audit-artifacts/stage15.9-4cpu-1791900228440378900`;
- marker: `[S15.9] exploit mitigations: PASSED`.

The canary is intentionally not described as a virtual guard page. Kernel task,
privilege, and double-fault stacks remain static storage, so an unmapped kernel
stack guard-page design, plus hardware-specific SMEP/SMAP enablement and
qualification, remains open. Dynamic linking/TLS/RELRO are still rejected by
the loader until their complete implementation exists.
---

## Source: docs/audit-stage1-5-acceptance-2026-09-16.md

# Stages 1â€“5 acceptance audit â€” 2026-09-16

The complete Stage 1â€“5 boot gate passed with exit 33 on 1, 2 and 4 CPU QEMU
profiles. The gate covers boot, memory and paging, scheduler/task lifecycle,
VFS/storage, Ring-3 userspace, VirtIO networking, and the bounded cache/failure
regressions currently available in this repository.

This is acceptance evidence for the implemented QEMU scope. Production gaps
remain: non-QEMU hardware drivers and qualification, DMA/interrupt-backed
storage completion, Unicode normalization/larger path ABI, collision-resistant
and full-capacity metadata identity, full durable multi-operation journal replay,
and unclean-shutdown hardware qualification. Bounded long-name create/lookup/delete/rename,
named listing, directory growth, VFS uid/gid/mode enforcement, and the bounded
FAT32 ownership metadata sidecar and pending-intent recovery are covered by
the implementation and boot gate.
---

## Source: docs/audit-stage1-5-batch-journal-2026-10-06.md

# Stage 1â€“5 bounded batch data-journal audit â€” 2026-10-06

## Scope

The FAT32 persistence layer now has a separate `WDJ2` batch journal for up to
four file replacements. The existing `WDJ1` single-file format and WMD1/WMD2
ABIs are unchanged. A prepared batch records each path, prior existence, prior
bytes, intended length, and intended checksum. Metadata intents are published
before data and finalized after all data writes; mount recovery removes stale
metadata intents when rolling data back. A committed batch is retired only
after all new images match their recorded lengths and checksums; otherwise
recovery restores the prior images.

The new `persist_paths_atomic` API rejects empty, duplicate, malformed, over-
capacity, and over-four-entry batches. Journal records have bounded parsing,
path confinement, version checks, and a whole-record checksum.

## Evidence

- `cargo check -p wovenhat-kernel --target x86_64-unknown-none` â€” passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings` â€” passed.
- `cargo test -p libwoven` â€” passed.
- `python scripts/test-storage-qemu.py --cpus 1` â€” passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 2` â€” passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 4` â€” passed, exit 33.
- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\RUN-STAGE10.7.ps1` â€” passed, including host regressions, warning-denying Clippy, host tests, the 1/2/4-CPU memory matrix, live networking, and isolated Stage 10.4â€“10.7 preservation boots.

The workspace-wide `cargo test --workspace --all-targets` remains unsuitable
for this `no_std` kernel because it attempts to link the kernel's panic and
allocation handlers against the host `std` test harness; that pre-existing
failure was preserved and not suppressed.

## Limits

This increment implements the bounded multi-file data rollback/replay
boundary. The live disposable FAT32 mutation test exercises a two-file
`persist_paths_atomic` commit and readback on the QEMU storage path. Its
recovery subtest simulates a prepared partial write, a committed partial
write, and a fully committed batch before invoking recovery and verifying
on-disk bytes. The live commit also verifies a WMD2 inode metadata record
(`uid=1001`, `gid=1002`, `mode=0640`) after the batch completes. The kernel
self-test additionally injects pending WMD1/WMD2 metadata intents into the
prepared and partial-commit recovery cases and verifies that rollback removes
them. The kernel
self-test covers WDJ2 encoding, parsing, checksum rejection, and capacity
bounds. It does not establish
physical storage/DMA qualification, unclean-shutdown hardware evidence,
arbitrary-size transactions, metadata transactions, or full Stage 1â€“5
production completion.
---

## Source: docs/audit-stage1-5-data-journal-2026-10-01.md

# Stage 1â€“5 durable data journal â€” 2026-10-01

FAT32 persistence now writes a bounded on-volume `WOVENJR.BIN` transaction
record before replacing a file. Version 2 records contain the target path,
whether the previous file existed, the previous file bytes up to the VFS
capacity, the intended new length, a prepared/committed state, and checksums
for both the rollback payload and intended replacement. Version 1 records are
still accepted during recovery.

The ordering is:

1. write and flush the prepared record;
2. write the replacement data and metadata;
3. write and flush the committed record;
4. remove and flush the journal record.

Mount recovery rolls back a prepared record to the previous bytes (or removes
the newly-created target). A committed version 2 record is discarded only
after the target's length and checksum match the intended replacement;
otherwise recovery restores the previous bytes. Invalid or tampered records
fail closed rather than being replayed. The journal file is excluded from VFS
import.

The Stage 1â€“5 self-test covers prepared/committed encoding and payload
tampering. The complete QEMU gate passed on 1, 2 and 4 CPUs with exit 33 after
the journal was integrated into live FAT32 persistence.

This closes bounded single-file data rollback and checksum replay. Multi-file
atomic transactions, files larger than the bounded VFS payload, and physical
power-loss qualification remain open.

## Version 2 replay hardening

Version 2 records also store the intended replacement length and checksum.
During mount recovery, a committed record is retired only when the target
exists with the expected length and checksum. A mismatch re-enters the
rollback path. Legacy version 1 records remain readable and retain their prior
committed-record behavior.
---

## Source: docs/audit-stage1-5-dma-allocation-2026-10-14.md

# Stage 1â€“5 DMA allocation cleanup audit â€” 2026-10-14

## Scope

This increment hardens the existing AHCI and NVMe block-driver DMA ownership
paths without changing the `BlockDevice` ABI or claiming physical-device
qualification. A DMA page now returns its allocator frame if the physical
memory mapping cannot be established. AHCI also releases all pages when
controller initialization fails before the controller becomes globally owned.
The existing NVMe `DmaPage` drop path remains the owner of successful and
failed initialization allocations.

The controller objects are intentionally process-lifetime resources after
successful initialization: the controller lock owns the DMA pages while the
device can issue commands. No page is released while hardware may still
reference it.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-3-test -- -D warnings` â€” passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-4-test -- -D warnings` â€” passed.
- `cargo clippy --all-targets -- -D warnings` â€” passed.
- `cargo test -p wovenhat-os --tests --no-fail-fast` â€” passed, including the
  WovenFS crash, page-cache, PCI, async-runtime, display, and Wi-Fi suites.
- NVMe QEMU controller gate, 1/2/4 CPUs â€” passed (exit 33):
  `audit-artifacts/stage13.3-1cpu-1791970727679324400`,
  `audit-artifacts/stage13.3-2cpu-1791970800950732500`,
  `audit-artifacts/stage13.3-4cpu-1791970842780539000`.
- AHCI QEMU controller gate, 1/2/4 CPUs â€” passed (exit 33):
  `audit-artifacts/stage13.4-1cpu-1791970885814072600`,
  `audit-artifacts/stage13.4-2cpu-1791970942613349500`,
  `audit-artifacts/stage13.4-4cpu-1791971012860405400`.

## Limits

This is allocator/error-path evidence only. AHCI/NVMe physical controller
qualification, DMA/IOMMU isolation, interrupt-driven completion, torn-sector
behavior, power-loss recovery, arbitrary-size/full-volume transactions, and
the inserted USB flash device remain unqualified. The flash device was not
written or erased by this audit.
---

## Source: docs/audit-stage15-durable-vfs-closure-2026-10-14.md

# Stage 15 durable VFS closure audit â€” 2026-10-14

## Scope and verified integration

This review examines the current `master` source, rather than assuming that a merged pull request implies release acceptance.

- `kernel/src/task.rs::open_current` checks `FileRead` and path authority; creating a missing `/wfs/` file additionally checks `FileWrite` and path authority, persists it via `wfs::ensure_vfs_path`, then creates a read-only VFS cache entry.
- `kernel/src/task.rs::write_current` checks `FileWrite` and scope, dispatches `/wfs/` writes to `wfs::write_vfs_path_at`, and publishes the returned image via `vfs::commit_backend_write`.
- `kernel/src/task.rs::unlink_current` checks `FileWrite` and path authority and unlinks the native file before removing the VFS node.
- `kernel/src/wfs.rs::write_vfs_path_at` serializes native mutations with `MUTATION`, writes the complete file image, and returns success only after native write completion.
- `kernel/src/vfs.rs::commit_backend_write` checks descriptor offset and generation, updates the VFS image and file-frame cache, and leaves generic VFS writes disabled for the native-backed node.

**Correction to earlier milestone notes:** ordinary task-level file-write dispatch is already present on `master`. The remaining work is *concurrency, failure and reboot qualification*, not first-time syscall wiring.

## Closure risks requiring proof

1. **Durable-write/cache-publication split.** The native transaction commits before `commit_backend_write` verifies descriptor offset and generation. If another writer moves the descriptor or invalidates the node between these operations, a syscall may report failure although the durable bytes changed. Test concurrent writers and define serialization or reconciliation semantics.
2. **Native/VFS multiwriter ordering.** Native writes are protected by `wfs::MUTATION`, but the VFS publication happens after releasing that lock. Two successful native writes could publish cached images out of order. Test interleavings with separate descriptors and compare on-disk, VFS and file-frame contents.
3. **Crash consistency.** Simulate interruption at each journal/checkpoint boundary, remount and check that the file is entirely old or entirely new; repeat on physical media with controlled power-loss testing.
4. **Open-handle invalidation.** Test unlink/recreate while descriptors and memory mappings exist, then check generation, cache invalidation and access checks.
5. **Authorization.** Verify `FileRead`/`FileWrite` and path-scoped grants through real syscalls, including denied writes, denied create, denied unlink and sibling-prefix traversal paths.
6. **Capacity and failure injection.** Exercise full disk, maximum filename, maximum file length, out-of-range offset, device I/O errors and failed VFS publication. Ensure reported errors and persisted data agree.

## Acceptance gate

- Unit and integration tests for each risk above, including injected failures.
- QEMU 1/2/4 CPU test matrix; tests must inspect *persisted data after remount*, not only pass markers.
- `cargo clippy` warning-denying gate for kernel and relevant targets, plus host integration tests.
- Real hardware disk tests (or explicitly documented waiver) for persistence and power-loss.
- Record exact commands, SHA, logs, failures and outcomes in the Architecture & Codebase Guide.
- Keep this PR in draft until the acceptance evidence exists; do not claim production readiness on merge alone.

## Next implementation priority

Make native durable commit and VFS publication linearizable with respect to concurrent writers, without holding the VFS registry lock during disk I/O. Add an explicit per-file operation coordination strategy and a regression test that demonstrates the race before changing behavior.
---

## Source: docs/audit-stage1-5-fault-injection-2026-10-14.md

# Stage 1â€“5 FAT32 write-fault audit â€” 2026-10-14

## Result

The FAT32 mutation self-test now runs a deterministic write-budget fault at
each sector-write boundary of a bounded file creation. For every injected
`DeviceFault`, it verifies that the volume remains structurally checkable, the
new file is absent, and the pre-existing file still returns its original
bytes. It also verifies successful completion after the fault range.

The test is part of `fat32::self_test()`, so the existing Stage 1â€“5 storage
acceptance gate exercises it in the freestanding kernel as well as the host
regression suite. The production `BlockDevice` ABI is unchanged; the fault
device exists only in the self-test implementation.

## Boundary and limits

The injected device models a sector write that fails and returns to the caller.
It does not model power removal during a device programming cycle, volatile
controller caches, torn sectors, DMA completion ordering, or a reboot into a
new kernel. FAT32 FSInfo is an advisory hint and may be reported as stale by
the checker after an injected failure; FAT and directory structure are the
authoritative recovery assertions in this bounded test.

Physical flash/ATA qualification, torn-write recovery, arbitrary-size or
full-volume rollback, and unclean-shutdown reboot evidence remain open and are
not claimed by this audit.
---

## Source: docs/audit-stage1-5-immediate-rollback-2026-10-13.md

# Stage 1â€“5 immediate batch-rollback hardening â€” 2026-10-13

## Scope

`persist_paths_atomic` now treats the on-volume WDJ2 record as an active
transaction, not only as reboot evidence. After the prepared record is durable,
any later write, metadata, commit-record, or journal-retirement error invokes
the same generation/checksum-aware recovery path before returning the original
error. The file-page cache is invalidated before recovery so subsequent VFS
reads cannot retain pages from the failed attempt. Recovery supports direct
FAT32 devices and FAT32 partitions discovered through the existing MBR/GPT
paths. A committed batch is retained when all new images match; an incomplete
or prepared batch restores prior bytes and removes stale metadata intents.

The recovery result is deliberately not substituted for the original write
error. If rollback itself fails, the operation still fails closed and the
durable WDJ2 record remains available for the next mount attempt.

## Evidence

- `cargo clippy -p wovenhat-os --tests -- -D warnings` â€” passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` â€” passed.
- `cargo test -p wovenhat-os --test wovenfs_crash` â€” 11 passed.
- `python scripts/test-storage-qemu.py --cpus 1 --timeout 180` â€” passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 2 --timeout 180` â€” passed, exit 33.
- `python scripts/test-storage-qemu.py --cpus 4 --timeout 180` â€” passed, exit 33.

Serial evidence is retained at:

- `target/storage-regression-1-debug/serial.log`
- `target/storage-regression-2-debug/serial.log`
- `target/storage-regression-4-debug/serial.log`

## Limits

This closes a runtime-recovery defect in the bounded WDJ2 transaction. It does
not close the documented Stage 1â€“5 gaps: metadata transactions beyond the
bounded sidecars, arbitrary-size/full-volume rollback, physical storage/DMA
qualification, or unclean-shutdown testing on physical media.
---

## Source: docs/audit-stage1-5-journal-truncation-2026-10-14.md

# Stage 1â€“5 journal truncation audit â€” 2026-10-14

The storage self-test now enumerates every shortened byte prefix of a valid
prepared WDJ2 record and requires the parser to reject each one. It also keeps
the existing checksum-tamper rejection and maximum-capacity round-trip tests.
This prevents a reboot recovery path from treating an incomplete journal
record as an authoritative transaction.

This is software validation of record framing and checksum boundaries. It does
not establish that a physical flash/ATA device writes a sector atomically, nor
does it replace power-loss, DMA completion, or real-device reboot
qualification.
---

## Source: docs/audit-stage1-5-long-name-collision-2026-10-01.md

# Stage 1â€“5 long-name collision hardening â€” 2026-10-01

The FAT32 long-name mutation path now checks the user-visible, NFC-normalized
name before generating or comparing the implementation's 8.3 alias.

- Creating an existing long-name file reuses its existing directory entry and
  performs the normal data replacement with rollback on allocation or write
  failure.
- Creating a directory at an existing long-name file is rejected.
- Renaming onto an existing long name is rejected even when alias generation
  would choose a different short alias.
- The existing long-name record is preserved during overwrite; the old data
  chain is released only after the replacement directory entry is durable.

The FAT32 self-test now covers long-name overwrite and long-name rename
collision rejection. The complete Stage 1â€“5 QEMU gate passed on 1, 2 and 4
CPUs with exit 33.

| CPU profile | Result |
| --- | --- |
| 1 CPU | PASS |
| 2 CPUs | PASS |
| 4 CPUs | PASS |

This closes a software correctness gap in the bounded FAT32 mutation layer.
It does not claim physical storage qualification, interrupt/DMA-backed block
completion, or full multi-operation data rollback; those remain explicit
follow-up boundaries in the production gap audit.

---

## Source: docs/audit-stage1-5-metadata-capacity-2026-10-01.md

# Stage 1â€“5 metadata capacity audit â€” 2026-10-01

## Scope

This increment removes the original four-sector ownership-metadata ceiling
without changing the legacy WMD1 layout. It does not claim an unbounded
metadata store, native WovenFS, or full multi-operation crash atomicity.

## Implementation

`kernel/src/fat32.rs` retains the WMD1 region at the end of the FAT32 reserved
area and adds a WMD3 extension region in eight earlier reserved sectors when
the volume has at least 18 reserved sectors. The reader, writer, finalizer, and
remover search both regions. Old volumes with fewer reserved sectors continue
to use only WMD1, so the extension cannot overlap the existing journal or inode
metadata regions.

Each sector stores 21 records. The compatible capacity therefore grows from
84 records to 252 records. The test disk models the extension sectors, and the
Stage 1â€“5 self-test persists and reads 100 additional metadata records, which
crosses the old four-sector limit.

## Evidence

- `rustfmt --check kernel/src/fat32.rs`: passed.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage1-5-test`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage1-5-test -- -D warnings`: passed.
- `python -m unittest discover -s tests -p 'test_*.py'`: 11 tests passed.
- Stage 1â€“5 QEMU acceptance: 1, 2, and 4 CPUs passed with exit 33.

## Boundaries

The table is still bounded by reserved-sector geometry. Multi-operation
transaction replay, hardware/DMA qualification, and physical unclean-shutdown
testing remain open production work.

---

## Source: docs/audit-stage1-5-metadata-transactions-2026-10-14.md

# Stage 1â€“5 bounded metadata transactions â€” 2026-10-14

The WDJ2 batch journal now carries version 2 records while retaining parsing
for the existing version 1 record. Each v2 entry stores the previous and new
inode/sidecar ownership metadata alongside the existing bounded file snapshot.

Recovery no longer treats matching file bytes as sufficient for a committed
batch. It verifies the new inode metadata too. A prepared or incomplete
committed batch restores both the previous file bytes and previous metadata,
then removes only the transaction's pending sidecar and intent. This avoids
deleting a pre-existing ownership record during rollback.

WDJ1 remains readable, and the WDJ2 magic and existing v1 record layout remain
unchanged for old media; new records use the versioned v2 entry extension.
The transaction is still bounded at sixteen files and `NODE_CAPACITY` bytes
per file. It is not a full-volume or arbitrary-size journal.

Validation:

- Stage 1â€“5 freestanding Clippy with `-D warnings` passed;
- host integration tests passed;
- storage/SMP QEMU acceptance passed on 1, 2, and 4 CPUs with exit 33;
- the live FAT32 recovery fixture exercised prepared rollback, incomplete
  committed rollback, and complete committed metadata verification.

Remaining Stage 1â€“5 requirements are non-QEMU hardware/DMA qualification,
arbitrary-size/full-volume rollback, and unclean-shutdown/power-loss testing.
---

## Source: docs/audit-stage1-5-transaction-boundary-2026-10-14.md

# Stage 1â€“5 transaction boundary audit â€” 2026-10-14

`persist_paths_atomic` now rejects more than 255 entries with
`PersistError::TooLarge` before checking mount state or touching a block
device. The kernel storage self-test exercises a 256-entry request and checks
that exact result. This keeps the bounded WDJ2 limit explicit and fail-closed;
it does not claim arbitrary-size or full-volume rollback.
---

## Source: docs/audit-stage1-5-wdj2-capacity-2026-10-14.md

# Stage 1â€“5 WDJ2 capacity audit â€” 2026-10-14

The versioned WDJ2 batch journal now accepts up to 255 file entries. The
previous four-entry and sixteen-entry records remain valid because the on-disk
magic, version, entry encoding, checksum, and recovery state are unchanged.
The encoder and parser remain bounded by the full one-byte entry-count range,
the recovery allocation limit, the per-file node-size limit, and the available
FAT32 volume; they do not claim arbitrary-size or full-volume transactions.

The kernel self-test now constructs and round-trips a maximum-capacity
prepared record, including the metadata-snapshot fields, before any live-device
work. Warning-denying freestanding Clippy for `stage1-5-test`, the full host
test suite, and the release build pass.
The same self-test verifies that a 256-entry request is rejected before record
encoding, preserving the fail-closed boundary beyond the 255-entry limit.

Fresh QEMU acceptance passed with exit 33 and the required Stage 1â€“5 marker:

- `audit-artifacts/stage1-5-1cpu-1791962142731816000`
- `audit-artifacts/stage1-5-2cpu-1791962209812403600`
- `audit-artifacts/stage1-5-4cpu-1791962253016653000`
- `target/storage-regression-1-debug`
- `target/storage-regression-2-debug`
- `target/storage-regression-4-debug`

Each disposable storage image passed the complete storage/boot suite on 1/2/4
CPUs. The disposable images are workspace-local and are not the inserted USB
disk.

The broader disposable ATA/FAT32 storage boot regression also passed on all
CPU counts, after the shared PCI `qemu-test` feature guard was corrected.

The earlier bounded-capacity audit recorded the corresponding Stage 1â€“5
artifacts under `audit-artifacts/stage1-5-*`; those historical artifacts are
retained and the newer `target/storage-regression-*` logs are additional
evidence for this 255-entry increment.

This increment removes the former sixteen-entry implementation ceiling. It
does not close metadata transactions beyond the bounded sidecars,
arbitrary-size/full-volume rollback, physical DMA/torn-write recovery, or
unclean-shutdown/power-loss evidence.
---

## Source: docs/audit-stage15-wfs-vfs-write-through-2026-10-14.md

# Stage 15 WovenFS VFS write-through audit â€” 2026-10-14

This increment closes the bounded software write-through boundary between the
task file authority and native WovenFS. It does not claim full POSIX filesystem
or physical flash qualification.

## Implemented boundary

- Production task file operations create, write, and unlink `/wfs/<name>` files
  through native WovenFS before publishing or removing the VFS name.
- The adapter resolves validated nested absolute paths below `/wfs`; traversal,
  empty components, separators inside names, NULs, and prefix-confusion paths
  are rejected.
- Native directory creation, empty-directory removal, and file/directory rename
  update parent entries in the same bounded journal transaction as the inode
  and allocation bitmap changes. Destination replacement is refused rather
  than performed partially.
- ATA mount publication now recursively imports validated WovenFS directories
  and files below `/wfs`, with bounded depth and visited-inode tracking so
  malformed cycles or namespace growth fail closed. Imported files remain
  read-only in the VFS.
- `Volume::check()` now validates the complete reachable tree, including
  nested names, inode references, cycles/duplicate links, and orphaned live
  inodes, before a volume is accepted by the mount path.
- Directory rename rejects a destination below the moved directory using a
  bounded, cycle-detecting ancestry walk. Same-path rename is an explicit
  validated no-op; traversal capacity exhaustion fails closed.
- `task::write_current` commits the native journaled transaction before it
  refreshes the VFS cache and descriptor offset. Direct generic VFS mutation
  remains read-only for `/wfs`, preventing a capability bypass.
- Native create/write/unlink operations serialize their complete
  read-modify-write transaction with a rank-5 preemption-aware mutex. The ATA
  device lock is rank 10; no IRQ-disabled guard is held over device I/O.

## Validation

The 2026-10-20 lifecycle increment adds explicit `/wfs` publication
ownership. Remount invalidates cached descendants and open descriptions before
importing the journal-replayed tree; generic syscall paths fail closed while
unpublished, while the authorized native adapter remains usable for
pre-publication preparation. The stale-handle/remount assertion passed on
1/2/4-CPU scratch-device QEMU. Evidence is under
`target/wovenfs-device-scratch-{1,2,4}cpu`.

- `cargo clippy --all-targets -- -D warnings` â€” passed.
- Freestanding Clippy passed for `stage11-5-test`, `stage15-1-test`, and
  `stage15-2-test` with `-D warnings`.
- `cargo test -p wovenhat-os --tests --no-fail-fast` â€” passed, including all
  12 WovenFS crash-recovery tests.
- Stage 15.1 QEMU acceptance passed on 1, 2, and 4 CPUs (exit 33):
  `stage15.1-1cpu-1791981155975268700`,
  `stage15.1-2cpu-1791981222170305100`, and
  `stage15.1-4cpu-1791981275434721400`.
- Fresh Stage 15.1 evidence after nested-directory integration:
  `stage15.1-1cpu-1792003097975720200`,
  `stage15.1-2cpu-1792003179899219100`, and
  `stage15.1-4cpu-1792003230834435200`.
- The durable-first publication hardening was rechecked on 1/2/4 CPUs:
  `stage15.1-1cpu-1792006921025775400`,
  `stage15.1-2cpu-1792006991018191200`, and
  `stage15.1-4cpu-1792007054767355600`.
- Fresh scratch-device Stage 15.2 evidence passed on 1, 2, and 4 CPUs with
  exit 33 under `target/wovenfs-device-scratch-{1,2,4}cpu`.
- Cycle-guard hardening rerun passed Stage 15.1 on 1, 2, and 4 CPUs with exit
  33: `stage15.1-1cpu-1792014304834644400`,
  `stage15.1-2cpu-1792014406693740800`, and
  `stage15.1-4cpu-1792014471416201400`.
- Recursive-import Stage 15.2 scratch-device rerun passed on 1, 2, and 4 CPUs
  with exit 33 under `target/wovenfs-device-scratch-{1,2,4}cpu`; the fixture
  creates and reads `/docs/note.txt` through the mounted VFS.
- Full-tree consistency rerun passed Stage 15.1 on 1, 2, and 4 CPUs with exit
  33: `stage15.1-1cpu-1792021922584370300`,
  `stage15.1-2cpu-1792022028479024300`, and
  `stage15.1-4cpu-1792022100180585200`.

An earlier 1-CPU timeout is retained as
`stage15.1-1cpu-1791980850302510700`; the exact rerun passed.

## Limits and next prerequisite

Mounted-system policy, including whether WovenFS replaces the FAT32 system
volume, remains open. The recursive importer is intentionally bounded to
16 path levels and 256 visited inodes and skips files larger than the VFS
payload capacity. Physical qualification remains incomplete: the inserted USB
flash disk was inventoried read-only, while the destructive harness requires
Administrator elevation and explicit authorization to erase it. No destructive
write was issued in this audit. Scratch-device QEMU is not physical flash,
power-loss, or boot qualification.
---

## Source: docs/audit-stage16-1-out-of-kernel-application-2026-10-14.md

# Stage 16.1 out-of-kernel application gate â€” accepted (2026-10-14)

The acceptance harness now exposes Stage 16.1 directly. It builds
`apps/hello` outside the kernel workspace, signs the resulting ELF into a WAPP
package, embeds that package through `WOVENHAT_APP_IMAGE`, verifies it before
ELF parsing, installs it into the VFS, reads it back, loads the verified
filesystem bytes, spawns it in Ring 3, and verifies completion and exit status.

Final 1/2/4-CPU evidence:

- `target/application-1cpu`
- `target/application-2cpu`
- `target/application-4cpu`

All three runs passed with exit 33 and markers `[S16.1] signed WAPP package
verifies PASS`, `[S16.1] tampered WAPP package is refused PASS`,
`[S16.1] unsigned external image is refused PASS`, and
`[S16.1] out-of-kernel application: PASSED`.

The package format is produced by `tools/wapp-packager`; release use can set
`WOVENHAT_APP_SIGNING_SEED_HEX` and
`WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX`. The acceptance harness intentionally
uses the fixed development vector and does not provision a production key.

As a separate provisioning check, the same 1-CPU run was repeated with an
explicit non-test signing seed and matching trusted public key. It passed the
same verification, tamper refusal, filesystem, execution, and exit checks.
The private seed is not recorded in the repository or audit documents.

The gate does not claim a package manager, multi-application image assembly,
FAT32 reboot persistence, shared libraries, TLS, or stack-size negotiation;
those remain explicit platform requirements.
---

## Source: docs/audit-stage17-19-revalidation-2026-10-14.md

# Stages 17â€“19 compositor revalidation â€” 2026-10-14

After scheduler-driven termination cleanup was tightened, the exposed
compositor gates were rerun on 1/2/4 CPUs. Every run exited 33 and emitted its
required pass marker.

Stage 18.1:

- `audit-artifacts/stage18.1-1cpu-1791958078399045900`
- `audit-artifacts/stage18.1-2cpu-1791958154582784300`
- `audit-artifacts/stage18.1-4cpu-1791958224128481000`

Stage 19.1:

- `audit-artifacts/stage19.1-1cpu-1791958284631266200`
- `audit-artifacts/stage19.1-2cpu-1791958401331279900`
- `audit-artifacts/stage19.1-4cpu-1791958464093830200`

Stage 19.3:

- `audit-artifacts/stage19.3-1cpu-1791958520133014600`
- `audit-artifacts/stage19.3-2cpu-1791958674407996000`
- `audit-artifacts/stage19.3-4cpu-1791958727747729400`

Stage 19.10:

- `audit-artifacts/stage19.10-1cpu-1791958777408129100`
- `audit-artifacts/stage19.10-2cpu-1791958848733373300`
- `audit-artifacts/stage19.10-4cpu-1791958904921519500`

Dedicated compositor stages:

Stage 19.11:

- `target/hello-gui-wm-1cpu`
- `target/hello-gui-wm-2cpu`
- `target/hello-gui-wm-4cpu`

Stage 19.12:

- `target/hello-gui-pointer-1cpu`
- `target/hello-gui-pointer-2cpu`
- `target/hello-gui-pointer-4cpu`

Stage 19.13:

- `target/hello-gui-alpha-1cpu`
- `target/hello-gui-alpha-2cpu`
- `target/hello-gui-alpha-4cpu`

Stage 19.14:

- `target/hello-gui-scaled-1cpu`
- `target/hello-gui-scaled-2cpu`
- `target/hello-gui-scaled-4cpu`

All twelve dedicated runs exited 33 and emitted their required pass marker.
The dedicated harness is `scripts/test-hello-gui-compositor-qemu.py`; these
runs use the release `hello-gui` image and the local QEMU UEFI firmware.

These are software/QEMU gates. USB HID pointer decoding, multiple-monitor
hardware, GPU acceleration, and physical display qualification remain explicit
hardware-dependent limits.
---

## Source: docs/audit-stage17-1-font-and-software-compositing-2026-10-14.md

# Stage 17.1 font and software compositing â€” accepted (2026-10-14)

The WovenHat font coverage and bounded software compositor gate passed on 1,
2, and 4 CPUs with exit 33:

- `audit-artifacts/stage17.1-1cpu-1791957821262520800`
- `audit-artifacts/stage17.1-2cpu-1791957966098533500`
- `audit-artifacts/stage17.1-4cpu-1791958016323174300`

The gate covers the expanded font table, clipping, stride, pixel formats,
damage coalescing, and framebuffer presentation. GPU acceleration and
multi-monitor hardware qualification remain outside this software gate.
---

## Source: docs/audit-stage19-12-pointer-2026-10-13.md

# Stage 19.12 pointer routing audit â€” 2026-10-13

The previously recorded Stage 19.11 open item for pointer-driven focus is now
validated. The complete Stage 19.12 gate passed on 1, 2, and 4 CPUs:

- 1 CPU: `target/hello-gui-pointer-1cpu`
- 2 CPUs: `target/hello-gui-pointer-2cpu`
- 4 CPUs: `target/hello-gui-pointer-4cpu`
- marker: `[S19.12] pointer input: PASSED`

The gate exercises IRQ12 mouse packet decoding, IRQ1 keyboard decoding, the
event-driven window-manager sleep, click-to-focus on each window, key routing
to the focused client, and lease/focus teardown. The USB HID mouse path and
multi-monitor pointer semantics remain separate hardware/product work.
---

## Source: docs/audit-stage19-13-transparency-2026-10-13.md

# Stage 19.13 transparency and shadow audit â€” 2026-10-13

The complete Stage 19.13 compositor gate passed on 1, 2, and 4 CPUs:

- 1 CPU: `target/hello-gui-alpha-1cpu`
- 2 CPUs: `target/hello-gui-alpha-2cpu`
- 4 CPUs: `target/hello-gui-alpha-4cpu`
- marker: `[S19.13] translucency and shadows: PASSED`

The gate verifies refusal of unprivileged blending, compositor-owned backdrop
and lease, client sharing, independently recomputed translucent/shadow regions,
clean client/compositor exits, and lease release. This is software compositor
qualification under QEMU; hardware GPU acceleration and multi-monitor output
remain outside the boundary.
---

## Source: docs/audit-stage19-8-timer-event-wait-2026-10-15.md

# Stage 19.8 timer/event wait-source increment â€” 2026-10-15

## Scope

The existing syscall 111 wait path covered input, mailbox, and compositor
damage. Timer and manual-reset event objects were available through the older
completion-handle API but could not be sources in one event-driven wait. This
increment adds syscall 206 (`WaitAnyObjects`) and the safe `libwoven` wrapper.

## Safety and ownership

- The kernel copies all 24-byte source records before registering a waiter.
- Records are bounded to eight, require unique one-bit masks, and validate the
  timer/event kind before touching an object.
- Object lookup remains owner- and generation-checked.
- An existing completion-handle waiter or another task's waiter is refused; a
  single task may register across multiple distinct objects without replacing
  any waiter. Manual-reset events additionally retain up to four distinct task
  waiters and broadcast wakeups to all of them; one-shot timers remain single-
  consumer.
- Timer expiry is consumed once; manual-reset events remain level-triggered.
- Every task registration is removed on success, timeout, or error.
- Object locks are released before task wakeups and no userspace pointer is
  retained while blocked.

## Evidence

- `cargo clippy -p wovenhat-os --all-targets -- -D warnings` â€” passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage19-8-test -- -D warnings` â€” passed.
- Same freestanding Clippy command for `stage19-13-test` â€” passed, protecting
  the neighboring feature gate from the new cfg-gated self-test.
- Stage 19.8 QEMU combined-wait gate, exit 33:
  - `target/hello-gui-multiplex-1cpu`
  - `target/hello-gui-multiplex-2cpu`
  - `target/hello-gui-multiplex-4cpu`

The userspace `hello-gui --timer-event` scenario creates a one-shot timer and
manual-reset event, waits on both, verifies the timer source, sets and waits on
the event source, and closes both objects. It passed on all three CPU counts.

Each serial log contains `[S19.8] timer/event object registration is level-safe
PASS` and `[S19.8] combined wait: PASSED`.

## Limit

This closes the timer/event source integration boundary for the new ABI and
adds bounded event fan-out. It does not claim physical hardware qualification,
timer fan-out, or completion-port replacement; one-shot timer expiry remains a
single-consumer resource by design.
---

## Source: docs/audit-stage19-calculator-2026-10-14.md

# Stage 19 WovenCalculator application audit â€” 2026-10-14

WovenCalculator is the first roadmap Stage 19 system application delivered as
a separate crate. It is not linked into the kernel and uses `libwoven` for all
userspace operations.

## Safety and ABI boundary

- The expression evaluator is allocation-free, bounded to 96 input bytes and
  16 parenthesis levels, and uses checked signed 64-bit arithmetic.
- Empty, malformed, unbalanced, divide-by-zero, overlong, and overflowing
  expressions return typed errors; no malformed input path panics.
- The binary is a static ET_EXEC image linked with the same large-code-model,
  W^X, no-RELRO policy as the existing external applications.
- Release packaging uses the existing WAPP signer. The GUI mode requests only
  the normal display syscalls and has no kernel-side privilege bypass.

## Evidence

- Three host parser tests passed.
- Host library Clippy and freestanding binary Clippy passed with `-D warnings`.
- Signed WAPP packaging succeeded.
- `scripts/test-calculator-qemu.py` passed on 1/2/4 CPUs (exit 33):
  `audit-artifacts/woven-calculator-1cpu`,
  `audit-artifacts/woven-calculator-2cpu`, and
  `audit-artifacts/woven-calculator-4cpu`.

The QEMU application test proves package verification, filesystem round-trip,
ELF loading, Ring-3 execution, and clean zero exit using the existing Stage
16.1 acceptance boundary.

## Scope remaining

This is one application, not completion of the roadmap's full WovenFiles,
WovenTerminal, WovenSettings, WovenMonitor, WovenSecurity, WovenEditor,
WovenImages, WovenUpdate, and WovenStore suite. Catalog installation into a
boot image and a full GUI toolkit remain later integration work.
---

## Source: docs/audit-stage19-editor-2026-10-14.md

# Stage 19 WovenEditor audit â€” 2026-10-14

WovenEditor is a separate static Ring-3 application. It parses bounded
`--view PATH` and `--write PATH TEXT` commands, previews at most 2048 bytes,
and writes only through the additive safe `libwoven::write_file` wrapper. The
kernel still enforces `FileRead`/`FileWrite` capability, descriptor ownership,
path policy, and storage synchronization.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-editor-{1,2,4}cpu`.

The seven-application catalog and production-image integration consume the
package. Full interactive GUI editing, selection, undo, and atomic document
replacement remain future application/UI work; the bounded ABI write path is
the accepted kernel-facing boundary here.
---

## Source: docs/audit-stage19-files-2026-10-14.md

# Stage 19 WovenFiles application audit â€” 2026-10-14

WovenFiles is the third roadmap Stage 19 system-application increment. It is a
separate static Ring-3 image using `libwoven` read-only filesystem calls.

The default invocation lists `/mnt`; `--path PATH` lists another authorized
directory and `--view PATH` emits a bounded 2048-byte preview. Directory
enumeration is capped at 128 entries, file output is copied through a fixed
buffer, descriptors are closed on every read path, and invalid UTF-8 or open/
read failures return a nonzero status. No write or privilege authority is
introduced.

Freestanding Clippy passed with `-D warnings`. Signed WAPP packaging and the
existing verified external-application Ring-3 acceptance passed on 1/2/4 CPUs:

- `audit-artifacts/woven-files-1cpu`
- `audit-artifacts/woven-files-2cpu`
- `audit-artifacts/woven-files-4cpu`

The app is read-only and argument-driven for deterministic acceptance. A full
interactive GUI, persistent boot catalog, and write/mutation workflow remain
later integration work.
---

## Source: docs/audit-stage19-images-2026-10-14.md

# Stage 19 WovenImages audit â€” 2026-10-14

WovenImages is a separate static Ring-3 application. It lists a bounded
directory and recognizes PNG, JPEG, GIF, and BMP headers from at most 32 bytes
of a selected file. It never decodes unbounded pixel data or mutates storage;
unknown signatures fail closed as `unknown`.

The two host parser/header tests and warning-denying freestanding Clippy
passed. The signed external-application harness passed on 1, 2, and 4 CPUs
with exit 33; serial/QEMU evidence is preserved in
`audit-artifacts/woven-images-{1,2,4}cpu`.

The eight-application catalog and production-image integration consume the
package. Full image decoding, thumbnails, editing, and GPU-backed presentation
remain future graphics/application work.
---

## Source: docs/audit-stage19-monitor-2026-10-14.md

# Stage 19 WovenMonitor audit â€” 2026-10-14

WovenMonitor is a separate static Ring-3 application. It has a bounded mode
parser and read-only reports for UTC hardware-clock time, compositor geometry
when `DisplayIo` is available, and a capped `/mnt` directory-entry count.
Unknown or overlong modes fail closed; no write, display-scale mutation, or
new capability was introduced.

Host parser tests and warning-denying freestanding Clippy passed. The signed
external-application harness passed on 1/2/4 CPUs with exit 33 and preserved
serial/QEMU evidence in `audit-artifacts/woven-monitor-{1,2,4}cpu`.

The application is intentionally limited to ABI data available to an ordinary
Ring-3 process. Kernel-wide CPU/memory counters and privileged device telemetry
remain outside the current ABI and are not fabricated by this app.
---

## Source: docs/audit-stage19-production-system-app-boot-2026-10-14.md

# Stage 19 production system-app boot audit â€” 2026-10-14

## Production image boundary

The `system-apps` image feature embeds the ten signed WAPP packages from the
release install plan. Before changing the VFS it verifies every package against
the kernel trust root, parses the ELF payload, validates the normal loader, and
checks all declared paths for collisions. It then installs package files first
and publishes `/share/applications/*.app` descriptors only after all packages
are present. Any failed package or catalog insertion removes only entries and
directories created by that transaction.

Production images require `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` at compile
time. There is no test-key fallback under `system-apps`. The QEMU harness uses
the documented deterministic test key explicitly; release provisioning must
replace it with the deployment key and pass the same public key to the WAPP
signer.

## Validation

Warning-denying freestanding Clippy passes for `system-apps`. The normal-boot
shell/QMP acceptance passed on 4 CPUs, with serial markers showing ten
signed applications installed before shell startup and the existing SMP/PS2
smoke markers. The focused 1/2/4-CPU catalog matrix and the
`system-apps-boot` check passed. The subsequent full release chain also passed,
including the 1/2/4-CPU catalog matrix. The production image was built with
the feature and an explicit trusted public key.

## Limits

The catalog is currently RAM-backed during boot; persistent FAT32/WovenFS
installation, rollback across power loss, key rotation/revocation, and native
hardware qualification remain separate production requirements. No removable
media was modified.
---

## Source: docs/audit-stage19-security-2026-10-14.md

# Stage 19 WovenSecurity audit â€” 2026-10-14

WovenSecurity is a separate static Ring-3 application. It provides bounded,
read-only process identity and newest-first security-ledger reporting through
the existing `libwoven` audit ABI. If `TaskInspect` is unavailable it reports
that the ledger is unavailable; it never grants, revokes, or interprets
security policy.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-security-{1,2,4}cpu`.

The six-application Stage 19 catalog gate and 4-CPU production system-app
boot consume this package. Persistent security-policy UI and user-facing
consent workflows remain separate desktop/service work.
---

## Source: docs/audit-stage19-settings-2026-10-14.md

# Stage 19 WovenSettings audit â€” 2026-10-14

WovenSettings is a separate static Ring-3 application. It provides bounded,
read-only identity, UTC clock, and compositor display-policy reports through
the ordinary `libwoven` ABI. Invalid and overlong modes fail closed; the app
does not mutate settings or request privileged device authority.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-settings-{1,2,4}cpu`.

The five-application Stage 19 catalog gate passed on 1/2/4 CPUs, and the
4-CPU production `system-apps` boot installed all five packages before shell
startup. Persistent settings storage and a policy service remain outside this
bounded read-only application increment.
---

## Source: docs/audit-stage19-store-2026-10-14.md

# Stage 19 WovenStore audit â€” 2026-10-14

WovenStore is a separate static Ring-3 application. It enumerates the bounded
`/share/applications` catalog and reports installed descriptors without
installing, removing, or trusting packages. Package acquisition, signature
policy, dependency resolution, and transactional installation remain owned by
a future privileged package service.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-store-{1,2,4}cpu`.

The ten-application catalog and production-image integration consume the
package. Full store UI and package-service workflows remain separate platform
work.
---

## Source: docs/audit-stage19-system-app-catalog-2026-10-14.md

# Stage 19 multi-application catalog audit â€” 2026-10-14

## Scope

The feature-gated `stage19-app-catalog-test` acceptance consumes the output of
`scripts/package-system-apps.py` and exercises the ten accepted external
applications as one install plan:

- WovenCalculator at `/bin/wovenhat-calculator`;
- WovenTerminal at `/bin/wovenhat-terminal`; and
- WovenFiles at `/bin/wovenhat-files`; and
- WovenMonitor at `/bin/wovenhat-monitor`.
- WovenSettings at `/bin/wovenhat-settings`.
- WovenSecurity at `/bin/wovenhat-security`.
- WovenEditor at `/bin/wovenhat-editor`.
- WovenImages at `/bin/wovenhat-images`.
- WovenUpdate at `/bin/wovenhat-update`.
- WovenStore at `/bin/wovenhat-store`.

Each package is verified as WAPP v1, mutated-package refusal is checked, the
package is written to and read back from the VFS byte-for-byte, and the
read-back payload is passed through the normal verified ELF loader. The ten
generated `/share/applications/*.app` descriptors are published only after the
corresponding package has passed those checks.

## Acceptance

`scripts/test-system-app-catalog-qemu.py` passed sequentially on 1, 2, and 4
virtual CPUs, each with exit 33 and the marker
`[S19-APP] ten signed applications installed PASS`. Serial and QEMU logs are
preserved in `audit-artifacts/system-app-catalog-1cpu`,
`audit-artifacts/system-app-catalog-2cpu`, and
`audit-artifacts/system-app-catalog-4cpu`.

The kernel feature also passes warning-denying freestanding Clippy. The
packaging command was rerun for the acceptance build and produced the same
bounded package sizes and manifest structure.

Repository-wide `cargo clippy --all-targets -- -D warnings` also passes. A
repository-wide `cargo fmt --all -- --check` reports formatting drift in
pre-existing files outside this increment; those unrelated files were not
rewritten during this acceptance so historical code and evidence remain
unchanged.

## Limits

This catalog gate is separate from the feature-gated production-image boot
installer. That installer now performs bounded pre-verification, package-first
publication, and rollback of entries and directories created by the current
RAM-backed transaction. Persistent-media installation, power-loss recovery,
key rotation/revocation, and native hardware qualification remain open
requirements. No removable media was modified.
---

## Source: docs/audit-stage19-system-app-packaging-2026-10-14.md

# Stage 19 system-app packaging audit â€” 2026-10-14

## Scope

The accepted WovenCalculator, WovenTerminal, WovenFiles, WovenMonitor,
WovenSettings, WovenSecurity, WovenEditor, WovenImages, WovenUpdate, and
WovenStore binaries are now
packaged together by `scripts/package-system-apps.py`. The script builds each
separate app workspace with the pinned freestanding target, signs every ELF
using `tools/wapp-packager`, validates the WAPP v1 envelope and ELF payload,
and writes a manifest with SHA-256 hashes for packages and catalog entries.

## Safety and limits

The packager writes only under its selected output directory and does not
install to a boot volume or erase removable media. Catalog descriptors contain
fixed executable paths and no privilege escalation. The release `system-apps`
image feature consumes the plan in RAM after validation; Stage 16.1 still
accepts one package per boot through `WOVENHAT_APP_IMAGE`.

## Validation

The packaging command was run after the app-specific host checks, freestanding
Clippy checks, and signed QEMU 1/2/4 acceptance gates recorded in the ten
application audits. The resulting manifest and package hashes are generated
under `target/system-apps/0.8.0` and are intentionally ignored build evidence.

## Result

The reproducible signed packaging/catalog boundary passes for all ten accepted
apps. Stage 19 remains incomplete until persistent media update/rollback
policy, key lifecycle integration, and full GUI integration are accepted.
---

## Source: docs/audit-stage19-terminal-2026-10-14.md

# Stage 19 WovenTerminal application audit â€” 2026-10-14

WovenTerminal is the second roadmap Stage 19 system application delivered as
a separate crate. It uses `libwoven` and the existing signed WAPP/ELF path;
it does not add kernel privileges or an alternate ABI.

## Application boundary

- The command parser is allocation-free and rejects commands over 128 bytes,
  ambiguous prefixes, and missing arguments.
- `ls` caps output at 128 entries. `cat` caps each invocation at 16 chunks of
  512 bytes, closes its descriptor on every path, and reports open/read
  failures instead of panicking.
- `calc` delegates checked arithmetic to the bounded WovenCalculator core.
- `--interactive` reads stdin one byte at a time through the existing file
  descriptor ABI, buffers at most 128 bytes per line, and terminates cleanly
  on EOF or read failure.
- The application is a static ET_EXEC image with W^X-compatible segments and
  is signed by the existing WAPP packager.

## Evidence

- Two host command-parser tests passed.
- Host library Clippy and freestanding binary Clippy passed with `-D warnings`.
- Signed WAPP packaging and Ring-3 execution passed on 1/2/4 CPUs (exit 33):
  `audit-artifacts/woven-terminal-1cpu`,
  `audit-artifacts/woven-terminal-2cpu`, and
  `audit-artifacts/woven-terminal-4cpu`.

The QEMU acceptance proves package verification, filesystem round-trip, ELF
loading, and clean zero-exit execution. The acceptance invokes the
deterministic argv path; interactive hardware/console input and boot-catalog
installation remain separate integration evidence.
---

## Source: docs/audit-stage19-update-2026-10-14.md

# Stage 19 WovenUpdate audit â€” 2026-10-14

WovenUpdate is a separate static Ring-3 application. It performs a bounded,
read-only inventory of `/updates` and explicitly refuses to model installation
or rollback. No package is activated, deleted, or trusted by the client;
signature verification, A/B selection, power-loss recovery, and rollback are
reserved for the privileged update service.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-update-{1,2,4}cpu`.

The nine-application catalog and production-image integration consume the
package. The update-service transaction and persistent media policy remain
separate roadmap requirements.
---

## Source: docs/audit-stage20-14a-monitors-2026-10-15.md

# Stage 20.14a two monitors through virtio-gpu audit â€” 2026-10-15

Roadmap Stage 18.9, Multi-monitor Desktop, first part: a display device
with several outputs.

## What changed

**`kernel/src/virtio_pci.rs`: the modern virtio 1.x PCI transport.**
- It walks the device's capability list for the vendor capabilities:
  common configuration, notification (with its offset multiplier), ISR
  status, and device configuration. These are mapped from their BARs.
- It resets the device and negotiates `VIRTIO_F_VERSION_1` with features
  the device offers.
- It sets up 16-descriptor split queues in one DMA page each.
- `exchange` posts a two-descriptor request and response chain, notifies
  the queue, and polls the used ring, with a bound.

virtio-net keeps its legacy transport.

**`kernel/src/virtio_gpu.rs`: a 2D driver.**
- `probe` finds the device (1af4:1050) and reads the scanout count from
  device configuration.
- `display_info` returns each head's reported mode.
- `set_mode` creates a B8G8R8X8 resource, the kernel framebuffer's
  little-endian 0x00RRGGBB layout. It attaches backing memory in 16-page
  runs and binds the resource to the scanout.
- `present` transfers to the host and flushes.

Both modules are compiled only for `stage20-14-test` until the extended
desktop (20.14b) uses them.

## Gate

`[S20.14] two monitors: PASSED`. Harness:
`scripts/test-stage20-14-monitors.py`.

The machine is q35, with the standard VGA as boot console and
`virtio-gpu-pci,max_outputs=2`. The kernel brings the GPU up and gives both
heads a 640x400 resource. QEMU reports head 1 as disabled, because no UI is
attached; binding a resource to it enables it.

The kernel draws:
- on head 0, an exact gradient: R = xÂ·255/639, G = yÂ·255/399, B = 0x40;
- on head 1, a 16-pixel checkerboard.

The harness captures each head from the host (QMP `screendump`, device
`gpu0`, head 0 or 1) and requires every one of the 2 Ã— 256,000 pixels to
match.

## Evidence (`audit-artifacts/`)

Stage 20.14 on 1/2/4 CPUs, all passing: `stage20.14-1cpu-1792018526128537300`, `stage20.14-2cpu-1792017952477129400`, `stage20.14-4cpu-1792018036357418900`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All four built, and each failed the gate as intended:
- **Head 1 never presented:** all 256,000 of its pixels differ.
- **R8G8B8X8 format instead of B8G8R8X8:** every pixel of both heads has
  red and blue swapped.
- **Transfer replaced by a flush:** the device answers the command with an
  error (0x1203).
- **Both resources bound to head 0:** head 0 shows the checkerboard, and
  head 1 keeps the default 640x480.

After the restore, the gate passes again.

**Lint.** Warning-denying Clippy passes for production,
`stage20-14-test`, the Stage 13 features that share the PCI bus-master
helper, and host `--all-targets`.

## Limits

- **Synchronous commands.** Commands complete by polling the used ring,
  not through interrupts.
- **No cursor.** The cursor queue is not set up.
- **Fixed mode in the gate.** The resource size is chosen by the driver;
  mode changes reported by the host are not followed.
- **Not yet presenting the desktop.** The desktop compositor does not drive
  the heads yet; that is Stage 20.14b.
---

## Source: docs/audit-stage20-14b-desktop-monitors-2026-10-15.md

# Stage 20.14b desktop across two monitors audit â€” 2026-10-15

Roadmap Stage 18.9, Multi-monitor Desktop, completed on top of the Stage
20.14a virtio-gpu driver.

## What changed

**Compositor (`kernel/src/compositor.rs`).**
- `attach_gpu_outputs` makes a virtio-gpu with two or more scanouts the
  display. Every head runs at head 0's mode (at most 1280x800), placed
  left to right; a single-head GPU is left alone.
- The surface is resized to the combined geometry. `MAX_WIDTH` is now
  2560, enough for two 1280-pixel monitors.
- `present_state` presents to the GPU heads when there are any, otherwise
  to the boot framebuffer:
  - each damaged region, in physical pixels, is split by head;
  - each part is scaled into that head's backing rows (`Head::write_row`);
  - only that rectangle is transferred and flushed
    (`Gpu::present_rect`).
- Activation does not require the terminal framebuffer when a GPU drives
  the display.
- `outputs_for` reports each monitor in surface coordinates.
- A normal boot attaches the outputs after driver bring-up and logs
  `[DISPLAY] virtio-gpu: N monitors`.

**Syscall 207, `GraphicsCompositorOutputs`.** It returns the monitors left
to right. 206 is Codex's `WaitAnyObjects`, so audio recording is now
reserved 208. libwoven exposes it as
`compositor_outputs`.

**WovenShell.**
- The first monitor is the primary: the taskbar, panels, launcher, new
  windows, the lock prompt and tiling use its width.
- The wallpaper, pointer, full repaints and window dragging cover the
  whole desktop (`desktop_width`).
- Windows are drawn on every monitor.
- An edge snap happens only at the desktop's outer edges. The right edge
  snaps into the last monitor.
- Changing the scale (Quick Settings) refreshes the monitors.

## Bugs found by the gate

- **Drops on the second monitor snapped back to the primary.** The edge
  snap compared the drop point with the primary monitor's width, so any
  drop on the second monitor counted as the right edge. The 2-CPU run
  failed with "window snapped".
- **Windows on the second monitor were clipped.** The shell clipped
  windows to the primary monitor's width.
- **The scale toggle reset the primary.** It set the primary width to the
  whole desktop.
- **The gate's own drag check raced the shell (gate defect).** It read the
  live surface, which the shell writes in several pieces per frame. On
  4 CPUs it once saw a frame half drawn and failed. The check now reads
  only between completed frames: `compositor::between_frames` evaluates it
  under the compositor lock, and only when all damage has been presented.
  On failure it reports which half failed (`injected`, `arrived`,
  `stayed`).

## Gate

`[S20.14B] desktop across monitors: PASSED`. Harness:
`scripts/test-stage20-14b-desktop.py`.

The machine is q35, with the standard VGA as boot console and a
two-output virtio-gpu. The kernel attaches the GPU outputs and starts the
shell. It then checks against its own rendering:
- the taskbar is on the primary monitor only;
- window A opens on the primary;
- A, dragged by its title bar one monitor-width right, appears on the
  second monitor;
- A stays there for a second, so the drop is complete and no snap or
  resize follows. Both checks judge only completed frames;
- everything drawn has been presented.

The kernel then prints six sample pixels with their physical head
coordinates. The harness captures both heads from the host (QMP
`screendump`) and requires each sample to match.

## Evidence (`audit-artifacts/`)

Stage 20.14b on 1/2/4 CPUs, all passing: `stage20.14b-1cpu-1792044894887039300`, `stage20.14b-2cpu-1792044145193099700`, `stage20.14b-4cpu-1792044420265005400`

Three extra 4-CPU runs also passed, and the gate passed again on 1 CPU
after the negative controls restored the source.

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each failed the gate as intended:
- **Heads never written:** the captures are black.
- **Head offset ignored:** head 1 shows head 0's content.
- **Drag clamped to the primary monitor:** A never reaches the second
  monitor.
- **Taskbar across both monitors:** the taskbar check fails.
- **Edge snap at the primary's edge** (the original bug):
  - Before the "stays" check, this control was not caught: on 1 CPU the
    gate saw A after the moves but before the release was processed.
  - With the frame-aligned check, it is caught: no completed frame shows
    A on the second monitor.

**Regression**, all passing on 1/2/4 CPUs, against the final tree (this
stage plus the presentation-performance fixes):
- Stage 20.14a;
- Stages 20.1â€“20.13;
- Stages 17.1, 18.1, 19.1, 19.3 and 19.10;
- hello-gui, and the compositor gates 19.2, 19.4â€“19.9 and 19.11â€“19.14;
- the Stage 13.12 production boot;
- the release gate (`scripts/test-release.py`): all 98 checks pass. The first attempt failed one
  check, `security-1`, because the disk filled while compiling; it passed
  after build space was reclaimed. Evidence: `audit-artifacts/release-validation-1792056031/`.

20.7, the spring-fade gate, failed twice on 1 CPU while this stage was
being validated. The cause was over-drawing in the shell and the kernel,
which left the fade at about 5 fps under emulation. It is fixed separately;
see the
[presentation performance audit](audit-presentation-performance-2026-10-15.md).
After the fix, 20.7 passed six consecutive 1-CPU runs.

**Lint.** Warning-denying Clippy passes for:
- production;
- every Stage 20 feature, including 20.14 and 20.14b;
- `stage13-12-test`, `stage13-3-test`, `stage19-10-test` and
  `stage19-14-test`;
- woven-shell, hello-gui and host.

## Limits

- **Monitor layout.**
  - Monitors share one mode and sit side by side.
  - Hot-plugging monitors and per-monitor scale are not handled.
  - A normal boot uses a virtio-gpu only when it has two or more
    scanouts.
- **No native GPUs.** Physical multi-monitor needs native GPU drivers.
- **Terminal limit on a normal boot.** If the firmware's boot framebuffer
  is the same virtio-gpu, the text terminal cannot draw once the desktop
  releases the display.
- **Polled commands.** Presentation commands are synchronous and polled.
---

## Source: docs/audit-stage20-15-desktop-session-2026-10-15.md

# Stage 20.15: the WovenHat desktop as the session audit â€” 2026-10-15

## The gap

A normal or release boot never started the desktop. WovenShell ran only
inside the Stage 20 test gates, so a real boot showed the kernel's text
console and its early demo window.

## What changed

**Session (`kernel/src/session.rs`).**
- A system-apps boot starts WovenShell as the session, with sign-in when
  `/etc/wovenhat/accounts` exists.
- The session holds Console, DisplayIo, InputIo, Ipc, FileRead,
  ProcessCreate, AudioPlayback and PowerControl.
- Its launcher lists the installed system applications.

**F12, the diagnostic console.**
- PS/2 scancode 0x58 or USB usage 0x45 switches between the desktop and
  the kernel's console.
- The key is the kernel's own. WovenInput intercepts it before routing, so
  it never reaches the userspace ABI.
- In console mode the compositor holds back the session's frames, and the
  terminal gets the display and the keyboard.
- Returning repaints the whole surface and gives focus back.
- Testers keep `note`, `savelog` and `poweroff`.

**Power control.**
- Capability `PowerControl` (25) is within the system-service ceiling
  only; applications never get it.
- Syscall 209, `PowerControl`: 0 powers off, 1 restarts. It is audited as
  `Action::PowerControl`.
- `libwoven::power_off` and `restart` wrap it.
- Quick Settings gains "Power off" and "Restart" at the top. The panel
  grows upward, so every existing row keeps its screen position.
- A refusal raises a toast with the reason.

**Release flow.**
- `test-release.py` and `test-shell-qemu.py` build WovenShell and embed it
  (`WOVENHAT_SHELL_IMAGE`).
- The system-apps boot check opens the console with F12 before typing
  `smptest`.
- WovenShell's sources are in the validated-image hash.

## Bugs found by this stage

- **ET_EXEC images were relocated under ASLR (Stage 11 loader).**
  - Production kernels randomise every image's bias, but an ET_EXEC image
    holds absolute addresses.
  - WovenShell, linked at 0x400000000000, faulted on an instruction fetch
    at 0x4000000005c1 the first time a production kernel loaded it.
  - Test kernels use a zero bias, so no gate had seen it.
  - `elf::Image::is_position_independent` now records the image type, and
    `load_elf` randomises only ET_DYN images.
- **The shell image outgrew the VFS's 64 KiB file limit** (66,928 bytes).
  The Stage 20.1â€“20.3 gates, which install it into the VFS, failed. The
  release profile now strips symbols (60,904 bytes).

## Gate

`scripts/test-stage20-15-session.py`. The session starts through
`session::start`, with an application in the catalog. With real i8042
input:
1. the launcher lists the application;
2. F12 shows the console: the session's frames stop, the terminal owns
   the display, and a typed key reaches the kernel shell;
3. F12 returns, restoring display and focus;
4. Quick Settings' Power off turns the machine off, and QEMU must exit by
   itself after the click.

The release-style system-apps boot also starts the desktop, opens the
console with F12, and runs `smptest` there.

## Evidence (`audit-artifacts/`)

- **20.15, 1/2/4 CPUs:** `stage20.15-1cpu-1792080508992804300`, `stage20.15-2cpu-1792077643020280800`, `stage20.15-4cpu-1792077659800304500`.
- **Release gate:** `release-validation-1792079997/`. It ran after both
  fixes; its validated image is the one written to the live-test stick.

Chain results:

```
clippy woven-shell: ok
clippy hello-gui: ok
lint-all-features: exit 0 []
host tests: ok
libwoven tests: ok
20.15 1cpu: PASS (35s)
20.15 2cpu: PASS (15s)
20.15 4cpu: PASS (20s)
negative control 20.15 F12 not intercepted: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', "[S20.15] F12 shows the diagnostic console; the session's frames stop FAIL", '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077682633163100']
negative control 20.15 console mode does not hold the session back: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', '[S20.15] a key typed at the console reaches the kernel shell FAIL', '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077733739535800']
negative control 20.15 session without PowerControl: caught ['QEMU exit status 35, expected 0 (turned off by the guest)', '[S20.15] the machine turned off from the desktop FAIL', '[S20.15] desktop session: FAILED', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage20.15-1cpu-1792077770727566600']
20.1 1cpu: FAIL (28s) ['[S20.1] install into the filesystem FAIL', '[S20.1] woven-shell desktop: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-1cpu', '[S20.1] install into the filesystem FAIL']
20.2 1cpu: FAIL (29s) ['[S20.2] install shell and application FAIL', '[S20.2] desktop applications: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-apps-1cpu', '[S20.2] install shell and application FAIL']
20.3 1cpu: FAIL (27s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-1cpu', '[S20.3] catalog and programs installed FAIL']
20.4 1cpu: PASS (33s)
20.5 1cpu: PASS (34s)
20.6 1cpu: PASS (34s)
20.7 1cpu: PASS (34s)
20.8 1cpu: PASS (38s)
20.9 1cpu: PASS (27s)
20.10 1cpu: FAIL (40s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-1cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.11 1cpu: PASS (57s)
20.12 1cpu: PASS (36s)
20.13 1cpu: PASS (34s)
20.3 2cpu: FAIL (13s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-2cpu', '[S20.3] catalog and programs installed FAIL']
20.3 4cpu: FAIL (22s) ['[S20.3] catalog and programs installed FAIL', '[S20.3] application launcher: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-launcher-4cpu', '[S20.3] catalog and programs installed FAIL']
20.10 2cpu: FAIL (27s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-2cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.10 4cpu: FAIL (25s) ['[S20.10] the Quick Settings button opens its panel FAIL', '[S20.10] quick settings: FAILED', 'FAILED (exit 35). Evidence: C:\\WovenHat-OS-wt\\target\\woven-shell-quick-settings-4cpu', '[S20.10] the Quick Settings button opens its panel FAIL']
20.11 2cpu: PASS (50s)
20.11 4cpu: PASS (50s)
20.12 2cpu: PASS (20s)
20.12 4cpu: PASS (21s)
20.14b 1cpu: PASS (57s)
19.2 1cpu: PASS (38s)
13.12 production 1cpu: PASS (23s)
13.12 production 2cpu: PASS (7s)
13.12 production 4cpu: PASS (6s)
system-apps boot (desktop session, F12, smptest): PASS (50s)
release: PASS (1227s)
DONE
```

Reruns after the fixes:

```
20.1 1cpu: PASS (55s)
20.1 2cpu: PASS (31s)
20.1 4cpu: PASS (36s)
20.2 1cpu: PASS (77s)
20.2 2cpu: PASS (24s)
20.2 4cpu: PASS (24s)
20.3 1cpu: PASS (41s)
20.3 2cpu: PASS (21s)
20.3 4cpu: PASS (26s)
20.10 1cpu: PASS (55s)
20.10 2cpu: PASS (27s)
20.10 4cpu: PASS (26s)
20.11 1cpu: PASS (69s)
20.15 1cpu: PASS (37s)
DONE
```

**Negative controls**, each caught, on 1 CPU with the source restored byte
for byte:
- **F12 not intercepted.**
- **Console mode not holding the session back:** the key typed at the
  console never reaches the kernel shell.
- **Session without PowerControl:** the machine stays on.

## Limits

- **VFS file limit.** The VFS's 64 KiB per-file limit constrains the shell
  image, which now has about 4.6 KB of headroom.
- **Fixed-address shell.** WovenShell is still ET_EXEC, so it runs without
  ASLR. Building it as a position-independent executable would restore
  ASLR.
- **No terminal redraw.** The console starts on a cleared screen; earlier
  text is not redrawn.
- **Fixed hotkey.** F12 cannot be rebound.
---

## Source: docs/audit-stage20-4-graphics-2026-10-19.md

# Stage 20.4 graphics-track acceptance audit â€” 2026-10-19

## Result

`python scripts/test-woven-shell-qemu.py --stage 20.4 --cpus N` passed for
`N = 1`, `2`, and `4`.

The final serial marker for each run was:

`[S20.4] windows that move and close: PASSED`

## Accepted behavior

- Surface pixel budgets reject oversized, owner-over-budget, and
  system-over-budget allocations, and release capacity after destruction.
- WovenShell takes the display and opens Alpha, then opens the larger Wide
  window with its close control.
- Real i8042 input drags Alpha by its title bar. The gate checks that the
  window moved, the old region was repainted, and the new region is correct.
- Clicking Wide's close control asks the owner to exit and removes the window.
- Alpha retains focus, receives its exit key, and the display returns to the
  terminal after shell teardown.

## Evidence and limits

The harness preserves per-run serial and build evidence in
`tar\target\woven-shell-windows-{1,2,4}cpu`.

This is software/2D WovenShell window-manager acceptance. It does not claim
pointer forwarding into application clients, USB HID qualification, hardware
GPU presentation, or direct QMP automation of a production WovenFiles drag.
---

## Source: docs/audit-stage21-10-package-rollback-2026-10-20.md

# Stage 21.10 WovenPackage bounded rollback audit â€” 2026-10-20

## Scope

This increment adds one durable rollback generation to the package service.
Rollback is bounded so payload storage cannot grow without limit.

## Implemented contract

- Each package transaction persists the current catalog as the previous
  generation before mutation.
- The previous payload generation remains available while it is rollbackable.
- A checksummed WRBK intent is durable before catalog restoration.
- Recovery replays a partially completed rollback idempotently.
- The next transaction retires only previous generations that are no longer
  active or the current rollback target.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-10-test -- -D warnings`: passed.
- Stage 21.10 QEMU passed with exit 33 on 1/2/4 CPUs; evidence:
  `audit-artifacts/stage21.10-{1,2,4}cpu-*`.
- Stage 21.9 QEMU passed again on 1/2/4 CPUs after rollback integration.

## Remaining acceptance prerequisites

Multi-package rollback, power-loss injection at every boundary, sandbox
enforcement, userspace package-service IPC/ownership, and physical USB
qualification remain open. No physical USB write was performed.
---

## Source: docs/audit-stage21-11-package-sandbox-2026-10-20.md

# Stage 21.11 WovenPackage sandbox launch audit â€” 2026-10-20

## Scope

This increment connects signed package capability declarations to the Ring-3
launch boundary without changing the existing two-argument spawn ABI.

## Implemented contract

- Package declarations must be subsets of the normal user-domain ceiling.
- File policy permits only system, temporary, mounted, and home scopes for
  declared file-read authority, and temporary/home for file-write authority.
- Device policy is derived from declared network/storage/display/input/device
  capabilities.
- Service discovery is restricted to the application service class.
- Invalid declarations destroy the prepared address space before publication.
- Valid declarations are installed as the task's immutable sandbox profile.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-11-test -- -D warnings`: passed.
- Stage 21.11 QEMU passed with exit 33 on 1/2/4 CPUs; evidence:
  `audit-artifacts/stage21.11-{1,2,4}cpu-*`.

## Remaining acceptance prerequisites

The long-lived userspace WovenPackage IPC broker, multi-package power-loss
recovery, and physical USB qualification remain open. No physical USB write
was performed.
---

## Source: docs/audit-stage21-1-wovenpackages-2026-10-20.md

# Stage 21.1 WovenPackages bounded envelope audit â€” 2026-10-20

## Scope

This increment establishes the first bounded `WPKG` package envelope in
`libwoven/src/package.rs` and exercises it through a feature-gated kernel
self-test. It is intentionally a format/parser increment, not an installer or
package-service acceptance.

## Implemented contract

- Magic `WPKG`, version `1`, fixed 92-byte header.
- Manifest, application, resource, and 64-byte signature fields.
- Manifest version, capability bitmap, bounded package name, and up to 16
  bounded dependencies.
- Manifest maximum 4096 bytes, application maximum 64 MiB, resource maximum
  16 MiB, names maximum 48 bytes.
- Exact-length and overflow checks before borrowed payload slices are returned.
- Parser tests for valid payloads, truncation, bad names, and length mismatch.
- Kernel `stage21-1-test` validates a package in freestanding execution and
  rejects a mutated magic field.

The parser does not authenticate the signature. The future package service
must define and enforce the trust root and signature domain before install.

## Validation

- `cargo test --workspace`: passed; 59 `libwoven` unit tests plus all workspace
  integration tests passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-1-test -- -D warnings`: passed.
- `python scripts/test-stage21-1-packages.py --cpus 1`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-1cpu-1792513802229723100`.
- `python scripts/test-stage21-1-packages.py --cpus 2`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-2cpu-1792513815638396400`.
- `python scripts/test-stage21-1-packages.py --cpus 4`: passed, exit 33,
  evidence `audit-artifacts/stage21.1-4cpu-1792513829341085500`.

## Remaining acceptance prerequisites

Package producer/signing tool, trust-root verification, persistent WovenStore
transactions, update/removal/rollback behavior, sandbox declaration
enforcement, service ownership/IPC, power-loss tests, and physical USB
qualification remain unimplemented. No physical USB write was performed.
---

## Source: docs/audit-stage21-2-wpkg-authentication-2026-10-20.md

# Stage 21.2 WovenPackages producer and authentication audit â€” 2026-10-20

## Scope

This increment adds the WPKG v1 producer and the kernel authentication
boundary. It does not install packages or mutate persistent storage.

## Implemented contract

- `tools/wpkg-packager` accepts an ELF, bounded manifest name/version,
  capability bits, bounded dependencies, and optional resource files.
- Producer output is parsed by the shared `libwoven::package` parser before it
  is written.
- Signature input is
  `SHA-256("WOVENHAT-WPKG-V1" || immutable_header_prefix || manifest ||
  application || resources)`.
- Kernel verification uses the provisioned WAPP application trust root and
  fails closed on malformed, untrusted, or tampered packages.
- Existing WAPP packaging remains unchanged.

## Validation

- `cargo test --manifest-path tools/wpkg-packager/Cargo.toml`: passed.
- `cargo clippy --manifest-path tools/wpkg-packager/Cargo.toml -- -D warnings`:
  passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-2-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Producer smoke test generated a valid 18,857-byte WPKG from the existing
  WovenCalculator ELF with capabilities and a dependency.
- Stage 21.2 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.2-1cpu-1792514011162163500`.
- Stage 21.2 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.2-2cpu-1792514024591383500`.
- Stage 21.2 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.2-4cpu-1792514038336540400`.

## Remaining acceptance prerequisites

Trust-key provisioning policy, persistent WovenStore installation, dependency
resolution, update/removal transactionality, rollback and power-loss recovery,
sandbox enforcement, service/IPC ownership, and physical USB qualification
remain unimplemented. No physical USB write was performed.
---

## Source: docs/audit-stage21-3-wovenstore-transactions-2026-10-20.md

# Stage 21.3 WovenStore transaction audit â€” 2026-10-20

## Scope

This increment adds the bounded metadata transaction core in
`libwoven/src/store.rs`. It is intentionally storage-independent; the caller
must authenticate WPKG before submitting a manifest and must persist a
successful commit through the future package service.

## Implemented contract

- Fixed catalog capacity of 32 installed entries.
- Explicit allowed-capability mask for sandbox declarations.
- Dependency presence checks before installation.
- Monotonic version updates; stale/equal versions are rejected.
- Dependent-protected removal.
- Copy-on-write transaction working state with explicit commit or rollback.
- Bounded metadata and application digest storage; package payloads are not
  copied into the catalog.

## Validation

- `cargo test -p libwoven`: passed; 61 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-3-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.3 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.3-1cpu-1792514233459105300`.
- Stage 21.3 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.3-2cpu-1792514246539222000`.
- Stage 21.3 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.3-4cpu-1792514260377814200`.

## Remaining acceptance prerequisites

Authenticated package-service integration, persistent WovenFS catalog/payload
publication, crash/power-loss recovery, dependency ordering across a package
batch, IPC authority, and physical USB qualification remain unimplemented.
No physical USB write was performed.
---

## Source: docs/audit-stage21-4-wovenstore-batches-2026-10-20.md

# Stage 21.4 WovenStore batch resolution audit â€” 2026-10-20

## Scope

This increment adds bounded dependency-ordered batch installation to the
WovenStore transaction core. It preserves the copy-on-write transaction
boundary and does not perform persistent storage I/O.

## Implemented contract

- At most 32 requests are accepted in one batch.
- Requests may be supplied in reverse dependency order.
- Missing dependencies are deferred across bounded passes.
- Cycles or absent dependencies fail with `MissingDependency`.
- Any non-dependency error aborts immediately without committing the working
  catalog.

## Validation

- `cargo test -p libwoven`: passed; 62 tests.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-4-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.4 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.4-1cpu-1792514391320435900`.
- Stage 21.4 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.4-2cpu-1792514406768911700`.
- Stage 21.4 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.4-4cpu-1792514420345269700`.

## Remaining acceptance prerequisites

Persistent WovenFS catalog/payload publication, crash and power-loss recovery,
package-service IPC authority, and physical USB qualification remain open.
No physical USB write was performed.
---

## Source: docs/audit-stage21-5-wovenstore-records-2026-10-20.md

# Stage 21.5 WovenStore durable record codec audit â€” 2026-10-20

## Scope

This increment defines the bounded `WSTC` v1 catalog record used by a future
package service for durable WovenFS publication. It does not perform storage
I/O or claim reboot durability.

## Implemented contract

- Fixed maximum record size and 32-entry catalog capacity.
- Magic, version, exact length, capability policy, entry metadata, and
  dependency fields.
- Checksum over the immutable header and entry bytes.
- Decode rejection for corruption, malformed lengths, invalid names, duplicate
  entries, and invalid dependency fields.
- Host round-trip and checksum-corruption tests.

## Validation

- `cargo test -p libwoven`: passed; 63 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.5 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.5-1cpu-1792514640957039400`.
- Stage 21.5 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.5-2cpu-1792514655302555900`.
- Stage 21.5 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.5-4cpu-1792514670539406700`.

## Remaining acceptance prerequisites

Native WovenFS write/read integration, power-loss recovery, package service
authority/IPC, persistent payload publication, and physical USB qualification
remain unimplemented. No physical USB write was performed.
---

## Source: docs/audit-stage21-6-wovenstore-persistence-2026-10-20.md

# Stage 21.6 WovenStore native persistence audit â€” 2026-10-20

## Scope

This increment wires the bounded WSTC catalog record to the existing
capability-gated native WovenFS writer and direct readback path. It uses only a
disposable QEMU ATA image; no physical USB is written.

## Implemented contract

- Stage 21.6 reuses the audited Stage 15.2 WovenFS scratch-volume prerequisite.
- `UpdateState::authorized_writer` is the only path used to obtain the private
  WovenFS writer token, and it requires `Capability::StorageIo`.
- The probe creates `/wfs/packages`, writes `/wfs/packages/catalog.wstc`
  through the native journal, reads it back from the mounted volume, and
  decodes the checksum-verified record.
- The feature gate suppresses earlier unrelated FAT32/Stage 15.2 exit blocks
  while retaining the required native storage implementation.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-6-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.6 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.6-1cpu-1792515029512587600`.
- Stage 21.6 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.6-2cpu-1792515043566058500`.
- Stage 21.6 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.6-4cpu-1792515057767369300`.

## Remaining acceptance prerequisites

Atomic catalog-plus-payload publication, persistent install/update/removal
transactions, power-loss injection and recovery, package-service IPC and
authority ownership, and physical USB qualification remain unimplemented.
---

## Source: docs/audit-stage21-7-package-publication-2026-10-20.md

# Stage 21.7 authenticated WovenPackage publication audit â€” 2026-10-20

## Scope

This increment adds the first payload publication path above the authenticated
WPKG verifier and native WovenFS catalog persistence. It validates one package
with a small ELF payload on a disposable ATA image.

## Implemented contract

- The service accepts a `Package` returned by the kernel WPKG verifier.
- Application and resource data are split into bounded native-WovenFS chunks.
- Chunk paths include package name and version, preserving the old generation
  during an update staging attempt.
- The WSTC catalog is committed only after all payload chunks are durable.
- Chunk and catalog failures clean up chunks created by the attempted install.
- The service's writer token is capability-gated through `StorageIo`.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-7-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.7 first boot attempt was rejected by the harness because an earlier
  Stage 15.2 exit block ran before the package marker; evidence is preserved at
  `audit-artifacts/stage21.7-1cpu-1792515202218981500`. The guard was corrected
  and the gate rerun.
- Stage 21.7 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.7-1cpu-1792515241512085800`.
- Stage 21.7 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.7-2cpu-1792515253821407400`.
- Stage 21.7 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.7-4cpu-1792515268155840600`.

## Remaining acceptance prerequisites

Multi-package transactional recovery, update/removal persistence, reboot and
power-loss injection, userspace package-service IPC/ownership, and physical
USB qualification remain unimplemented. No physical USB write was performed.
---

## Source: docs/audit-stage21-8-package-recovery-2026-10-20.md

# Stage 21.8 WovenPackage install-intent recovery audit â€” 2026-10-20

## Scope

This increment adds a bounded, checksummed WPKJ install intent around the
Stage 21.7 payload/catalog publication path. The gate simulates an interrupted
prepared install and validates orphan cleanup on a disposable ATA image.

## Implemented contract

- `Prepared` intent is durable before payload chunks are written.
- `Committed` intent is written after catalog publication.
- Recovery validates the journal checksum and package identity.
- Prepared intents without a matching catalog entry remove their application
  and resource chunks before the journal is cleared.
- Committed intents represented by the catalog are retained and cleared.
- A malformed journal fails closed and is cleared rather than trusted.

## Validation

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-8-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 21.8 QEMU passed with exit 33 on 1 CPU, evidence
  `audit-artifacts/stage21.8-1cpu-1792515508984877300`.
- Stage 21.8 QEMU passed with exit 33 on 2 CPUs, evidence
  `audit-artifacts/stage21.8-2cpu-1792515523112811000`.
- Stage 21.8 QEMU passed with exit 33 on 4 CPUs, evidence
  `audit-artifacts/stage21.8-4cpu-1792515537897023500`.

## Remaining acceptance prerequisites

Multi-package journal recovery, persistent update/removal transactions,
power-loss injection at every publication boundary, userspace package-service
IPC/ownership, and physical USB qualification remain unimplemented. No
physical USB write was performed.
---

## Source: docs/audit-stage21-9-update-removal-2026-10-20.md

# Stage 21.9 WovenPackage update/removal audit â€” 2026-10-20

## Scope

This increment closes the single-package update and removal publication gap
left by Stage 21.8. The catalog persists exact payload chunk counts, and the
intent record carries both the incoming and superseded generation.

## Implemented contract

- WSTC v2 records application and resource chunk counts per entry.
- Updates journal the old generation and retain it for the bounded rollback
  window; later transactions reclaim it when it is no longer active or
  rollback-capable.
- Removal writes a prepared intent, commits the catalog, and records the
  committed state before clearing the intent; payload cleanup is bounded by
  the rollback-generation policy.
- Recovery preserves the old generation when a removal did not reach catalog
  commit and cleans it when the catalog already reflects removal.
- Existing install and recovery behavior remains compatible with the new
  bounded record/journal layouts.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy -p libwoven --all-targets -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-9-test -- -D warnings`: passed.
- Stage 21.9 QEMU passed with exit 33 on 1/2/4 CPUs; evidence is under
  `audit-artifacts/stage21.9-{1,2,4}cpu-*`.
- Stage 21.6, 21.7, and 21.8 regression gates passed on 1/2/4 CPUs after
  these ABI and journal changes; newer timestamped evidence is retained under
  `audit-artifacts`.

## Remaining acceptance prerequisites

This is not full package-production acceptance. Multi-package recovery,
power-loss injection at every boundary, sandbox enforcement, userspace
service/IPC ownership, and physical USB qualification remain open.
No physical USB write was performed.
---

## Source: docs/audit-stage22-1-userspace-service-ipc-2026-10-20.md

# Stage 22.1 userspace service IPC ABI audit â€” 2026-10-20

## Scope

Expose the existing capability- and owner-aware IPC service registry to
userspace so the WovenPackage service can later run outside the kernel.

## Implementation

- Added syscall numbers 211â€“214 for endpoint creation, service publish,
  unpublish, and discovery.
- Added bounded service-name copy-in using `MAX_IPC_SERVICE_NAME` before IPC
  registry access.
- Required `Capability::Ipc` for every operation; endpoint handles are created
  for the current process and service handles remain process-local.
- Added typed `libwoven` wrappers and a dedicated Stage 22.1 QEMU marker.
- Added handle-addressed message send/receive syscalls 215â€“216 and typed
  `libwoven` wrappers in the Stage 22.2 follow-on.
- Corrected the Stage 22.1 feature graph to inherit the native WovenFS
  prerequisite and suppressed duplicate inherited self-test exits.

## Validation

- `cargo test -p libwoven`: 63 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-1-test -- -D warnings`: passed.
- QEMU acceptance: passed on 1 CPU (exit 33), 2 CPUs (exit 33), and 4 CPUs
  (exit 33).
- Accepted evidence:
  - `audit-artifacts/stage21.22-1cpu-1792517099255354900`
  - `audit-artifacts/stage21.22-2cpu-1792517111947242900`
  - `audit-artifacts/stage21.22-4cpu-1792517125292213600`

Stage 22.2 transport evidence:

- `audit-artifacts/stage21.22-1cpu-1792517297224254700`
- `audit-artifacts/stage21.22-2cpu-1792517311818527300`
- `audit-artifacts/stage21.22-4cpu-1792517324885076600`

The receive ABI was subsequently hardened to prefix each copied message with
the sender process ID, which is required for a broker to route responses using
the existing mailbox syscall. The corrected ABI passed the affected matrix:

- `audit-artifacts/stage21.22-1cpu-1792517632981988900`
- `audit-artifacts/stage21.22-2cpu-1792517647927733400`
- `audit-artifacts/stage21.22-4cpu-1792517661629792600`

## Failed attempt retained

The first 1-CPU attempt is preserved at
`audit-artifacts/stage21.22-1cpu-1792516996924333800`. It reached the
inherited FAT32 mutation probe and exited before `[S22.1]` because the new
feature did not yet inherit `stage15-2-test`. The feature graph and guards were
fixed; the rerun passed. This was a harness/configuration defect, not a
runtime IPC failure.

## Limits and next prerequisites

This gate validates the ABI surface and the existing kernel registry probe; it
does not yet exercise a real Ring-3 package broker/client. The transport is
validated through the existing kernel endpoint probe; next work must add the
versioned broker/client lifecycle and protocol, then repeat the same host,
Clippy, and 1/2/4-CPU acceptance discipline. No physical USB write was
performed.
---

## Source: docs/audit-stage22-3-wovenpackage-ipc-protocol-2026-10-20.md

# WovenPackage IPC protocol foundation audit â€” 2026-10-20

## Implementation

`libwoven::package_ipc` adds an allocation-free, bounded `WPKI` version-1
request/response codec. Requests carry an operation (`Install`, `Remove`,
`Rollback`, or `Query`), a request ID, and a validated path. Responses carry
the request ID, status, and value. Package payload bytes remain outside the
IPC frame and must be resolved through the authenticated storage service.

Path validation rejects empty, oversized, invalid-byte, and `..` traversal
segments. Frame lengths, version, magic, and operation values fail closed.

## Validation

- `cargo test -p libwoven`: 66 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

## Limits

This is a codec foundation only. No Ring-3 broker/client is wired to the
protocol yet, and no package operation is dispatched from these frames. The
next implementation must add service ownership/startup, decode-and-dispatch
with capability checks, response correlation, and 1/2/4-CPU QEMU acceptance.
No physical USB write was performed.
---

## Source: docs/audit-stage22-3-wovenpackage-service-2026-10-20.md

# Stage 22.3 WovenPackage Ring-3 broker audit â€” 2026-10-20

## Scope

Prove that the WovenPackage service boundary exists as a signed external
Ring-3 process and can publish, receive, dispatch, correlate, and reply over
the Stage 22 IPC ABI.

## Implementation

- Added `apps/woven-package`, a signed no-std service image.
- The service publishes `woven.package` with `Capability::Ipc`.
- The fixture creates a bounded `/tmp/vfs-self-test` record through the kernel
  VFS, discovers the service, sends a versioned `WPKI` query for that record,
  validates the correlated request ID, status, and existence value, and reaps
  the process.
- The receive ABI carries the sender PID; the broker uses the existing mailbox
  send path for the response.
- Service buffers are static bounded allocations, not user-stack allocations.
- Added `scripts/test-stage22-3-package-service.py` and feature-gated kernel
  acceptance `stage22-3-test`.

## Validation

- `cargo clippy --release --target x86_64-unknown-none -- -D warnings` in
  `apps/woven-package`: passed.
- `cargo test -p libwoven`: 66 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-3-test -- -D warnings`: passed.
- Stage 22.3 QEMU: passed on 1 CPU (exit 33), 2 CPUs (exit 33), and 4 CPUs
  (exit 33).
- Evidence:
  - `audit-artifacts/stage22.3-1cpu-1792519316113927900`
  - `audit-artifacts/stage22.3-2cpu-1792519342386172500`
  - `audit-artifacts/stage22.3-4cpu-1792519366024822100`

## Limits

The broker's query is an end-to-end protocol and storage-authority existence
proof.
Install, remove, and rollback deliberately return a storage-authority status
until a userspace
storage/package transaction API is available. Production system-app startup
wiring and physical USB qualification remain open. Earlier failed attempts,
including the preserved disk-full build failure and broker request-routing
failures, remain in their timestamped audit directories.
---

## Source: docs/audit-stage22-4-wovenpackage-startup-2026-10-20.md

# Stage 22.4 production WovenPackage startup audit â€” 2026-10-20

## Scope

Verify that the signed Ring-3 WovenPackage service is part of the production
`system-apps` image build and is started and publication-checked during boot.

## Implementation

- Added `scripts/build-package-service.py` to build and sign the service image.
- Added `WOVENHAT_PACKAGE_IMAGE` to the `system-apps` kernel image path.
- Added bounded startup publication validation owned by the spawned service PID.
- Updated the system-apps shell gate to preserve CPU-specific evidence and
  require the service publication marker.
- Corrected package-sandbox helper feature gates so production Clippy does not
  compile unreferenced Stage 21-only helpers.

## Validation

- Production system-apps shell/PS2/IOAPIC QMP gate passed on 1 CPU.
- Production system-apps shell/PS2/IOAPIC QMP gate passed on 2 CPUs.
- Production system-apps shell/PS2/IOAPIC QMP gate passed on 4 CPUs.
- Each serial log contains `[SERVICES] woven-package published as pid 1`.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- Stage22 freestanding Clippy, host all-targets Clippy, Python syntax, and diff
  checks: passed.

## Limits

The broker is now production-started, but its mutation operations deliberately
return storage status. A signed package payload/transaction ABI and its
crash-recovery acceptance remain required before package-management release
closure. Physical USB qualification remains separate and unperformed.
---

## Source: docs/audit-stage22-5-wovenpackage-transactions-2026-10-20.md

# Stage 22.5 WovenPackage transaction authority audit

Date: 2026-10-20
Owner: Codex
Scope: signed Ring-3 WovenPackage install/remove/rollback authority

## Result

Accepted as a bounded Stage 22.5 increment. The required QEMU transaction
gate passed on 1, 2, and 4 CPUs, each with exit code 33 and the serial marker
`[S22.5] WovenPackage transactions: PASSED`.

## Implementation boundary

Syscall 217 is exposed through `libwoven::package_transaction` and dispatched
by `apps/woven-package`. The kernel validates the operation and source path,
requires `FileRead`, `FileWrite`, and `StorageIo`, and then issues the private
WovenFS writer authority. Install reads a broker-owned VFS source into bounded
heap storage, verifies the WPKG envelope, and publishes authenticated,
chunked payloads with durable catalog and journal records. Remove and rollback
reuse recovered catalog state and durable rollback intent. Package bytes do
not cross the IPC control frame.

## Commands and results

The following commands were run from `C:\WovenHat-OS`:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-5-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test -p libwoven --lib
python scripts/test-stage22-5-package-transactions.py --cpus 1
python scripts/test-stage22-5-package-transactions.py --cpus 2
python scripts/test-stage22-5-package-transactions.py --cpus 4
python -m py_compile scripts/test-stage22-5-package-transactions.py
```

The focused transaction script rebuilt and signed the Ring-3 broker and WPKG
fixture for each run. The 66-test `libwoven` suite passed. Both Clippy runs
passed with `-D warnings`. The QEMU evidence is retained at:

```text
audit-artifacts/stage22.5-1cpu-1792521732550569700
audit-artifacts/stage22.5-2cpu-1792521764182836500
audit-artifacts/stage22.5-4cpu-1792521793861888600
```

## Limits and open prerequisites

The syscall path is bounded by the normal 512-byte path limit. A single source
package is limited by the 64 KiB VFS node capacity; native WovenFS publication
is chunked by its file limit. The service-enabled kernel task stack is 256 KiB
and remains protected by the task canary. This gate does not establish larger
source ingress, signing-key rotation, a complete update daemon, power-loss
timing qualification, or physical USB behavior. No physical USB write was
performed; the attached flash drive remains outside this accepted scope.
---

## Source: docs/audit-stage23-1-wovenai-broker-2026-10-20.md

# Stage 23.1 WovenAI broker audit

Date: 2026-10-20
Owner: Codex
Scope: bounded controlled-IPC WovenAI broker boundary

## Result

Accepted as the first Stage 23 increment. The dedicated QEMU gate passed on
1, 2, and 4 CPUs, each with exit code 33 and the serial marker
`[S23.1] WovenAI broker: PASSED`.

## Implemented boundary

`libwoven::ai_ipc` defines the fixed-layout `WAI1` protocol. Requests carry an
operation, correlation ID, and at most 224 bytes of validated UTF-8 input;
responses are fixed-size and return status plus value. Syscall 218 copies the
bounded request only after checking both `Capability::Ipc` and
`Capability::Ai`. The signed Ring-3 `apps/woven-ai` service publishes
`woven.ai`, returns `BROKER_ABI` for `Describe`, and returns `NoProvider` for
`Complete`. The probe is launched in the user domain with explicit `Ipc` and
`Ai` capabilities. The broker has `Ipc` only.

No model inference, provider plugin, cloud credential, file access, network
access, secret access, application control, or device access is implemented by
this stage.

## Commands and results

Run from `C:\WovenHat-OS`:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage23-1-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo clippy --manifest-path apps/woven-ai/Cargo.toml --target x86_64-unknown-none --release -- -D warnings
cargo test -p libwoven --lib
python -m py_compile scripts/test-stage23-1-ai-broker.py
python scripts/lint-all-features.py --only stage21
python scripts/test-stage23-1-ai-broker.py --cpus 1
python scripts/test-stage23-1-ai-broker.py --cpus 2
python scripts/test-stage23-1-ai-broker.py --cpus 4
```

The library suite passed 69 tests. The QEMU evidence is retained at:

```text
audit-artifacts/stage23.1-1cpu-1792522717245706600
audit-artifacts/stage23.1-2cpu-1792522745744098400
audit-artifacts/stage23.1-4cpu-1792522775899902400
```

## Limits and next prerequisites

The input ceiling is 224 UTF-8 bytes and the IPC transport remains bounded by
the existing message limit. Completion is intentionally unavailable until the
provider stages define local/provider ownership. Later stages must add model
resource accounting, provider isolation, credential handling, explicit data
capabilities, and separate hardware qualification. No physical USB write or
physical AI-device qualification was performed.
---

## Source: docs/audit-stage23-2-wovenai-local-provider-2026-10-20.md

# Stage 23.2 audit â€” bounded WovenAI local provider

Date: 2026-10-20

## Scope

This stage adds the first local WovenAI provider. It is deliberately bounded:
fixed integer weights, a fixed feature vocabulary, no allocation, bounded WAI1
UTF-8 input, an explicit `MODEL_ABI`, and no filesystem, network, secret,
device, or accelerator access. It is not a general neural-model runtime.

## Acceptance

The dedicated gate was run serially with the existing QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 1` â€” PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-1cpu-1792523271142588200`
- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 2` â€” PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-2cpu-1792523298416240400`
- `python scripts/test-stage23-1-ai-broker.py --stage 2 --cpus 4` â€” PASS,
  exit 33, `[S23.2] WovenAI local provider: PASSED`; evidence:
  `audit-artifacts/stage23.2-4cpu-1792523326958053700`

The probe verifies the declared `Ipc + Ai` capability boundary, service
publication, broker discovery, local-provider descriptor/model ABI, expected
classification result, and broker teardown. The denied probe must fail with
the capability-denied exit path.

## Static and host checks

- `cargo test -p libwoven --lib` â€” 71 passed.
- Kernel freestanding Clippy with `stage23-1-test` and `stage23-2-test` â€” pass.
- Host all-targets Clippy â€” pass.
- `apps/woven-ai` freestanding release Clippy â€” pass.
- Python syntax validation and Stage 21 feature lint â€” required release checks.

## Limits and next prerequisites

This acceptance does not qualify arbitrary model files, tensor runtimes,
hardware acceleration, cloud providers, semantic search, assistant actions,
or physical USB media. Those remain later stages and require their own
capability, resource, recovery, and 1/2/4-CPU acceptance evidence.
---

## Source: docs/audit-stage23-3-wovenai-provider-plugin-2026-10-20.md

# Stage 23.3 audit â€” WovenAI provider-plugin boundary

Date: 2026-10-20

## Scope

This stage adds a separate Ring-3 provider process and broker delegation path.
The provider advertises a bounded ABI, owns its WovenVault credential handle,
and returns only fixed-size WAI1 responses. The broker never receives the
credential bytes. The acceptance provider is an offline deterministic fixture;
it is not an arbitrary cloud API or HTTPS/TLS implementation.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 1` â€” PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-1cpu-1792523642289709300`
- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 2` â€” PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-2cpu-1792523675601682100`
- `python scripts/test-stage23-1-ai-broker.py --stage 3 --cpus 4` â€” PASS,
  exit 33, `[S23.3] WovenAI cloud provider: PASSED`; evidence:
  `audit-artifacts/stage23.3-4cpu-1792523708187643800`

The gate starts the provider with `Ipc + Secrets + NetworkIo`, starts the
broker with `Ipc` only, verifies both service publications, checks unauthorized
client denial, confirms an authorized probe receives the provider-marked
result, and reaps both processes.

## Static and host checks

- `cargo test -p libwoven --lib` â€” 73 passed.
- Stage 23.3 freestanding kernel Clippy with `-D warnings` â€” pass.
- Host all-targets Clippy â€” pass.
- `apps/woven-ai` freestanding release Clippy â€” pass.
- Python syntax, `git diff --check`, and all Stage 21 feature lints â€” pass.
- Stage 23.1 and 23.2 regression gates â€” pass on 1/2/4 CPUs.

## Limits and next prerequisites

Real cloud endpoint configuration, TLS/certificate verification, credential
provisioning and rotation, retry/backoff, quota/billing controls, network
availability policy, and physical network/USB qualification remain open.
---

## Source: docs/audit-stage23-4-wovenai-capability-security-2026-10-20.md

# Stage 23.4 audit â€” WovenAI capability security

Date: 2026-10-20

## Scope

This stage turns the AI authority rule into an executable WovenGuard acceptance
gate. Local AI receives only `Ipc + Ai`; cloud provider authority is explicitly
`Ipc + Secrets + NetworkIo`; broker authority remains `Ipc`. File masks,
device masks, domain ceilings, and capability membership are checked through
the same policy functions used at launch.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 1` â€” PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-1cpu-1792524134603006000`
- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 2` â€” PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-2cpu-1792524151189661900`
- `python scripts/test-stage23-1-ai-broker.py --stage 4 --cpus 4` â€” PASS,
  exit 33, `[S23.4] WovenAI capability security: PASSED`; evidence:
  `audit-artifacts/stage23.4-4cpu-1792524165773461400`

The gate verifies that local AI has no file/device authority, cloud authority
allows network only when explicitly declared, and system services cannot hold
the sensitive AI/secret/camera/microphone capabilities.

## Static checks and limits

- Stage 23.4 freestanding kernel Clippy with `-D warnings` â€” pass.
- Prior Stage 23.1/23.2/23.3 QEMU gates remain green.
- The policy gate does not qualify HTTPS/TLS, real cloud providers, semantic
  search, assistant actions, or physical USB/network hardware.
---

## Source: docs/audit-stage23-5-wovenai-semantic-search-2026-10-20.md

# Stage 23.5 audit â€” bounded semantic search core

Date: 2026-10-20

## Scope

This stage adds the first bounded search index for applications, files,
settings, messages, documents, and commands. It validates UTF-8 titles,
normalizes bounded ASCII terms, scores query-term coverage, and returns stable
ranked hits from fixed-capacity storage without allocation.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 1` â€” PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-1cpu-1792524571689446000`
- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 2` â€” PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-2cpu-1792524589227010700`
- `python scripts/test-stage23-1-ai-broker.py --stage 5 --cpus 4` â€” PASS,
  exit 33, `[S23.5] WovenAI semantic search: PASSED`; evidence:
  `audit-artifacts/stage23.5-4cpu-1792524606220735100`

## Static and host checks

- `cargo test -p libwoven --lib` â€” 76 passed.
- Stage 23.5 freestanding kernel Clippy with `-D warnings` â€” pass.
- Host all-targets Clippy â€” pass.
- Python syntax validation and `git diff --check` â€” pass.

## Limits and next prerequisites

The accepted core does not yet ingest durable filesystem/message sources,
provide vector embeddings, evaluate ranking quality, or expose a user-facing
search service. Those remain required follow-up gates before claiming complete
semantic search.
---

## Source: docs/audit-stage23-6-wovenassistant-actions-2026-10-20.md

# Stage 23.6 audit â€” WovenAssistant permissioned actions

Date: 2026-10-20

## Scope

This stage adds a fixed-capacity action planner. Actions have bounded validated
targets, map to explicit Woven permission authorities, and require a separate
confirmation for side effects. The planner does not execute platform actions;
execution remains a permission-broker and capability-policy responsibility.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 1` â€” PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-1cpu-1792524840375472300`
- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 2` â€” PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-2cpu-1792524861370942600`
- `python scripts/test-stage23-1-ai-broker.py --stage 6 --cpus 4` â€” PASS,
  exit 33, `[S23.6] WovenAssistant actions: PASSED`; evidence:
  `audit-artifacts/stage23.6-4cpu-1792524875749738800`

## Static and host checks

- `cargo test -p libwoven --lib` â€” 78 passed.
- Stage 23.6 freestanding kernel Clippy with `-D warnings` â€” pass.
- Host all-targets Clippy â€” pass.
- Python syntax validation and `git diff --check` â€” pass.

## Limits and next prerequisites

Permission-broker integration, user confirmation UX, audit-visible action
records, cancellation/teardown, and actual application/file/network executors
remain open. The acceptance does not claim complete assistant functionality or
physical hardware qualification.
---

## Source: docs/audit-stage23-7-wovenai-security-monitor-2026-10-20.md

# Stage 23.7 audit â€” advisory WovenAI security monitor

Date: 2026-10-20

## Scope

This stage adds fixed-size advisory explanations over the existing security
audit event ABI. WovenGuard remains the only enforcement authority; the
monitor cannot grant, revoke, or mutate capability policy.

## Acceptance

The dedicated gate was run serially with the QEMU/OVMF harness:

- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 1` â€” PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-1cpu-1792525067904963600`
- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 2` â€” PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-2cpu-1792525085640869300`
- `python scripts/test-stage23-1-ai-broker.py --stage 7 --cpus 4` â€” PASS,
  exit 33, `[S23.7] WovenAI security monitor: PASSED`; evidence:
  `audit-artifacts/stage23.7-4cpu-1792525103539579600`

## Static and host checks

- `cargo test -p libwoven --lib` â€” 80 passed.
- Stage 23.7 freestanding kernel Clippy with `-D warnings` â€” pass.
- Host all-targets Clippy â€” pass.
- Python syntax validation and `git diff --check` â€” pass.

## Limits and next prerequisites

Anomaly detection, monitor UI, notification transport, response automation,
and real-world policy tuning remain open. The monitor is advisory only and
does not qualify production security operations or physical hardware.
---

## Source: docs/audit-stage24-10-lockout-2026-10-20.md

# Stage 24.10 â€” kernel identity lockout and rate limiting

Date: 2026-10-20

## Scope

The owner-bound kernel identity runtime now applies a bounded per-account
lockout policy to both plaintext and trusted-login-manager digest paths. Five
failed authentication attempts enter a five-second cooldown; successful
authentication clears the failure state. The fixed table is bounded to the
maximum identity user count, resets on authority installation, and fails closed
if its capacity is unexpectedly exhausted. WovenShell now sends a candidate
digest for every entered PIN so failed login-manager attempts reach the kernel
policy instead of being rejected only in userspace.

## Acceptance

- `cargo test -p libwoven`: 86 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed, including the lockout self-test.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.10-lockout-1cpu-1792533000000000000`
- `audit-artifacts/stage24.10-lockout-2cpu-1792533000000000000`
- `audit-artifacts/stage24.10-lockout-4cpu-1792533000000000000`

The first 4-CPU attempt emitted the known migration-stress slot-capacity
panic; the unchanged rerun passed. The final passing logs are preserved above;
the failure is recorded here because the harness reuses its fixed target log
paths.

## Limits and next prerequisites

The lockout state is runtime-only and is reset after reboot or authority
reload; durable policy state and administrator-configurable thresholds remain
open. The hardware-demo gate does not inject five failed PINs through the UI,
so the dedicated proof is the Stage 24.5 kernel self-test. Account creation,
deletion, and group administration, malformed-request telemetry,
hardware-backed credentials, release-key provisioning, and physical
USB/power-loss qualification remain release prerequisites.
---

## Source: docs/audit-stage24-11-admin-abi-2026-10-20.md

# Stage 24.11 â€” bounded identity administration ABI

Date: 2026-10-20

## Scope

The identity policy now exposes bounded administrator operations for adding a
user, deleting a user, and adding a group. The kernel ABI reserves syscalls
226â€“228. Each request is copied from userspace into fixed-size storage and
requires the caller's owner-bound identity session to be active, elevated, and
administrator-backed. A caller cannot delete its own account or an account
with an active session. All decisions enter the security ledger with stable
public action IDs 31â€“33.

## Acceptance

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.11-admin-abi-1cpu-1792533600000000000`
- `audit-artifacts/stage24.11-admin-abi-2cpu-1792533600000000000`
- `audit-artifacts/stage24.11-admin-abi-4cpu-1792533600000000000`

## Limits and next prerequisites

The ABI and policy are live, but this slice does not yet persist new users or
groups to the account file/WID2 snapshot, expose an administration UI, or
provide group membership mutation/deletion. Those are required before calling
identity administration production-complete. Hardware credentials,
malformed-request telemetry, release-key provisioning, and physical
USB/power-loss qualification also remain open.
---

## Source: docs/audit-stage24-12-admin-persistence-2026-10-20.md

# Stage 24.12 â€” durable identity administration

Date: 2026-10-20

## Scope

Administrator add-user, delete-user, and add-group operations now publish
through the authenticated WID2 dual-copy store. User additions also update the
trusted account file with the authority-generated salt and digest; deletions
remove the account file entry. Mutation failures restore the prior live
authority and attempt to restore the previous WID2 snapshot. The scheduler
migration self-test now waits for bounded reclamation of coverage-probe slots
before starting stress jobs.

## Acceptance

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 24.5 and production `system-apps` freestanding Clippy passed with `-D warnings`.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.12-admin-persistence-1cpu-1792534200000000000`
- `audit-artifacts/stage24.12-admin-persistence-2cpu-1792534200000000000`
- `audit-artifacts/stage24.12-admin-persistence-4cpu-1792534200000000000`

## Limits and next prerequisites

The QEMU fixture has no ATA WovenFS disk, so the live gate proves safe
no-durable-device behavior; native WID2 persistence remains covered by the
Stage 24.3 storage self-test. Group membership mutation/deletion, an
administration UI, durable/configurable lockout, hardware-backed credentials,
malformed-request telemetry, release-key provisioning, and physical
USB/power-loss qualification remain open.
---

## Source: docs/audit-stage24-13-group-membership-2026-10-21.md

# Stage 24.13 group membership administration audit

Date: 2026-10-21

## Implemented

- Added bounded elevated-administrator operations for adding and removing supplementary group membership.
- Rejects ordinary callers, unknown users/groups, duplicate membership, full groups, and removal of a user's primary group.
- Added syscalls 229 and 230 with stable security-ledger actions 34 and 35.
- Persists the changed authority through WID2 and rolls the live authority back when publication fails.
- Keeps membership storage bounded by the existing fixed user/group limits.

## Acceptance evidence

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Production `system-apps` kernel Clippy with signed image environment: passed.
- Signed system-apps hardware-demo shell acceptance passed on 1, 2, and 4 virtual CPUs, covering desktop sign-in, ten signed applications, PS/2/IOAPIC input, SMP migration stress, scheduler/barrier checks, and acknowledged TLB shootdowns.
- Logs: `audit-artifacts/stage24.13-group-membership-{1,2,4}cpu-1792534800000000000`.

## Limits and next prerequisites

Administration UI/query APIs, durable/configurable lockout, hardware-backed credentials, malformed-request telemetry, release-key provisioning, physical USB/power-loss recovery, and release qualification remain open. QEMU has no ATA WovenFS device; the durable WID2 path is covered by the native storage self-test and the live gate proves safe operation when no durable disk is present.
---

## Source: docs/audit-stage24-14-admin-query-ui-2026-10-21.md

# Stage 24.14 acceptance audit â€” bounded administration query UI

Date: 2026-10-21

## Delivered

This stage adds a bounded, read-only administration query ABI and integrates it
with WovenSecurity. An elevated owner-bound administrator session can query
non-secret user and group records, including IDs, account/group names, primary
GIDs, administrator flags, and bounded member IDs. The ABI has fixed maximum
record counts and explicit version/kind/count framing; it never returns salts,
password hashes, PINs, or session credentials. The kernel copies all request
and response buffers through the existing userspace validation path and records
the query as audit action 36.

WovenSecurity now provides a keyboard-driven administrator prompt and bounded
directory view. The session is closed and PIN storage is cleared on window
close or failed query. The hardware-demo fixture promotes only its first
volatile demo account to administrator so the UI path is testable; production
account loading does not promote accounts.

## Acceptance evidence

- `cargo test -p libwoven`: 88 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity freestanding Clippy and production system-apps kernel Clippy:
  passed.
- `git diff --check`: passed.
- Focused WovenSecurity 1-CPU QEMU acceptance passed; serial evidence contains
  `WovenSecurity: administrator directory ready`.
- Signed system-apps shell acceptance passed on 1, 2, and 4 CPUs.

Preserved logs are under
`audit-artifacts/stage24.14-admin-query-ui-20261021/`, with per-CPU shell
serial logs and the focused WovenSecurity serial log.

## Security and lifecycle review

The query requires an owner-bound live session and elevated administrator
state. The query syscall itself is not gated by the desktop AccountManage
capability because WovenSecurity is a normal signed application; the identity
authority remains the security boundary and requires the administrator
elevation proof. Mutation syscalls retain their existing capability and
elevation checks. Output is bounded before allocation and every UI-owned
session is reclaimed on close.

## Limits and next prerequisites

This is a query/directory surface, not the final mutation-management UI.
Configurable durable lockout, hardware-backed credentials, malformed-request
telemetry, release-key provisioning, physical USB qualification, and the
remaining production/release gates are still open. The attached USB device was
not written, reformatted, or otherwise modified.
---

## Source: docs/audit-stage24-15-admin-mutation-ui-2026-10-21.md

# Stage 24.15 acceptance audit â€” administration mutation UI

Date: 2026-10-21

## Delivered

WovenSecurity now exposes the complete bounded identity administration flow
behind its elevated owner-bound session: add regular user, add group, add and
remove supplementary membership, and delete an inactive user. Each operation
uses fixed buffers, bounded ASCII/decimal input, explicit Enter transitions,
kernel-side identity validation, and a directory refresh after success. The
directory displays stable user/group IDs so the member and delete operations
are usable without hidden state.

The two mutation syscall capability checks that only applied to the desktop
session were removed from the admin-add entry points. The identity runtime is
the authorization boundary: every mutation still requires the calling process
to own an active elevated administrator session. Unauthenticated processes and
ordinary sessions cannot mutate identity state.

The disposable `hardware-demo` fixture has an explicit volatile fallback when
no durable WovenFS volume exists. Production builds do not enable this feature
and continue to fail closed on durable publication or trusted account-file
failure.

## Acceptance evidence

- `cargo test -p libwoven`: 88 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity freestanding Clippy: passed.
- Production system-apps kernel Clippy: passed.
- Combined `system-apps,hardware-demo` kernel Clippy: passed.
- `git diff --check`: passed.
- Focused WovenSecurity 1-CPU QEMU acceptance passed, exercising directory
  authentication, group creation, user creation, membership add/remove, and
  user deletion.
- Signed system-apps shell acceptance passed on 1, 2, and 4 CPUs.

Preserved logs are under
`audit-artifacts/stage24.15-admin-mutation-ui-20261021/`. The focused serial
log contains `group added`, `user added`, `membership added`, `membership
removed`, and `user deleted` markers.

## Limits and next prerequisites

The mutation UI is complete for the existing identity ABI, but durable
mutation behavior still requires a real WovenFS-backed device; the no-disk
QEMU path deliberately uses only the test-feature volatile fallback. Durable
and configurable lockout, hardware-backed credentials, malformed-request
telemetry, release-key provisioning, physical USB qualification, and the
remaining production/release gates remain open. The attached USB device was
not written or modified.
---

## Source: docs/audit-stage24-16-durable-lockout-policy-2026-10-21.md

# Stage 24.16 audit â€” durable/configurable lockout policy

Date: 2026-10-21
Owner: Codex
Status: accepted

## Delivered

- Added a bounded administrator lockout threshold of 1 through 32 failed
  authentications, defaulting to 5.
- Persisted the policy in the existing WID1 reserved header byte. Version 1
  snapshots remain readable with the default policy; version 2 snapshots
  retain the selected policy; durable WID2 records continue to wrap this
  authenticated WID1 payload.
- Added userspace syscall 232 and audit action 37. Authorization is enforced
  by the owner-bound elevated identity session, not by the desktop UI.
- Applied the persisted threshold to plaintext and verified-digest login and
  unlock paths. The fixed five-second cooldown, bounded table, success reset,
  and fail-closed behavior remain unchanged.
- Added the WovenSecurity fixed-buffer decimal editor and exercised it through
  the live administrator UI.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity app target Clippy with `-D warnings`: passed.
- Kernel `system-apps` Clippy with `-D warnings`: passed.
- Kernel `system-apps,hardware-demo` Clippy with `-D warnings`: passed.
- Focused WovenSecurity QEMU administrator exercise: passed on 1 CPU,
  including the threshold update marker.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs, including
  the ten packaged applications, desktop startup, PS/2/IOAPIC input, and SMP
  checks.
- Evidence: `audit-artifacts/stage24.16-lockout-policy-20261021`.

## Limits and next prerequisites

The cooldown duration is intentionally still a fixed kernel policy; this
stage makes only the failure threshold configurable. Hardware-backed
credentials, malformed-request telemetry, release-key provisioning, and
physical hardware qualification remain release prerequisites. The volatile
authority fallback is compiled only for the disposable `hardware-demo`
fixture; production durable publication remains fail-closed. The attached USB
device was not written, reformatted, or otherwise modified.
---

## Source: docs/audit-stage24-17-tpm2-platform-inventory-2026-10-21.md

# Stage 24.17 audit â€” TPM2 platform discovery boundary

Date: 2026-10-21
Owner: Codex
Status: accepted as an integration boundary

## Delivered

- Added bounded `Tpm2Info` data to validated ACPI discovery.
- Validated the TPM2 signature, table length, non-zero control-area address,
  and retained platform class/start method metadata.
- Logged an explicit boot result. QEMU without a TPM reports:
  `[TPM] ACPI TPM2 unavailable; hardware credential root not provisioned`.
- Added ACPI self-test coverage for a valid TPM2 descriptor and zero-address
  rejection.
- Kept TPM table presence strictly informational. No random value, firmware
  address, or software measurement is promoted into trusted credentials.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.17-tpm2-platform-inventory-20261021`.
- Read-only host inventory found the attached USB as a separate 15.7-GB GPT
  disk; it was not written, formatted, mounted, or passed destructively to
  QEMU.

## Explicitly incomplete

This stage does not claim hardware-backed identity. The next implementation
must provide CRB transport, validate PCR/attestation evidence against a
provisioned policy, and implement sealed credential-root
provisioning/recovery. A TPM
query on the Windows host required Administrator elevation, so no physical
TPM result was asserted from the host. Physical qualification remains open.
---

## Source: docs/audit-stage24-18-tpm2-crb-probe-2026-10-21.md

# Stage 24.18 audit â€” bounded TPM2 CRB probe

Date: 2026-10-21
Owner: Codex
Status: accepted as a transport increment

## Delivered

- Added a serialized TPM2 CRB `GetRandom` transaction over the validated ACPI
  control area.
- Validated CRB interface type, command/response capacity and address ranges,
  fatal status, TPM response code, response size, and bounded output length.
- Used a fixed poll-count timeout because the probe runs before timer IRQs are
  enabled; it cannot spin forever on a wedged device.
- Wiped probe output immediately after inventory logging. Random output is not
  persisted and is explicitly not treated as a stable credential root.
- Kept absent TPM hardware fail-closed. Current QEMU firmware has no TPM2
  table, so the live gate exercises the absence path rather than claiming a
  hardware transaction.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.18-tpm2-crb-probe-20261021`.

## Explicitly incomplete

This is not hardware-backed identity. Production credentials still require
authenticated PCR/attestation policy, sealed TPM NV/root provisioning,
recovery/rotation, and physical TPM qualification. The host TPM query required
Administrator elevation; no TPM presence was asserted from Windows. The USB
flash disk was not modified.
---

## Source: docs/audit-stage24-19-pcr-evidence-2026-10-21.md

# Stage 24.19 audit â€” bounded PCR evidence read

Date: 2026-10-21
Owner: Codex
Status: accepted as an evidence increment

## Delivered

- Added SHA-256 `PCR_Read` command framing for one bounded PCR index.
- Added response validation for TPM error code, total response length, one
  SHA-256 selection, selected-PCR bit, one digest, and exactly 32 digest bytes.
- Added malformed-selection coverage to the QEMU-feature codec self-test.
- Boot reads PCR 7 only when a validated TPM2 descriptor is present and wipes
  the digest after logging a four-byte inventory prefix.
- Kept PCR output informational. It is not compared to a release policy and
  cannot authorize identity or volume credentials.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.19-pcr-evidence-20261021`.
- Current QEMU firmware has no TPM2 ACPI table, so live acceptance verifies
  the fail-closed absence path and codec self-test rather than a hardware PCR.

## Explicitly incomplete

PCR reads are not signed attestation. Production identity still requires a
provisioned expected PCR policy or verified TPM quote, attestation-key and
certificate validation, sealed NV credential-root provisioning, recovery and
rotation, and physical TPM qualification. The attached USB disk was not
modified.
---

## Source: docs/audit-stage24-1-wovenidentity-2026-10-20.md

# Stage 24.1 audit â€” WovenIdentity policy core

Date: 2026-10-20

Implemented `libwoven::identity::Authority` with fixed-capacity local users,
groups, PBKDF2-HMAC-SHA256 credential verification, profiles, sessions,
lock/unlock state, and password-confirmed elevation. The implementation stores
only salts and derived hashes and fails closed on invalid names, passwords,
duplicate identities, capacity exhaustion, locked elevation, wrong
credentials, and non-administrator elevation. User insertion preflights group
capacity so a full group cannot leave a partially created user.

Validation:

- `cargo test -p libwoven --lib`: 83 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-1-test -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.1] WovenIdentity core: PASSED` on 1/2/4 CPUs after
  the elevation-boundary correction.

Evidence:

- `audit-artifacts/stage24.1-1cpu-1792525654517891200`
- `audit-artifacts/stage24.1-2cpu-1792525721415869300`
- `audit-artifacts/stage24.1-4cpu-1792525782428769900`

Limits: this stage does not claim kernel task credential switching, durable
identity database persistence/recovery, capability-broker integration,
lockout/rate limiting, administration UI, or hardware-backed credentials.
---

## Source: docs/audit-stage24-20-signed-pcr-policy-2026-10-21.md

# Stage 24.20 audit â€” signed PCR policy boundary

Date: 2026-10-21
Owner: Codex
Status: accepted as a policy-verification boundary

## Delivered

- Added fixed-size `TPP1` policy framing: version, signed length, PCR index,
  reserved bytes, expected SHA-256 digest, and Ed25519 signature.
- The caller supplies the trusted policy key; the envelope cannot select its
  own authority.
- Signature failure, malformed version/length/index/reserved fields, and PCR
  digest mismatch fail closed.
- Added QEMU-feature self-tests for valid policy, tampered signed data, and
  exact PCR matching.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel `system-apps` and `qemu-test` Clippy with `-D warnings`: passed.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs.
- Evidence: `audit-artifacts/stage24.20-signed-pcr-policy-20261021`.
- Current QEMU has no TPM2 ACPI table; live evidence remains the fail-closed
  absence path plus codec self-tests, not a physical TPM transaction.

## Explicitly incomplete

No runtime release policy is provisioned. This stage does not verify a TPM
quote or attestation key/certificate, does not create or unseal a TPM NV
credential root, and does not bind WovenIdentity to PCR policy. Physical TPM
qualification and the attached USB disk remain untouched.
---

## Source: docs/audit-stage24-21-nv-public-metadata-2026-10-21.md

# Stage 24.21 audit â€” bounded TPM NV public metadata

Date: 2026-10-21

## Delivered

- Added fixed-size no-session `NV_ReadPublic` command framing.
- Added fail-closed parsing of the TPM response code, response size, NV
  public area, bounded authorization policy, data size, and trailing Name.
- Kept the boundary metadata-only: it does not read or write secret NV data,
  create an index, authorize a session, or promote any value to a credential
  root.
- Added a QEMU-feature codec self-test for the command and response shape.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.21-nv-public-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so no live NV command
was issued. A production credential root still requires a physical TPM2 test
matrix, authorization-session implementation, policy-bound sealed object or
NV provisioning, quote/attestation-key verification, rollback/recovery
handling, secure release-key custody, and hardware qualification. The
attached USB was inventory-only; it was not written, formatted, or mounted.
---

## Source: docs/audit-stage24-22-credential-root-policy-2026-10-21.md

# Stage 24.22 audit â€” credential-root NV policy validation

Date: 2026-10-21

## Delivered

- Added `validate_credential_root_nv` to validate the decoded public metadata
  of the reserved 32-byte credential-root NV object.
- Required the expected NV index, SHA-256 name algorithm, 32-byte data size,
  32-byte PCR policy, and policy-authorized writes.
- Rejected ordinary owner/object authorization writes and policy deletion.
- Added valid, owner-write, and policy-mismatch codec self-test coverage.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.22-credential-root-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage performs
no live TPM transaction. It does not create an NV index, start an authorization
session, read or write credential bytes, validate a TPM quote, or provide
recovery. Those operations require a physical TPM2 test matrix and remain
next-stage work. The attached USB was inventory-only and was not written,
formatted, or mounted.
---

## Source: docs/audit-stage24-23-policy-session-2026-10-21.md

# Stage 24.23 audit â€” bounded TPM policy-session framing

Date: 2026-10-21

## Delivered

- Added bounded policy `StartAuthSession` request framing.
- Restricted the request to null TPM/bind handles, bounded caller nonce,
  empty encrypted salt, null symmetric protection, and SHA-256 policy hash.
- Added fail-closed response parsing for TPM status, response size, nonzero
  session handle, bounded TPM nonce, and exact response framing.
- Added executable QEMU self-test coverage for command and response fields.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.23-policy-session-20261021/`.

## Diagnostic record and limits

The first executable self-test exposed an offset assertion defect and exited
35. The field offsets were corrected, temporary diagnostics were removed, and
the rerun passed. This stage still performs no live TPM transaction on the
current QEMU platform, which has no TPM2 ACPI descriptor. It does not bind an
authorization session to an NV command, create or write an object, validate a
TPM quote, or provide recovery. The attached USB was inventory-only and was
not written, formatted, or mounted.
---

## Source: docs/audit-stage24-24-policy-pcr-2026-10-21.md

# Stage 24.24 audit â€” bounded TPM PolicyPCR transcript

Date: 2026-10-21

## Delivered

- Added bounded `PolicyPCR` request framing for one nonzero policy session,
  one PCR index, and one SHA-256 expected digest.
- Added exact response-size and TPM response-code validation for the policy
  command response.
- Added executable QEMU self-test coverage for the PCR selection, digest,
  session handle, successful response, and malformed response rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.24-policy-pcr-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so the command codec
is tested with synthetic responses and no live TPM transaction occurs. The
stage does not run `PolicyPCR` against a physical TPM, bind an authorization
session to NV write commands, create or write credentials, validate a quote,
or provide recovery. Physical TPM testing, secure provisioning, and release
qualification remain required. The attached USB was inventory-only and was
not written, formatted, or mounted.
---

## Source: docs/audit-stage24-25-nv-write-policy-2026-10-21.md

# Stage 24.25 audit â€” policy-session NV write framing

Date: 2026-10-21

## Delivered

- Added sessions-tagged `NV_Write` framing for a bounded 32-byte
  credential-root payload.
- Bound the auth handle and NV index to the same caller-selected index.
- Enforced a nonzero policy-session handle, zero NV offset, bounded caller
  nonce, continue-session attribute, and empty HMAC for the policy-session
  command shape.
- Added response validation for TPM status, sessions tag, zero parameter
  area, bounded response nonce/HMAC, and exact response length.
- Added executable QEMU self-test coverage for valid and malformed frames.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.25-nv-write-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and writes no device state. It does not create an NV index,
establish owner authorization, persist a credential, validate a TPM quote, or
provide recovery. Physical TPM testing, secure provisioning, quote
verification, and release qualification remain required. The attached USB
was inventory-only and was not written, formatted, or mounted.
---

## Source: docs/audit-stage24-26-nv-define-space-2026-10-21.md

# Stage 24.26 audit â€” bounded TPM NV definition framing

Date: 2026-10-21

## Delivered

- Added sessions-tagged owner-authorized `NV_DefineSpace` framing.
- Bounded the caller-supplied owner password-session authorization to 32
  bytes and kept it transient in the command buffer.
- Encoded the fixed credential-root index, SHA-256 name algorithm, 32-byte
  PCR policy, exact NV attributes, and 32-byte data size.
- Added executable QEMU self-test coverage for command handles,
  authorization area, public metadata, and size fields.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.26-nv-define-space-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and creates no device state. It does not persist owner
authorization, write credential bytes, validate a TPM quote, or provide
recovery. Physical TPM testing, secure provisioning, quote verification, and
release qualification remain required. The attached USB was inventory-only
and was not written, formatted, or mounted.
---

## Source: docs/audit-stage24-27-nv-read-policy-2026-10-21.md

# Stage 24.27 audit â€” policy-session NV read-back

Date: 2026-10-21

## Delivered

- Added bounded policy-session `NV_Read` framing for the fixed index, zero
  offset, and 32-byte credential-root value.
- Added fail-closed response parsing for the sessions tag, TPM response code,
  parameter size, exact data length, response nonce, attributes, HMAC, and
  total response size.
- Added executable QEMU self-test coverage for exact read-back contents and
  malformed parameter-size rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.27-nv-read-policy-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor, so this stage issues
no live command and reads no device state. It does not create or write an NV
index, persist a credential, validate a TPM quote, or provide recovery.
Physical TPM provisioning, quote verification, and release qualification
remain required. The attached USB was inventory-only and was not written,
formatted, or mounted.
---

## Source: docs/audit-stage24-28-credential-root-readback-2026-10-21.md

# Stage 24.28 audit â€” credential-root read-back promotion boundary

Date: 2026-10-21

## Delivered

- Added a combined credential-root promotion validator.
- Required the expected NV index, SHA-256 metadata, exact policy, write
  restrictions, 32-byte size, and exact returned root value.
- Used a constant-time comparison for the 32-byte expected/actual root.
- Added executable QEMU self-test coverage for valid promotion and a
  one-byte credential mismatch.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.28-credential-root-readback-20261021/`.

## Limits and next prerequisites

The current QEMU platform has no TPM2 ACPI descriptor. No credential root is
stored or promoted, no TPM quote is verified, and no physical hardware state
is changed. Crash recovery, physical TPM qualification, secure owner-key
custody, and release acceptance remain required. The attached USB was
inventory-only and was not written, formatted, or mounted.
---

## Source: docs/audit-stage24-29-quote-attestation-2026-10-21.md

# Stage 24.29 audit â€” bounded TPM quote attestation parsing

Date: 2026-10-21

## Delivered

- Added bounded parsing of the TPM `TPMS_ATTEST` quote structure.
- Validated generated-value magic, quote type, qualified-signer bound,
  qualifying nonce, clock metadata, one SHA-256 PCR selection, and exact
  32-byte PCR digest.
- Bound the parsed nonce and digest to caller-supplied expectations.
- Added executable QEMU self-test coverage for valid evidence and nonce
  mismatch rejection.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: PASS, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 2`: PASS.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 4`: PASS.
- Serial evidence: `audit-artifacts/stage24.29-quote-attestation-20261021/`.

## Diagnostic record and limits

The first executable run exposed a self-test fixture shadowing bug that copied
an 8-byte NV nonce into a 16-byte quote slot and caused a kernel panic. The
fixture was corrected to use an explicitly named quote nonce, and the rerun
passed. No signature or TPM attestation key was verified; the current QEMU
platform has no TPM2 ACPI descriptor and no physical state was changed. The
attached USB was inventory-only and was not written, formatted, or mounted.
---

## Source: docs/audit-stage24-2-wovenidentity-snapshot-2026-10-20.md

# Stage 24.2 audit â€” authenticated WovenIdentity snapshot

Date: 2026-10-20

Implemented a fixed-size authenticated `WID1` snapshot for local identity
policy state. It stores bounded users, groups, administrator roles, profile
display names, salts, and PBKDF2-HMAC-SHA256 digests. Sessions are excluded so
authentication cannot survive reboot. Decoding validates the checksum, version,
counts, identifiers, names, memberships, padding, and duplicate records before
returning a fresh authority.

Validation:

- `cargo test -p libwoven --lib`: 84 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-2-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.2] WovenIdentity snapshot: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.2-1cpu-1792526063075640900`
- `audit-artifacts/stage24.2-2cpu-1792526104893458300`
- `audit-artifacts/stage24.2-4cpu-1792526146509891500`

Limits: this stage does not claim kernel filesystem persistence, atomic
power-loss recovery, key management, credential rotation, lockout/rate
limiting, or task/session credential integration.
---

## Source: docs/audit-stage24-30-quote-signature-shape-2026-10-21.md

# Stage 24.30 audit - bounded TPM quote signature framing

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now decodes bounded `TPMT_SIGNATURE` frames for SHA-256
RSASSA, RSAPSS, and ECDSA schemes. RSA signatures are bounded to 1-512 bytes;
ECDSA `r` and `s` components are bounded to 1-72 bytes. The decoder returns
the parsed shape and exact consumed length, rejecting unsupported schemes,
wrong hashes, malformed lengths, and short frames at the caller boundary.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.30-quote-signature-shape-20261021`.

## Diagnostic record and limits

The first synthetic RSA fixture declared a 256-byte TPM2B payload but used a
buffer one two-byte length field short. The fixture was corrected to the
declared 262-byte frame and the executable QEMU self-test was rerun
successfully. This stage validates wire shape only. It does not implement
RSA/ECC public-key loading, cryptographic verification, quote-key binding,
live TPM execution, recovery, or physical hardware qualification. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-31-rsa-quote-verification-2026-10-21.md

# Stage 24.31 audit - bounded TPM RSA quote verification

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now verifies SHA-256 TPM quote signatures for RSASSA
PKCS#1 v1.5 and RSASSA-PSS. The bounded `QuoteRsaPublicKey` accepts 2048,
3072, and 4096-bit modulus shapes, validates the RSA key through RustCrypto,
and applies TPM's exponent-zero-as-65537 rule. Verification covers the exact
`TPMS_ATTEST` byte slice and returns a dedicated policy-signature failure for
cryptographic mismatch.

The TPM self-test now runs after heap initialization. This is required because
the no-std RSA implementation uses bounded allocation internally; the earlier
placement caused a real allocator failure before the heap existed.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.31-rsa-quote-verification-20261021`.

## Limits

The fixed self-test fixture verifies a valid 2048-bit RSASSA signature and
rejects a one-bit tamper. This stage does not implement ECC/P-256 quote
verification, live TPM public-key retrieval and attestation-key validation,
credential authorization integration, crash recovery, or physical TPM
qualification. The attached USB device was inventory-only and remains
untouched.
---

## Source: docs/audit-stage24-32-ecc-p256-quote-verification-2026-10-21.md

# Stage 24.32 audit - bounded ECC/P-256 quote verification

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now verifies TPM ECDSA/P-256 SHA-256 quote signatures
against an uncompressed SEC1 public key assembled from bounded 32-byte affine
coordinates. TPM `r` and `s` values must each be exactly 32 bytes, matching
the P-256 field width. The verifier validates that the point is on the curve
and returns fail-closed policy-signature errors for cryptographic mismatch.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.32-ecc-p256-quote-verification-20261021`.

## Limits

The deterministic self-test verifies a valid P-256 signature and rejects a
tampered component. This stage does not retrieve an attestation key from a
physical TPM, validate its TPM object attributes/name, bind it to a persistent
credential policy, implement recovery, or qualify physical hardware. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-33-tpm-read-public-boundary-2026-10-21.md

# Stage 24.33 audit - bounded TPM ReadPublic attestation-key boundary

Date: 2026-10-21

## Delivered

`kernel/src/tpm2.rs` now encodes `TPM2_ReadPublic` for a selected object
handle, executes it through the existing serialized CRB transport, and
decodes the exact no-session response. The public-area validator requires
SHA-256 naming, fixed-TPM/fixed-parent/sensitive-origin/restricted/signing
attributes, and rejects decrypt-only objects. RSA public areas are limited to
2048-4096-bit moduli and validated through RustCrypto; ECC public areas are
limited to valid NIST P-256 affine points. Name and qualified-name TPM2Bs are
bounded and trailing bytes are rejected.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.33-tpm-read-public-boundary-20261021`.

## Limits

The QEMU self-test uses a bounded synthetic `ReadPublic` response and does
not assert physical TPM execution. It does not yet bind the returned TPM2B
name to a provisioned attestation-key identity, persist the key, authorize a
credential decision, implement recovery, or qualify physical hardware. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-34-tpm-name-binding-2026-10-21.md

# Stage 24.34 audit - TPM attestation-key Name binding

Date: 2026-10-21

## Delivered

The `TPM2_ReadPublic` decoder now recomputes the TPM Name as the SHA-256 hash
of the exact `TPMTPublic` bytes prefixed by the SHA-256 algorithm identifier,
then requires the returned Name TPM2B to match. The bounded qualified Name is
retained. `bind_attestation_public_name` compares that parsed identity with a
caller-provisioned 34-byte expected Name and returns the key only on an exact
match.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.34-tpm-name-binding-20261021`.

## Limits

The QEMU fixture covers recomputation, returned-Name validation, and expected
Name mismatch rejection using a synthetic public response. This stage does
not provision or persist an expected physical TPM object identity, execute a
live `ReadPublic` against hardware, bind quotes into credential authorization,
implement recovery, or qualify physical hardware. The attached USB device was
inventory-only and remains untouched.
---

## Source: docs/audit-stage24-35-signed-attestation-name-policy-2026-10-21.md

# Stage 24.35 audit - signed TPM attestation-Name policy

Date: 2026-10-21

## Delivered

`verify_signed_attestation_name_policy` validates the fixed-size TAN1 policy
envelope: magic, version, signed-length field, SHA-256 Name algorithm, and a
34-byte TPM Name are checked before the Ed25519 signature is verified against
the caller-provisioned trust root. It returns only the signed Name; a record
found in persistent storage cannot create authority by itself.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.35-signed-attestation-name-policy-20261021`.

## Limits

The QEMU self-test verifies a deterministic signed record and rejects a
tampered signature. This stage does not persist the record in WovenFS or a
TPM NV index, retrieve the live TPM object, bind the verified Name to a
credential authorization transaction, implement recovery, or qualify physical
hardware. The attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-36-durable-attestation-name-policy-2026-10-21.md

# Stage 24.36 audit - durable signed TPM Name policy boundary

Date: 2026-10-21

## Delivered

The TPM2 module now exposes feature-gated durable APIs for the fixed TAN1
policy path `/wfs/tpm-attestation-name-policy`. `persist_attestation_name_policy`
verifies the complete Ed25519-signed envelope before invoking the accepted
journal-backed WovenFS writer. `read_attestation_name_policy` reads into a
caller-owned fixed buffer, rejects any length other than 106 bytes, and
revalidates the signature before returning the Name. Storage errors are
reported separately from policy and signature failures.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.36-durable-attestation-name-policy-20261021`.

## Limits

The QEMU run exercises the signed-policy validation path; lower-stage QEMU
exits before the physical WovenFS device path can exercise a TPM policy
record. No physical TAN1 record is provisioned, and no boot-time loader yet
requires one. Live TPM retrieval, quote authorization, recovery, and physical
hardware qualification remain required. The attached USB device was
inventory-only and remains untouched.
---

## Source: docs/audit-stage24-37-quote-authorization-integration-2026-10-21.md

# Stage 24.37 audit - fail-closed TPM quote authorization integration

Date: 2026-10-21

## Delivered

`authorize_quote` accepts a structured request containing the signed TAN1
policy, trusted policy key, validated `ReadPublic` result, quote bytes,
signature bytes, expected nonce, PCR index, and expected PCR digest. It then
enforces this order:

1. Verify the signed expected TPM Name.
2. Bind the validated public key to that Name.
3. Validate the quote attestation and its nonce/PCR digest.
4. Decode the complete TPMT_SIGNATURE frame.
5. Verify RSA RSASSA/RSAPSS or ECDSA P-256 cryptographically according to the
   bound key type.

No key supplied independently of the validated public-area result can create
authority, and an identity mismatch fails before signature processing.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.37-quote-authorization-integration-20261021`.

## Limits

The QEMU self-test exercises the fail-closed policy/identity ordering; it does
not perform a live physical TPM quote or make a credential decision during
boot. Boot-time policy loading, live TPM transport qualification, recovery,
and physical hardware testing remain required. The attached USB device was
inventory-only and remains untouched.
---

## Source: docs/audit-stage24-38-boot-tpm-policy-loader-2026-10-21.md

# Stage 24.38 audit - boot-time TPM Name policy loader

Date: 2026-10-21

## Delivered

The release `system-apps` boot path now invokes
`tpm2::load_attestation_name_policy_at_boot` after the durable filesystem
mount attempt and before WovenShell startup. The loader reads the exact
bounded TAN1 record from WovenFS and verifies it with
`signed_app::TRUSTED_PUBLIC_KEY`, the compile-time application release trust
root. It reports a validated Name only on success.

The verified envelope is retained in an allocation-free kernel-owned slot for
later quote authorization. The slot is cleared on every load failure, so a
previously accepted policy cannot survive a failed replacement attempt.

Storage absence is reported as unavailable hardware credentials. Malformed
records, invalid policy metadata, and signature failures are rejected and
also leave hardware credentials disabled. The desktop boot remains available
when no physical TPM policy has been provisioned.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same acceptance with 2 CPUs: passed.
- Same acceptance with 4 CPUs: passed.

The 1/2/4-CPU serial logs include the expected
`hardware credentials disabled` marker for the unprovisioned QEMU image.
Serial evidence is preserved in
`audit-artifacts/stage24.38-boot-tpm-policy-loader-20261021`.

## Limits

QEMU has no live physical TPM and therefore exercises the fail-closed missing
policy path, not successful physical provisioning. A production image still
requires authenticated policy provisioning, live TPM quote execution,
credential decision integration, recovery testing, and physical hardware
qualification. The attached USB device was inventory-only and remains
untouched.
---

## Source: docs/audit-stage24-39-tpm-quote-execution-2026-10-21.md

# Stage 24.39 audit - bounded TPM2 Quote execution

Date: 2026-10-21

## Delivered

The TPM2 CRB layer now exposes `tpm2::quote`, which builds a bounded
sessions-tagged `TPM2_Quote` request for one signing handle, a qualifying
nonce of at most 32 bytes, one SHA-256 PCR, and an explicit RSA-SSA,
RSA-PSS, or ECDSA scheme. The command uses an empty password session only for
wire authorization framing; it does not claim to authorize a non-empty object
policy.

The response parser requires the sessions response tag, exact response size,
successful TPM return code, bounded `TPMS_ATTEST` and `TPMT_SIGNATURE`
parameters, supported signature algorithms, and an exact empty password
authorization response. Quote and signature bytes are returned in fixed-size
kernel-owned buffers. They remain evidence and cannot create authority until
the existing `authorize_quote` path verifies the signed Name, ReadPublic
binding, nonce/PCR digest, and cryptographic signature.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The executable QEMU self-test covers command sizing, handle/session fields,
nonce, scheme, PCR selection count/bitmap, and bounded synthetic response
decoding. Serial evidence is preserved in
`audit-artifacts/stage24.39-tpm-quote-execution-20261021`.

## Limits

The QEMU environment has no physical TPM, so it cannot prove a successful
live quote. The current API intentionally supports only an empty password
session; production attestation keys requiring non-empty authorization need a
subsequent policy/session-authenticated quote path. Credential decision
integration, recovery, and physical TPM qualification remain open. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-3-wovenidentity-storage-2026-10-20.md

# Stage 24.3 audit â€” kernel-authorized WovenIdentity storage

Date: 2026-10-20

Implemented `kernel/src/identity_storage.rs`, a StorageIo-authorized adapter
that persists authenticated `WID1` policy snapshots inside two native WovenFS
`WID2` records. Each record carries a non-zero generation and wrapper checksum;
recovery selects the newest valid copy and validates the inner identity
snapshot. The adapter never writes sessions.

Validation:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-3-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.3] WovenIdentity durable storage: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.3-1cpu-1792526461019999200`
- `audit-artifacts/stage24.3-2cpu-1792526482958590800`
- `audit-artifacts/stage24.3-4cpu-1792526505203409200`

The failed pre-fix run is retained at
`audit-artifacts/stage24.3-1cpu-1792526354775283900` and showed the harness
had no IDE disk. The launcher now creates an explicit disposable 16 MiB image.

Limits: this stage does not claim user-facing persistence, key management,
credential rotation, lockout/rate limiting, or kernel task/session binding.
---

## Source: docs/audit-stage24-40-policy-session-quote-2026-10-21.md

# Stage 24.40 audit - policy-session TPM2 Quote framing

Date: 2026-10-21

## Delivered

`QuoteSessionAuth` extends the bounded quote path with an explicit
non-password session handle, caller nonce, session attributes, and
authorization HMAC. `quote_with_policy_session` rejects the password-session
handle and passes the caller-supplied authorization material unchanged into
the TPM sessions-tagged command. The encoder bounds nonce and HMAC sizes and
places the authorization area before quote parameters as required by the TPM
command layout.

The API does not fabricate an HMAC or claim to create a policy session. The
existing `authorize_quote` path remains mandatory after the TPM returns quote
evidence.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The executable QEMU self-test covers empty-password and policy-session quote
command offsets, session sizes, caller nonce, attributes, HMAC, PCR
selection, and downgrade rejection. Serial evidence is preserved in
`audit-artifacts/stage24.40-policy-session-quote-20261021`.

## Limits

This stage does not yet start the policy session, execute `PolicyPCR`, or
derive the authorization HMAC from provisioned TPM/session material. It also
does not perform a live physical TPM quote in this environment. Credential
decision integration, recovery, and physical qualification remain open. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-41-live-policy-session-2026-10-21.md

# Stage 24.41 audit - live policy-session transcript execution

Date: 2026-10-21

## Delivered

`start_policy_session` now executes the bounded `TPM2_StartAuthSession`
command through the serialized CRB transport and validates the successful
response, non-zero session handle, bounded TPM nonce, and exact response
length. `apply_policy_pcr` executes `TPM2_PolicyPCR` for one SHA-256 PCR and
expected digest, then accepts only the exact successful response framing.

Both operations are state-changing TPM transcript steps, not authorization
decisions. The session handle and initial TPM nonce are returned to the
caller; no caller-supplied packet is promoted to credential authority.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers the existing bounded StartAuthSession and PolicyPCR
wire fixtures; serial evidence is preserved in
`audit-artifacts/stage24.41-live-policy-session-20261021`.

## Limits

`PolicyPCR` changes TPM session state, but this boundary does not yet expose
the refreshed TPM nonce after that transition or derive the quote command's
authorization HMAC. A physical TPM is not present in QEMU. Credential
decision integration, recovery, and physical qualification remain open. The
attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-42-policy-session-hmac-2026-10-21.md

# Stage 24.42 audit - policy-session quote HMAC derivation

Date: 2026-10-21

## Delivered

`derive_quote_session_hmac` constructs the exact SHA-256 cpHash input for
`TPM2_Quote` (command code, signing handle, qualifying data, signature
scheme, and PCR selection), then computes the authorization HMAC over cpHash,
the returned TPM session nonce, caller nonce, and session attributes using the
caller-supplied attestation-key auth value. The bounded wrapper
`quote_with_derived_policy_hmac` passes the resulting 32-byte HMAC through
the non-password quote path. The password-session handle is rejected there.

No key material is read from a TPM response, and no fallback HMAC is
fabricated. The existing quote identity, nonce/PCR, and cryptographic
verification boundary remains mandatory.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers deterministic HMAC derivation and detects an
auth-value change; serial evidence is preserved in
`audit-artifacts/stage24.42-policy-session-hmac-20261021`.

## Limits

The derivation currently supports only the null-salt/null-bind session shape
created by this kernel. It does not yet implement provisioned key lifecycle,
session-state refresh after every TPM transition, credential decision
integration, recovery, or physical TPM qualification. The attached USB
device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-43-managed-policy-session-2026-10-21.md

# Stage 24.43 audit - managed TPM policy-session lifecycle

Date: 2026-10-21

## Delivered

`PolicySessionLease` gives each started policy session a monotonic generation,
active state, and nonce-stale state. Managed PolicyPCR and quote operations
reject flushed or stale leases. A successful managed quote marks the lease
stale so it cannot silently reuse a nonce for another authority decision.
`flush_policy_session` executes bounded `TPM2_FlushContext`; only an exact
successful response invalidates the lease, while a failed cleanup remains
retryable.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers exact FlushContext framing and invalid-handle
rejection. Serial evidence is preserved in
`audit-artifacts/stage24.43-managed-policy-session-20261021`.

## Limits

This lease does not yet implement TPM context save/load across suspend or
reboot, nor does it expose refreshed session nonce state after every
authenticated command. Provisioned-key lifecycle, credential decisions,
recovery, and physical TPM qualification remain open. The attached USB
device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-44-policy-session-context-2026-10-21.md

# Stage 24.44 audit - TPM policy-session context save/load

Date: 2026-10-21

## Delivered

The policy-session lifecycle now includes bounded `TPM2_ContextSave` and
`TPM2_ContextLoad` framing. Context responses require the no-sessions tag,
exact response size, successful TPM code, non-zero saved handle, and a
non-empty context blob no larger than the fixed kernel bound. Context loads
validate the exact 14-byte handle response.

Successful save invalidates the active lease and returns a snapshot containing
the validated context and session metadata. A snapshot is consumed only after
a successful load, which returns a fresh generation; failed loads remain
available for retry.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers exact context save/load framing, blob bounds, and
restored-handle decoding. Serial evidence is preserved in
`audit-artifacts/stage24.44-policy-session-context-20261021`.

## Limits

This stage does not establish secure cross-reboot context custody or refresh
session nonce state after every TPM transition. Provisioned-key lifecycle,
credential decisions, recovery, and physical TPM qualification remain open.
The attached USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-45-tpm-context-custody-2026-10-21.md

# Stage 24.45 audit - durable TPM context custody boundary

Date: 2026-10-21

## Delivered

Opaque TPM-produced policy-session context blobs now have a fixed-size `TPS1`
WovenFS record containing version, commit sequence, context metadata, session
handle/nonce metadata, and a SHA-256 integrity digest. Persistence uses two
journal-backed paths and advances one copy per commit. Recovery validates both
copies, selects the unique newest valid sequence, and rejects ties or an
entirely invalid set.

This is custody only. The recovered snapshot is still passed to the bounded
TPM `ContextLoad` path, whose TPM response is required before a new managed
lease exists. A filesystem record cannot itself create session authority.

## Verification

- `cargo test -p libwoven`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features qemu-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- Same system-apps hardware acceptance with 1, 2, and 4 CPUs: passed.

The QEMU self-test covers record encoding, metadata bounds, sequence field,
and tamper-digest rejection. Serial evidence is preserved in
`audit-artifacts/stage24.45-tpm-context-custody-20261021`.

## Limits

This stage does not yet wire custody recovery into boot, establish a separate
secure-storage authorization policy, or prove cross-reboot behavior on a
physical TPM. Session nonce refresh, provisioned-key lifecycle, credential
decisions, recovery, and physical qualification remain open. The attached
USB device was inventory-only and remains untouched.
---

## Source: docs/audit-stage24-46-boot-tpm-context-recovery-2026-10-21.md

# Stage 24.46 audit - boot-time TPM context recovery

Date: 2026-10-21

## Delivered

System-apps boot now retains the discovered TPM2 handle across ACPI reporting.
After WovenFS mount and signed TAN1 policy loading, it loads the newest
validated `TPS1` custody candidate, requires TPM2 `ContextLoad`, and flushes
the restored policy-session lease immediately. The current authority consumer
is intentionally absent, so successful recovery is still reported as
hardware-credentials-disabled. Every absence, storage error, malformed record,
TPM rejection, and cleanup failure remains fail-closed.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

Serial evidence is preserved in
`audit-artifacts/stage24.46-boot-tpm-context-recovery-20261021`.

## Limits

QEMU has no durable TPM context, so a successful physical cross-reboot
`ContextLoad` is not proven. Secure storage authorization, session nonce
refresh, provisioned-key lifecycle, credential decisions, recovery policy, and
physical qualification remain open. The attached USB device was not written
or otherwise modified.
---

## Source: docs/audit-stage24-47-tpm-session-nonce-refresh-2026-10-21.md

# Stage 24.47 audit - TPM policy-session nonce refresh

Date: 2026-10-21

## Delivered

The TPM quote response decoder now validates the complete authorization area
and preserves the bounded response TPM nonce in `QuoteExecution`. Password
session responses retain their valid empty nonce behavior. Policy-session quote
execution rejects an empty refreshed nonce. The managed lease copies the new
nonce before marking the quote one-shot stale, preventing a later lifecycle
operation from treating the previous nonce as current authorization state.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers both empty password-session response handling
and non-empty policy-session nonce extraction. Serial evidence is preserved in
`audit-artifacts/stage24.47-tpm-session-nonce-refresh-20261021`.

## Limits

QEMU does not provide a live TPM session, so physical response-nonce behavior
and cross-reboot custody are not proven. Durable custody authorization,
provisioned-key lifecycle, credential decisions, recovery policy, and physical
qualification remain open. The attached USB device was not modified.
---

## Source: docs/audit-stage24-48-tpm-custody-authorization-2026-10-21.md

# Stage 24.48 audit - TPM custody authorization boundary

Date: 2026-10-21

## Delivered

TPM policy-session custody now has a kernel-owned `PolicySessionCustodyAuthority`
token with private construction and a monotonic generation. Issuance requires
a validated aligned CRB descriptor and, in system-apps release boots, a
successfully retained signed TAN1 policy. Both `persist_policy_session_snapshot`
and `load_policy_session_snapshot` require the token in addition to the native
WovenFS writer/read boundary. A storage-only caller cannot create or recover
TPM credential-session custody.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers aligned-descriptor issuance and invalid
descriptor rejection. Serial evidence is preserved in
`audit-artifacts/stage24.48-tpm-custody-authorization-20261021`.

## Limits

QEMU has no live TPM and therefore cannot prove physical policy loading,
cross-reboot context restore, or TPM-backed credential use. Provisioned-key
lifecycle, credential decisions, recovery policy, and physical qualification
remain open. The attached USB device was not modified.
---

## Source: docs/audit-stage24-49-credential-root-lifecycle-2026-10-21.md

# Stage 24.49 audit - credential-root lifecycle

Date: 2026-10-21

## Delivered

`CredentialRootLease` is a kernel-owned, non-copyable lifecycle object for a
32-byte root that has already passed TPM NV metadata and exact readback
validation. Installation rejects zero roots and assigns a monotonic
generation. Rotation validates the replacement first, rejects same-root
replays, wipes the old root, and advances the generation. Derivation exposes
only a domain-separated HMAC result; the root is never returned. Explicit
revocation and `Drop` wipe the root and prevent further derivation.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU TPM self-test covers zero-root refusal, installation, derivation,
generation-changing rotation, and post-revocation refusal. Serial evidence is
preserved in `audit-artifacts/stage24.49-credential-root-lifecycle-20261021`.

## Limits

QEMU does not provide a live TPM, so NV define/write/read execution and
physical provisioned-root persistence are not proven. Credential decision
integration, reboot recovery policy, and physical qualification remain open.
The attached USB device was not modified.
---

## Source: docs/audit-stage24-4-wovenidentity-kernel-seam-2026-10-20.md

# Stage 24.4 audit â€” owner-bound kernel identity seam

Date: 2026-10-20

Implemented the kernel identity runtime seam. It binds each identity session
to an owning process ID in a fixed table, validates ownership for lock,
unlock, elevation, and close, and applies credentials only through a crate-only
task API after releasing the identity lock. If no live process can receive the
credentials, the session and binding are cleaned up and the operation fails.

Validation:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-4-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.4] WovenIdentity task binding: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.4-1cpu-1792526710240384400`
- `audit-artifacts/stage24.4-2cpu-1792526744522828500`
- `audit-artifacts/stage24.4-4cpu-1792526778145524000`

Limits: userspace syscall framing, durable runtime loading, process-exit
binding reclamation, audit actions, lockout/rate limiting, and WovenShell
integration remain unimplemented.
---

## Source: docs/audit-stage24-50-hardware-credential-decision-2026-10-21.md

# Stage 24.50 audit - hardware credential decision boundary

Date: 2026-10-21

## Delivered

`authorize_hardware_credential` binds a bounded identity context to the active
credential-root generation by deriving the expected TPM quote nonce. It then
requires the complete existing `authorize_quote` chain: signed TAN1 policy,
attestation-key Name binding, expected nonce/PCR evidence, and RSA/ECC
signature verification. Successful output is a generation-tagged
`CredentialDecision` containing attestation evidence only. Inactive or revoked
roots are rejected before quote authorization.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-memory-qemu.py --cpus 1 --timeout 180`: passed, exit 33.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- Same system-apps hardware acceptance with 2 and 4 CPUs: passed.

The QEMU self-test covers decision rejection for mismatched attestation data
and for a revoked root. Serial evidence is preserved in
`audit-artifacts/stage24.50-hardware-credential-decision-20261021`.

## Limits

QEMU has no live TPM, so a successful physical hardware credential decision is
not proven. The identity session/login path does not yet consume
`CredentialDecision`, and reboot recovery and physical qualification remain
open. The attached USB device was not modified.
---

## Source: docs/audit-stage24-51-identity-hardware-consumption-2026-10-21.md

# Stage 24.51 audit - identity hardware-decision consumption

Date: 2026-10-21

## Delivered

- Added a move-only `CredentialDecision` handoff from the TPM boundary into a
  bounded one-entry kernel pending slot.
- Added syscall 233, `IdentityConsumeHardware`, which consumes a pending
  decision only for the exact owner and session.
- Bound consumption to the active account name context used to derive the TPM
  challenge, the credential-root generation, and a one-shot hardware binding.
- Rejected absent, locked, mismatched-owner/session, mismatched-context, and
  duplicate consumption paths fail-closed; `IdentityHardware` records the
  result.
- Added the broker-facing `authorize_and_publish_hardware_credential` entry
  point. TPM quote material remains kernel-only and is not copied to
  userspace.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed.
- Memory QEMU, 1 CPU: passed, exit 33.
- Signed system-app hardware demo, 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.51-identity-hardware-consumption-20261021`.

## Limits and next prerequisites

No live physical TPM broker has published a decision yet, so this stage does
not claim physical credential login, reboot recovery, or release readiness.
Physical TPM qualification and real hardware testing remain required. The
attached USB device was not modified or used as a write target.
---

## Source: docs/audit-stage24-52-physical-tpm-broker-2026-10-21.md

# Stage 24.52 audit - physical TPM credential broker boundary

Date: 2026-10-21

## Delivered

- Added `HardwareCredentialBroker`, a kernel-owned configuration and execution
  boundary for physical TPM credential requests.
- Validated the CRB descriptor, active credential-root lease, signed TAN1
  attestation Name policy, ReadPublic key binding, PCR selection, signing
  handle, and bounded signing authorization value before installation.
- Added a one-request path that derives the exact account challenge, starts a
  fresh managed policy session, applies `PolicyPCR`, executes authenticated
  `TPM2_Quote`, verifies the quote, flushes the session on every success or
  failure path, and publishes only a move-only `CredentialDecision`.
- Added syscall 234 and libwoven request/consume wrappers. The account context
  is taken from the kernel owner/session binding; userspace cannot provide a
  replacement account name or receive TPM evidence.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with the signed-app
  build inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in, ten
  applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.52-physical-tpm-broker-20261021`.

## Limits and next prerequisites

The current QEMU firmware exposes no TPM2 ACPI device, so no broker is
installed and no physical credential login is claimed. Release provisioning
must supply the validated NV root, PCR policy, signed attestation Name policy,
attestation handle/public key, and signing authorization value. Cross-reboot
recovery, physical TPM testing, and hardware qualification remain open. The
attached USB device was not modified or used as a write target.
---

## Source: docs/audit-stage24-53-signed-tpm-provisioning-2026-10-21.md

# Stage 24.53 audit - signed TPM broker provisioning record

Date: 2026-10-21

## Delivered

- Added the fixed-size signed `TCB1` broker-policy envelope.
- Authenticated and bounded the reserved credential-root NV index,
  attestation signing handle, quote scheme, PCR selection, expected PCR
  digest, attestation-object Name, and NV policy.
- Added WovenFS durable persistence and boot-time read/revalidation APIs.
- Kept authorization secrets out of the durable record; a valid file cannot
  by itself install a broker, create a root, or enable hardware login.
- Added QEMU self-test coverage for malformed-record rejection.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with signed-app inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.53-signed-tpm-provisioning-20261021`.

## Limits and next prerequisites

The current QEMU firmware exposes no TPM2 device and has no provisioning
record. Physical TPM provisioning and secure authorization-secret injection
remain required before installing the broker. Live credential login,
cross-reboot recovery, and hardware qualification remain open. The attached
USB device was not modified or used as a write target.
---

## Source: docs/audit-stage24-54-kernel-derived-tpm-secret-2026-10-21.md

# Stage 24.54 audit - kernel-derived TPM authorization secret

Date: 2026-10-21

## Delivered

- Extended `CredentialRootLease` with its validated NV index and policy
  binding; rotations now reject metadata changes.
- Added kernel-only attestation authorization derivation using a
  domain-separated credential-root derivation.
- Added provisioned broker installation checks for the signed `TCB1` policy,
  exact attestation Name, and matching root metadata.
- Wiped the derived authorization value before installation returns. No raw
  authorization secret is accepted from userspace or stored in WovenFS.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel freestanding Clippy for `stage24-5-test`: passed.
- Kernel freestanding Clippy for `system-apps`: passed with signed-app inputs.
- Memory QEMU 1 CPU: passed, exit 33.
- Signed system-app hardware demo 1/2/4 CPUs: passed, including sign-in,
  ten applications, desktop, PS/2, IOAPIC, and smptest.
- Serial evidence is preserved under
  `audit-artifacts/stage24.54-kernel-derived-tpm-secret-20261021`.

## Limits and next prerequisites

No physical TPM is available in the QEMU image, so the provisioning install
path has not been exercised against a live device. Physical TPM provisioning,
live credential login, cross-reboot recovery, and hardware qualification
remain open. The attached USB device was not modified or used as a write
target.
---

## Source: docs/audit-stage24-55-physical-tpm-readiness-2026-10-21.md

# Stage 24.55 audit - physical TPM readiness preflight

Date: 2026-10-21

## Delivered

- Added `scripts/test-physical-tpm-readiness.ps1`.
- The preflight records host TPM visibility/readiness, TPM query errors,
  QEMU/swtpm availability, signed broker-record presence, and disk identity.
- It writes only a JSON/text report under `audit-artifacts`; it never opens a
  raw disk, dismounts a volume, changes disk state, provisions a TPM, or writes
  media.

## Observed result

- TPM query: not available to the current non-administrator context (`Access
  denied`). TPM readiness is therefore unproven.
- QEMU: present; `swtpm`: absent.
- Signed broker provisioning record: absent from the release-validation tree.
- USB candidate: Disk 1, `VendorCo ProductCode USB Device`,
  15,726,735,360 bytes, serial `4045141272666606378`, not boot/system.
- Safety report: raw disk opened `false`, disk state changed `false`, media
  written `false`, TPM provisioned `false`.

## Limits and next prerequisites

This report is not physical hardware qualification. An administrator TPM
inventory, a provisioned physical TPM, signed broker inputs, live TPM quote
execution, reboot recovery, and device-matrix qualification remain required.
The attached USB device was not modified.
---

## Source: docs/audit-stage24-56-tpm-broker-record-generator-2026-10-21.md

# Stage 24.56 audit - signed TPM broker record generator

Date: 2026-10-21

## Delivered

- Added `tools/tpm-broker-config`, a standalone Rust release tool.
- It validates the reserved NV index, non-zero signing handle, PCR range,
  supported quote scheme, SHA-256 attestation Name, non-zero digest, and
  non-zero NV policy.
- It requires `WOVENHAT_TPM_POLICY_SIGNING_SEED_HEX` and
  `WOVENHAT_TPM_POLICY_TRUSTED_PUBLIC_KEY_HEX`, checks that they match, and
  emits exactly 182 bytes (`TCB1` signed bytes plus Ed25519 signature).
- It has no development-key fallback, TPM access, raw-disk access, or media
  write path.

## Verification

- `cargo clippy --manifest-path tools/tpm-broker-config/Cargo.toml -- -D warnings`:
  passed.
- Synthetic positive generation: passed, 182-byte output.
- Missing signing-key negative control: rejected with exit 2.
- Invalid NV-index negative control: rejected with exit 2.
- Exact-tree memory QEMU 1 CPU: passed, exit 33.
- Exact-tree signed system-app hardware demo 1/2/4 CPUs: passed, including
  sign-in, ten applications, desktop, PS/2, IOAPIC, and smptest.
- The existing Stage 24.55 read-only preflight remained unchanged; no TPM or
  USB state was modified.
- Evidence is preserved under
  `audit-artifacts/stage24.56-tpm-broker-record-generator-20261021`.

## Limits and next prerequisites

The generated record used synthetic policy values and the repository test
signing key; it is not a production credential artifact. A release operator
must use the dedicated production policy key, then validate the record against
the physical TPM's actual attestation Name/PCR/NV state. Physical provisioning,
live quote execution, reboot recovery, and hardware qualification remain open.
---

## Source: docs/audit-stage24-57-tpm-credential-root-read-2026-10-21.md

# Stage 24.57 audit â€” physical TPM credential-root read boundary

Date: 2026-10-21
Owner: Codex
Repository: `phase2/userspace-platform`

## Delivered

- Added bounded `NV_ReadPublic` access for the configured credential-root
  index.
- Added authenticated policy-session `NV_Read` framing and cpHash/HMAC
  derivation for a single bounded 32-byte read at offset zero.
- Added the kernel promotion path: validate signed NV metadata, start a
  managed policy session, apply the configured PCR policy, read the root,
  refresh the TPM nonce, flush the session on every path, and wipe transient
  root/authentication state.
- Kept the root and TPM authorization material out of userspace and durable
  storage.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Kernel Clippy passed with `system-apps` and `stage24-5-test` feature gates.
- Exact-tree memory acceptance passed on 1 CPU.
- Exact-tree signed system-app acceptance passed on 1, 2, and 4 CPUs,
  including desktop readiness, PS/2/IOAPIC, SMP migration, and acknowledged
  TLB-shootdown checks.
- Evidence copies are in
  `audit-artifacts/stage24.57-tpm-credential-root-read-20261021`.

## Limits and next gate

The test machine exposes no accessible TPM and has no `swtpm` backend. QEMU
therefore exercises the fail-closed no-TPM path, not a live NV read or PCR
policy execution. Physical TPM provisioning, live read/quote qualification,
reboot persistence, and hardware validation remain mandatory before any
production-release claim. The attached USB was not opened, written, or
modified.
---

## Source: docs/audit-stage24-58-tpm-broker-boot-activation-2026-10-21.md

# Stage 24.58 audit â€” physical TPM broker boot activation

Date: 2026-10-21
Owner: Codex
Repository: `phase2/userspace-platform`

## Delivered

- Added the system-app boot activation path for the signed TPM broker.
- Revalidated the signed broker configuration and attestation Name policy
  after durable storage mount.
- Read and Name-validated the configured attestation public area directly from
  the TPM, generated a fresh TPM caller nonce, and invoked the bounded
  PCR-policy credential-root read from Stage 24.57.
- Installed the kernel-owned broker only after all checks succeed; transient
  nonce and policy material are wiped, and all errors leave the broker absent.
- Preserved a usable desktop when no TPM or signed records are available.

## Verification

- `cargo test -p libwoven --quiet`: 89 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- Exact-tree memory acceptance passed on 1 CPU.
- Exact-tree signed system-app acceptance passed on 1, 2, and 4 CPUs.
- All QEMU boots logged the expected fail-closed marker:
  `[TPM] broker activation unavailable; no live TPM; hardware credentials disabled`.
- Evidence copies are in
  `audit-artifacts/stage24.58-tpm-broker-boot-activation-20261021`.

## Limits and next gate

The host and QEMU environment expose no live TPM and no `swtpm` backend, so
the successful activation branch, physical credential login, reboot recovery,
and hardware qualification remain unverified. The attached USB was not
opened, written, or modified.
---

## Source: docs/audit-stage24-59-change-pin-2026-10-21.md

# Stage 24.59 Change-PIN acceptance audit â€” 2026-10-21

## Delivered

- Added a deterministic kernel/QEMU self-test for the real WovenShell Change-
  PIN UI path.
- Covered sign-in, Quick Settings, current/new/confirmation PIN entry,
  durable record rotation, relock, old-PIN rejection, and new-PIN unlock.
- Synchronised the kernel identity authority and `/etc/wovenhat/accounts` with
  one bounded kernel-generated salt. The existing rollback path restores the
  authority if the account-record rewrite fails.

## Verification

- Production-feature kernel Clippy with packaged signed system-app images:
  passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p libwoven --quiet`: 95 passed.
- Stage 24.59 QEMU acceptance on 1, 2, and 4 CPUs: passed.

The serial and QEMU logs are preserved in
`audit-artifacts/stage24.59-change-pin-20261021`.

## Limits and next prerequisites

This stage closes the password-change gap. Account creation was previously
accepted in Stage 24.15. Physical TPM activation, real-device reboot recovery,
and hardware/USB qualification remain open release gates; this work performed
no USB writes.
---

## Source: docs/audit-stage24-59-low-memory-2026-10-22.md

# Stage 24.59 low-memory regression audit â€” 2026-10-22

The 256 MiB Change-PIN regression fixture initially failed before the desktop
because the bootstrap heap mapped 2,473,984 bytes while the compositor needed
a 4,096,000-byte allocation. The production 4 GiB floor remains unchanged;
this fixture is retained as a low-memory regression profile.

The heap bootstrap now reserves its bounded 8 MiB ceiling whenever the
available frame count can afford that reservation, while retaining the
proportional 256 KiBâ€“8 MiB policy below that threshold. This keeps the heavy
desktop fixture viable without making the heap unbounded or changing runtime
growth limits.

Verification:

- `cargo test -p wovenhat-kernel`: passed.
- `rustfmt --edition 2021 --check kernel/src/heap.rs`: passed.
- Stage 24.59 QEMU, 1/2/4 CPU, 256 MiB fixture: passed with exit 33.
- The gate passed kernel/user-space boot, Ada sign-in, Quick Settings,
  current/new/confirmation PIN entry, durable rotation, old-PIN rejection,
  and new-PIN unlock.

Fresh evidence is retained in
`target/woven-shell-change-pin-{1,2,4}cpu`.

The shell's new 3D renderer remains opt-in until its full pixel-exact
acceptance matrix is re-derived; the accepted Change-PIN path runs with the
flat default.
---

## Source: docs/audit-stage24-5-wovenidentity-syscall-abi-2026-10-20.md

# Stage 24.5 audit â€” WovenIdentity syscall ABI

Date: 2026-10-20

Added additive syscalls 219â€“223 for identity session open, lock, unlock,
elevation, and close. Kernel handlers copy bounded user buffers before parsing;
length-prefixed name/password frames reject truncation, empty fields, and
oversized values. Safe `libwoven::identity` wrappers use bounded stack-owned
request buffers and preserve the existing error sentinel ABI.

Validation:

- `cargo test -p libwoven --lib`: 84 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.5] WovenIdentity syscall ABI: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.5-1cpu-1792526996850760700`
- `audit-artifacts/stage24.5-2cpu-1792527013189914600`
- `audit-artifacts/stage24.5-4cpu-1792527027362429900`

Limits: this gate does not claim durable runtime loading, a live Ring-3 login
fixture, process-exit binding reclamation, audit actions, lockout/rate limiting,
or WovenShell migration.
---

## Source: docs/audit-stage24-60-release-key-provisioning-2026-10-21.md

# Stage 24.60 release-key provisioning boundary â€” 2026-10-21

## Scope

Production WAPP packaging now fails closed unless both
`WOVENHAT_APP_SIGNING_SEED_HEX` and `WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX` are
explicitly supplied. The packager still permits the deterministic key only
for test/development packaging when production mode is not requested. The
production catalog and package-service launchers propagate an explicit release
mode, and release packaging requests that mode when it has to rebuild the
catalog.

The signing seed is read only from the process environment and is not written
to catalogs, manifests, logs, or output artifacts. The packager derives the
public key and rejects a mismatched trusted public key.

## Verification

- `scripts/test-release-key-provisioning.py`: passed; missing-seed and
  mismatched-public-key production invocations rejected, matching explicit-key
  invocation succeeded.
- `cargo fmt --manifest-path tools/wapp-packager/Cargo.toml`: passed.
- `cargo clippy --manifest-path tools/wapp-packager/Cargo.toml -- -D warnings`:
  passed.
- `python scripts/package-system-apps.py --production`: passed with explicit
  test credentials used only as a deterministic acceptance fixture; ten WAPPs
  were generated.
- `python scripts/build-package-service.py --production`: passed with the
  same explicit acceptance fixture.
- `package-release.py` now always regenerates the catalog in production mode,
  preventing a stale test-key catalog from being copied into a release.
- `git diff --check`: passed.

## Limits

The acceptance fixture is not a production secret. Release operators must
inject a separately controlled signing seed and matching public key. Hardware
TPM/USB qualification and the repository-wide pre-existing formatting debt
remain separate release gates.
---

## Source: docs/audit-stage24-6-live-identity-2026-10-20.md

# Stage 24.6 â€” live WovenIdentity runtime and WovenShell login

Date: 2026-10-20

## Scope

This increment imports the bounded salted account file into the kernel
identity authority before WovenShell starts, binds account-backed sign-in and
lock/unlock to owner-bound kernel sessions, applies the authenticated UID/GID
to the shell process, and releases all identity bindings when the owner exits.
The trusted WovenShell login manager verifies the PIN once, then uses the
capability-gated digest handoff; the kernel compares that digest against its
own authority record before creating or unlocking a session. The ordinary
password syscall path remains available for non-login-manager callers.

## Acceptance

- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- `python scripts/test-shell-qemu.py --system-apps --hardware-demo --cpus 1`: passed.
- The same live production shell gate passed on 2 and 4 CPUs.

Final evidence is preserved in:

- `audit-artifacts/stage24.6-live-1cpu-1792530000000000000`
- `audit-artifacts/stage24.6-live-2cpu-1792530000000000000`
- `audit-artifacts/stage24.6-live-4cpu-1792530000000000000`

The serial logs prove account provisioning, kernel credential loading, shell
desktop readiness, `ada` sign-in, diagnostic-console handoff, and the SMP
input/migration checks. The QMP harness now waits for the shell's actual
desktop-ready marker before injecting login input.

## Limits and next prerequisites

This does not yet load the authenticated WID2 snapshot into the live runtime,
implement account administration or PIN rotation, add kernel lockout/rate
limiting, emit identity audit actions, or provide hardware-backed credential
storage. The digest handoff is restricted to the trusted system session's
`AccountManage` capability and must remain audited as the login-manager trust
boundary. Physical USB boot and power-loss qualification remain separate
release gates.
---

## Source: docs/audit-stage24-7-identity-audit-2026-10-20.md

# Stage 24.7 â€” WovenIdentity audit actions

Date: 2026-10-20

## Scope

Identity session operations now enter the kernel security ledger with stable
action identifiers. Open, lock, unlock, elevation, and close each record the
calling task, session target where applicable, and allowed/denied outcome.
The public `libwoven::audit_read` decoder exposes these actions as IDs 26â€“30,
preserving the existing append-only ledger ABI.

## Acceptance

- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

The first 4-CPU attempt exposed an existing timing-sensitive migration-stress
slot exhaustion panic; its serial and QEMU logs remain in the normal
`target/shell-regression-4cpu` output. A clean rerun passed, and the final
acceptance logs are preserved in:

- `audit-artifacts/stage24.7-identity-audit-1cpu-1792531200000000000`
- `audit-artifacts/stage24.7-identity-audit-2cpu-1792531200000000000`
- `audit-artifacts/stage24.7-identity-audit-4cpu-1792531200000000000`

## Limits and next prerequisites

This slice audits valid identity syscall outcomes. User-buffer copy/parse
failures return before an identity operation is formed and are not yet
represented as separate malformed-request events. The identity subsystem still
needs durable WID2 runtime recovery, administration and PIN rotation, kernel
lockout/rate limiting, hardware-backed credentials, and physical USB boot and
power-loss qualification before a release claim is justified.
---

## Source: docs/audit-stage24-8-wid2-runtime-2026-10-20.md

# Stage 24.8 â€” WID2 runtime recovery

Date: 2026-10-20

## Scope

The system-apps identity startup now attempts to recover the newest valid
authenticated WID2 dual-copy snapshot directly from native WovenFS before
starting WovenShell. Snapshot decoding is bounded and revalidates the
libwoven authority structure; if no valid durable copy is available, boot
falls back to the existing bounded `/etc/wovenhat/accounts` bootstrap. Durable
storage remains read-only from this startup path and session state is never
loaded from disk.

## Acceptance

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-3-test -- -D warnings`: passed.
- `cargo test -p libwoven`: 85 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with the signed system-app image environment.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

The QEMU image has no ATA WovenFS disk, so the live logs prove the bounded
account-file fallback and fail-closed no-durable-copy behavior. The existing
Stage 24.3 durable WID2 self-test remains the native dual-copy write/recovery
proof. Evidence is preserved in:

- `audit-artifacts/stage24.8-wid2-runtime-1cpu-1792531800000000000`
- `audit-artifacts/stage24.8-wid2-runtime-2cpu-1792531800000000000`
- `audit-artifacts/stage24.8-wid2-runtime-4cpu-1792531800000000000`

The 4-CPU acceptance needed one rerun after the known migration-stress slot
capacity flake; the final rerun passed and the failed diagnostic remains in
the ordinary `target/shell-regression-4cpu` output.

## Limits and next prerequisites

There is not yet a hardware-backed credential root, account administration or
PIN rotation flow, lockout/rate limiting, malformed-request audit telemetry,
or power-loss/physical-device qualification. A production image must also
provision a real trusted application key instead of the deterministic test key
used by this acceptance harness.
---

## Source: docs/audit-stage24-9-pin-rotation-2026-10-20.md

# Stage 24.9 â€” live PIN rotation consistency

Date: 2026-10-20

## Scope

PIN rotation now updates the live kernel identity authority and the bounded
account file as one guarded operation. The kernel verifies the old PIN,
generates a fresh salt, updates its in-memory credential, performs the
trusted account-file rewrite, and rolls the authority credential back if the
file transaction fails. WovenShell reloads its account cache after success so
future lock/unlock operations use the new credential.

## Acceptance

- `cargo test -p libwoven`: 86 passed, including atomic password-rotation coverage.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.9-pin-rotation-1cpu-1792532400000000000`
- `audit-artifacts/stage24.9-pin-rotation-2cpu-1792532400000000000`
- `audit-artifacts/stage24.9-pin-rotation-4cpu-1792532400000000000`

## Limits and next prerequisites

The hardware-demo fixture does not drive the PIN-change UI, so this slice's
rotation-specific proof is the bounded host policy test and reviewed kernel /
shell integration; the live gates prove no boot or SMP regression. Account
creation/deletion and group administration, lockout/rate limiting,
malformed-request telemetry, hardware-backed credentials, real release-key
provisioning, and physical USB/power-loss qualification remain open.
---

## Source: docs/audit-stage25-10-secondary-cpu-barrier-2026-10-21.md

# Stage 25.10 audit - secondary-CPU suspend barrier

Date: 2026-10-21

## Scope

This stage adds the CPU-coordination prerequisite needed before a future BSP
PM1 S3 write. It does not enter a firmware sleep state.

## Implementation

`kernel/src/smp.rs` adds a private `CpuSuspendBarrier`. The BSP snapshots the
online mask, requests each online secondary CPU to evacuate scheduler-owned
work, and waits for the existing CPU-owned hotplug checkpoint to park it. The
barrier records the exact completed set. A failed request immediately attempts
to restore the completed prefix. Rollback attempts every parked CPU in reverse
order and verifies that the original online mask is restored.

`power::prepare_suspend` now takes this barrier after driver quiescence,
wake-trampoline staging, CPU-context capture, and FACS-vector installation.
Failure unwinds the vector, trampoline, CPU barrier, and driver plan. Abort
restores the vector and all parked CPUs before completing driver rollback.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.10-1cpu-1792575579462473700`
- 2 CPUs: `audit-artifacts/stage25.10-2cpu-1792575623913254100`
- 4 CPUs: `audit-artifacts/stage25.10-4cpu-1792575642628049000`

## Security and concurrency review

Only the BSP can create the barrier. Scheduler evacuation and CPU ownership
changes use the previously accepted hotplug checkpoint and lock ordering;
the new power path does not take a scheduler lock directly or sleep under an
IRQ lock. Partial failure is bounded and attempts every cleanup operation.
The private token prevents an unrelated caller from re-onlining or altering a
different CPU set.

## Limits and prerequisites

The barrier parks secondary CPUs but does not save their architectural
registers, restore them after firmware wake, write PM1 S3 controls, or provide
firmware wake handling. Those remain required before a suspend action or a
production hardware claim.
---

## Source: docs/audit-stage25-11-pm1-s3-request-2026-10-21.md

# Stage 25.11 audit - PM1 S3 request boundary

Date: 2026-10-21

## Scope

This stage adds the internal PM1 control-word request boundary after the
previous stages have prepared drivers, wake memory, vectors, CPU context, and
secondary-CPU ownership. It does not expose a suspend action to userspace.

## Implementation

`power::enter_suspend` validates the discovered ACPI mode, PM1a block, and
`_S3` types, then reads PM1a and optional PM1b. It preserves unrelated control
bits, replaces SLP_TYP with the bounded firmware values, and sets SLP_EN. The
request is issued only after the private `SuspendPreparation` token owns all
rollback resources.

On hardware where firmware accepts S3, the request does not return. If the
machine remains running, the path restores the FACS vectors, wake trampoline,
secondary CPUs, and drivers through `abort_suspend`, returning
`StillRunning`; a failed cleanup returns `RollbackFailed`.

The `stage25-11-test` self-test checks the pure control-word encoding and an
out-of-range type rejection. It does not access I/O ports or request S3.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.11-1cpu-1792576039149377100`
- 2 CPUs: `audit-artifacts/stage25.11-2cpu-1792576252034971100`
- 4 CPUs: `audit-artifacts/stage25.11-4cpu-1792576273864200600`

After a final fail-closed review of the PM1b error path, the exact committed
tree was rerun as:

- 1 CPU: `audit-artifacts/stage25.11-1cpu-1792576842603079000`
- 2 CPUs: `audit-artifacts/stage25.11-2cpu-1792576881588102400`
- 4 CPUs: `audit-artifacts/stage25.11-4cpu-1792576903007970400`

## Security and concurrency review

The transition remains private to the power subsystem; no untrusted process
can write PM1. The values come from bounded ACPI discovery, and unrelated PM1
bits are preserved. I/O writes occur with interrupts disabled. A returning
firmware path cannot silently abandon quiesced drivers or parked CPUs because
rollback is mandatory and failure is surfaced.

## Limits and prerequisites

This stage does not restore CPU registers, execute the wake-entry restoration
path, restart secondary CPUs after firmware wake, provide a suspend syscall or
policy, or establish physical hardware suspend/resume qualification. The
QEMU gate intentionally avoids issuing S3.
---

## Source: docs/audit-stage25-12-cpu-context-restore-2026-10-21.md

# Stage 25.12 audit - CPU resume-context restoration primitive

Date: 2026-10-21

## Scope

This stage implements the architectural CPU-state restoration primitive needed
after a future firmware wake. It validates and restores a live kernel context;
it does not itself enter S3 or claim that firmware has resumed the machine.

## Implementation

`CpuResumeContext` is now `#[repr(C)]` and is restored by a documented
non-returning assembly boundary. The sequence disables interrupts, reloads
CR0, CR4, CR3, and EFER, reconstructs packed descriptor pointers for the saved
GDT and IDT, loads both tables, switches to the saved RSP, restores RFLAGS,
and jumps directly to a kernel-owned continuation. The caller validates the
context before entering this unsafe boundary.

The `stage25-12-test` continuation records the success marker and exits the
QEMU test device. It exercises the actual control-register and descriptor
restore instructions using the current live context, rather than only testing
a byte encoder.

## Acceptance evidence

The exact feature and marker passed on all required CPU counts:

- 1 CPU: `audit-artifacts/stage25.12-1cpu-1792577216406089600`
- 2 CPUs: `audit-artifacts/stage25.12-2cpu-1792577279770004200`
- 4 CPUs: `audit-artifacts/stage25.12-4cpu-1792577302737476600`

The exact final tree, including the warning-denying default-feature annotation
pass, was rerun as:

- 1 CPU: `audit-artifacts/stage25.12-1cpu-1792577520657008000`
- 2 CPUs: `audit-artifacts/stage25.12-2cpu-1792577557169550100`
- 4 CPUs: `audit-artifacts/stage25.12-4cpu-1792577578206893300`

## Security and concurrency review

The restore target and continuation are kernel-owned; no userspace pointer or
firmware-provided address is accepted. The context is validated before the
assembly boundary, and interrupts remain disabled until the saved flags are
restored. The sequence does not allocate, lock, or schedule while architectural
state is transient.

## Limits and prerequisites

This stage does not create or consume a persistent firmware-wake record, wire
the low-memory wake entry to a stored suspend token, restore drivers or
secondary CPUs after a real S3 wake, or qualify physical hardware behavior.
---

## Source: docs/audit-stage25-13-wake-record-2026-10-21.md

# Stage 25.13 audit - persistent firmware-wake record integration

Date: 2026-10-21

## Scope

This stage connects the completed preparation transaction to the low-memory
wake path. It does not issue S3 in automated acceptance and does not claim
physical firmware wake.

## Implementation

`PENDING_SUSPEND` is a single kernel-owned slot containing the complete private
preparation token. `enter_suspend` captures a fresh CPU context immediately
before PM1 request, rejects duplicate pending transitions, and stores the
record before writing sleep controls. Pre-transition failures consume the slot
and call the existing full rollback path.

`wake_entry` snapshots the validated context and invokes the Stage 25.12
non-returning restore primitive. The continuation consumes the pending token
and calls `abort_suspend`, restoring FACS vectors, releasing the trampoline,
re-onlining secondary CPUs, and resuming drivers. A missing record fails to a
parked diagnostic path rather than dereferencing arbitrary memory.

## Acceptance evidence

The guarded non-destructive gate passed on all required CPU counts:

- 1 CPU: `audit-artifacts/stage25.13-1cpu-1792577693918265200`
- 2 CPUs: `audit-artifacts/stage25.13-2cpu-1792577749038729500`
- 4 CPUs: `audit-artifacts/stage25.13-4cpu-1792577772792940000`

## Security and concurrency review

The record contains only fixed-size kernel-owned state and is protected by a
kernel spin lock. It cannot be populated from userspace, and duplicate
transitions are rejected. Wake restoration snapshots the context before
calling non-returning assembly, releasing the record lock before the context
switch. Rollback remains ordered and attempts all owned cleanup operations.

## Limits and prerequisites

The automated gate does not issue PM1 S3. Real firmware wake, testing on the
attached hardware, firmware-specific wake behavior, and physical suspend/
resume qualification remain required before production release.
---

## Source: docs/audit-stage25-14-suspend-admission-2026-10-21.md

# Stage 25.14 suspend admission audit â€” 2026-10-21

## Scope

This stage adds the stable privileged suspend action without enabling a
hardware S3 transition. Action `2` on `PowerControl` and
`libwoven::suspend()` fail closed until the wake path can restore the
interrupted userspace syscall frame. The rejection is audited and performs no
driver, CPU, FACS, or PM1 mutation.

## Verification

- `cargo fmt --all -- --check` passed after formatting the existing tree.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-14-test -- -D warnings` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed: 95 library tests plus all integration and doc
  test targets.
- `cargo build -p wovenhat-kernel --target x86_64-unknown-none` passed after
  the feature-gated dead-code correction.
- Focused QEMU gate passed sequentially:
  - 1 CPU: `audit-artifacts/stage25.14-1cpu-1792578385038056800`
  - 2 CPUs: `audit-artifacts/stage25.14-2cpu-1792578599539880300`
  - 4 CPUs: `audit-artifacts/stage25.14-4cpu-1792578621521441400`

## Limits and next prerequisite

QEMU verifies the admission invariant only; it does not issue S3. Physical
firmware wake, driver restoration after real S3, battery/thermal policy, and
task-owned return to the interrupted userspace caller remain unqualified.
The next stage must commit and verify that task-owned resume target before
enabling a real suspend request.
---

## Source: docs/audit-stage25-15-userspace-resume-target-2026-10-21.md

# Stage 25.15 userspace resume target audit â€” 2026-10-21

## Scope

Suspend action `2` now validates the complete syscall frame supplied by the
existing interrupt entry against the current task's kernel stack. The frame is
retained in the committed suspend record. The wake continuation restores all
owned resources, writes a successful return value, and transfers through the
existing syscall epilogue to the interrupted userspace caller.

## Verification

The required gates passed:

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-15-test -- -D warnings` passed.
- `cargo clippy --offline --all-targets -- -D warnings` passed.
- `cargo build --offline -p wovenhat-kernel --target x86_64-unknown-none` passed.
- `cargo test --offline --all` passed: 95 library tests plus all integration
  and doc-test targets.
- Focused QEMU gate passed sequentially:
  - 1 CPU: `audit-artifacts/stage25.15-1cpu-1792579335112039300`
  - 2 CPUs: `audit-artifacts/stage25.15-2cpu-1792579392886205500`
  - 4 CPUs: `audit-artifacts/stage25.15-4cpu-1792579413003466500`

The retained prior-stage compatibility gate was also rerun after the shared
commit integration:

- Stage 25.14, 1 CPU: `audit-artifacts/stage25.14-1cpu-1792579508247837200`
- Stage 25.14, 2 CPUs: `audit-artifacts/stage25.14-2cpu-1792579565826183400`
- Stage 25.14, 4 CPUs: `audit-artifacts/stage25.14-4cpu-1792579583277334400`

The focused QEMU test does not issue S3; it verifies record ownership and
invalid-pointer rejection only.

## Limits

This stage does not prove that a physical machine enters S3, wakes through the
FACS vector, or resumes every real driver. Physical firmware and USB-boot
qualification remain separate release gates.
---

## Source: docs/audit-stage25-1-acpi-power-capabilities-2026-10-21.md

# Stage 25.1 audit - ACPI power capability boundary

Date: 2026-10-21

## Scope

This increment adds a bounded, read-only power-management boundary. ACPI
discovery recognizes `_S3` sleep types in addition to the existing `_S5`
power-off types. Syscall 235 and `libwoven::power_capabilities` report three
capability bytes: power-off, suspend, and restart. The query requires the
`PowerControl` capability, records an audit event, validates the requested
userspace output length, and performs no transition.

The always-on ACPI structural self-test now includes a bounded synthetic AML
package covering `_S3` discovery, rejection of an unrelated `_S5` lookup, and
rejection of unsupported sleep-state selectors.

## Verification

- Signed production kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- The signed 1/2/4-CPU boots exercised the ACPI structural self-test, including
  the new `_S3` AML fixture.
- The source change was compiled into the signed production image used by
  those QEMU runs.
- `git diff --check`: run as part of the stage handoff.

## Limits and next prerequisites

This is not suspend/resume implementation. It does not save or restore driver
state, quiesce devices, select wake sources, expose battery/thermal/lid
telemetry, or qualify real firmware and hardware. A later Stage 25 increment
must add an event-driven transition state machine and per-driver ownership and
rollback rules before any S3 entry is exposed. Physical TPM/USB qualification
also remains open.
---

## Source: docs/audit-stage25-2-cpu-idle-capabilities-2026-10-21.md

# Stage 25.2 audit - CPU idle capability boundary

Date: 2026-10-21

## Scope

This increment adds a read-only CPUID-derived CPU idle capability boundary.
Syscall 236 and `libwoven::cpu_idle_capabilities` report three bytes for
architectural `HLT`, optional `MONITOR/MWAIT`, and invariant-TSC support. The
query requires `PowerControl`, records an audit event, validates the user
buffer length and copy, and changes no CPU state. The scheduler's existing
interrupt-safe `STI+HLT` path is unchanged.

## Verification

- Signed production-kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Workspace all-targets host Clippy with `-D warnings`: pass.
- Full workspace `cargo test --all`: pass, including 95 `libwoven` tests and
  all repository integration suites.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- `git diff --check`: pass before handoff.

## Limits and next prerequisites

This is not an idle-policy implementation. It does not execute MWAIT, write
frequency-control MSRs, select deep C-states, account residency per CPU, or
implement suspend/resume. Those features require per-CPU coordination,
interrupt/wakeup proofs, hardware-model policy, and physical qualification.
---

## Source: docs/audit-stage25-3-cpu-frequency-inventory-2026-10-21.md

# Stage 25.3 audit - CPU frequency inventory boundary

Date: 2026-10-21

## Scope

This increment adds a read-only CPUID leaf `0x16` frequency inventory path.
Syscall 237 and `libwoven::cpu_frequency_info` return an availability byte
and bounded base, maximum, and bus MHz values. The query requires
`PowerControl`, records an audit event, validates the 13-byte userspace buffer,
and performs no P-state or MSR write.

## Verification

- Signed production-kernel Clippy with `system-apps` and `-D warnings`: pass.
- `libwoven` all-targets Clippy with `-D warnings`: pass.
- `libwoven` host tests: pass, 95 tests.
- Workspace all-targets host Clippy with `-D warnings`: pass.
- Full workspace `cargo test --all`: pass, including 95 `libwoven` tests and
  all repository integration suites.
- Signed system-apps QEMU shell acceptance: pass on 1 CPU, 2 CPUs, and 4
  CPUs; release shell, PS/2/IOAPIC input, and SMP smoke markers remained
  green.
- `git diff --check`: pass before handoff.

## Limits and next prerequisites

This does not implement frequency scaling, a governor, thermal throttling,
per-CPU policy, or physical frequency measurement. Those require validated
hardware-specific controls, rollback and interrupt coordination, thermal
policy, and physical qualification.
---

## Source: docs/audit-stage25-4-suspend-preparation-2026-10-21.md

# Stage 25.4 suspend-preparation audit (2026-10-21)

## Accepted scope

Stage 25.4 adds a bounded, reversible preparation boundary for a future ACPI
S3 transition. `driver::suspend_all` snapshots bound drivers without holding
the driver-table lock across hardware callbacks, suspends them in registration
order, and rolls back already-suspended drivers in reverse order if any hook
refuses. `driver::resume_all` also resumes in reverse order and reports an
aggregate result.

`power::prepare_suspend` requires the discovered ACPI PM1a control block, an
`_S3` sleep state, and ACPI mode before creating the preparation token.
`power::abort_suspend` consumes that token and restores driver state. The
preparation token is intentionally private and cannot be confused with a
completed sleep transition.

## Verification

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed, including the full libwoven and integration-test suite.
- Signed system-apps QEMU acceptance passed on 1, 2, and 4 CPUs via `scripts/test-shell-qemu.py --system-apps --cpus N`.
- `git diff --check` passed with only the repository's existing CRLF conversion warning.

The QEMU serial evidence is retained in `target/shell-regression-1cpu/`,
`target/shell-regression-2cpu/`, and `target/shell-regression-4cpu/`.

## Explicit limits

This stage does not write ACPI `SLP_EN`, enter S3, install a wake vector,
coordinate CPU resume, select wake sources, or add a suspend syscall. It also
does not provide device-specific save/restore beyond the existing driver
hooks, battery/thermal/lid reporting, or physical suspend/resume qualification.
---

## Source: docs/audit-stage25-5-facs-wake-capabilities-2026-10-21.md

# Stage 25.5 FACS wake-capability audit (2026-10-21)

## Accepted scope

ACPI discovery now follows the FADT firmware-control pointer, prefers the
validated 64-bit X_FACS address when present, and retains FACS metadata only
when the containing firmware region, `FACS` signature, and minimum length are
valid. The retained record includes the FACS address/length/version and both
firmware waking-vector fields.

Syscall 238 and `libwoven::suspend_resume_capabilities` expose only bounded
capability flags and the FACS version to a caller with `PowerControl`:
advertised S3, validated FACS, nonzero 32-bit vector, and nonzero 64-bit
vector. No firmware address is copied to userspace, and the query does not
write FACS or PM1 control state.

The ACPI self-test covers valid FACS metadata plus short-length and signature
rejection. Invalid FACS data fails closed to â€œnot availableâ€ rather than
making an untrusted wake structure usable.

## Verification

- Default production-kernel Clippy with `-D warnings` passed.
- `system-apps` production-kernel Clippy with `-D warnings` passed.
- Host all-targets Clippy with `-D warnings` passed.
- `cargo test --all` passed: 95 libwoven tests and all integration suites.
- Signed system-apps QEMU acceptance passed on 1, 2, and 4 CPUs.
- `git diff --check` passed with only the repository's CRLF conversion warning.

Serial evidence remains in `target/shell-regression-1cpu/serial.log`,
`target/shell-regression-2cpu/serial.log`, and
`target/shell-regression-4cpu/serial.log`.

## Explicit limits

This stage does not allocate an OS wake trampoline, write the FACS waking
vector, save/restore CPU state, enter S3, handle firmware wake, coordinate
secondary CPUs, or expose a suspend action. It also does not qualify a real
machine's firmware or power-loss behavior.
---

## Source: docs/audit-stage25-6-low-wake-page-2026-10-21.md

# Stage 25.6 low-memory wake-page reservation audit (2026-10-21)

## Accepted scope

SMP early preparation now reserves two distinct usable pages below 640 KiB:
the existing AP startup page and a separate firmware-reachable wake page.
Both are selected before the frame allocator excludes low memory, and neither
is returned to general allocation. If firmware exposes only one suitable low
page, the wake page remains unavailable rather than aliasing the AP page.

The suspend-preparation token now requires the distinct wake page and carries
its physical address internally for the future trampoline builder. No code is
written to the page yet, and no FACS waking-vector field is changed.

## Verification

- Default production-kernel Clippy with `-D warnings` passed.
- The prior Stage 25.5 `system-apps` production-kernel Clippy, host
  all-targets Clippy, full host tests, and signed QEMU 1/2/4-CPU matrix remain
  green; the final low-page change is also covered by the next gate rerun.
- `git diff --check` passed with only the repository's CRLF conversion warning.

## Explicit limits

This stage does not build executable wake code, install a FACS vector, save or
restore CPU state, quiesce secondary CPUs for sleep, enter S3, or handle
firmware wake. Physical memory-map and firmware behavior still require real
hardware qualification.
---

## Source: docs/audit-stage25-7-wake-trampoline-2026-10-21.md

# Stage 25.7 wake-trampoline staging audit (2026-10-21)

## Accepted scope

The existing AP real-mode-to-long-mode transition image is now reusable for
the future S3 wake path. Staging clones the current kernel root, builds a
private low-memory identity map for the first 1 GiB, patches the reserved wake
page with the transition image, and supplies a dedicated aligned wake stack and
entry point. The allocated page-table frames are owned by a private
`WakeTrampoline` token and are released on abort or self-test teardown.

Suspend preparation now stages this image after driver quiescence and rolls
back both the image resources and driver state if staging fails. The guarded
`stage25-7-test` QEMU gate installs and tears down the image without entering
S3 or modifying firmware state.

## Verification

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-7-test -- -D warnings` passed.
- Default production-kernel Clippy with `-D warnings` passed.
- Host all-targets Clippy with `-D warnings` passed.
- `cargo test --all` passed.
- `scripts/test-stage25-7-wake-trampoline.py` passed on 1, 2, and 4 CPUs.
- `git diff --check` passed with only the repository's CRLF conversion warning.

Per-CPU evidence is retained under `audit-artifacts/stage25.7-*`.

## Explicit limits

This stage does not write a FACS waking vector, save CPU/register state,
coordinate or restart secondary CPUs, enter ACPI S3, handle firmware wake, or
claim physical suspend/resume qualification. The wake entry currently
reinitializes the descriptor/interrupt foundation and parks until the later
resume-state boundary is implemented.
---

## Source: docs/audit-stage25-8-cpu-resume-context-2026-10-21.md

# Stage 25.8 audit - CPU resume-context capture

Date: 2026-10-21

## Scope

This stage implements the next bounded suspend/resume prerequisite: capture
the current CPU state needed by a future wake path and retain it in the private
suspend preparation token. It does not perform a power transition.

## Implementation

`kernel/src/power.rs` adds `CpuResumeContext` with CR0, CR3, CR4, EFER,
RFLAGS, RSP, and GDT/IDT descriptor base and limit fields. The capture uses
privileged architectural reads and small x86_64 inline-assembly reads for
RFLAGS, RSP, SGDT, and SIDT. Validation requires long-mode paging, PAE, a
page-aligned CR3, canonical saved addresses, nonzero descriptor limits, and
the architectural RFLAGS bit.

`prepare_suspend` captures and validates this context after the existing
ACPI, wake-page, driver-quiescence, and wake-trampoline prerequisites. The
context is copied into kernel-owned storage and is not derived from or retained
as a userspace pointer. `abort_suspend` validates the token before teardown.
The `stage25-8-test` feature reports a boot self-test marker without changing
the normal boot path.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.8-1cpu-1792574752351773000`
- 2 CPUs: `audit-artifacts/stage25.8-2cpu-1792574817503473400`
- 4 CPUs: `audit-artifacts/stage25.8-4cpu-1792574865427387800`

The remaining host, kernel Clippy, and full test results are recorded in the
stage commit and final handoff after this audit is completed.

## Security and concurrency review

The context is private, fixed-size, `Copy` data. It contains no userspace
pointer and no firmware address is exposed. Capture occurs on the preparing
CPU before any future transition; this stage does not yet assert that other
CPUs are quiesced, so no multi-CPU resume claim is made beyond the boot
self-test. Existing driver rollback and wake-trampoline ownership remain
transactional.

## Limits and prerequisites

Register restoration, AP/secondary-CPU coordination, FACS vector installation,
wake-source policy, PM1 S3 writes, firmware wake, and physical suspend/resume
qualification remain required before exposing a suspend action or calling the
OS production-ready.
---

## Source: docs/audit-stage25-9-facs-wake-vector-2026-10-21.md

# Stage 25.9 audit - transactional FACS wake-vector installation

Date: 2026-10-21

## Scope

This stage closes the next bounded prerequisite between wake-trampoline
staging and a future S3 transition: ownership of the ACPI FACS waking-vector
mutation. It does not request a power transition.

## Implementation

`kernel/src/hal/acpi.rs` adds a private `FacsWakeVector` owner token and
`install_facs_wake_vector`/`restore_facs_wake_vector`. Discovery already
validates the FACS signature, minimum length, and physical memory range. The
install path uses that validated direct map, copies the existing 32-bit and
64-bit vector values, and writes the reserved low wake-page address to both
forms with volatile stores while interrupts are disabled. A small lock
serializes the firmware-field mutation. The token restores the exact previous
values on abort.

`power::prepare_suspend` now stages the trampoline and then installs the
vector. Any vector failure releases the trampoline and resumes drivers.
`abort_suspend` restores the vector before releasing the trampoline and
resuming drivers. The byte-level self-test rejects targets that cannot fit in
the 32-bit field and verifies installation and rollback without modifying
firmware memory.

## Acceptance evidence

The guarded QEMU gate passed with the exact feature and marker on all required
CPU counts:

- 1 CPU: `audit-artifacts/stage25.9-1cpu-1792575149733404100`
- 2 CPUs: `audit-artifacts/stage25.9-2cpu-1792575203237815100`
- 4 CPUs: `audit-artifacts/stage25.9-4cpu-1792575220341503800`

## Security and concurrency review

Only firmware memory already validated by ACPI discovery is written. The
private token owns the previous values, preventing a caller from supplying an
arbitrary restore address or userspace pointer. The mutation is serialized and
interrupt-safe. Preparation remains private, so no untrusted process can
request a firmware vector write. Driver and trampoline rollback remains
failure-ordered and explicit.

## Limits and prerequisites

The OS still does not expose suspend, coordinate or stop secondary CPUs,
restore CPU registers, write PM1 S3 controls, perform firmware wake, or claim
physical suspend/resume qualification. Those must be implemented and tested
before production release.
---

## Source: docs/audit-stage26-1-accessibility-core-2026-10-21.md

# Stage 26.1 accessibility core audit â€” 2026-10-21

## Scope

This stage adds the bounded `libwoven::accessibility` module: accessibility
tree nodes and roles, deterministic keyboard focus navigation, screen-reader
focus announcements, captions, high-contrast palette selection, reduced-motion
duration policy, scale, and magnification.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo test --offline -p libwoven` passed: 100 tests, including five new
  accessibility tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo test --offline --all` passed: all workspace unit and integration tests
  passed, including the 100 `libwoven` tests.
- `cargo clippy --offline --all-targets -- -D warnings` passed for the full
  workspace.

This is a userspace library increment, so no kernel/QEMU behavior changed and
the 1/2/4-CPU kernel acceptance gate is not claimed as evidence for this
module. The next integration gate must exercise the accessibility contract
through a desktop surface before Stage 26 can be considered complete.

## Limits

No screen reader process, compositor-wide tree projection, translated string
catalog, RTL shaping, or physical accessibility device qualification is
implemented by this bounded slice.
---

## Source: docs/audit-stage26-5-i18n-foundation-2026-10-21.md

# Stage 26.5 bounded internationalization foundation audit - 2026-10-21

## Scope

This increment adds `libwoven::i18n`: fixed-size English and Chichewa locale
identifiers, bounded translation catalog insertion/lookup, Gregorian date
validation and locale-aware date ordering, caller-buffer integer formatting,
and Unicode scalar validation.
The follow-up adds a transactional Latin combining-mark normalization subset;
unsupported combining sequences fail closed. The latest follow-up adds a
caller-buffered strong-direction run resolver for common Latin/Hebrew/Arabic
text and numbers; it is explicitly not full UAX#9.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo test --offline -p libwoven` passed: 109 tests, including nine new
  i18n tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo test --offline --all` passed: 109 `libwoven` tests and all workspace
  integration suites passed.
- `cargo clippy --offline --all-targets -- -D warnings` passed for the full
  workspace.
- `cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none
  --features system-apps -- -D warnings` passed with the generated signed
  application image environment.
- `python scripts/test-shell-qemu.py --system-apps --cpus 1` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 2` passed.
- `python scripts/test-shell-qemu.py --system-apps --cpus 4` passed.

The QEMU runs are regression evidence for the production system-app image and
SMP boot path. During this follow-up, the first two 1-CPU attempts reproduced
a user-mode stack write fault at the former 8 KiB shell-stack boundary before
F12 diagnostics could open. The bounded `UserStack::SIZE` was increased to
16 KiB while retaining the guard page; the corrected 1/2/4-CPU runs passed.
This library increment adds no i18n-specific kernel feature gate or kernel
self-test of its own.

## Limits

This is not full internationalization. Complete Unicode normalization and
property data, full bidi/RTL ordering, script shaping, keyboard layouts, IMEs, and
translated application catalogs remain open prerequisites. The WovenShell
lock screen now consumes bounded English/Chichewa catalogs, but this is not
application-wide translation coverage or reviewed localization.
---

## Source: docs/audit-stage27-1-update-ab-state-2026-10-20.md

# Stage 27.1 bounded update A/B state audit â€” 2026-10-20

## Implemented boundary

`kernel/src/update.rs` now owns the bounded A/B activation state machine:

- a verified candidate is staged only in the inactive slot;
- generation rollback and empty/oversized/digestless candidates are rejected;
- activation changes only the next-boot pending record, never the current
  active slot;
- three failed boot attempts mark the candidate bad and select the known-good
  slot; and
- a candidate becomes active only after an explicit service-owned health-check
  success report.

The transition object is allocation-free and bounded to two slots. The
package boundary now also accepts only a bounded version-1 `WUPD` envelope:
the manifest generation, declared image length, image digest, and exact image
bytes are covered by a domain-separated SHA-256 digest and verified with the
kernel-provisioned Ed25519 trust root before a candidate is admitted. A
metadata tamper self-test changes the signed generation and is rejected. The
privileged update service must still persist each state record atomically on
WovenFS; this increment does not claim that service integration.

The state object also has a fixed-size `WUAB` record codec. It serializes the
two slots, active/pending selectors, and a monotonic record sequence with a
CRC-32. Decode rejects wrong magic/version/length, invalid slot invariants,
invalid attempts, and checksum failures. This is the record boundary for a
future service-owned dual-copy commit; it does not itself perform device I/O.

## Validation

- `cargo test --workspace` passed, including `tests/update_state.rs`.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-1-test -- -D warnings` passed.
- Dedicated QEMU acceptance passed with exit 33:
  - `audit-artifacts/stage27.1-1cpu-1792512141099830700`
  - `audit-artifacts/stage27.1-2cpu-1792512158205516300`
  - `audit-artifacts/stage27.1-4cpu-1792512173673074900`
  - `audit-artifacts/stage27.1-1cpu-1792512350452245500`
  - `audit-artifacts/stage27.1-2cpu-1792512351912529200`
  - `audit-artifacts/stage27.1-4cpu-1792512368502642400`

An earlier signed-envelope run failed its signature assertion; the recorded
diagnostic evidence was retained, the test trust seed was corrected to match
the repository's provisioned public key, and the fresh matrix above passed.

## Limits and next work

The state machine and envelope verifier are not persistent storage, an update
daemon, an installer, or a power-loss proof. Next work must bind them to a
privileged service, durable A/B records, and recovery boot evidence. Physical
USB qualification remains separately blocked until the selected device is
explicitly authorized for erasure in an elevated session.

## Stage 27.2 durable record acceptance (2026-10-20)

The durable boundary now writes exact-length `WUAB` records through the native
WovenFS ATA transaction path. The acceptance formats a disposable scratch
volume, commits two records, remounts and reads both records, selects the
newest valid sequence, and falls back to the older record after corrupting the
newer copy. The update service owns the authority token in this gate; no user
VFS cache is used for record selection.

Validation passed with exit 33:

- `audit-artifacts/stage27.2-1cpu`
- `audit-artifacts/stage27.2-2cpu`
- `audit-artifacts/stage27.2-4cpu`

The dedicated script uses only disposable scratch images. No physical USB
write, format, partition change, or boot was performed.

Stage 27.2 still does not provide a long-lived userspace update daemon,
power-loss injection/recovery across every commit boundary, installer/boot
slot integration, or physical-media qualification.

After the Stage 27.2 feature and WovenFS changes, the inherited Stage 27.1
gate was rerun successfully on 1/2/4 CPUs:

- `audit-artifacts/stage27.1-1cpu-1792512964891684100`
- `audit-artifacts/stage27.1-2cpu-1792512978915669200`
- `audit-artifacts/stage27.1-4cpu-1792512993391527100`

## Capability-gated service boundary (2026-10-20)

The native record adapter now issues `AuthorizedWfsWriter` only when the
current kernel task carries `Capability::StorageIo`; the token's constructor
is private to the kernel crate. Update record commit/read helpers consume that
token and map WovenFS failures to the update boundary. Stage 27.2's live gate
exercises this authority path before committing either copy. This is a
service-ownership boundary, not yet a complete userspace daemon or a new
userspace syscall/API.
---

## Source: docs/audit-stage27-3-update-package-producer-2026-10-21.md

# Stage 27.3 signed update-package producer audit â€” 2026-10-21

## Scope

This increment adds `tools/wupd-packager`, the release boundary for the
kernel's version-1 `WUPD` envelope. It emits the exact layout consumed by
`kernel::update::verify_package`: bounded generation and image length, SHA-256
image digest, and an Ed25519 signature covering a domain separator, all
immutable manifest fields, and the exact image bytes.

The producer rejects empty or oversized input, zero/non-numeric generations,
input/output aliasing, missing production signing material, and a trusted key
that does not match the signing seed.

## Verification

- `cargo test --offline --manifest-path tools/wupd-packager/Cargo.toml` passed:
  four tests, including layout, digest, signature coverage, and key parsing.
- `cargo clippy --offline --manifest-path tools/wupd-packager/Cargo.toml
  --all-targets -- -D warnings` passed.
- CLI boundary test passed for development packaging, missing production seed,
  mismatched production trust key, and matching production key packaging.
- `python scripts/test-stage27-1-update.py --cpus 1` passed.
- `python scripts/test-stage27-1-update.py --cpus 2` passed.
- `python scripts/test-stage27-1-update.py --cpus 4` passed.

The Stage 27.1 QEMU runs retain serial evidence under the corresponding
`audit-artifacts/stage27.1-*` directories. They revalidate the existing kernel
signature/activation/rollback boundary alongside the producer increment.

## Limits

This is a package-production boundary, not a complete update system. The
privileged long-lived update daemon, IPC/syscall protocol, image-slot storage,
bootloader handoff, power-loss injection and recovery, key rotation, and
physical USB qualification remain open. No physical disk was written.
---

## Source: docs/audit-stage27-4-update-ipc-contract-2026-10-21.md

# Stage 27.4 bounded update IPC contract audit â€” 2026-10-21

## Scope

This increment adds `libwoven::update_ipc`, a fixed-capacity request/response
ABI for the future privileged WovenUpdate service. Requests distinguish
staging, activation, boot success, boot failure, and query operations. Stage
requests carry only a validated `/updates/` candidate path and positive
generation; lifecycle requests carry only a validated slot; image bytes never
cross IPC.

The codec rejects malformed headers, unsupported versions, unknown operations,
invalid generation/slot combinations, traversal or non-`/updates/` paths, and
oversized messages before a service can act on them.

## Verification

- `cargo test --offline -p libwoven` passed: 113 tests, including four new
  update-IPC tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- Existing Stage 27.1 1/2/4-CPU QEMU evidence remains valid because this
  increment changes only the userspace protocol library and no kernel image,
  feature gate, or boot path.

## Limits

This is an ABI contract, not the update daemon. No syscall dispatch, service
registration, capability issuance, signed-package staging, durable image-slot
write, bootloader handoff, rollback persistence, or power-loss injection is
implemented by this increment. Physical USB and boot-media qualification also
remain open.
---

## Source: docs/audit-stage27-5-recovery-environment-2026-10-22.md

# Stage 27.5 WovenRecovery environment audit - 2026-10-22

## Scope

This audit covers the bounded WovenRecovery environment foundation: signed
Ring-3 application loading, capability and boot-mode authorization, status,
filesystem-check, serial-log retrieval, and fail-closed reset/reinstall
admission. It does not claim destructive recovery, installation, boot-slot
handoff, or physical-device qualification.

## Implementation

- `Capability::Recovery` is included in the bounded system-service ceiling.
- Syscall 240 validates capability, authorization mode, operation, and output
  length before copying results to userspace.
- Status and filesystem results use fixed 32-byte records; log reads are
  bounded to 4096 bytes.
- Reset and reinstall return `ADMISSION_NOT_IMPLEMENTED`; no disk mutation is
  reachable through this foundation.
- The recovery application is signed as WAPP and reports denied,
  unavailable, and successful outcomes distinctly.

## Acceptance evidence

- `python scripts/test-stage27-5-recovery.py --cpus 1`: PASS, exit 33.
- `python scripts/test-stage27-5-recovery.py --cpus 2`: PASS, exit 33.
- `python scripts/test-stage27-5-recovery.py --cpus 4`: PASS, exit 33.
- Recovery app freestanding Clippy with `-D warnings`: PASS.
- Kernel Clippy with `stage27-5-recovery-test` and `-D warnings`: PASS.
- Workspace all-target Clippy with `-D warnings`: PASS.
- `cargo test --offline -p libwoven`: 115 passed, 0 failed.

Serial, QEMU, and disk-image evidence is retained under
`audit-artifacts/stage27.5-recovery-{1,2,4}cpu`.

## Limits and next prerequisites

Reset/reinstall are admission-only and intentionally not release-complete.
The next work must implement and independently test safe destructive recovery,
installer integration, update-slot sizing and bootloader handoff, power-loss
recovery, and physical qualification before release-certification stages.
---

## Source: docs/audit-stage27-5-update-ipc-2026-10-22.md

# Stage 27.5 end-to-end WovenUpdate IPC audit â€” 2026-10-22

## Scope

This gate exercises the actual signed `woven-update` userspace image rather
than only calling the kernel transaction functions. The acceptance client:

1. builds and signs the daemon image;
2. loads it through verified ELF/package validation;
3. grants only `Ipc`, `FileRead`, `FileWrite`, and `StorageIo`;
4. discovers `woven.update`;
5. sends a WUDI Query and verifies the active-slot response;
6. sends a truncated frame and verifies the fixed bad-request response; and
7. terminates, waits for, and verifies service cleanup.

## Results

| Check | Result | Evidence |
|---|---|---|
| WovenUpdate host tests | 2 passed | command output, 2026-10-22 |
| WovenUpdate freestanding Clippy | passed | `-D warnings` |
| Stage 27.5 kernel Clippy | passed | `x86_64-unknown-none`, `-D warnings` |
| Workspace all-target Clippy | passed | `-D warnings` |
| QEMU 1 CPU | passed, exit 33 | `audit-artifacts/stage27.5-1cpu` |
| QEMU 2 CPUs | passed, exit 33 | `audit-artifacts/stage27.5-2cpu` |
| QEMU 4 CPUs | passed, exit 33 | `audit-artifacts/stage27.5-4cpu` |

The first failed run is retained and was diagnosed: the daemon had allocated a
4096-byte receive buffer while the kernel IPC syscall accepts at most 256
bytes. The daemon now uses a 256-byte transport buffer, while the WUDI codec
continues to reject malformed or oversized frames. No assertions were
weakened.

## Remaining limits

This accepts the daemon IPC exchange only. It does not provide arbitrary-size
block-device update slots, bootloader target/attempt persistence, power-loss
injection, WovenRecovery, installer integration, or physical USB/hardware
qualification.
---

## Source: docs/audit-stage27-5-update-service-2026-10-21.md

# Stage 27.5 update-service audit â€” 2026-10-21

## Implemented

- Added the capability-gated kernel transaction boundary in
  `kernel/src/update_service.rs`.
- Added syscall 239 and the safe `libwoven::update_transaction` wrapper.
- Verified the signed WUPD envelope before any state transition.
- Wrote the inactive slot before committing the next dual-copy WUAB record.
- Added sequence arbitration, missing-copy recovery, and fail-closed invalid
  or tied-copy handling.
- Implemented bounded stage, activate, boot-failed, boot-succeeded, and query
  operations.
- Added `scripts/test-stage27-5-update-service.py`.

## Evidence

Commands were run serially in the project-local target directory:

```text
cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none --features stage27-5-test -- -D warnings  PASS
cargo clippy --offline -p libwoven --all-targets -- -D warnings                                      PASS
cargo test --offline -p libwoven                                                                        113 passed
python scripts/test-stage27-5-update-service.py --cpus 1                                                PASS
python scripts/test-stage27-5-update-service.py --cpus 2                                                PASS
python scripts/test-stage27-5-update-service.py --cpus 4                                                PASS
```

Retained QEMU evidence is under `audit-artifacts/stage27.5-{1,2,4}cpu/`.

## Limits and next prerequisites

The current WovenFS implementation limits one durable file to 4 KiB. The
service rejects larger images with `ImageTooLarge`; it does not split or
silently truncate them. This stage therefore proves the transaction and
authority boundary, not production image installation. Remaining work before
release includes the long-lived privileged WovenUpdate daemon and WUDI IPC
loop, a block-device/resumable slot backend for production image sizes,
bootloader handoff and persisted boot-attempt decisions, power-loss/fault
injection recovery, release-key operations, and destructive physical USB
qualification under an explicitly approved device identity.
---

## Source: docs/audit-stage27-5-update-service-process-2026-10-21.md

# Stage 27.5 WovenUpdate service-process audit â€” 2026-10-21

## Scope

This slice connects the signed `woven-update` system application to the
production `system-apps` boot path. The kernel starts it as a system service
with `Ipc`, `FileRead`, `FileWrite`, and `StorageIo`, and waits for the
`woven.update` publication before completing service startup.

The daemon itself publishes `woven.update`, receives bounded WUDI requests,
rejects malformed requests before privileged dispatch, forwards validated
operations to syscall 239 through `libwoven`, and returns bounded responses.

## Evidence

| Check | Result | Retained evidence |
|---|---|---|
| `apps/woven-update` host tests | 2 passed | command output on 2026-10-21 |
| `apps/woven-update` freestanding Clippy, `-D warnings` | passed | command output on 2026-10-21 |
| production system-app shell gate, 1 CPU | passed | `target/shell-regression-1cpu/serial.log` |
| production system-app shell gate, 2 CPUs | passed | `target/shell-regression-2cpu/serial.log` |
| production system-app shell gate, 4 CPUs | passed | `target/shell-regression-4cpu/serial.log` |

Each serial log contains both:

```text
[SERVICES] woven-package published as pid 1
[SERVICES] woven-update published as pid 2
```

The QEMU gate also reports `Normal release shell + PS/2 IOAPIC + smptest:
PASS` for each CPU count.

## Limits and next prerequisites

This audit proves boot-time publication and service ownership only. It does
not prove a client/server transaction exchange, production-sized block-device
slot storage, bootloader handoff, power-loss recovery, release-key
provisioning, or physical USB qualification. Those remain release gates.
---

## Source: docs/audit-stage28-disposable-acceptance-2026-10-22.md

# Stage 28 disposable-image acceptance harness â€” 2026-10-22

## Scope

`test-stage28-disposable-acceptance.py` composes the existing validated-plan
validator, file-image layout builder, FAT32-compatible formatter, EFI artifact
preparer, and file-backed executor. It creates a new 64 MiB regular temporary
image and never opens a device path, mounts a filesystem, invokes a partition
tool, or accesses physical media.

The executor is run successfully and with both atomic-replacement failure
injections on a separate disposable copy. Its existing contract replaces the
whole file with a deterministic mock filesystem payload, so it is not mixed
with the final partitioned-image bytes. The final image is assembled by the
layout, formatter, and EFI stages and is independently re-verified.

## Acceptance checks

- Planner emits a valid canonical GPT/FAT32 file-only plan.
- Executor success produces a verified mock manifest digest.
- Executor `before-replace` and `after-replace` failures restore the source
  byte-for-byte.
- Layout builder verifies protective MBR, primary/backup GPT metadata, bounds,
  alignment, and metadata digest.
- Formatter verifies deterministic FAT32-compatible data-region metadata and
  all injected rollback cases.
- EFI preparation verifies the exact `EFI/BOOT/BOOTX64.EFI` envelope, payload
  digest, and all injected rollback cases.
- A deterministic acceptance manifest records the GPT, data, EFI, executor,
  and manifest digests.

## Limits

This is disposable file-image acceptance only. It is not physical installation,
filesystem mounting, bootloader installation, ISO generation, firmware boot
qualification, hardware qualification, or release certification. The EFI
artifact remains `bootability: not-qualified`.
---

## Source: docs/audit-stage28-file-image-filesystem-2026-10-22.md

# Stage 28 disposable file-image filesystem formatter audit â€” 2026-10-22

## Scope

This slice adds a formatter/verifier for the one `data` partition produced by
`scripts/build-file-image-layout.py`. It is intentionally limited to regular
file images created by the test harness. It does not format physical disks,
mount filesystems, install bootloaders, generate installation media, or qualify
hardware.

## Implemented boundary

- Refuses symlinks, non-regular files, device paths, protected identities, and
  missing harness authorization.
- Revalidates the prior layout metadata and requires exactly one data region.
- Writes only that region, leaving the protective MBR, GPT metadata, and ESP
  bytes untouched.
- Emits deterministic FAT32-compatible metadata: BPB, backup boot sector,
  FSInfo and backup FSInfo, two matching FATs, reserved entries, root cluster,
  and label `WOVENHAT`.
- Verifies sector geometry, FAT/root bounds, signatures, copies, label, and
  byte-for-byte deterministic output.
- Restores the original selected region after injected failures before write,
  after write, and after verification.

## Validation

The focused Stage 28 chain passed:

```text
34 tests: OK
python -m py_compile: OK
git diff --check: OK
```

The chain includes the new filesystem tests and the existing layout, package,
ISO-probe, inventory, plan, executor, and UI-model tests. No physical disk or
mount operation was attempted.

## Limits and prerequisites

The result is not a production filesystem formatter or installer. Real
partitioning, filesystem creation on approved hardware, bootloader writes,
power-loss recovery, ISO/USB boot qualification, and hardware testing remain
separate release-certification prerequisites.
---

## Source: docs/audit-stage28-file-image-layout-2026-10-22.md

# Stage 28 disposable file-image layout audit - 2026-10-22

## Scope

This slice adds `scripts/build-file-image-layout.py`, a file-only test
boundary that prepares a disposable image from a validated installer plan.
The image must already be an explicitly supplied regular file, and the caller
must provide the explicit `--test-harness-created` authorization. Symlinks,
device/physical-drive paths, protected identities, missing files, and invalid
capacities are rejected.

## Layout and verification

The builder writes a protective MBR, primary and backup GPT-like headers and
entry arrays, an 8 MiB 1 MiB-aligned ESP region, and 1 MiB-aligned plan data
regions. Canonical metadata records the deterministic disk identity, regions,
capacity, and file-only mode; its SHA-256 is checked during verification.
Verification also checks signatures, metadata integrity, capacity, bounds,
alignment, and overlap after sorting regions.

## Validation

The focused three-test suite covers deterministic build/signature output,
authorization and protected-target refusal, metadata checksum tampering, and
overlap tampering. Python compilation and `git diff --check` are required.

## Limits

This is disposable-image preparation, not production installation. It does
not invoke partitioning APIs, open physical media, mount or format a
filesystem, write a bootloader, access hardware, or claim ISO/USB support.
---

## Source: docs/audit-stage28-file-install-executor-2026-10-22.md

# Stage 28 disposable file-backed installation executor audit â€” 2026-10-22

This slice adds `scripts/execute-file-install.py` and focused host coverage in
`tests/test_file_install_executor.py`. It consumes the JSON emitted by the
validated installer-plan boundary and requires an explicitly supplied regular,
non-symlink image file. The plan target identity is bound to that image's
pre-install SHA-256 as `file-image:<digest>`.

The executor writes a deterministic bounded mock filesystem payload containing
an execution manifest and fixed test files. It stages both a backup and a new
image in the target directory, fsyncs them, and uses `os.replace` for atomic
replacement. Verification failure and injected failure before or after replace
restore the original image and clean temporary files.

The executor rejects device-looking paths, protected target identities,
symlinks, mismatched image digests, and out-of-bound images. It contains no
physical-device access, `\\.\PhysicalDrive` support, mount, format, partition,
or bootloader command. This is not real installation, does not create a
bootable OS image, and is not Stage 28 acceptance or release certification.
---

## Source: docs/audit-stage28-installer-plan-validator-2026-10-22.md

# Stage 28 installer plan validator audit â€” 2026-10-22

This slice adds `scripts/validate-installer-plan.py`, a pure validator for a
caller-supplied immutable inventory snapshot and explicit installation
settings. It requires an exact path/serial identity match, rejects missing or
ambiguous matches, and protects system/boot, removable, read-only, backup,
external, and USB-labelled disks.

The validator bounds disk and partition sizes, requires sector-aligned
partitions, reserves one MiB, limits partition count, and validates the
filesystem, username, supported timezone/keyboard, and DHCP/static network
fields. Its canonical JSON includes a stable schema, normalized settings,
summary, and explicit `disk_io: none` / `installation: not-implemented`
operation markers.

Focused tests cover valid plans, protected and ambiguous targets, overflow and
capacity failures, invalid settings, and deterministic output. The validator
does not open device paths, invoke discovery, partition, format, install, or
install a bootloader. It does not claim installation support.
---

## Source: docs/audit-stage28-installer-ui-model-2026-10-22.md

# Stage 28 installer UI/state model audit - 2026-10-22

## Scope

This slice adds `scripts/installer-ui-model.py` and
`tests/test_installer_ui_model.py`. It is a pure state model for a future
installer UI, not a graphical application and not an installer executor.

## Implemented behavior

- Screens/events cover welcome, target selection, partition-plan review,
  user/timezone/keyboard/network settings, progress, cancellation, success,
  and failure.
- Only the ordered flow is legal: welcome, target, plan review, settings,
  progress.
- Target selection requires caller-provided `safe: true` and
  `ambiguous: false`; the model does not independently inspect a disk.
- Summaries and messages are bounded: 128 characters for target/plan/settings,
  256 for failure messages, and 128 for progress messages.
- Control characters are rejected. Progress is limited to 0..100 and cannot
  move backward.
- Progress and success require a separate `confirm_install` event carrying
  `confirmed: true`. This is an explicit future-operation gate, not execution.
- Cancellation is two-phase (`request_cancel`, then `cancelled`). Failure and
  cancellation are terminal and reject subsequent events.

## Validation

The focused installer host suite passes, including the existing package,
ISO-probe, and plan-validator tests plus the new UI-model tests. Python
compilation checks and `git diff --check` pass.

## Explicit limits

The model performs no disk or filesystem I/O; it invokes no discovery,
partition, format, mount, bootloader, subprocess, or graphical-rendering
operation. It does not create an ISO, write a target, roll back installation,
or claim graphical installer, real installation, physical hardware, or release
certification support. A future executor must independently validate the plan,
recheck target identity, and implement power-loss-safe semantics before using
the confirmation boundary.
---

## Source: docs/audit-stage28-iso-probe-2026-10-22.md

# Stage 28 ISO capability probe - 2026-10-22

## Scope

This slice adds a read-only, file-only capability probe at
`scripts/probe-installer-iso.py` with focused tests in
`tests/test_installer_iso_probe.py`. It does not generate an ISO, mount a
filesystem, access a physical disk, or alter the existing package-installer
slice.

The probe checks the exact prerequisites needed before an ISO implementation
could be attempted: a regular file-backed source image with protective MBR and
primary GPT signatures; an ISO builder executable (`xorriso`, `mkisofs`,
`genisoimage`, or `oscdimg`); and a file-backed EFI tree containing a non-empty
`EFI/BOOT/BOOTX64.EFI`.

It reports missing prerequisites as blockers and never reports ISO support. A
future builder must generate an artifact and boot that exact artifact in QEMU
before the capability can be accepted.

## Validation

```text
python -m unittest tests/test_installer_iso_probe.py tests/test_package_installer.py
python -m py_compile scripts/probe-installer-iso.py tests/test_installer_iso_probe.py
python scripts/probe-installer-iso.py --source-image target/release-validation/wovenhat-os-uefi.img
git diff --check
```

Results:

- 6 focused tests passed (3 probe, 3 unchanged installer tests).
- Python compilation checks passed.
- Diff whitespace check passed.
- The live probe exited with status 2 and reported these blockers:
  - the requested release-validation image path is not present;
  - no supported ISO builder is installed;
  - no EFI tree was supplied;
  - `EFI/BOOT/BOOTX64.EFI` therefore cannot be verified.

## Limits

No ISO was generated, and no QEMU ISO boot validation was run. ISO support is
not implemented or claimed. The existing Stage 28 file-backed GPT UEFI image
artifact remains the only accepted installer artifact boundary.
---

## Source: docs/audit-stage28-target-disk-inventory-2026-10-22.md

# Stage 28 target-disk inventory audit â€” 2026-10-22

This slice adds `scripts/inventory-target-disks.py` and focused host tests in
`tests/test_target_disk_inventory.py`. It parses caller-supplied JSON fixtures
representing normalized records, Windows PowerShell `Get-Disk` output, or
Linux `lsblk -J` output. It identifies a uniquely marked system/boot disk and
marks removable, USB, external, and backup-labelled devices as protected
candidates when those signals are present.

The classifier is fail-closed. Missing path/number, serial, or positive
capacity; duplicate serials; or zero/multiple system disks prevent
`safe_target_selection`. The report is informational only and has no operation
that opens a device, executes a discovery command, mounts, initializes,
formats, partitions, writes, or erases media.

Validation for this bounded slice:

- five focused fixture tests pass;
- Python compilation checks pass;
- CLI smoke output is generated from a temporary fixture only;
- no physical disk or command output was collected by the implementation.

This does not implement target selection, partitioning, installation,
rollback, ISO generation, or hardware qualification.
---

## Source: docs/audit-stage28-uefi-boot-artifact-2026-10-22.md

# Stage 28 UEFI boot artifact preparation audit â€” 2026-10-22

## Scope

This slice adds `scripts/prepare-uefi-boot-artifact.py` and
`tests/test_uefi_boot_artifact.py`. It consumes the verified disposable GPT
layout produced by the earlier Stage 28 layout slice, locates its single ESP,
and writes a bounded envelope containing the exact artifact path
`EFI/BOOT/BOOTX64.EFI`, byte length, SHA-256 digest, and payload bytes.

The source may be an explicit regular file or the deterministic test fixture.
Writes are limited to the ESP region, are followed by verification, and restore
the complete original ESP on injected pre-write, post-write, or post-verify
failure.

## Validation

- Stage 28 focused installer tests, including this slice
- Python compilation checks
- `git diff --check`

## Exact limits

- File-backed disposable images only; an explicit test-harness authorization is
  required.
- Artifact size is greater than zero and at most 4 MiB; the ESP is bounded by
  the prior layout metadata.
- The envelope is not a FAT filesystem directory entry and no mount is used.
- No physical/device path, symlink, partition API, filesystem formatter,
  firmware flash, bootloader installer, or hardware access is performed.
- The artifact is not parsed as PE/COFF and no signature policy is asserted.
- The result reports `bootability: not-qualified`; no OS-kernel bootability or
  QEMU boot claim is made by this slice.
---

## Source: docs/audit-stage28-woveninstaller-bounded-2026-10-22.md

# Stage 28 WovenInstaller bounded artifact slice â€” 2026-10-22

## Scope

This slice adds `scripts/package-installer.py`, a file-only artifact boundary
for the existing UEFI disk image emitted by `build.rs` and recorded by
`target/release-validation/validated-image.json`. It copies the image into a
new directory, verifies its digest, checks sector alignment plus the
protective-MBR/GPT signatures, and emits `installer-manifest.json` and
`SHA256SUMS`.

The packager does not enumerate, open, partition, format, mount, or write a
physical disk. Existing `scripts/write-live-usb.ps1` remains a separate,
destructive, administrator-only operation and is not called by this slice.

## Validation

Command:

```text
python -m unittest tests/test_package_installer.py
```

Result: 3 tests passed.

The test fixture is a synthetic file image and covers valid GPT-shape
acceptance, malformed-image rejection, digest-preserving copy, metadata
limits, and refusal to overwrite an existing output directory.

## Limits and next prerequisites

This is not full Stage 28 acceptance. It does not implement a graphical
installer, disk detection, partitioning, ISO generation, installation into a
target disk, rollback, or physical boot qualification. The metadata records
the ISO gap explicitly and identifies the artifact as suitable for QEMU and as
input to a separately reviewed USB writer.
---

## Source: docs/audit-stage28-woveninstaller-package-catalog-2026-10-22.md

# Stage 28 WovenInstaller package/catalog wiring audit â€” 2026-10-22

## Scope

The existing `scripts/package-system-apps.py` path now includes the already
implemented `apps/woven-installer` image. It builds the freestanding ELF,
signs it with the existing WAPP v1 producer, validates the envelope, copies
the package to `packages/woven-installer.wapp`, and emits a stable catalog
descriptor at `catalog/woven-installer.app`.

The manifest record uses `/bin/wovenhat-installer` for both `install_path` and
`Exec`. It declares `capabilities: []`, `privileged: false`, and
`interface: display-only`. A fail-closed metadata validator rejects the
installer if any install, disk, partition, format, or bootloader capability
is introduced. The prior ten app definitions remain in their original order
and are not changed.

## Validation

The focused package-manifest tests cover entry preservation, stable paths,
the non-privileged display-only contract, and forbidden-capability rejection.
The installer app is also checked with a freestanding release build and
warning-denying Clippy. Python compilation and the packaging script's
manifest smoke check pass.

## Limits

This is packaging/catalog wiring only. It does not alter kernel recovery,
grant installation authority, partition or format disks, install a
bootloader, or claim the complete `system-apps` QEMU gate. Physical media and
the attached drives are not accessed.
---

## Source: docs/audit-stage28-woven-installer-shell-2026-10-22.md

# Stage 28 WovenInstaller Ring-3 shell audit â€” 2026-10-22

## Scope

This slice adds only `apps/woven-installer`. It adapts an already validated
installer UI/state snapshot to a deterministic textual fallback. It is not an
installer and does not claim graphical, physical-media, or boot integration.

## Safety boundary

- Input is capped at 256 bytes and fields are bounded to the existing model's
  text limits.
- The snapshot must contain `validated=1` and a non-empty plan.
- Unknown and duplicate fields, control characters, invalid screen names,
  invalid progress, missing screen prerequisites, and unvalidated plans fail
  closed.
- There is no install subcommand, disk/file/partition/filesystem/bootloader
  operation, network call, subprocess call, or destructive syscall.
- Progress and success are display-only state-model screens and explicitly
  report `install_action=unavailable (fail-closed)`.

## Validation

- Host parser tests: 3 passed.
- Freestanding release build: `x86_64-unknown-none` passed.
- Freestanding Clippy: `cargo clippy --offline --manifest-path apps/woven-installer/Cargo.toml --target x86_64-unknown-none -- -D warnings` passed.
- App formatting check passed after formatting the new files.
- `git diff --check` passed.

## Limits and next prerequisites

No QEMU gate was run or claimed. This disjoint app is not embedded in the
kernel, so a QEMU boot would exercise neither a new process launch path nor an
installer integration path. The next installer work still requires an explicit
reviewed authority boundary, real installation semantics, boot handoff,
rollback/power-loss coverage, and physical qualification; none is implied by
this shell.
---

## Source: docs/audit-stage28-woveninstaller-system-app-integration-2026-10-22.md

# Stage 28 WovenInstaller production system-app integration audit

## Scope

This slice wires the already signed `apps/woven-installer` image into the
kernel production `system-apps` image and the Stage 19 catalog-test image. It
does not add disk, filesystem, partition, bootloader, subprocess, or physical
media access.

## Contract

- Build input: `WOVENHAT_INSTALLER_IMAGE`.
- Stable binary path: `/bin/wovenhat-installer`.
- Catalog path: `/share/applications/woven-installer.app`.
- Catalog Exec: `/bin/wovenhat-installer`.
- Capability grant: none; the package remains non-privileged and display-only.
- Existing ten catalog entries and paths remain unchanged.

## Validation

- `python -m unittest discover -s tests -p 'test_package_system_apps.py' -v`: passed (3 tests).
- `python -m py_compile` on all modified packaging/QEMU launchers: passed.
- `python scripts/package-system-apps.py --version 0.8.0`: passed; 11 signed packages emitted.
- Production kernel Clippy with all signed image environment variables,
  WovenShell, package service, and trusted key supplied:
  `cargo clippy --offline -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed.
- `python scripts/test-system-app-catalog-qemu.py --cpus 1`: passed, exit 33,
  eleven-app marker.
- `python scripts/test-system-app-catalog-qemu.py --cpus 2`: passed, exit 33,
  eleven-app marker.
- `python scripts/test-system-app-catalog-qemu.py --cpus 4`: passed, exit 33,
  eleven-app marker.
- `git diff --check`: passed.

Evidence is preserved in `audit-artifacts/system-app-catalog-{1,2,4}cpu`.
No physical disk was opened, partitioned, formatted, or modified.
---

## Source: docs/audit-stage31-driver-restart-2026-10-22.md

# Stage 31 driver-restart slice â€” 2026-10-22

## Scope

The Stage 13.2 PCI lifecycle gate now exercises a real discovered QEMU PCI
function through bind, interrupt/BAR ownership, ordered unbind, a fresh bind
with a new owner transaction, a second teardown, and BAR-lease verification.
Generation-safe topology rescan still follows the second teardown.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings` â€” passed.
- `python scripts/test-stage10-runtime.py --stage 13.2 --cpus 1` â€” passed, exit 33.
- Serial markers include `driver restart bind=true`,
  `driver restart teardown=true leases=0`, and the final Stage 13.2 pass.
- Evidence: `audit-artifacts/stage13.2-1cpu-*/serial.log`.

## Limits

This qualifies explicit PCI driver rebind/teardown and generation safety in
QEMU. It does not yet implement automatic driver-crash supervision, physical
PCI hotplug certification, or native hardware qualification.
---

## Source: docs/audit-stage31-low-resource-2026-10-22.md

# Stage 31 low-resource filesystem evidence â€” 2026-10-22

Added `full_data_region_rejects_write_without_partial_commit` to
`tests/wovenfs_crash.rs`. The test formats a 48-sector volume with eight
inodes, fills the available data region, attempts a two-sector write when only
one block remains, and requires `Error::NoSpace`. It then remounts the image,
verifies the previously committed file byte-for-byte, confirms the failed file
has size zero, and runs the filesystem consistency check.

Command:

```text
cargo test --test wovenfs_crash
```

Result: PASS, 16 tests. This is deterministic host coverage of WovenFS
disk-full atomicity and consistency. It does not yet qualify whole-kernel low
RAM behavior, block-device failure injection, or physical media exhaustion.
---

## Source: docs/audit-stage31-low-resource-device-2026-10-22.md

# Stage 31 low-resource device-path evidence â€” 2026-10-22

The existing Stage 15.2 ATA/WovenFS QEMU gate was run against disposable 4
MiB scratch images:

```text
python scripts/test-wovenfs-device.py --cpus 1 --size-mib 4 --timeout 300
python scripts/test-wovenfs-device.py --cpus 4 --size-mib 4 --timeout 300
```

Both runs passed with exit 33. The gate completed device write/readback,
formatting, durable file creation, remount verification, VFS namespace and
authority checks, unlink, and the final WovenFS marker. Evidence is retained
under:

- `target/wovenfs-device-scratch-1cpu-1792676503835808200`
- `target/wovenfs-device-scratch-4cpu-1792676540559963400`

This is stronger than the host-only disk-full test because it exercises the
ATA path and QEMU device. The image is still disposable emulation; physical
media exhaustion, block-device fault injection, and whole-kernel low-RAM
qualification remain open.
---

## Source: docs/audit-stage31-network-loss-2026-10-22.md

# Stage 31 network-loss slice â€” 2026-10-22

`scripts/test-network-qemu.py` now supports `--link-loss`. The harness starts
QEMU with an opt-in QMP control channel, waits for the normal DHCP/DNS/ICMP and
UDP proof, sends `set_link` down for `net0`, sends `set_link` up, and then
requires the host TCP echo, the guest network markers, the SMP/TLB marker, and
exit 33. QMP errors and all existing guest failure markers are fatal.

Passing commands:

```text
python scripts/test-network-qemu.py --cpus 1 --timeout 300 --link-loss
python scripts/test-network-qemu.py --cpus 4 --timeout 300 --link-loss
python scripts/test-network-qemu.py --cpus 8 --timeout 600 --link-loss
python scripts/test-network-qemu.py --cpus 16 --timeout 900 --link-loss
```

Evidence is retained in:

- `target/network-regression-1-debug/serial.log`
- `target/network-regression-4-debug/serial.log`
- `target/network-regression-8-debug/serial.log`
- `target/network-regression-16-debug/serial.log`

This slice proves recovery from an emulated virtio link flap. It does not
qualify physical NIC loss, packet corruption, Wi-Fi recovery, driver restart,
or the complete Stage 31 torture suite.
---

## Source: docs/audit-stage31-packet-corruption-2026-10-22.md

# Stage 31 packet-corruption boundary audit â€” 2026-10-22

Added `scripts/test-stage31-packet-corruption.py`, which runs the real network
QEMU harness with the Stage 14.2 ICMPv6 checksum and ingress-hardening gates.
Malformed checksum input must be rejected without poisoning the network state.

Verification passed on 1/2/4 CPUs. Each run also completed DHCP, DNS, ICMP,
host UDP/TCP round trips, SMP/TLB markers, and exit 33. The decisive guest
marker is `[S14.2E] WovenNet ICMPv6 checksum boundary: PASSED`.

Evidence is retained under
`audit-artifacts/stage31-packet-corruption-{1,2,4}cpu`.

This is partial Stage 31 coverage: deterministic malformed-checksum rejection
is proven, but randomized corruption across the complete Ethernet/IP/TCP/UDP
receive paths is still open.
---

## Source: docs/audit-stage31-process-crash-2026-10-22.md

# Stage 31 process-crash containment audit â€” 2026-10-22

Added a feature-gated Ring-3 probe that writes to an unmapped user address.
The kernel's user-fault path must terminate only that process, preserve the
kernel, and expose the signed fault status to the waiter.

Verification:

- `cargo check -p wovenhat-kernel --features stage31-process-crash-test --target x86_64-unknown-none`: passed.
- Warning-denying kernel Clippy for `stage31-process-crash-test`: passed.
- Stage 31 process-crash QEMU gate: passed on 1/2/4 CPUs, exit 33.
- Each serial log records a user page fault at `0x400000300000` and
  `[S31.PROCESS-CRASH] faulting process contained and reaped: PASSED status=-11`.

Evidence is retained under
`target/stage31-process-crash-{1,2,4}cpu`.

This covers isolated process-fault containment only. Driver crashes,
service restart, packet corruption, sudden shutdown, and physical hardware
faults remain separate Stage 31 scenarios.
---

## Source: docs/audit-stage31-scalability-2026-10-22.md

# Stage 31 supported SMP scalability audit â€” 2026-10-22

The bounded Stage 31 supported-suite matrix was executed after the 16-CPU
control-plane and capacity change. The five existing suites passed at every
requested profile: 1, 2, 4, 8, and 16 CPUs (25 runs total, exit 33).

The 8/16 evidence includes:

- `target/memory-regression-8-debug/serial.log`
- `target/memory-regression-16-debug/serial.log`
- `target/storage-regression-8-debug/serial.log`
- `target/storage-regression-16-debug/serial.log`
- `target/network-regression-8-debug/serial.log`
- `target/network-regression-16-debug/serial.log`
- `audit-artifacts/woven-security-8cpu/`
- `audit-artifacts/woven-security-16cpu/`
- `target/shell-regression/diagnostic-8cpu-1792672468010623100/serial.log`
- `target/shell-regression/diagnostic-16cpu-1792673058433796700/serial.log`

Representative commands were:

```text
python scripts/test-memory-qemu.py --cpus 8 --timeout 300
python scripts/test-memory-qemu.py --cpus 16 --timeout 300
python scripts/test-storage-qemu.py --cpus 8 --timeout 300
python scripts/test-storage-qemu.py --cpus 16 --timeout 900
python scripts/test-network-qemu.py --cpus 8 --timeout 300
python scripts/test-network-qemu.py --cpus 16 --timeout 900
python scripts/test-security-qemu.py --cpus 8 --timeout 300
python scripts/test-security-qemu.py --cpus 16 --timeout 900
python scripts/test-shell-qemu.py --cpus 8
python scripts/test-shell-qemu.py --cpus 16
```

The first 16-CPU storage run exceeded 300 seconds after making progress and
was retained as `target/storage-regression-16-debug/serial.log`; the rerun
with 900 seconds passed. The first 16-CPU shell run exposed fixed kernel
preemption-test budgets and was retained under the corresponding earlier
`diagnostic-16cpu-*` directory. Scaling those diagnostic budgets, without
removing assertions or reducing workload rounds, produced the passing rerun.

Post-change checks also passed:

- freestanding kernel Clippy with `--no-default-features -- -D warnings`;
- host `cargo clippy --all-targets -- -D warnings`;
- `cargo test -p libwoven` (119 tests passed).

This audit accepts the supported SMP matrix, not the complete Stage 31 torture
or production-readiness milestone. Randomized fault injection, low-resource
and loss recovery, service/driver restart, sudden-shutdown recovery,
repeated-boot acceptance, and native hardware qualification remain open.
---

## Source: docs/audit-stage31-service-lifecycle-2026-10-22.md

# Stage 31 service-lifecycle repeat evidence â€” 2026-10-22

The Stage 31 repeated-runner now accepts `recovery` and invokes the existing
Stage 27.5 durable-recovery QEMU gate sequentially. Each run copies the
child output, result metadata, and corresponding Stage 27.5 evidence into its
own directory.

Command:

```text
python scripts/test-stage31-repeated-boots.py --suite recovery --cpus 1 --count 2 --timeout 300
```

Result: PASS. Both runs exited 0; the first took 40.5 seconds and the second
54.1 seconds. Evidence is retained in
`audit-artifacts/stage31-repeated/recovery-1cpu/run-0001` and `run-0002`.

This proves repeatable startup, durable-recovery execution, and bounded
teardown for this existing gate at 1 CPU. It does not implement or qualify a
generic crash supervisor, automatic service restart, physical power-loss
recovery, driver restart, packet corruption, or native hardware behavior.
The runner currently limits this suite to the Stage 27.5-supported 1/2/4 CPU
profiles.
---

## Source: docs/audit-stage31-service-restart-2026-10-22.md

# Stage 31 service-restart slice â€” 2026-10-22

## Scope

The signed `woven-update` service acceptance now exercises a complete
restart boundary: terminate and reap the first daemon, verify endpoint
cleanup, reload the signed image, spawn a fresh process, verify republished
ownership, rediscover a new capability handle, service a query, and tear the
second daemon down cleanly.

## Evidence

- `python scripts/test-stage27-5-update-service.py --cpus 1` â€” passed, exit
  33.
- Serial marker: `[S27.5] IPC daemon restart/republication: PASSED`.
- Evidence: `audit-artifacts/stage27.5-1cpu/serial.log`.
- `$env:WOVENHAT_UPDATE_IMAGE=target/woven-update.wapp; cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-5-test -- -D warnings` â€” passed.

## Limits

This is signed userspace-service restart qualification, not a general
supervisor, driver restart, automatic retry policy, sudden power-loss
recovery, or native hardware certification. Those require separate owner,
restart, and hardware evidence.

## 2026-10-23 bounded production supervisor increment

`kernel/src/service_supervisor.rs` now registers the production package,
update, and installer service PID generations. The kernel event loop detects
an exited child, reaps it through the owning parent, applies a bounded
100/200/400-tick retry backoff, reloads the verified image, and publishes a
replacement generation. Each service has at most three restart attempts;
stale results cannot replace a newer PID and budget exhaustion disables the
entry with a serial audit record.

Evidence:

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features
  system-apps -- -D warnings` passed using the existing packaged artifacts
  and trusted test key.
- The Windows host has no local OVMF image, so a live crash/restart QEMU
  execution was not claimed in this increment; CI must execute the packaged
  system-app acceptance before this slice is runtime-accepted.

The explicit PCI driver rebind slice remains separate and still lacks an
automatic driver-crash supervisor.
---

## Source: docs/audit-stage31-update-service-lifecycle-2026-10-22.md

# Stage 31 update-service lifecycle repeat evidence â€” 2026-10-22

The Stage 31 repeated-runner now accepts `update-service` and invokes the
existing signed Stage 27.5 `woven-update --service` QEMU gate sequentially.
Each iteration retains the service gate's serial, QEMU, and package evidence
under its own repeated-run directory.

Command:

```text
python scripts/test-stage31-repeated-boots.py --suite update-service --cpus 1 --count 2 --timeout 300
```

Result: PASS. The repeated 1-CPU runs both exited 0; the first took 87.9
seconds and the second 56.3 seconds. Additional one-run profiles passed at
2 CPUs (43.4 seconds) and 4 CPUs (47.6 seconds). Evidence is retained in
`audit-artifacts/stage31-repeated/update-service-1cpu/run-0001` and
`run-0002`, with the scalable profiles under
`update-service-2cpu/run-0001` and `update-service-4cpu/run-0001`.

This boundary is intended to prove repeatable signed service publication,
request handling, malformed-request rejection, and orderly teardown. It does
not implement automatic restart after a crashed service, driver restart,
physical power-loss recovery, or native hardware qualification.
---

## Source: docs/audit-stage6-2026-09-16.md

# Stage 6 SMP maturity validation â€” 2026-09-16

Stage 6 bounded SMP is accepted at its documented foundation boundary.

Validation completed with `python scripts/test-release.py`:

- kernel and host Clippy with `-D warnings` passed;
- all standalone Rust host tests passed, including the FAT32 and Unicode
  regressions;
- memory, storage, and live network suites passed on 1, 2, and 4 CPUs;
- legacy PIC fallback passed;
- 4-CPU release memory, storage, and network suites passed;
- release build and shell/SMP smoke passed.

The release report is retained at
`target/release-validation/results.json`. Every QEMU gate required exit status
33 and the Stage 6 scheduler, barrier, migration, reschedule-IPI, timer
preemption, stale-translation, and acknowledged-shootdown markers.

The bounded contract remains explicit: general userspace migration, concurrent
filesystem/network/device-I/O service execution, NUMA-aware page allocation,
non-contiguous hotplug, APIC-ID-above-255 hardware qualification, and
complete lock graph coverage and priority inheritance are deferred work. Bounded contiguous-prefix AP offline/re-online control is covered by the
[hotplug audit](audit-stage6-hotplug-2026-09-16.md). CPU-domain placement and
x2APIC MSR transport are covered by the follow-up
[NUMA/x2APIC audit](audit-stage6-numa-2026-09-16.md).

Interrupt-safe worker locks now enforce bounded per-CPU rank/nesting checks;
the focused evidence is retained in the
[lock-order audit](audit-stage6-lock-order-2026-09-16.md).
---

## Source: docs/audit-stage6-dma-2026-09-16.md

# Stage 6 VirtIO DMA audit â€” 2026-09-16

VirtIO network initialization now obtains a physically contiguous DMA arena
from the frame allocator instead of relying on the bootloader's static BSS
layout. The arena contains both legacy split-virtqueue regions and packet
buffers. The direct physical-memory mapping provides the kernel virtual
address, while the physical base is programmed into the device descriptors.

The allocator reserves the whole run from one usable memory range and applies
the current NUMA-domain preference. If initialization fails after reservation,
an ownership guard returns every reserved frame to the allocator. A
`network-test` image reports transport initialization errors explicitly and
halts, so a missing DMA capability cannot be mistaken for a successful network
qualification.

Validation completed:

```powershell
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features network-test -- -D warnings
python scripts/test-network-qemu.py --cpus 1
python scripts/test-release.py
```

The focused 1-CPU live DHCP/DNS/ICMP/UDP/TCP gate passed. The complete release
matrix passed network and storage on 1, 2, and 4 CPUs in debug mode, plus the
4-CPU release network gate. Repeated Stage 6 hotplug tests also passed on 2
and 4 CPUs after the allocator change.

This closes the software-side contiguous-DMA allocation gap for the current
VirtIO network path. Broader DMA-capable drivers, IOMMU isolation, and
hardware throughput qualification remain separate production requirements.
---

## Source: docs/audit-stage6-frame-reclamation-2026-09-17.md

# Stage 6 frame reclamation audit â€” 2026-09-17

The physical allocator previously kept only 4,096 returned frames in a static
table. A valid `deallocate_frame` call after that table filled returned false,
so runtime address-space teardown could permanently lose reusable RAM. The
table was adequate for a single 8 MiB heap-map rollback, but it was not a
general reclamation policy.

Each usable physical range now reserves its own page-aligned allocation bitmap
from the start of that same range. The bitmap pages never enter the frame
pool, and allocator statistics count only the remaining allocatable frames.
Every successful single-frame or contiguous allocation marks its bits. A
return checks range ownership, that the frame was handed out below the
allocation cursor, and that its bit is still set. This rejects a double free
even when the returned frame no longer fits the hot cache.

The existing 4,096-entry table remains a fast reuse cache. When it fills,
further returns clear their bitmap bits and increment an overflow count;
allocation scans the used portion of the affected ranges after draining the
cache. Each range retains the lowest overflow address as a scan hint, avoiding
a repeated search from the start. Domain preference is preserved for both
cached and overflow frames.
The allocator no longer fails a valid return solely because the cache filled.

The QEMU boot probe allocates and returns 4,352 frames, verifies that the
overflow path was reached, reacquires the same frame set, and rejects a
duplicate return. The memory gate requires its serial marker. This proves
the overflow path on the 256 MiB QEMU profiles; high-RAM scan latency,
low-memory behavior, and physical NUMA placement still need qualification.

The final source passed all 33 checks in
`target/release-validation/results.json`: warning-denying kernel and host
Clippy, host tests, 1/2/4-CPU QEMU debug boot/storage/network suites, legacy
PIC, four-CPU release suites, release build, and shell smoke. Dedicated
twice-cycled hotplug gates also passed, with evidence under
`audit-artifacts/stage6-hotplug-2cpu-1789636337456573500/` and
`audit-artifacts/stage6-hotplug-4cpu-1789636368058079500/`.
---

## Source: docs/audit-stage6-heap-2026-09-17.md

# Stage 6 heap capacity and mapping audit â€” 2026-09-17

The previous kernel heap eagerly mapped 256 KiB, tracked at most 512 live
allocations, and kept at most 128 free intervals. Reusing an aligned free
block discarded its leading padding. An allocation attempted after the live
table filled could consume a free block before failing. Deallocation silently
dropped an interval when the free table filled. These paths could lose usable
heap memory long before physical RAM was exhausted.

The heap now maps one sixteenth of remaining physical RAM at boot, clamped to
256 KiBâ€“8 MiB, and tracks 2,048 live allocations. The current 256 MiB QEMU
boot reports `[HEAP] mapped=8388608 bytes`. Allocation checks metadata
capacity before removing a free block. Each live entry retains its complete
reserved span, including alignment padding; deallocation coalesces adjacent
intervals and rewinds the bump pointer when the highest span becomes free.
The free metadata table holds 2,049 intervals, the maximum for a coalesced
contiguous arena with at most 2,048 live allocations. Reported free bytes
include both holes and the untouched tail.

`paging::map_range` now releases frames and unmaps pages it added when a
later page in the same call fails. A boot probe pre-maps the second page of a
two-page range, verifies that the combined mapping fails with `AlreadyMapped`,
then checks the first page was rolled back while the second stayed mapped.
The ordinary map/write/unmap boot probe now returns its test frame as well.
The reclaimed-frame hot cache holds 4,096 entries, enough to return every data
frame from the maximum 8 MiB heap map plus bootstrap page-table and probe
frames during this early-boot transaction. A later
[frame-reclamation follow-up](audit-stage6-frame-reclamation-2026-09-17.md)
adds per-region allocation bitmaps, so general runtime returns are no longer
bounded by the hot cache.
The rollback probe covers leaf mappings and their data frames; it does not
prove that a failed page-table allocation returns every intermediate table
frame. That path remains part of the broader memory-allocator audit.

`tests/heap.rs` runs three production-state tests for alignment recovery,
fragmented coalescing, and live-table exhaustion without free-space loss.
The QEMU memory gate exercises the RAM-scaled heap, a 300 KiB allocation,
and the mapping rollback probe. The final source passed
`python scripts/test-release.py`: warning-denying kernel/host Clippy, host
tests, 1/2/4-CPU memory/storage/network, legacy PIC, 4-CPU release suites,
release build, and shell/PS/2 smoke. Dedicated twice-cycled 2/4-CPU AP
hotplug also passed. Logs are retained in `target/release-validation` and the
dated `audit-artifacts/stage6-hotplug-{2,4}cpu-*` directories.

This is still a bounded heap. Runtime growth above the eagerly mapped size
cannot call the rank-10 pager from arbitrary allocation contexts that may
already hold higher-ranked locks. The fixed live-object table, allocator
performance under broad multicore workloads, and low-memory hardware
qualification remain open.

The later [heap-metadata follow-up](audit-stage6-heap-metadata-2026-09-17.md)
removes both fixed metadata tables. The 2,048-object limit described above is
historical; mapped heap capacity and runtime growth remain separate limits.
---

## Source: docs/audit-stage6-heap-growth-2026-09-17.md

# Stage 6 runtime heap growth audit â€” 2026-09-17

The heap previously mapped at most 8 MiB during boot and could not expand.
The global allocator can run while paging, frame, process, or device guards
are held, so calling the rank-10 pager while the rank-50 heap guard is held
would invert the lock order and can deadlock under SMP.

Boot mapping remains RAM-scaled and capped at 8 MiB. Runtime growth now maps
256 KiB chunks, with one growth owner at a time. The owner reads the current
boundary under the heap guard, releases it, maps the next chunk through the
pager, and publishes the new boundary only after mapping succeeds. The owner
keeps local task preemption deferred while growing, while IRQs stay available
between pager critical sections for remote TLB acknowledgement. A failed
chunk leaves the previously published heap intact; completed earlier chunks
remain usable. Growth stays within a 64 GiB virtual window in the original
heap P4 slot shared by already-created user address spaces.

A failed allocation can grow synchronously and retry when the caller has no
ranked guard and local IRQs are enabled. Allocations under guards still use
only mapped pages. Successful allocations request proactive growth when the
untouched tail or total free space falls below 1 MiB; the BSP idle task maps
ahead in bounded batches. Total free bytes are maintained incrementally, so
this pressure check and heap statistics do not traverse the free list.

The host heap test checks that a failed large allocation leaves the original
arena unchanged, then succeeds after a mapped extension while an older live
payload remains intact. The QEMU memory gate allocates beyond the initial
8 MiB boundary after SMP startup, checks that the mapping grew, validates
the payload and exact free-byte recovery, then exercises reserve maintenance
under pressure. Its serial record includes the final mapped-byte count.
The 256 MiB QEMU profiles grew from the boot mapping of 8,388,608 bytes to
10,223,616 bytes during that probe.

The final source passed all 33 checks in
`target/release-validation/results.json`: warning-denying kernel/host Clippy,
host tests, 1/2/4-CPU debug QEMU memory/storage/network, legacy PIC, 4-CPU
release gates, release build, and shell smoke. Dedicated twice-cycled hotplug
gates passed with evidence under
`audit-artifacts/stage6-hotplug-2cpu-1789638980824115500/` and
`audit-artifacts/stage6-hotplug-4cpu-1789639011999404000/`.

The synchronous path requires a safe paging context. A sudden allocation
under a higher-ranked guard can still fail if its demand exceeds mapped
headroom before idle maintenance runs. Low-memory rollback, broad concurrent
allocator throughput, and physical NUMA placement remain open qualification
work; runtime capacity also depends on available physical frames.
---

## Source: docs/audit-stage6-heap-metadata-2026-09-17.md

# Stage 6 heap metadata follow-up â€” 2026-09-17

The kernel heap previously stored live allocations in 2,048 static slots and
free intervals in a paired 2,049-slot table. A small allocation could fail
when those slots filled even with mapped heap bytes still available. The
number of allocatable objects was therefore an independent capacity limit.

The heap now places an allocation header immediately before each aligned
payload. The header records the complete reserved span, including alignment
padding and any short remainder absorbed from a free interval. Freed spans
carry their next pointer and size in their own mapped bytes. The free list is
address ordered, coalesces neighboring spans, and rewinds the bump cursor
when the highest span becomes free. Neither live-object nor free-interval
tracking consumes a fixed external slot table. The rank-50 IRQ-safe heap
guard remains the sole metadata writer and still never calls the rank-10
pager while held.

The host harness now backs the production heap state machine with a real
page-aligned arena. It verifies more than 2,048 simultaneous live objects,
alignment padding recovery, 4,096-way fragmentation/coalescing, and a
deterministic mixed-size/alignment workload that checks every live payload
before release. The QEMU memory gate requires a boot probe with 4,096
simultaneous `Box<u64>` objects and exact allocation/free-byte recovery.

The final source passed all 33 release checks in
`target/release-validation/results.json`, including warning-denying Clippy,
host tests, 1/2/4-CPU debug QEMU memory/storage/network gates, legacy PIC,
four-CPU release gates, and shell smoke. Dedicated twice-cycled hotplug gates
passed with evidence in
`audit-artifacts/stage6-hotplug-2cpu-1789637845298615200/` and
`audit-artifacts/stage6-hotplug-4cpu-1789637873756294400/`.

The mapped heap remains capped at its boot-time 8 MiB maximum, and free-list
search remains linear in the number of holes. Runtime mapping growth, low-RAM
behavior, and broader multicore allocator throughput remain open.

A later [runtime-growth follow-up](audit-stage6-heap-growth-2026-09-17.md)
removes the boot-time 8 MiB mapping ceiling in safe paging contexts. The
boot-time ceiling above describes the state of this earlier metadata patch.
---

## Source: docs/audit-stage6-hotplug-2026-09-16.md

# Stage 6 bounded CPU-offline audit â€” 2026-09-16

The Stage 6 SMP control plane now supports a bounded offline/re-online
transition for the highest-numbered online AP. The request is accepted only
for a contiguous online prefix and only after the scheduler evacuates every
non-running task that has a legal online destination. The AP then reaches its
CPU-owned idle checkpoint, masks its local timer, parks with interrupts
disabled, and publishes the reduced online mask and count before the requester
acknowledges completion.

The transition is deliberately conservative. Running tasks are never moved;
dead tasks are transferred to the BSP for reclamation; pinned or otherwise
non-migratable work rejects the request. Cancellation and rejection states are
explicit and bounded. Re-online reuses the parked AP bootstrap context and
reclaims the prior offline-idle slot before restoring the local timer. The
focused gate repeats this lifecycle twice to prove bounded slot reuse.

Validation command:

```powershell
python scripts/test-stage6-hotplug.py
python scripts/test-stage6-hotplug.py --cpus 4
```

Result: PASS on the development QEMU profiles (`-smp 2` and `-smp 4`, exit
status 33). Each run proves offline and re-online with the corresponding
online-prefix markers:

- `[SMP] online=2 expected=2`
- `[SMP] topology/NUMA affinity: PASSED`
- `[S6.HOTPLUG] offline AP: PASSED online=1 mask=0x1`
- `[S6.HOTPLUG] lifecycle: PASSED cycles=2 online=2 mask=0x3` (2 CPU run)
- `[S6.HOTPLUG] lifecycle: PASSED cycles=2 online=4 mask=0xf` (4 CPU run)

The latest rerun after IPC, WovenGuard, and process-spawn lock conversion
retains logs under
`audit-artifacts/stage6-hotplug-2cpu-1789623218303485500/` and
`audit-artifacts/stage6-hotplug-4cpu-1789623244801570400/`.

Warning-denying freestanding Clippy also passed with the hotplug feature:

```powershell
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage6-hotplug-test -- -D warnings
```

This closes the software gap for the bounded contiguous-prefix AP
offline/re-online lifecycle. Non-contiguous hotplug topology, APIC-ID-above-255
hardware, and multi-node NUMA qualification remain separate production
requirements.

The 2026-09-17 [non-contiguous hotplug follow-up](audit-stage6-noncontiguous-hotplug-2026-09-17.md)
supersedes the contiguous-prefix software restriction above. Its QEMU gate
parks and restores CPU 1 while CPU 3 remains active; physical hardware
qualification is still open.
---

## Source: docs/audit-stage6-lock-order-2026-09-16.md

# Stage 6 interrupt-safe lock-order audit â€” 2026-09-16

`irq_lock::IrqMutex` now records a bounded held-lock stack for each logical
CPU. A lock may declare a monotonic rank; lower-ranked nesting is rejected
before spinning. Recursive acquisition, more than eight nested locks, and
non-LIFO guard release are deterministic failures. Rank zero remains an
explicit unranked compatibility mode for independent single-lock domains.

The audited asynchronous operation path declares the production order:

```text
async operation table (10) -> completion-port allocation (10) -> port slot (20)
```

The scheduler and memory-management domains now declare the complementary
order:

```text
scheduler (10) -> process table (20)
paging (10) -> COW table (30) -> physical-frame allocator (40)
pager queue/state (10) -> scheduler (10)
teardown registries (notifications, threads, async events) (10)
pipe table (10) -> scheduler wake/block handoff
IPC namespace (10) -> WovenGuard lineage (10) -> paging/frame release (10/40)
diagnostic/audit ledger (40)
ATA device and isolated catalog registries (10)
```

The tracker is active in the freestanding kernel. Host test doubles implement
the same `with_rank` constructor so host behavior cannot silently diverge from
the production API.

Validation completed:

```powershell
cargo test --test irq_lock
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings
python scripts/test-release.py
```

The interrupt-lock tests passed (2/2), warning-denying freestanding Clippy
passed, and the complete release matrix passed, including async runtime,
memory/storage/network 1/2/4-CPU gates, legacy PIC, release gates, and shell/
SMP smoke. The Stage 6 hotplug lifecycle also passed twice on both 2- and
4-CPU QEMU profiles after the tracker was enabled.

During release qualification the tracker caught a termination-path inversion
where completion-port ownership was released while `PROCESS_TABLE` was held.
The termination path now marks the process and takes its file table under the
rank-20 guard, then performs completion, event, notification, VFS, and pipe
teardown after the guard is dropped. The complete release matrix was rerun
after this fix and passed. The same ownership-transfer rule now covers the
self-exit and wait/reap paths: VFS references and IPC endpoint unregistering
occur only after the process-table guard has been released.

The 2026-09-17 follow-up converted the IPC namespace and WovenGuard lineage
registries to the same ranked IRQ-safe mutex. Qualification then caught and
fixed IPC registration under `PROCESS_TABLE` during userspace spawn and fork;
all registration rollback paths now release the process-table guard before
touching IPC. The complete release matrix and both 2/4-CPU hotplug cycles were
rerun after that correction and passed.

After the bounded audit/ATA/snapshot/WovenFS/driver/PCI registry conversion,
the complete release matrix was rerun on 2026-09-17. All host tests, 1/2/4-CPU
debug suites, legacy PIC, 4-CPU release suites, release build, and shell smoke
passed; `target/release-validation/results.json` retains the result.

The next follow-up placed the file-frame cache under a rank-10 IRQ mutex and
added call-site lines to non-LIFO lock diagnostics. Fork and descriptor
duplication now retain VFS/pipe references outside `PROCESS_TABLE` and
revalidate the source descriptor before publication. Open-file descriptions
and anonymous pipes now carry non-wrapping slot epochs, so a close/reuse race
cannot turn a stale kernel handle into a reference to a different object.
Pipe creation also reserves both descriptor slots before publishing either
end and releases the new pipe on failure.

The final follow-up passed `python scripts/test-release.py`: warning-denying
kernel/host Clippy, host tests, memory/storage/network QEMU suites on 1/2/4
CPUs, legacy PIC, 4-CPU release suites, release build, and shell/SMP smoke.
`target/release-validation/results.json` records the gate results. Dedicated
hotplug cycles passed twice on both 2 and 4 CPUs; their serial evidence is in
`audit-artifacts/stage6-hotplug-*`.

A trial VFS IRQ-lock conversion exposed an existing slow-I/O boundary:
disk-backed node materialization and descriptor reads ran while VFS
registry/open-description guards were held. The conversion was withdrawn
until the I/O was split into an unlocked transaction. Disk reads now snapshot
the node, backing path, version, and shared offset, release both guards for
I/O, then revalidate before returning bytes or advancing the offset. Lazy
materialization does the same for unlink, shared mappings, and writes. Bounded
retries reject conflicting changes and generation checks reject slot reuse.
Renames advance node versions, and prefix callbacks run after a bounded path
snapshot is taken. The VFS registry and open-description table now use rank-10
IRQ mutexes. The file-size path explicitly drops its nested registry guard
before the open-description guard.

This closes the bounded lock-order coverage gap for the audited scheduler,
process, paging, COW, frame-allocation, pager, async-operation, completion-port,
file/block/network worker, pipe, IPC namespace, WovenGuard lineage, audit, ATA,
file-frame cache, VFS, and isolated catalog domains. Other subsystem locks
still use compatibility mutexes; broader filesystem mutation transactions and
priority inheritance remain separate Stage 6 work.

The subsequent pipe wait audit found a wake/block race in `wake_task` plus
`block_current`: a peer could wake a still-running task immediately before it
blocked, losing the only wakeup. Pipe readers and writers now use the
scheduler's latched `signal_event`/`wait_for_event` handshake. A full waiter
table returns `Full` instead of silently dropping a waiter that would then
sleep forever. The pipe boot self-test covers duplicate registration and the
bounded waiter capacity.
The full release matrix passed after this change, including warning-denying
lint, host tests, 1/2/4-CPU memory/storage/network gates, legacy PIC, 4-CPU
release gates, and shell smoke. Dedicated 2/4-CPU hotplug passed twice-cycled
offline/re-online tests again.

After the VFS I/O split and rank-10 conversion, `python scripts/test-release.py`
passed warning-denying lint, host tests, 1/2/4-CPU memory/storage/network
gates, legacy PIC, 4-CPU release gates, release build, and shell smoke.
Dedicated 2/4-CPU twice-cycled hotplug also passed. One earlier focused
1-CPU storage boot stopped at the FAT32 mkdir mutation check while the VFS
refactor was in progress; a subsequent run, the full release matrix, and two
additional focused runs passed. The storage self-test now logs the underlying
persist error if that intermittent failure recurs. Five more sequential
1-CPU FAT32 storage boots passed with automatic failure-log preservation.
The original failed serial log was overwritten before it could be copied, so
the cause of that one transient failure remains unproven; it is not treated
as evidence that unrestricted concurrent filesystem mutation is complete.

The heap metadata lock is now rank-50 IRQ-safe. Boot-time heap page mapping
occurs before that guard is taken, so runtime allocation never nests a
lower-ranked paging lock beneath the heap. The full release matrix passed
after this change, as did twice-cycled 2/4-CPU hotplug. The 256 KiB heap
capacity and fixed metadata tables remain separate scalability limits.

The bounded device, keyboard decoder, journal-intent, swap-state,
mount-record, and key-vault tables now use IRQ mutexes. Swap disk transfers
remain outside its rank-40 state guard. The keyboard wrapper captures the
caller's interrupt state before taking its rank-10 decoder guard; otherwise
the guard itself would make ordinary IRQ-driven input look like early-boot
legacy polling. Focused QEMU checks for Stage 1-5 journal, Stage 12.3 key
vault, Stage 12.5 mount records, and normal 4-CPU PS/2 shell input passed.
At this checkpoint, terminal, shell, and other compatibility mutexes still
needed separate slow-path and lock-order review; the terminal and shell
follow-up is recorded below.
The corrected batch passed `python scripts/test-release.py`, including the
1/2/4-CPU boot, FAT32, network, legacy PIC, release, and PS/2 shell gates.
Dedicated twice-cycled 2/4-CPU hotplug passed again. The feature-specific
journal, key-vault, and mount-record QEMU probes also passed.

The clean FAT32 page cache no longer holds its mutex while loading a page
from ATA. Cache hits and 4 KiB publication use a rank-20 IRQ guard; misses
perform the physical read after dropping it. Invalidation advances an epoch,
and an in-flight load refuses to publish if that epoch changed. A focused
host test covers this race. The complete release matrix and twice-cycled
2/4-CPU hotplug passed after the split. Concurrent FAT32 mutation still
lacks a single cross-layer transaction spanning disk and VFS publication,
so unrestricted filesystem concurrency remains open.

The terminal's previous plain spin mutex would keep the timer and device IRQs
masked for an entire framebuffer scroll or clear if simply changed to an IRQ
mutex. It now uses a rank-10 `PreemptMutex`: a per-CPU guard suppresses local
timer task switches while the lock is held, but leaves IRQs live. The per-CPU
rank stack is updated with IRQs briefly masked so an interrupt cannot see a
partial update. Terminal syscall writes use the existing I/O-depth wrapper
to enable IRQs through rendering and restore entry IF afterward. A boot-time
probe waits for two timer ticks under the local guard and checks task ID,
depth, and IF restoration. The shell's short current-directory lock is now
rank-10 IRQ-safe; callers copy cwd before allocation or console output.
Neither guard spans a deliberate block or task switch. Cross-layer lock
qualification and priority inheritance remain open Stage 6 work.

The final source passed `python scripts/test-release.py` (warning-denying
kernel/host Clippy, host tests, 1/2/4-CPU memory/storage/network, legacy PIC,
4-CPU release, and normal shell/PS/2 smoke). Twice-cycled 2/4-CPU AP
offline/re-online probes also passed. Evidence is in
`target/release-validation/results.json` and the dated
`audit-artifacts/stage6-hotplug-{2,4}cpu-*` directories.

The remaining unranked kernel IRQ-mutex instances were the socket runtime
and VirtIO-net transport. They now declare ranks 20 and 30, respectively,
matching the runtime-to-transport packet path. The default unranked
constructor was removed from kernel code; the host guard test uses an explicit
rank-zero fixture to continue checking nested IF restoration. This makes all
current kernel IRQ-mutex construction sites explicit about rank. It does not
replace stress or proof for every cross-subsystem lock path, and priority
inheritance remains open.

The ranked network change passed the full `python scripts/test-release.py`
matrix, including live UDP/TCP exchange at 1, 2, and 4 CPUs and four-CPU
release mode. Dedicated twice-cycled 2/4-CPU AP hotplug probes passed again.
The report and dated hotplug logs are retained under `target/release-validation`
and `audit-artifacts`.
---

## Source: docs/audit-stage6-noncontiguous-hotplug-2026-09-17.md

# Stage 6 non-contiguous CPU lifecycle audit â€” 2026-09-17

The earlier hotplug path accepted only the highest online AP and treated
`online_count()` as the last usable CPU index. Removing a middle CPU would
have omitted higher online CPUs from scheduler placement and TLB shootdowns,
and would have waited for an offline CPU's shootdown acknowledgement.

The online population and mask are now separate concepts. A parked AP keeps
its logical slot, APIC identity, and bootstrap stack. The control plane
serializes requests, accepts any online non-BSP AP for removal, decrements
the population on publication, and later permits that same slot to rejoin.
The AP publishes mask changes under the TLB-shootdown serialization lock;
shootdowns snapshot the mask and wait only for CPUs in that snapshot.
Scheduler load placement, explicit migration, kernel spawning, and affinity
selection iterate actual online slots rather than the population prefix.
A draining AP, and an AP between online publication and idle-task rebuild,
remain eligible for interrupt routing but are excluded from new scheduler
placement.

Offline preparation now validates every task before committing any owner or
affinity change. A pinned task or task without an allowed destination rejects
the request without partially moving earlier tasks. The CPU-owned idle
checkpoint repeats this validation before parking. The feature gate checks
the rejection invariant with a movable task followed by a pinned one.

The 4-CPU QEMU gate repeats CPU 1 offline/online twice. While CPU 1 is parked,
it checks `online=3` and mask `0xd`, rejects scheduling to CPU 1, accepts
rescheduling CPU 3, runs a new CPU-3 worker, and completes an acknowledged
TLB shootdown before restoring mask `0xf`. The existing 2/4-CPU tail-AP
lifecycle remains part of the same gate. Dedicated test logs are retained in
dated `audit-artifacts/stage6-hotplug-{2,4}cpu-*` directories.
The final source passed warning-denying freestanding Clippy with the hotplug
feature, `python scripts/test-release.py` (host tests, 1/2/4-CPU debug
memory/storage/network, legacy PIC, 4-CPU release gates, and shell smoke),
and the dedicated twice-cycled 2/4-CPU hotplug gates. The release report is
`target/release-validation/results.json`; the final dedicated logs are under
`audit-artifacts/stage6-hotplug-2cpu-1789634636887171200/` and
`audit-artifacts/stage6-hotplug-4cpu-1789634661181304400/`.

This is software qualification in the QEMU topology. Physical CPU removal
and return with a non-contiguous topology, APIC IDs above 255, and multi-node
NUMA placement are still hardware-only gates. Concurrent hotplug requests
are rejected while one request is active; broad stress of request
cancellation, concurrent process creation, and device I/O during a real
hardware transition remains open.

## Request cancellation and retry follow-up

An AP now claims an offline request with `1 -> 2` and an online request with
`6 -> 8`. A requester may cancel only the unclaimed state, atomically restoring
the prior stable online state or offline state `5`. If the AP has claimed the
operation, the requester waits for success or a terminal rejection rather
than reporting a cancelled request while the AP can still change topology.
The AP publishes rejection only after restoring its interrupt state. The
requester then rearms the stable state, so both operations can be retried.
If a claimed transition never reaches a terminal state within the second
deadline, the hotplug control plane stays locked against new requests; this
contains the uncertainty but requires operator diagnosis of the failed AP.

The QEMU gate uses feature-only hooks to force an AP rejection of each
operation and to hold each request unclaimed through its timeout. It checks
the AP actually visited both rejection hooks and both hold hooks, verifies
the online count/mask stayed stable, and successfully retries offline and
online operations afterward. These probes run on both 2- and 4-CPU profiles
before the original tail and middle-CPU lifecycle cycles. The focused logs
include `[S6.HOTPLUG] rejection recovery: PASSED`, the state-1 and state-6
timeout records, and `[S6.HOTPLUG] timeout recovery: PASSED`.
The follow-up source passed all 33 checks in
`target/release-validation/results.json`, including warning-denying Clippy,
host tests, 1/2/4-CPU debug QEMU gates, release QEMU gates, and shell smoke.
The focused follow-up evidence is under
`audit-artifacts/stage6-hotplug-2cpu-1789635217388184700/` and
`audit-artifacts/stage6-hotplug-4cpu-1789635182453579800/`.

This covers deterministic cancellation and retry under QEMU. Concurrent
process creation, device I/O, and physical hardware faults during a claimed
transition still require stress and hardware qualification.
---

## Source: docs/audit-stage6-numa-2026-09-16.md

# Stage 6 NUMA/topology placement audit - 2026-09-16

The scheduler now consumes bounded ACPI SRAT CPU-affinity records. Each MADT
processor entry is assigned a proximity domain when a matching SRAT processor
or x2APIC affinity entry is present. The SMP bring-up publishes the domain for
each online logical CPU. Initial placement and the conservative one-task
rebalancer prefer a same-domain CPU before comparing runnable load; systems
without SRAT retain the deterministic single-domain fallback.

SRAT memory-affinity records are retained as bounded ranges and applied to the
physical-frame allocator. Allocation prefers a range in the current CPU's
proximity domain, falls back to another usable range when that domain is
exhausted, and preserves each frame's domain when it is reclaimed.
The allocator self-test covers affinity-range mapping, and the runtime marker
`[MEMORY] frame allocator NUMA domains=1 regions=80` records the QEMU fallback.

The parser validates entry lengths, bounds, enabled flags, duplicate-safe CPU
matching, and the bounded entry count. ACPI's structural self-test covers a
synthetic local-APIC affinity record. The QEMU evidence below exercises the
runtime publication and scheduler path:

| CPUs | Result | Evidence |
| ---: | --- | --- |
| 1 | PASS | `target/memory-regression-1-debug/serial.log`, domains=1, mask=0x1 |
| 2 | PASS | `target/memory-regression-2-debug/serial.log`, domains=1, mask=0x3 |
| 4 | PASS | `target/memory-regression-4-debug/serial.log`, domains=1, mask=0xf, SRAT_MEM=0 |

Static validation passed with `cargo build -p wovenhat-kernel
--target x86_64-unknown-none` and warning-denying freestanding Clippy. The
QEMU matrix remains a single-domain topology, so multi-node latency and page
placement still require hardware or a NUMA-capable VM. Those are tracked as a
separate Stage 6 qualification rather than inferred from this fallback result.

The same SMP layer now supports x2APIC register access through MSR 0x800+
register windows, 64-bit destination ICR writes, and APIC-ID discovery from
the x2APIC ID register. It selects x2APIC when firmware has enabled it or
when the discovered topology contains an APIC ID above 255, and otherwise
retains xAPIC MMIO. The 1/2/4 CPU logs record `APIC mode: xAPIC`; the x2APIC
branch requires a host or VM that exposes x2APIC and is therefore not claimed
as hardware-qualified by this QEMU run.

After these changes, `python scripts/test-release.py` passed all release gates:
warning-denying kernel/host lint, standalone host regressions, 1/2/4 CPU debug
memory/storage/network, legacy-PIC fallback, 4 CPU release memory/storage/
network, release build, and shell/SMP smoke. The current freeze report is
`target/release-validation/results.json`.

Asynchronous block, file, and network workers now use the audited
`spawn_io_service` path. They can migrate between online CPUs while Ready;
their queue, VFS, ATA, and smoltcp state remains protected by global locks and
they retain no CPU-local ownership across event waits. A fresh 4-CPU memory
boot and live 4-CPU network round trip passed after this change. This closes
the scheduler-side service execution gap; unrestricted concurrent device
throughput still depends on DMA-capable drivers and remains open. The full
release matrix was rerun after this migration and passed; see
`target/release-validation/results.json`.
---

## Source: docs/audit-stage7-1-5-timeout-2026-09-16.md

# Stage 7.1.5 pager watchdog correction â€” 2026-09-16

The first repeated Stage 7.1.5 run stopped at 2-CPU run 14 after the AP-owned
pager workload had loaded `/bin/mmaptest` but before process exit was observed.
The preserved serial log contained no panic, fault, or failed subsystem marker;
the 200-tick watchdog expired while QEMU was under host contention.

The pager wait remains bounded at 200 timer ticks. The timeout still halts the
acceptance run and reports queued/completed pager work, so a real scheduler
deadlock cannot be hidden by an unbounded wait.

The termination lifecycle probe is now decoupled from the heavy mmap ELF
loader: it uses the existing blocking shell image, while the mmap image
continues through its own dedicated pager probe. A focused 50-run one-CPU
stress and 40-run two-CPU stress passed after that correction, with fresh 1/2/4
CPU memory suites also passing. Lifecycle trace points distinguish the
pre-kill check, kill publication, post-kill observation, and wait/reap
completion. The bounded wait now yields until the target is retired before
reaping, so a scheduler hand-off cannot be reported as a false wait failure.

The complete Stage 7.1.5 acceptance gate then passed on 2026-09-16: 50 one-CPU
memory runs, 100 two-CPU runs, 50 four-CPU runs, and live DHCP/DNS/ICMP plus
host-verified UDP/TCP round trips on 1, 2, and 4 CPUs. The retained serial logs
under `target/memory-regression-*` and `target/network-regression-*` are the
freeze evidence for this gate.
---

## Source: docs/audit-stage7-cow-parent-permissions-2026-10-15.md

# User page-table ancestor permissions â€” bounded correction accepted

## Final acceptance â€” 2026-10-15

The earlier COW/mapping correction in `ad11e1f1` is accepted after session 94658
exited 0 with full combined acceptance PASS. All 500 frozen inputs matched at
completion and independent inspection. The real libwoven fixture and original
thread assertions passed on 1/2/4 CPUs, along with full Stage 10.7 preservation,
all Stage 10.8â€“11.5 matrices, host tests and warning-denying host/fixture/kernel/
fifteen-feature lint. See [combined evidence](audit-exit-status-combined-acceptance-2026-10-15.md)
for exact paths, requirement coverage and retained per-run results.

This accepts the bounded ancestor-policy correction, not full Stage 7, a new
release or native hardware readiness. Wait ownership and signed-status decisions
are recorded separately, in prerequisite order. Compiler TLS, loader placement/
interoperability and broader production closure remain open. The pending
statements below are retained investigation history, not the final decision.

## Executable finding

The real libwoven status fixture passed six process-status cases and twelve
natural/cancelled joins, then faulted writing its forked user stack. Retained
failure: `audit-artifacts/stage11.2-status-1cpu-1792086619756210600/` (QEMU 35,
not success). Original paging/task/userspace snapshots are preserved there.
The diagnostic counterpart is `stage11.2-status-1cpu-1792086799166937200/`.
Its trace shows a shared stack frame at reference count two, the remaining
owner at count one, then another write fault with the leaf already writable.
The latter was rejected and the child died. Object/process/frame baselines
were restored; that does not make the failed fork behavior acceptable.

The local primary source for x86_64 0.15.5 confirms `Mapper::map_to` derives
ancestor flags from the leaf's PRESENT/WRITABLE/USER bits, while `update_flags`
updates only the leaf. `MappedPageTable::translate` reports the leaf flags,
not the effective ancestor intersection. New COW roots therefore acquired
read-only ancestors when read-only shared leaves were installed. The sole-owner
COW branch subsequently made only the leaf writable; the CPU still denied
the access. This explains why leaf-only checks and synthetic/kernel-physical
access tests did not prove actual user stack writes work.

## Integrated correction and scope

`page_permissions::ancestor_bits` keeps present user ancestors writable while
preserving the original leaf policy. It never adds USER to supervisor mappings,
does not propagate leaf-only NX/GLOBAL bits to ancestors and leaves kernel
ancestor write policy unchanged. Compile-time assertions bind its numeric
PRESENT/WRITABLE/USER masks to the paging library's flag definitions. Host
regressions cover read-only/NX user leaves and all eight low-bit combinations.

The existing recorded user-map path, COW share/remap/restore, private-file and
file-frame installation paths now explicitly use those ancestor flags. This
addresses the common failure mechanism, not just the single test's fork path.
Leaf W^X, RELRO, logical-writability authorization, reference retention, failed
map rollback, allocator limits, lock order and TLB synchronization are unchanged.
Existing unsafe mapping calls use the explicit-parent-flags variant; no new
unsafe block, kernel object, lock, capability, scheduler wait or pointer lifetime
is introduced. Only existing user-mapping parents gain the write bit; a read-only
leaf remains read-only and cannot be made writable through this helper.

The next run, `stage11.2-status-1cpu-1792087130955323000/`, had no stack exception
and reached the separate foreign-wait assertion (codes 92/93). That is evidence
that this failure was removed, not a passing full gate. The following run after
the independent wait-ownership correction passed the full one-CPU libwoven
fixture, original thread gate and exact resource baselines:
`stage11.2-status-1cpu-1792087223023180400/`.

The final revised focused matrix now passes on 1/2/4 CPUs, including all
resource and original thread assertions. Session 94658 is running full combined
acceptance under the new 500-file freeze in `audit-artifacts/exit-status-20261015/`.
Full inherited preservation remains required. This correction is **not accepted**
from focused passes. Keep all
failed evidence, run warning-denying host/kernel/affected-feature checks and
the unchanged Stage 10.7/10.8â€“11.5 preservation gates before accepting it.

## Stage 32 production build repair (2026-10-27; focused regression pass)

The desktop build exposed private instrumentation APIs and unconditional
optional state/test helpers as dead code under warnings-denied production
compilation. Exported the intended Rust APIs and gated optional state and
self-tests without changing record formats, syscall ABI, locks, allocations,
or object lifetime behavior. No warning suppressions or feature prerequisites
were added. Warning-denying default, tracing, profiler, and Stage 32.1-32.4
kernel Clippy plus host all-target Clippy and host all-target tests pass.
Stage 32.1, 32.2, 32.3 and 32.4 QEMU gates each pass on 1/2/4 CPUs (exit 33).
Prior evidence was copied under target/stage32-production-fix/prior-evidence-*
before rerunning the established harnesses. This focused repair is not full
Stage 32 or full historical Windows-chain acceptance.

## Desktop experience focused repair (2026-10-27; runtime validated, graphics acceptance incomplete)

User-reported lag/input problems were investigated against the real desktop.
The baseline 2-CPU qualification failed its terminal-close framebuffer check.
Fixes bound shell event-source work, batch painting, keep cursor/teardown
regions separate, route absolute HID pointers, and constrain taskbar backdrop
painting (which previously erased desktop shortcuts). Calculator no longer
renders its zero-filled startup input storage and uploads hover changes only
when its button state changes; the desktop owns the cursor.

The emulated demo chooses flat composition/reduced motion without changing
other boot profiles or removing 3D. The interactive launcher adds a USB tablet
and preserves unique logs. Host tests include normalized pointer endpoint/
scale/malformed-range regression coverage. No test thresholds, assertions,
credential iterations, capabilities or ownership checks were weakened.
Final matrix results and remaining limitations are recorded in stage-status.

Final runtime qualification passes on WHPX 1/2/4 CPUs. Stage 20.1/5/7/10/11/
12/15/17/18 pass on TCG 1/2/4 CPUs. Stage 20.2/3/4/6/8/9/13 pixel gates fail
on all three CPU counts; 20.2 and 20.6 are reproduced with the pre-change
shell at the same assertions on 1 CPU. Other failed gates remain open.
No complete stage acceptance is claimed. Full paths/results are in stage-status.
The already-enabled Windows hypervisor was probed, then the live desktop was
verified with PS/2 typing and USB absolute-pointer evaluation of 12-3=9.
No host security/firmware settings were changed. The launcher makes a private
read-only boot-image snapshot so subsequent builds cannot change a new VM's
boot disk. Kernel/app/host lints deny warnings; host and helper tests pass.


## Desktop software graphics acceptance closed - 2026-10-27

ACCEPTED AND CLOSED for the reported software/QEMU desktop repair at source
`cea33d1a2f6ef1ddb5cdb71a8f99d393952c0df7`. This supersedes the open desktop pixel failures in the historical records;
those historical failed artifacts remain preserved. All 87 required matrix
cases pass on 1/2/4 CPUs, including TCG/WHPX keyboard/mouse/performance,
software 3D, GPU recovery, two monitors and session behavior. The strict
clean-source GFX-07 checker passes 9/9 with its fatal detector unchanged.
Build, warning-denying lints, Rust tests and all 205 Python tests pass.

See [the acceptance record](graphics/desktop-acceptance-2026-10-27.md) for exact
source/evidence provenance, stronger completed-frame pixel assertions,
close-time repaint recovery, and validation limits. This closes software
desktop acceptance, not native GPU/HID, accelerated 3D, GFX-08, the whole
graphics mandate or the full historical Windows acceptance chain.
