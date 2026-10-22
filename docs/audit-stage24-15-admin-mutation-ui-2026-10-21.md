# Stage 24.15 acceptance audit — administration mutation UI

Date: 2026-10-21

## Delivered

WovenSecurity now exposes the complete bounded identity administration flow
behind its elevated owner-bound session: add regular user, add group, add and
remove supplementary membership, and delete an inactive user. Each operation
uses fixed buffers, bounded ASCII/decimal input, explicit Enter transitions,
kernel-side identity validation, and a directory refresh after success. The
directory displays stable user/group IDs so the member and delete operations
are usable without hidden state.

The two mutation syscall capability checks that only applied to the desktop
session were removed from the admin-add entry points. The identity runtime is
the authorization boundary: every mutation still requires the calling process
to own an active elevated administrator session. Unauthenticated processes and
ordinary sessions cannot mutate identity state.

The disposable `hardware-demo` fixture has an explicit volatile fallback when
no durable WovenFS volume exists. Production builds do not enable this feature
and continue to fail closed on durable publication or trusted account-file
failure.

## Acceptance evidence

- `cargo test -p libwoven`: 88 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity freestanding Clippy: passed.
- Production system-apps kernel Clippy: passed.
- Combined `system-apps,hardware-demo` kernel Clippy: passed.
- `git diff --check`: passed.
- Focused WovenSecurity 1-CPU QEMU acceptance passed, exercising directory
  authentication, group creation, user creation, membership add/remove, and
  user deletion.
- Signed system-apps shell acceptance passed on 1, 2, and 4 CPUs.

Preserved logs are under
`audit-artifacts/stage24.15-admin-mutation-ui-20261021/`. The focused serial
log contains `group added`, `user added`, `membership added`, `membership
removed`, and `user deleted` markers.

## Limits and next prerequisites

The mutation UI is complete for the existing identity ABI, but durable
mutation behavior still requires a real WovenFS-backed device; the no-disk
QEMU path deliberately uses only the test-feature volatile fallback. Durable
and configurable lockout, hardware-backed credentials, malformed-request
telemetry, release-key provisioning, physical USB qualification, and the
remaining production/release gates remain open. The attached USB device was
not written or modified.
