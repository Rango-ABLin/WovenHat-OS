# Stage 13.11F — BLE controller encryption/session authority

Date: 2026-10-05

The SMP flow owns the bounded LE Start Encryption command, preserves the live
peer identity and negotiated key size in a pending-encryption record, and
accepts controller Encryption Change only when the completion event matches
that record. Failure, disabled encryption, stale handles and peer identity
changes revoke authority instead of creating a secured session.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
QEMU 1/2/4 CPU acceptance with exit code 33. Physical controller transport,
SMP interoperability, bond persistence and hardware qualification remain open.
