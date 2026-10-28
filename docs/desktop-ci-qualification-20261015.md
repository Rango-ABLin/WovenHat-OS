# Desktop CI qualification — 2026-10-15

## Evidence at commit c8f56113dd7bbf7a1e5994cd3bd5ee1e292bbaf9

- Draft PR: https://github.com/Rango-ABLin/WovenHat-OS/pull/75
- Successful kernel workflow: https://github.com/Rango-ABLin/WovenHat-OS/actions/runs/38747964379
- Eight successful jobs: desktop-application, desktop-compositor, desktop-shell, core-foundation, wovenfs-lint, drivers-radio, runtime-storage-pcie, network-security.
- Desktop jobs retain warnings-denied Clippy and their original QEMU 1/2/4-CPU acceptance commands; application Stage 16.1, compositor Stages 17–19, shell Stage 20.
- Earlier monolithic desktop job in run https://github.com/Rango-ABLin/WovenHat-OS/actions/runs/38721409729 was cancelled after Stage 20.10 at four CPUs; splitting the suite removed the single shared 60-minute budget.

## Qualification boundary

**CI GREEN; NOT PRODUCTION-CLOSED.** The result establishes that this PR's CI gates completed, not that every historical Stage 1–19 closure criterion or physical hardware acceptance is met. No release approval is implied. Review the retained job artifacts and actual hardware, power-loss, filesystem persistence, security denial/revocation, graphics/input devices and installer/recovery evidence before marking the OS production-ready.

## Next gates

1. Review PR #75 and merge only with explicit approval.
2. Resolve Stage 10 OVMF VARS integration in draft PR #73 and nine-lane validation in draft PR #74, requiring complete successful checks.
3. Address security issue #72 (fast SHA-256 local PIN verifier) with a versioned work-factor KDF, safe migration, denial/lockout tests and resource-bounded verification.
4. Complete Stage 1–19 closure evidence ledger and real-hardware qualification; preserve CI and hardware evidence by source SHA.
