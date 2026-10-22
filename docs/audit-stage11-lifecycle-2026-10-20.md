# Stage 11 lifecycle callback audit — 2026-10-20

## Scope

This increment implements bounded ELF lifecycle callbacks for dynamically
linked user images: `DT_INIT`, `DT_FINI`, `DT_INIT_ARRAY`, and `DT_FINI_ARRAY`.
It does not claim lazy binding, library TLS, or full Stage 11.5 closure.

## Implementation

- ELF parsing validates callback addresses against executable load segments,
  rejects malformed array byte sizes, and bounds callback counts.
- The graph linker schedules constructors in dependency order and destructors
  in reverse order.
- A tracked RX trampoline re-enters the normal exit syscall with the original
  status in callee-saved `R12`.
- The loader prebuilds the bounded destructor return chain in a reserved
  writable stack region. Exit handling validates the syscall origin and stack
  range, snapshots paging state before lock-sensitive work, and rechecks the
  process owner before marking teardown pending.

## Validation

Host ELF scope tests: 33 passed. Stage 11.5 QEMU on 1/2/4 CPUs passed the complete
prior loader suite plus:

- constructors scheduled before Ring-3 entry;
- Ring-3 constructor execution;
- Ring-3 destructor execution and teardown;
- unresolved-import and argv rollback after lifecycle execution.

Evidence: `audit-artifacts/stage11.5-1cpu-1792507060371452000`,
`stage11.5-2cpu-1792507334555453100` and
`stage11.5-4cpu-1792507351184297200`.

## Limits and next gate

The callback bound is four functions per image, with the existing four-image
scope bound. Lazy binding and library TLS remain the next Stage 11
implementation items.
