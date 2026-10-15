# Stage 11.5 signed shared-library loading — 2026-10-15

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
There are at most forty image mappings (five images × eight segments). Image
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
the Stage 10.7 preservation launcher or the already completed Stage 11.2–11.5
matrix, so their fresh evidence is recorded explicitly above. The bounded
signed-graph/transactional-teardown increment is accepted with these limits;
Stage 11.5 as a whole and the release image are not certified by this decision.

## Remaining loader and production limits

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
open. This increment does not certify Stage 1–19 production readiness.
