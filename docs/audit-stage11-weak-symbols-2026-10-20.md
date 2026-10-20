# Stage 11 weak symbol binding increment — 2026-10-20

## Scope

The bounded ELF symbol scope now recognizes `STB_WEAK` definitions. Strong
definitions are preferred over weak definitions during main-image-first
interposition; a weak definition is retained as the fallback when no strong
definition exists. An undefined, default-visible weak relocation resolves to
zero when the bounded dependency scope has no matching definition. Existing
symbol visibility, section, address-range, relocation-target, W^X/RELRO and
transactional owner-teardown checks remain in force.

Version-table metadata is still rejected before resolution. This increment
does not claim GNU symbol-version semantics, lazy binding, constructors or
destructors, library TLS, or a complete dynamic-linker ABI.

## Validation

- `cargo test --test elf_scope`: 30 passed, including strong-over-weak
  interposition and undefined-weak-zero behavior.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 11.5 QEMU passed with exit 33 on 1/2/4 CPUs:
  `audit-artifacts/stage11.5-1cpu-1792504013870714100`,
  `stage11.5-2cpu-1792504112883675500` and
  `stage11.5-4cpu-1792504158087902300`.

The required runtime markers are weak relocation, Ring-3 execution and owner
teardown; all passed on each CPU configuration.
