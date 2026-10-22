# Stage 11 lazy PLT binding audit — 2026-10-20

## Scope

This increment defers writable PLT `R_X86_64_JUMP_SLOT` binding until the
first Ring-3 call. It does not claim library TLS or full Stage 11 closure.

## Implementation

- The loader retains validated slot/target pairs in an eight-entry bounded
  process table and emits per-slot RX stubs in the loader-owned resolver page.
- Each stub supplies its GOT slot in `R11`, invokes the internal lazy-bind
  syscall, and jumps to the returned target.
- The kernel accepts only slots registered in the current process address
  space, requires an executable target, and patches through the active-user
  copy path so forked address spaces break COW before mutation.
- PLT slots covered by RELRO remain eager; no writable alias is created to
  bypass RELRO. Exceeding the bound fails closed.

## Validation

Host ELF scope tests: 33 passed. Warning-denying freestanding Stage 11.5 and
host all-target Clippy passed, as did the 11 Python harness tests. Stage 11.5
QEMU acceptance passed on 1/2/4 CPUs. The dedicated fixture verifies the
writable PLT slot contains the resolver stub before first call and that the
Ring-3 call resolves, executes, and tears down with status 42.

Evidence: `audit-artifacts/stage11.5-1cpu-1792508068498107100`,
`stage11.5-2cpu-1792508101002664400` and
`stage11.5-4cpu-1792508116869852100`.

## Limit and next prerequisite

The process-local lazy table is bounded to eight slots to preserve the
kernel-stack copy budget. RELRO PLT slots are intentionally eager. Library TLS
is the next unresolved Stage 11 prerequisite.
