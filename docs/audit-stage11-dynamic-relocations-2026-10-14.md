# Stage 11 dynamic relocation audit — 2026-10-14

## Scope

The ELF loader now accepts a deliberately narrow ET_DYN/PT_DYNAMIC contract.
It validates the dynamic table and allows only bounded
R_X86_64_RELATIVE RELA entries whose targets lie in non-executable writable
load segments. Relocations are applied after all image bytes are copied and
before segment permissions and RELRO are finalized. The load bias is carried
through ASLR, so relative values and targets are relocated together.

The loader rejects PT_INTERP, DT_NEEDED, symbol-based relocations, legacy REL
tables, PLT relocations, text-relocation requests, duplicate dynamic headers,
malformed/truncated tables, unsupported dynamic tags, and relocation targets
outside writable data. Shared libraries, symbol resolution, versioned symbols,
lazy binding, and dynamic TLS remain unsupported and fail closed.

## Validation

- Loader self-tests cover a valid relative-relocation table and malformed
  dynamic/TLS/RELRO cases.
- Kernel and host warning-denying Clippy pass.
- Stage 11.5 QEMU acceptance passed on 1/2/4 CPUs:
  - audit-artifacts/stage11.5-1cpu-1791973749450962200
  - audit-artifacts/stage11.5-2cpu-1791973828064459400
  - audit-artifacts/stage11.5-4cpu-1791973879025768100

This is a production safety boundary for self-contained PIE images, not a
claim of a complete dynamic linker or shared-library runtime.
