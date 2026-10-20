# Stage 24.12 — durable identity administration

Date: 2026-10-20

## Scope

Administrator add-user, delete-user, and add-group operations now publish
through the authenticated WID2 dual-copy store. User additions also update the
trusted account file with the authority-generated salt and digest; deletions
remove the account file entry. Mutation failures restore the prior live
authority and attempt to restore the previous WID2 snapshot. The scheduler
migration self-test now waits for bounded reclamation of coverage-probe slots
before starting stress jobs.

## Acceptance

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Stage 24.5 and production `system-apps` freestanding Clippy passed with `-D warnings`.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.12-admin-persistence-1cpu-1792534200000000000`
- `audit-artifacts/stage24.12-admin-persistence-2cpu-1792534200000000000`
- `audit-artifacts/stage24.12-admin-persistence-4cpu-1792534200000000000`

## Limits and next prerequisites

The QEMU fixture has no ATA WovenFS disk, so the live gate proves safe
no-durable-device behavior; native WID2 persistence remains covered by the
Stage 24.3 storage self-test. Group membership mutation/deletion, an
administration UI, durable/configurable lockout, hardware-backed credentials,
malformed-request telemetry, release-key provisioning, and physical
USB/power-loss qualification remain open.
