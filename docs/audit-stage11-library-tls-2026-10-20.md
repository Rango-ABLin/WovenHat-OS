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
  `audit-artifacts/stage11.5-1cpu-1792510954965696000`,
  `audit-artifacts/stage11.5-2cpu-1792510968884579900`,
  `audit-artifacts/stage11.5-4cpu-1792510982932216800`.
- The exact compiler-fixture gate passed on 1, 2, and 4 CPUs. Evidence:
  `audit-artifacts/stage11.5-compiler-1cpu-1792510865272847800`,
  `audit-artifacts/stage11.5-compiler-2cpu-1792510895456748200`,
  `audit-artifacts/stage11.5-compiler-4cpu-1792510921851793300`.
- `cargo test --workspace`, host Clippy, and freestanding compiler-feature
  Clippy with the generated fixture paths all passed.

The compiler gate initially exposed a fixed-capacity kernel stack defect and
an incorrect ELF TLS-symbol discriminator. The stack reservation is now 256
KiB, and TLS symbols are recognized by `STT_TLS` with a non-undefined section
index. No assertion or timeout was weakened.

## Acceptance result

Stage 11 shared-library TLS is accepted for the current roadmap gate. Broader
physical-hardware qualification and later production-release requirements
remain governed by their later roadmap stages.
