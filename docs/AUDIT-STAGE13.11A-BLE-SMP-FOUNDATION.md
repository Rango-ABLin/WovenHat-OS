# Stage 13.11A — BLE SMP pairing and fixed-channel foundation

Date: 2026-10-05

The BLE Security Manager Protocol foundation is bounded above live LE link and
L2CAP ownership. It validates Pairing Request and Pairing Response fields,
enforces the supported key-size range, rejects unsupported authentication and
key-distribution bits, and accepts SMP traffic only on fixed L2CAP CID `0x0006`
for a live connection handle. Pairing state is bound to that handle and
advances only from validated request to negotiated parameters.

The dedicated Stage 13.11 gate passed on 1/2/4 CPUs with exit code 33 and
retained evidence under `audit-artifacts/stage13.11-*`. This remains a
deterministic foundation; cryptographic key derivation, user interaction,
bonding persistence, controller transport and hardware qualification remain
open.
