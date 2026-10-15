# Stage 11 bounded cross-image symbol scope — 2026-10-15

## Scope

This increment adds the first safe building block for shared-library loading in
`kernel/src/elf.rs`:

- `SymbolScope` owns one main image and at most four direct dependencies;
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
