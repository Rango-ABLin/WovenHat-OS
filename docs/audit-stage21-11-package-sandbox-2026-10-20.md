# Stage 21.11 WovenPackage sandbox launch audit — 2026-10-20

## Scope

This increment connects signed package capability declarations to the Ring-3
launch boundary without changing the existing two-argument spawn ABI.

## Implemented contract

- Package declarations must be subsets of the normal user-domain ceiling.
- File policy permits only system, temporary, mounted, and home scopes for
  declared file-read authority, and temporary/home for file-write authority.
- Device policy is derived from declared network/storage/display/input/device
  capabilities.
- Service discovery is restricted to the application service class.
- Invalid declarations destroy the prepared address space before publication.
- Valid declarations are installed as the task's immutable sandbox profile.

## Validation

- `cargo test -p libwoven`: passed, 63 tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none
  --features stage21-11-test -- -D warnings`: passed.
- Stage 21.11 QEMU passed with exit 33 on 1/2/4 CPUs; evidence:
  `audit-artifacts/stage21.11-{1,2,4}cpu-*`.

## Remaining acceptance prerequisites

The long-lived userspace WovenPackage IPC broker, multi-package power-loss
recovery, and physical USB qualification remain open. No physical USB write
was performed.
