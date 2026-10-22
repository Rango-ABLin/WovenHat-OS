# Stage 11 versioned symbol tables audit — 2026-10-20

## Scope

This increment implements bounded SysV ELF symbol-version metadata for the
Stage 11 graph loader: `DT_VERSYM`, `DT_VERDEF`, and `DT_VERNEED`. Definitions
and imports use exact requested version names; malformed or truncated records
fail closed.

## Implementation

- `kernel/src/elf.rs` records and validates file-backed version tables,
  bounded auxiliary chains, version indices, names, and hidden-version bits.
- `Image::relocated` shifts dynamic metadata addresses together with the
  relocated image, preserving version-table walks for shared objects.
- `SymbolScope` resolves strong and weak definitions by name and requested
  version, preserving existing visibility, bounds, relocation, W^X, and
  transactional teardown rules.
- `kernel/src/userspace/linker/acceptance.rs` adds signed versioned main and
  library fixtures, relocation verification, Ring-3 execution, and teardown.

## Validation

- `cargo test --workspace`: passed.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p wovenhat-os --test elf_scope`: 31 passed.
- Stage 11.5 QEMU passed on 1 CPU, evidence in
  `audit-artifacts/stage11.5-1cpu-1792505125207763300`.
- Stage 11.5 QEMU passed on 2 CPUs, evidence in
  `audit-artifacts/stage11.5-2cpu-1792505299339871100`.
- Stage 11.5 QEMU passed on 4 CPUs, evidence in
  `audit-artifacts/stage11.5-4cpu-1792505368683688700`.

## Limits and next prerequisites

This is not full dynamic-linker production completion. Constructors and
destructors, lazy binding, and library TLS remain outside the accepted scope;
the inherited Stage 11 and later-stage gates remain required.
