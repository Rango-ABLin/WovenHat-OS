# Compiler TLS follow-up — source review, not acceptance

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
