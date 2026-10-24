# Stage 24.1 audit — WovenIdentity policy core

Date: 2026-10-20

Implemented `libwoven::identity::Authority` with fixed-capacity local users,
groups, PBKDF2-HMAC-SHA256 credential verification, profiles, sessions,
lock/unlock state, and password-confirmed elevation. The implementation stores
only salts and derived hashes and fails closed on invalid names, passwords,
duplicate identities, capacity exhaustion, locked elevation, wrong
credentials, and non-administrator elevation. User insertion preflights group
capacity so a full group cannot leave a partially created user.

Validation:

- `cargo test -p libwoven --lib`: 83 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-1-test -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.1] WovenIdentity core: PASSED` on 1/2/4 CPUs after
  the elevation-boundary correction.

Evidence:

- `audit-artifacts/stage24.1-1cpu-1792525654517891200`
- `audit-artifacts/stage24.1-2cpu-1792525721415869300`
- `audit-artifacts/stage24.1-4cpu-1792525782428769900`

Limits: this stage does not claim kernel task credential switching, durable
identity database persistence/recovery, capability-broker integration,
lockout/rate limiting, administration UI, or hardware-backed credentials.
