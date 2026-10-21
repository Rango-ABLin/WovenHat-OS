# Stage 27.4 bounded update IPC contract audit — 2026-10-21

## Scope

This increment adds `libwoven::update_ipc`, a fixed-capacity request/response
ABI for the future privileged WovenUpdate service. Requests distinguish
staging, activation, boot success, boot failure, and query operations. Stage
requests carry only a validated `/updates/` candidate path and positive
generation; lifecycle requests carry only a validated slot; image bytes never
cross IPC.

The codec rejects malformed headers, unsupported versions, unknown operations,
invalid generation/slot combinations, traversal or non-`/updates/` paths, and
oversized messages before a service can act on them.

## Verification

- `cargo test --offline -p libwoven` passed: 113 tests, including four new
  update-IPC tests.
- `cargo clippy --offline -p libwoven --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- Existing Stage 27.1 1/2/4-CPU QEMU evidence remains valid because this
  increment changes only the userspace protocol library and no kernel image,
  feature gate, or boot path.

## Limits

This is an ABI contract, not the update daemon. No syscall dispatch, service
registration, capability issuance, signed-package staging, durable image-slot
write, bootloader handoff, rollback persistence, or power-loss injection is
implemented by this increment. Physical USB and boot-media qualification also
remain open.
