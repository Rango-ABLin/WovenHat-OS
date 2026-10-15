# Compiler static TLS/TCB implementation — integration, acceptance pending

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

## Evidence so far — not full acceptance

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
compiler shared-PIE matrices, exact full Stage 10.7 and Stage 10.8–11.5 gates.
Retain failed and final serial evidence and commit acceptance separately.

Dynamic-library TLS, general dynamic-linker interoperability and full Stage 11
remain open. Neither these focused tests nor source integration qualify the
combined tree, a release, physical hardware, or Stage 1–19 production readiness.

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
