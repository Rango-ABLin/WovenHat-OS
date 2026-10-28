# WovenHat userspace runtime completion design

Status: design baseline, 2026-10-28

This document defines the remaining work after the accepted bounded static musl
profile. It is intentionally a design, not a claim that the full POSIX or
dynamic runtime is implemented.

## Current boundary

The current profile has:

- a freestanding CRT with bounded argv/envp and aux-vector records;
- initial-exec PT_TLS and TLS-backed `errno`;
- checked anonymous mappings and a bounded allocator;
- generation-safe scheduler-backed thread create/join/cancel;
- bounded process-private mutexes/conditions and detached-thread cleanup;
- a loader that already parses bounded signed shared-object graphs, RELA/PLT
  metadata, symbol versions/weak symbols, RELRO, and static/shared TLS forms.

The missing work is the runtime ownership layer around those primitives.

## 1. Dynamic module manager

Add a process-owned `ModuleNamespace` in the userspace loader boundary. Each
module record contains an immutable image identity, load bias, mapped ranges,
dependency indices, exported symbol/version table, TLS image descriptor,
constructor/destructor arrays, relocation state, and an atomic reference count.

Loading is a transaction:

1. validate the signed image and canonical SONAME/needed names;
2. resolve the complete dependency graph into a temporary bounded plan;
3. reserve address-space, module-ID, TLS-module-ID, and relocation capacity;
4. map all images with W^X permissions and keep RELRO writable only during
   relocation;
5. resolve symbols in ELF scope order, including version and weak fallback;
6. apply REL/RELA/PLT relocations with checked targets;
7. allocate per-thread TLS templates and publish the namespace atomically;
8. run constructors in dependency order;
9. roll back every mapping, reference, TLS allocation, and reservation on any
   failure.

`dlopen` increments the namespace reference and dependency references.
`dlclose` decrements them, runs destructors in reverse dependency order only
   when the root and all dependants reach zero, then unmaps after a scheduler
   quiescence point. `dlsym` reads only a published immutable namespace and
   returns a generation-tagged module symbol handle; stale handles fail after
   unload.

No loader lock may be held while copying user memory, changing page tables,
running constructors, destructors, or blocking. Constructor recursion and
dependency cycles are rejected or resolved from the same bounded graph plan,
never by re-entering a partially published namespace.

## 2. Dynamic TLS and DTV

Keep the existing FS-base contract. Extend each process with a bounded module
TLS registry and each thread with a DTV array:

- module ID 0 is the static executable TLS block;
- IDs are generation-tagged and never reused while a thread can retain a DTV
  entry;
- a DTV entry stores module generation, base, size, and initialization state;
- `__tls_get_addr(module, offset)` validates the module generation and lazily
  allocates a private block from the process mapping budget;
- initialization copies the immutable PT_TLS file bytes and zero-fills the
  remainder before publishing the entry;
- module unload first marks the ID retiring, waits for all thread DTV epochs,
  then unmaps blocks and retires the ID permanently on generation exhaustion.

The kernel should expose one narrow `TLS_MODULE_RESOLVE` operation only if the
runtime cannot safely perform the lookup in userspace. It must return a
generation-bound base, not retain a userspace pointer, and must never allocate
or sleep under an IRQ lock.

Acceptance must cover local-exec, initial-exec, local-dynamic, and general-
dynamic compiler models, fresh threads, fork descendants, unload/reload,
stale module IDs, and constructor access to TLS.

## 3. Detached threads and cancellation

The thread record already distinguishes running/exited ownership. Extend it
with `join_state = Joinable | Detached | Reaped` and a cleanup stack owned by
the thread runtime.

- Detaching a running thread changes only the join state; resources remain
  owned until the thread reaches the exit safe point.
- A detached thread's exit path publishes its result, runs cleanup handlers,
  releases TLS/DTV and stack mappings, then reclaims the generation record.
- A join on a detached or already-reaped handle fails without touching
  resources.
- Cancellation is deferred by default and delivered at declared cancellation
  points. The target transitions to `CancelPending`, runs cleanup handlers in
  LIFO order, invokes TLS destructors, and exits with `PTHREAD_CANCELED`.
- Cleanup handlers must be copied into bounded thread-owned storage; no raw
  userspace callback or pointer may be retained after cancellation returns.

Asynchronous cancellation remains unsupported until every kernel boundary is
made restart-safe. It must not be enabled by an API alias.

## 4. Kernel-clock synchronization

Replace yield-only timed waits with a scheduler event registration:

- `pthread_cond_timedwait` records a monotonic deadline and releases the
  mutex before registering the waiter;
- the condition sequence counter, signal/broadcast, timeout, cancellation,
  and owner teardown race through one generation-checked waiter record;
- wakeup unregisters before reacquiring the mutex;
- timeout returns `ETIMEDOUT`, cancellation returns after cleanup, and a stale
  wake cannot consume a later wait.

Mutexes remain process-private unless a future shared-memory authority is
  explicitly added. Robust owner-death recovery requires an owner generation
  in the mutex word and an `EOWNERDEAD` transition; it must not be simulated by
  clearing a lock from another thread.

## 5. Allocator qualification

Replace the bootstrap arena with a runtime allocator only after the mapping
budget and teardown contract are explicit. The allocator must provide size-
checked bins, coalescing, alignment up to `max_align_t`, `calloc` overflow
checks, `realloc` atomic failure behavior, and per-thread contention tests.

The qualification matrix must include fragmentation, concurrent allocation,
mapping exhaustion, fork/exec teardown, cancellation during allocation, and
failure injection at every mapping/metadata boundary. A failed allocation may
return null; it must never corrupt the free list or bypass the process quota.

## 6. Delivery order and gates

Implement in this order:

1. module namespace transaction and `dlsym` read-only lookup;
2. module-ID/DTV allocation and general-dynamic TLS;
3. constructor/destructor and unload quiescence;
4. kernel-clock timed waits and cancellation cleanup;
5. allocator replacement and stress matrix;
6. full musl startup integration and compiler-produced dynamic PIE;
7. 1/2/4-CPU QEMU preservation, then signed physical qualification.

Each increment requires target and host warning-denying Clippy, host tests,
source/ABI contract checks, preserved prior-stage gates, and fresh serial/QEMU
evidence. No increment closes the broader runtime until the complete matrix
passes with no stale-handle, teardown, or resource-count discrepancy.
