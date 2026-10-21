# Stage 25.15 userspace resume target audit — 2026-10-21

## Scope

Suspend action `2` now validates the complete syscall frame supplied by the
existing interrupt entry against the current task's kernel stack. The frame is
retained in the committed suspend record. The wake continuation restores all
owned resources, writes a successful return value, and transfers through the
existing syscall epilogue to the interrupted userspace caller.

## Verification

The required gates passed:

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-15-test -- -D warnings` passed.
- `cargo clippy --offline --all-targets -- -D warnings` passed.
- `cargo build --offline -p wovenhat-kernel --target x86_64-unknown-none` passed.
- `cargo test --offline --all` passed: 95 library tests plus all integration
  and doc-test targets.
- Focused QEMU gate passed sequentially:
  - 1 CPU: `audit-artifacts/stage25.15-1cpu-1792579335112039300`
  - 2 CPUs: `audit-artifacts/stage25.15-2cpu-1792579392886205500`
  - 4 CPUs: `audit-artifacts/stage25.15-4cpu-1792579413003466500`

The retained prior-stage compatibility gate was also rerun after the shared
commit integration:

- Stage 25.14, 1 CPU: `audit-artifacts/stage25.14-1cpu-1792579508247837200`
- Stage 25.14, 2 CPUs: `audit-artifacts/stage25.14-2cpu-1792579565826183400`
- Stage 25.14, 4 CPUs: `audit-artifacts/stage25.14-4cpu-1792579583277334400`

The focused QEMU test does not issue S3; it verifies record ownership and
invalid-pointer rejection only.

## Limits

This stage does not prove that a physical machine enters S3, wakes through the
FACS vector, or resumes every real driver. Physical firmware and USB-boot
qualification remain separate release gates.
