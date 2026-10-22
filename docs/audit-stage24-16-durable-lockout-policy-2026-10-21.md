# Stage 24.16 audit — durable/configurable lockout policy

Date: 2026-10-21  
Owner: Codex  
Status: accepted

## Delivered

- Added a bounded administrator lockout threshold of 1 through 32 failed
  authentications, defaulting to 5.
- Persisted the policy in the existing WID1 reserved header byte. Version 1
  snapshots remain readable with the default policy; version 2 snapshots
  retain the selected policy; durable WID2 records continue to wrap this
  authenticated WID1 payload.
- Added userspace syscall 232 and audit action 37. Authorization is enforced
  by the owner-bound elevated identity session, not by the desktop UI.
- Applied the persisted threshold to plaintext and verified-digest login and
  unlock paths. The fixed five-second cooldown, bounded table, success reset,
  and fail-closed behavior remain unchanged.
- Added the WovenSecurity fixed-buffer decimal editor and exercised it through
  the live administrator UI.

## Verification

- `cargo test -p libwoven`: 89 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity app target Clippy with `-D warnings`: passed.
- Kernel `system-apps` Clippy with `-D warnings`: passed.
- Kernel `system-apps,hardware-demo` Clippy with `-D warnings`: passed.
- Focused WovenSecurity QEMU administrator exercise: passed on 1 CPU,
  including the threshold update marker.
- Signed system-app shell acceptance: passed on 1, 2, and 4 CPUs, including
  the ten packaged applications, desktop startup, PS/2/IOAPIC input, and SMP
  checks.
- Evidence: `audit-artifacts/stage24.16-lockout-policy-20261021`.

## Limits and next prerequisites

The cooldown duration is intentionally still a fixed kernel policy; this
stage makes only the failure threshold configurable. Hardware-backed
credentials, malformed-request telemetry, release-key provisioning, and
physical hardware qualification remain release prerequisites. The volatile
authority fallback is compiled only for the disposable `hardware-demo`
fixture; production durable publication remains fail-closed. The attached USB
device was not written, reformatted, or otherwise modified.
