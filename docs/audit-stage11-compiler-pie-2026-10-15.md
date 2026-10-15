# Compiler-produced signed shared PIE — accepted bounded correction

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
- Twenty-one existing Stage 10.8, 10.9 and 11.1–11.5 runtime boots pass on
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
10.8–11.5 preservation were required; the full decision above now supplies them.

Compiler-compatible TLS/TCB and fresh child initialization, virtual-address
congruence, dynamic-library TLS, general REL/PLT tables, symbol/version/weak
semantics, constructors/destructors and runtime loading remain open. See the
compiler-TLS follow-up audit. No combined-tree, release, physical-hardware or
Stage 1–19 production qualification is claimed.
