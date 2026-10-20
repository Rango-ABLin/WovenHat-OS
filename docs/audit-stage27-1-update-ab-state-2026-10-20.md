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
package boundary now also accepts only a bounded version-1 `WUPD` envelope:
the manifest generation, declared image length, image digest, and exact image
bytes are covered by a domain-separated SHA-256 digest and verified with the
kernel-provisioned Ed25519 trust root before a candidate is admitted. A
metadata tamper self-test changes the signed generation and is rejected. The
privileged update service must still persist each state record atomically on
WovenFS; this increment does not claim that service integration.

## Validation

- `cargo test --workspace` passed, including `tests/update_state.rs`.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-1-test -- -D warnings` passed.
- Dedicated QEMU acceptance passed with exit 33:
  - `audit-artifacts/stage27.1-1cpu-1792512141099830700`
  - `audit-artifacts/stage27.1-2cpu-1792512158205516300`
  - `audit-artifacts/stage27.1-4cpu-1792512173673074900`

An earlier signed-envelope run failed its signature assertion; the recorded
diagnostic evidence was retained, the test trust seed was corrected to match
the repository's provisioned public key, and the fresh matrix above passed.

## Limits and next work

The state machine and envelope verifier are not persistent storage, an update
daemon, an installer, or a power-loss proof. Next work must bind them to a
privileged service, durable A/B records, and recovery boot evidence. Physical
USB qualification remains separately blocked until the selected device is
explicitly authorized for erasure in an elevated session.
