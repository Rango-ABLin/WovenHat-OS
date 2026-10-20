# Stage 24.13 group membership administration audit

Date: 2026-10-21

## Implemented

- Added bounded elevated-administrator operations for adding and removing supplementary group membership.
- Rejects ordinary callers, unknown users/groups, duplicate membership, full groups, and removal of a user's primary group.
- Added syscalls 229 and 230 with stable security-ledger actions 34 and 35.
- Persists the changed authority through WID2 and rolls the live authority back when publication fails.
- Keeps membership storage bounded by the existing fixed user/group limits.

## Acceptance evidence

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Production `system-apps` kernel Clippy with signed image environment: passed.
- Signed system-apps hardware-demo shell acceptance passed on 1, 2, and 4 virtual CPUs, covering desktop sign-in, ten signed applications, PS/2/IOAPIC input, SMP migration stress, scheduler/barrier checks, and acknowledged TLB shootdowns.
- Logs: `audit-artifacts/stage24.13-group-membership-{1,2,4}cpu-1792534800000000000`.

## Limits and next prerequisites

Administration UI/query APIs, durable/configurable lockout, hardware-backed credentials, malformed-request telemetry, release-key provisioning, physical USB/power-loss recovery, and release qualification remain open. QEMU has no ATA WovenFS device; the durable WID2 path is covered by the native storage self-test and the live gate proves safe operation when no durable disk is present.
