# Stage 11 eager PLT REL increment — 2026-10-20

## Scope

The ELF loader now accepts a bounded `DT_JMPREL` table encoded as legacy
`DT_REL`. It validates `DT_PLTRELSZ`, `DT_PLTREL == 17`, the fixed 16-byte
entry format, file-backed placement and writable, non-executable relocation
targets. The loader reads each signed 64-bit addend from the mapped private
target and applies the existing checked relocation and symbol-scope rules
before W^X and RELRO protection.

The Stage 11.5 acceptance fixture signs and loads a DT_JMPREL/DT_REL main
image, verifies the relocated cross-image address, executes the import in
Ring 3, and checks process-owner teardown. The harness requires both runtime
markers.

## Validation

- `cargo test --test elf_scope`: 28 passed, including valid PLT REL metadata
  and bounded target validation.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 11.5 QEMU passed with exit 33 on 1/2/4 CPUs:
  `audit-artifacts/stage11.5-1cpu-1792503297814943600`,
  `stage11.5-2cpu-1792503342880325200` and
  `stage11.5-4cpu-1792503373152819300`.

## Limits

Symbol versioning/weak binding, lazy binding, constructors/destructors,
library TLS and the complete dynamic linker ABI remain open. This is an
accepted bounded Stage 11 increment, not full Stage 11 or production
readiness.
