# Stage 25.14 suspend admission audit — 2026-10-21

## Scope

This stage adds the stable privileged suspend action without enabling a
hardware S3 transition. Action `2` on `PowerControl` and
`libwoven::suspend()` fail closed until the wake path can restore the
interrupted userspace syscall frame. The rejection is audited and performs no
driver, CPU, FACS, or PM1 mutation.

## Verification

- `cargo fmt --all -- --check` passed after formatting the existing tree.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage25-14-test -- -D warnings` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed: 95 library tests plus all integration and doc
  test targets.
- `cargo build -p wovenhat-kernel --target x86_64-unknown-none` passed after
  the feature-gated dead-code correction.
- Focused QEMU gate passed sequentially:
  - 1 CPU: `audit-artifacts/stage25.14-1cpu-1792578385038056800`
  - 2 CPUs: `audit-artifacts/stage25.14-2cpu-1792578599539880300`
  - 4 CPUs: `audit-artifacts/stage25.14-4cpu-1792578621521441400`

## Limits and next prerequisite

QEMU verifies the admission invariant only; it does not issue S3. Physical
firmware wake, driver restoration after real S3, battery/thermal policy, and
task-owned return to the interrupted userspace caller remain unqualified.
The next stage must commit and verify that task-owned resume target before
enabling a real suspend request.
