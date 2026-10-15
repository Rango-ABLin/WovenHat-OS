# Stage 11 bounded cross-image symbol scope — 2026-10-15

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
it does not close Stage 11.5 or the Stage 1–19 production objective.
