# Stage 11 eager PLT RELA increment — 2026-10-20

## Scope

The ELF loader now accepts a bounded `DT_JMPREL` table when it is explicitly
encoded as `DT_RELA` with `DT_PLTRELSZ`, `DT_PLTREL` and `DT_RELAENT`. The table
must be file-backed by a load segment, contain no more than the existing
relocation bound, and target writable, non-executable mapped storage. Its
entries use the same validated eager relocation set as the ordinary RELA
table, including `R_X86_64_JUMP_SLOT`; no lazy binding, text relocations or
runtime symbol-table mutation was added.

The loader applies ordinary and PLT RELA entries transactionally before final
W^X and RELRO protection. Malformed encoding, missing metadata, out-of-file
tables, unsupported relocation types and read-only targets fail closed.

## Validation

- `cargo test --test elf_scope`: 25 passed, including valid PLT metadata,
  wrong `DT_PLTREL` encoding and read-only target rejection.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python scripts/test-stage10-runtime.py --stage 11.5 --cpus 1`: passed,
  exit 33, evidence under `audit-artifacts/stage11.5-1cpu-1792501620049883600`.
- The same Stage 11.5 gate passed on 2 CPUs and 4 CPUs, exit 33, with evidence
  under `stage11.5-2cpu-1792501673567079100` and
  `stage11.5-4cpu-1792501694886543100`.

## Limits

This does not implement legacy `DT_REL`, versioned/weak symbol semantics,
lazy binding, constructors/destructors, library TLS or the complete dynamic
linker ABI. It is an accepted bounded increment, not full Stage 11 or
production readiness.
