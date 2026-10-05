# Stage 13.11G — BLE identity/privacy foundation

Date: 2026-10-05

The BLE SMP identity foundation validates Identity Information and Identity
Address Information distribution, retains bounded IRK-backed identities, and
resolves private addresses only against stored identity material. Duplicate
identity replacement is deterministic and malformed or stale distribution is
rejected. The pending LTK distribution remains peer-bound, so privacy
resolution cannot bypass connection ownership.

The exact tip passed build, warning-denying kernel Clippy, 11 host tests, and
QEMU 1/2/4 CPU acceptance with exit code 33. Physical privacy-list hardware
behavior, SMP interoperability, persistent protected keys and qualification
remain open.
