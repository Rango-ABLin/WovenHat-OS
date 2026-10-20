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

The state object also has a fixed-size `WUAB` record codec. It serializes the
two slots, active/pending selectors, and a monotonic record sequence with a
CRC-32. Decode rejects wrong magic/version/length, invalid slot invariants,
invalid attempts, and checksum failures. This is the record boundary for a
future service-owned dual-copy commit; it does not itself perform device I/O.

## Validation

- `cargo test --workspace` passed, including `tests/update_state.rs`.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage27-1-test -- -D warnings` passed.
- Dedicated QEMU acceptance passed with exit 33:
  - `audit-artifacts/stage27.1-1cpu-1792512141099830700`
  - `audit-artifacts/stage27.1-2cpu-1792512158205516300`
  - `audit-artifacts/stage27.1-4cpu-1792512173673074900`
  - `audit-artifacts/stage27.1-1cpu-1792512350452245500`
  - `audit-artifacts/stage27.1-2cpu-1792512351912529200`
  - `audit-artifacts/stage27.1-4cpu-1792512368502642400`

An earlier signed-envelope run failed its signature assertion; the recorded
diagnostic evidence was retained, the test trust seed was corrected to match
the repository's provisioned public key, and the fresh matrix above passed.

## Limits and next work

The state machine and envelope verifier are not persistent storage, an update
daemon, an installer, or a power-loss proof. Next work must bind them to a
privileged service, durable A/B records, and recovery boot evidence. Physical
USB qualification remains separately blocked until the selected device is
explicitly authorized for erasure in an elevated session.

## Stage 27.2 durable record acceptance (2026-10-20)

The durable boundary now writes exact-length `WUAB` records through the native
WovenFS ATA transaction path. The acceptance formats a disposable scratch
volume, commits two records, remounts and reads both records, selects the
newest valid sequence, and falls back to the older record after corrupting the
newer copy. The update service owns the authority token in this gate; no user
VFS cache is used for record selection.

Validation passed with exit 33:

- `audit-artifacts/stage27.2-1cpu`
- `audit-artifacts/stage27.2-2cpu`
- `audit-artifacts/stage27.2-4cpu`

The dedicated script uses only disposable scratch images. No physical USB
write, format, partition change, or boot was performed.

Stage 27.2 still does not provide a long-lived userspace update daemon,
power-loss injection/recovery across every commit boundary, installer/boot
slot integration, or physical-media qualification.

After the Stage 27.2 feature and WovenFS changes, the inherited Stage 27.1
gate was rerun successfully on 1/2/4 CPUs:

- `audit-artifacts/stage27.1-1cpu-1792512964891684100`
- `audit-artifacts/stage27.1-2cpu-1792512978915669200`
- `audit-artifacts/stage27.1-4cpu-1792512993391527100`
