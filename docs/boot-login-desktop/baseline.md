# Baseline

Status: IN PROGRESS. No implementation code changed before baseline execution.

Revision reconciliation: initial local origin/master was stale. Fetch on
2026-11-04 found 148 additional commits through d21807ba. Audit checkpoint
fb7e8c2 was merged with that integration revision as aeb9d40a, preserving both
agents' documentation. A new Windows baseline is running at aeb9d40a;
runtime source matches d21807ba. New evidence:
`audit-artifacts/acceptance-20261104-190707-667/`. The older run below was
deliberately interrupted and cannot qualify current source.

Current frozen runtime revision: a980619e, incorporating baseline repairs,
the disposable fixture correction, and consistent workspace formatting.
The active serialized run is acceptance-20261104-192520-691 with
CARGO_NET_OFFLINE=true. The preceding attempt stopped at a bootloader build
before 2-CPU iteration 20 after worktree Cargo checks collided with shared
artifacts. No concurrent Cargo operation will run during this fresh chain.
See test-results.md for exact attempt outcomes and preserved failure paths.

| Item | Observed value |
|---|---|
| Repository | https://github.com/Rango-ABLin/WovenHat-OS.git |
| Integration branch/base | master, 70b49c74215d7aecc0469ba7d677458ccaf7c10a |
| Work branch | test/boot-login-baseline |
| Initial worktree | Clean |
| Toolchain | nightly-2026-07-10 |
| rustc | 1.99.0-nightly (af3d95584 2026-07-09) |
| Cargo | 1.99.0-nightly (59800466c 2026-07-07) |
| Installed targets | x86_64-pc-windows-msvc, x86_64-unknown-none, x86_64-unknown-uefi |
| Host | Windows 11, 10.0.26300, AMD64 / PowerShell (baseline host-test inventory) |
| QEMU | 11.1.0 (v11.1.0-12130-ge470268ff4) |
| Firmware | C:/Program Files/qemu/share/edk2-x86_64-code.fd, 3653632 bytes |
| Prescribed gate | powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./RUN-STAGE10.7.ps1 |
| Current evidence | audit-artifacts/acceptance-20261104-185726-462/ |

Reuse CARGO_BUILD_BUILD_DIR=<repository>/.cargo-build and
CARGO_TARGET_DIR=<repository>/target. Run only one acceptance chain.
The baseline starts with Python harness regressions before Cargo and QEMU.
Text printed by mocked harness tests is not runtime QEMU evidence.

Root `build.rs` generates wovenhat-os-uefi.img through UefiBoot; root
`src/main.rs --print-image` reports the artifact path. Interactive desktop
launch uses existing RUN-DESKTOP.ps1. Commands and profiles for the complete
new milestone remain to be verified; no new release artifact is claimed.

Historical status reports host/kernel tests and lints plus Stage20.19 input
isolation passing at 1/2/4 CPUs. The modern desktop production journeys,
full Windows gate and inherited matrices remain open. Historical results are
not substituted for this baseline. Known failures from this run are recorded
in test-results.md when observed.
