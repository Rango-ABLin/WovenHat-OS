# Compiler-produced signed shared PIE — integration, acceptance pending

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
These are focused results, not acceptance. Full host/kernel/sixteen-feature warning-denying
lint/tests, signed-status/placement preservation, exact Stage 10.7 and Stage
10.8–11.5 preservation are required before accepting this correction.

Compiler-compatible TLS/TCB and fresh child initialization, virtual-address
congruence, dynamic-library TLS, general REL/PLT tables, symbol/version/weak
semantics, constructors/destructors and runtime loading remain open. See the
compiler-TLS follow-up audit. No combined-tree, release, physical-hardware or
Stage 1–19 production qualification is claimed.
