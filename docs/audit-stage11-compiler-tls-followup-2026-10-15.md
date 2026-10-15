# Compiler TLS follow-up — source review, not acceptance

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
