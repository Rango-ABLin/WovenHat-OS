# Stage 13.11C — BLE SMP confirm/random verification boundary

Date: 2026-10-05

The SMP security boundary validates legacy Pairing Confirm and Pairing Random
PDUs against negotiated pairing state, peer address context and a temporary
key through the `c1` confirm function. The `s1` STK derivation boundary is also
exercised. Confirmation state is handle-bound and transitions to `Failed` on a
mismatch; malformed, stale or out-of-order inputs fail closed.

The exact tip passed warning-denying kernel Clippy, 11 host tests, and the
dedicated Stage 13.11 1/2/4-CPU QEMU gate with exit code 33. Evidence is
retained under `audit-artifacts/stage13.11-*`. Production pairing UX,
authenticated key-derivation integration, bonding persistence, controller
transport and hardware qualification remain open.
