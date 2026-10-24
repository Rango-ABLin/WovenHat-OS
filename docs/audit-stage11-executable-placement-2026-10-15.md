# Executable placement in the signed graph loader — accepted bounded correction

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
Stage 10.7 and Stage 10.8–11.5 1/2/4-CPU preservation were still required;
the subsequent full acceptance decision above supplies that evidence.
Compiler-produced shared-PIE interoperability, null DT_DEBUG handling,
compiler TLS/TCB and fresh child templates, library TLS and broader dynamic
linking remain separate open requirements. The minimal PIE probe does not
qualify those contracts. Neither a new release nor combined-tree/hardware
production readiness is claimed.
