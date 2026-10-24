# Stage 28 WovenInstaller Ring-3 shell audit — 2026-10-22

## Scope

This slice adds only `apps/woven-installer`. It adapts an already validated
installer UI/state snapshot to a deterministic textual fallback. It is not an
installer and does not claim graphical, physical-media, or boot integration.

## Safety boundary

- Input is capped at 256 bytes and fields are bounded to the existing model's
  text limits.
- The snapshot must contain `validated=1` and a non-empty plan.
- Unknown and duplicate fields, control characters, invalid screen names,
  invalid progress, missing screen prerequisites, and unvalidated plans fail
  closed.
- There is no install subcommand, disk/file/partition/filesystem/bootloader
  operation, network call, subprocess call, or destructive syscall.
- Progress and success are display-only state-model screens and explicitly
  report `install_action=unavailable (fail-closed)`.

## Validation

- Host parser tests: 3 passed.
- Freestanding release build: `x86_64-unknown-none` passed.
- Freestanding Clippy: `cargo clippy --offline --manifest-path apps/woven-installer/Cargo.toml --target x86_64-unknown-none -- -D warnings` passed.
- App formatting check passed after formatting the new files.
- `git diff --check` passed.

## Limits and next prerequisites

No QEMU gate was run or claimed. This disjoint app is not embedded in the
kernel, so a QEMU boot would exercise neither a new process launch path nor an
installer integration path. The next installer work still requires an explicit
reviewed authority boundary, real installation semantics, boot handoff,
rollback/power-loss coverage, and physical qualification; none is implied by
this shell.
