# Stage 11 shared-library TLS implementation audit (2026-10-20)

## Scope

The bounded ELF loader now plans one combined TLS block across the main image
and shared libraries, copies immutable PT_TLS initializers into a read-only
template, creates a fresh mutable TLS/TCB per process, and resolves the
toolchain's shared-object TLS relocation forms. The exact Rust/LLD fixture
contains a shared object with PT_TLS, a TLS initializer, an exported function,
and a TPOFF64 relocation.

## Validation

- `cargo test --workspace`: passed.
- Freestanding Stage 11.5 and host warning-denying Clippy: passed.
- Stage 11.5 QEMU acceptance passed on 1, 2, and 4 CPUs. Evidence:
  `audit-artifacts/stage11.5-1cpu-1792508930356487700`,
  `audit-artifacts/stage11.5-2cpu-1792509071141370400`,
  `audit-artifacts/stage11.5-4cpu-1792509098193013900`.

The compiler-fixture gate is not accepted: `--stage 11.5-compiler --cpus 1`
still hits the existing kernel task stack-canary failure during the paired
userspace baseline before reaching the compiler loader block. Evidence is
preserved at `audit-artifacts/stage11.5-compiler-1cpu-1792509025622559400`.
No assertion or timeout was weakened. The gate now requires an explicit PT_TLS
marker when that prerequisite stack regression is repaired.

## Remaining acceptance work

Repair the feature-specific stack-canary regression, rerun the compiler gate
on 1/2/4 CPUs, and confirm the PT_TLS marker plus per-process template and
teardown behavior. Until then, shared-library TLS is implemented but remains
open as a production roadmap gate.
