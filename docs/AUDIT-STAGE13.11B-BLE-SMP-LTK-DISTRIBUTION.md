# Stage 13.11B — BLE SMP LTK distribution

Date: 2026-10-05

The SMP foundation now parses legacy Encryption Information and Master
Identification PDUs, validates their lengths and key material, and stores the
result in the canonical address/type-keyed `LeBondStore`. Distribution is
accepted only for the live pairing handle and negotiated key size; malformed,
stale, duplicate or mismatched identity inputs fail closed.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
the dedicated Stage 13.11 1/2/4-CPU QEMU gate with exit code 33. Evidence is
retained under `audit-artifacts/stage13.11-*`. Cryptographic derivation,
user-confirmed pairing, controller transport and hardware qualification remain
open.
