# Stage 11 bounded DT_REL increment — 2026-10-20

## Scope

The ELF parser and graph loader now recognize a separate `DT_REL` table with
`DT_RELSZ` and `DT_RELENT`. The table is bounded by the existing relocation
limit, must be file-backed, and every target must be writable and
non-executable. Only the existing eager relocation types are accepted.

Because REL stores its addend at the relocation target, the loader reads the
already-mapped target bytes from the private inactive address space, interprets
the eight-byte value as a signed addend, and writes the checked result before
final W^X/RELRO protection. No userspace pointer is retained and no lazy
binding is introduced.

## Validation

- `cargo test --test elf_scope`: 27 passed, including valid REL metadata,
  wrong entry size and read-only target rejection.
- `cargo check -p wovenhat-kernel --target x86_64-unknown-none`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage11-5-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Inherited Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs, exit 33, with
  evidence under `audit-artifacts/stage11.5-1cpu-1792501845221185500`,
  `stage11.5-2cpu-1792501867464427000` and
  `stage11.5-4cpu-1792501878849660600`.

The Stage 11.5 QEMU harness now includes a dedicated signed DT_REL executable
fixture. It verifies the relocated target, executes the imported function in
Ring 3, and checks owner teardown. The required markers passed on 1/2/4 CPUs:

- `[S11.5] shared libraries DT_REL runtime relocation: PASSED`
- `[S11.5] shared libraries DT_REL Ring-3 execution and teardown: PASSED`

Evidence is retained under `audit-artifacts/stage11.5-1cpu-1792502825772958300`,
`stage11.5-2cpu-1792502960229582900` and
`stage11.5-4cpu-1792502998392183900`.

## Limits

PLT RELA is supported by the preceding increment. PLT REL, symbol
versioning/weak binding, constructors/destructors, lazy binding and library
TLS remain open. Full inherited release-chain preservation is also required
before Stage 11 closure.
