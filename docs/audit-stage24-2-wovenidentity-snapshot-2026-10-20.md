# Stage 24.2 audit — authenticated WovenIdentity snapshot

Date: 2026-10-20

Implemented a fixed-size authenticated `WID1` snapshot for local identity
policy state. It stores bounded users, groups, administrator roles, profile
display names, salts, and PBKDF2-HMAC-SHA256 digests. Sessions are excluded so
authentication cannot survive reboot. Decoding validates the checksum, version,
counts, identifiers, names, memberships, padding, and duplicate records before
returning a fresh authority.

Validation:

- `cargo test -p libwoven --lib`: 84 passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage24-2-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `python -m py_compile scripts/test-stage24-1-identity.py`: passed.
- `git diff --check`: passed.
- QEMU exit 33 and `[S24.2] WovenIdentity snapshot: PASSED` on 1/2/4 CPUs.

Evidence:

- `audit-artifacts/stage24.2-1cpu-1792526063075640900`
- `audit-artifacts/stage24.2-2cpu-1792526104893458300`
- `audit-artifacts/stage24.2-4cpu-1792526146509891500`

Limits: this stage does not claim kernel filesystem persistence, atomic
power-loss recovery, key management, credential rotation, lockout/rate
limiting, or task/session credential integration.
