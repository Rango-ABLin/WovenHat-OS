# Stage 27.1 bounded update A/B state audit — 2026-10-20

## Implemented boundary

`kernel/src/update.rs` now owns the bounded A/B activation state machine:

- a verified candidate is staged only in the inactive slot;
- generation rollback and empty/oversized/digestless candidates are rejected;
- activation changes only the next-boot pending record, never the current
  active slot;
- three failed boot attempts mark the candidate bad and select the known-good
  slot; and
- a candidate becomes active only after an explicit service-owned health-check
  success report.

The transition object is allocation-free and bounded to two slots. The
privileged update service must still verify signatures and persist each record
atomically on WovenFS; this increment does not claim those integrations.

## Validation

- `cargo test --workspace` passed, including `tests/update_state.rs`.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-1-test -- -D warnings` passed.
- Dedicated QEMU acceptance passed with exit 33:
  - `audit-artifacts/stage27.1-1cpu-1792511854725230900`
  - `audit-artifacts/stage27.1-2cpu-1792511867378290600`
  - `audit-artifacts/stage27.1-4cpu-1792511882178827400`

## Limits and next work

The state machine is not persistent storage, an update daemon, a signature
manifest verifier, an installer, or a power-loss proof. Next work must bind it
to a privileged service, a signed update envelope, durable A/B records, and
recovery boot evidence. Physical USB qualification remains separately blocked
until the selected device is explicitly authorized for erasure in an elevated
session.
