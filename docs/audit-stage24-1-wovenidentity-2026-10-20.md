# Stage 24.1 audit — WovenIdentity policy core

Date: 2026-10-20

Implemented `libwoven::identity::Authority` with fixed-capacity local users,
groups, PBKDF2-HMAC-SHA256 credential verification, profiles, sessions,
lock/unlock state, and password-confirmed elevation. The implementation stores
only salts and derived hashes and fails closed on invalid names, passwords,
duplicate identities, capacity exhaustion, locked elevation, and wrong
credentials.

Validation:

- `cargo test -p libwoven --lib`: 82 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-1-test -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.1] WovenIdentity core: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.1-1cpu-1792525338640464600`
- `audit-artifacts/stage24.1-2cpu-1792525397795034600`
- `audit-artifacts/stage24.1-4cpu-1792525454630403900`

Limits: this stage does not claim kernel task credential switching, durable
identity database persistence/recovery, capability-broker integration,
lockout/rate limiting, administration UI, or hardware-backed credentials.
