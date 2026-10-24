# Stage 24.11 — bounded identity administration ABI

Date: 2026-10-20

## Scope

The identity policy now exposes bounded administrator operations for adding a
user, deleting a user, and adding a group. The kernel ABI reserves syscalls
226–228. Each request is copied from userspace into fixed-size storage and
requires the caller's owner-bound identity session to be active, elevated, and
administrator-backed. A caller cannot delete its own account or an account
with an active session. All decisions enter the security ledger with stable
public action IDs 31–33.

## Acceptance

- `cargo test -p libwoven`: 87 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-5-test -- -D warnings`: passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features system-apps -- -D warnings`: passed with signed system-app images.
- `git diff --check`: passed.
- The signed system-apps hardware-demo shell gate passed on 1, 2, and 4 CPUs.

Evidence is preserved in:

- `audit-artifacts/stage24.11-admin-abi-1cpu-1792533600000000000`
- `audit-artifacts/stage24.11-admin-abi-2cpu-1792533600000000000`
- `audit-artifacts/stage24.11-admin-abi-4cpu-1792533600000000000`

## Limits and next prerequisites

The ABI and policy are live, but this slice does not yet persist new users or
groups to the account file/WID2 snapshot, expose an administration UI, or
provide group membership mutation/deletion. Those are required before calling
identity administration production-complete. Hardware credentials,
malformed-request telemetry, release-key provisioning, and physical
USB/power-loss qualification also remain open.
