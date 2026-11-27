# Stage 15.3 — WovenFS VFS integration contract (development draft)

Base: `master` at `a54ae3196d77a9ce12b03cc31b7a96754974ddd6`.
Status: **PLANNED — NOT ACCEPTED**. This document is a development gate, not proof of implementation.

## Objective
Make the Stage 15.1 native on-disk WovenFS available through the normal typed VFS without disrupting the existing FAT32 boot and persistence path. Do not claim that WovenFS is the default system filesystem until it is mounted, used by ordinary callers, and survives remount/reboot testing.

## Preconditions
- Confirm the latest master workflow is green; repair any red gate before adding features.
- Preserve Stage 1–5 storage acceptance, Stage 12.1–12.5, Stage 13.3/13.4 and Stage 15.1/15.2.
- Inspect the VFS adapter, WovenFS on-disk implementation and disk ownership before implementing. Do not confuse the legacy in-memory `wovenfs.rs` metadata table with the native on-disk implementation.
- Retain FAT32 as the default until an explicit migration decision and compatibility evidence.

## Implementation slices
1. **Mount ownership:** establish a single owner of a WovenFS volume and a bounded mount handle; prevent simultaneous conflicting writers and unsafe device reuse.
2. **VFS adapter:** map lookup, read directory, create, read, write and unlink onto WovenFS with explicit error translation and capability/permission checks. No silent fallback to FAT32.
3. **Persistence:** verify create/write/unlink through the VFS, unmount, remount, and compare contents and metadata. Test a rejected/corrupt volume without damaging another mount.
4. **Crash behavior:** retain exhaustive journal write-boundary fault injection; add VFS-facing recovery tests and verify that partial operations are never observable.
5. **SMP safety:** exercise concurrent access with 1/2/4 CPUs; assert lock order, bounded memory use and exclusive mutation ownership.
6. **Release documentation:** update `docs/stage-status.md` and the Architecture & Codebase Guide only after passing acceptance evidence is recorded.

## Required acceptance evidence
- `cargo clippy` with `-D warnings` for the new feature and all affected existing features.
- Host unit/integration tests covering VFS error mapping, mount lifecycle, namespace isolation and corruption refusal.
- QEMU 1/2/4 CPU passes with deterministic markers and exit 33; negative cases must fail with a non-success exit rather than timeout.
- Existing FAT32 storage and WovenFS gates remain green; no deletion or weakening of assertions.
- Record commit SHA, exact commands, run IDs, logs, and observed results. Never mark the stage PASS based solely on an old run or a printed marker.
- Physical native-device and actual power-loss qualification remain separate outstanding requirements.

## Stop conditions
Do not merge or advance to GUI work if current CI is red, if VFS integration can bypass capability checks, if concurrent mounts can mutate the same device unsafely, or if recovery tests expose partial committed state.

## Next engineering action
Inspect `kernel/src` VFS/storage integration and the native WovenFS implementation, then implement the smallest independently testable adapter slice. This branch intentionally begins with a reviewed acceptance contract; **no runtime code has been changed yet**.
