# Local PIN verifier security qualification — Issue #72

Status: **BLOCKED — implementation and review required**. This document records an observed vulnerability and required acceptance; it does not assert a security fix.

## Verified source finding
At master commit `68751dd427c56c8a78d8646b73e5ec9a893a138c`, `libwoven/src/lib.rs` module `accounts` parses `name:salt-hex:hash-hex` and verifies a PIN with a single `SHA-256(salt || PIN)`. A per-account salt and constant-time digest comparison are present, but do not impose a meaningful offline guessing cost. The unit test `pin_hash_is_sha256_of_salt_then_pin` explicitly codifies this legacy scheme.

## Required design before changing authentication
1. Choose and review a bounded work-factor KDF supported in the no-std environment. Prefer Argon2id if measured memory and boot-time budgets permit; otherwise use calibrated PBKDF2-HMAC-SHA256 with an explicit minimum iteration policy.
2. Define an unambiguous versioned credential record, including algorithm ID, work parameters, random per-account salt and derived verifier. Reject unsupported versions, overflow, malformed encodings and parameter values outside enforced min/max bounds.
3. Keep legacy parsing only as an explicit migration path. On successful legacy authentication, atomically rewrite to the new format using a crash-safe credential update. Do not downgrade new records, and do not discard existing accounts on migration failure.
4. Preserve constant-time comparison; limit input sizes; implement per-account backoff/lockout with auditable outcomes and safe persistence across reboot. Keep denial decisions independent of account enumeration.
5. Add KDF known-answer tests, wrong-PIN and malformed-record tests, parameter bounds, version rejection, migration and interrupted-write recovery, rate-limit tests, and concurrent login tests.
6. Require warnings-denied Clippy, host tests, 1/2/4-CPU QEMU authentication acceptance, security review, and measured hardware qualification before closing issue #72.

## Separation of concerns
The passing desktop CI in PR #75 does **not** qualify this security gate. No production-ready claim or merge is authorized by this document.
