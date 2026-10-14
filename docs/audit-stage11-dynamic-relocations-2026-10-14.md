# Stage 11 dynamic relocation audit — 2026-10-14

## Scope

The ELF loader now accepts a deliberately narrow ET_DYN/PT_DYNAMIC contract.
It validates the dynamic table and allows bounded RELA entries whose targets
lie in non-executable writable load segments. Relative entries and locally
defined symbol entries (`R_X86_64_64`, `R_X86_64_GLOB_DAT`, and
`R_X86_64_JUMP_SLOT`) are applied after all image bytes are copied and before
segment permissions and RELRO are finalized. SysV hash metadata provides a
bounded symbol count; section, string-table, image-range, and signed-addend
checks prevent the loader from guessing at an invalid symbol scope. The load
bias is carried through ASLR, so values and targets are relocated together.

The loader rejects PT_INTERP, DT_NEEDED, undefined symbols, legacy REL tables,
PLT relocation tables, text-relocation requests, duplicate dynamic headers,
malformed/truncated tables, unsupported dynamic tags, and relocation targets
outside writable data. Shared-library dependency mapping, cross-image symbol
resolution, versioned symbols, lazy binding, and dynamic TLS remain
unsupported and fail closed.

## Validation

- Loader self-tests cover a valid relative-relocation table and malformed
  dynamic/TLS/RELRO cases.
- Kernel and host warning-denying Clippy pass.
- Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs:
  - audit-artifacts/stage11.5-1cpu-1791973749450962200
  - audit-artifacts/stage11.5-2cpu-1791973828064459400
  - audit-artifacts/stage11.5-4cpu-1791973879025768100

The local-symbol follow-up reran the inherited Stage 11.3–11.5 matrix on the
current source and retained:

```text
audit-artifacts/stage11.3-1cpu-1791977053721810200
audit-artifacts/stage11.3-2cpu-1791977120029898500
audit-artifacts/stage11.3-4cpu-1791977170903077000
audit-artifacts/stage11.4-1cpu-1791977221235626900
audit-artifacts/stage11.4-2cpu-1791977286654426900
audit-artifacts/stage11.4-4cpu-1791977331645912200
audit-artifacts/stage11.5-1cpu-1791977377816478600
audit-artifacts/stage11.5-2cpu-1791977444546806600
audit-artifacts/stage11.5-4cpu-1791977498582992000
```

This is a production safety boundary for self-contained PIE images, not a
claim of a complete dynamic linker or shared-library runtime.
