# Test results

Status: IN PROGRESS.

Current baseline: aeb9d40a (runtime d21807ba), evidence
`audit-artifacts/acceptance-20261104-190707-667/`. TESTED — FAIL at host
all-targets Clippy, before runtime QEMU. Missing serial double in the
diagnostics integration crate and collapsible-if in Volume::probe were
introduced by integration history, before this assignment's runtime edits.
Both are repaired without weakening assertions or suppressing warnings.
Focused diagnostics tests passed 10/10, including actual panic serial emission;
WovenFS crash tests passed 20/20; host all-targets Clippy passed. Kernel feature
checks and full baseline rerun remain required. Target lint logs are retained
in `audit-artifacts/boot-login-baseline-repair-20261104/`. All results
below belong to the initial stale local revision unless explicitly stated.

BLD-A1 target Clippy passed with warnings denied for default, stage12-5-test
and stage15-2-test. Diagnostics test formatting passed after import ordering;
checking it recursively also exposed inherited formatting in diagnostics.rs.
Workspace `cargo fmt --all -- --check` is TESTED — FAIL, with 417 diff hunks
in the retained workspace-format.log, beginning at kernel/build.rs and
agent_coordination.rs. No bulk unrelated reformat or suppression was applied.
The full Windows gate rerun after repairs is
`audit-artifacts/acceptance-20261104-191149-393/`; result pending.

BLD-A2: initial disposable-image audit found stored_matches_current_kdf=false
in account-fixture-audit.json. After correcting the generator, both host
fixture tests pass; final output is account-fixture-tests-final.log in the
repair evidence directory. These tests were added after the current Windows
chain's initial Python discovery; their explicit focused run supplements that
chain and is not misreported as part of its original 206-test discovery.

Prescribed Windows baseline started on untouched runtime source at base
70b49c74215d7aecc0469ba7d677458ccaf7c10a. Evidence:
`audit-artifacts/acceptance-20261104-185726-462/acceptance-output.txt` and
`acceptance.txt`. Execution begins with Python harness regressions. Overall
result pending. Mocked desktop/session PASS output from host tests is not a
real QEMU acceptance result. No 1/2/4-CPU runtime pass claimed yet.

Intermediate results, not an overall gate pass: Python harness regressions
completed 206 tests in 84.806 seconds, OK with six skips; root build,
warning-denying host all-targets and freestanding kernel Clippy, and Cargo
host tests passed. The Stage 7.6 nested preservation gate is executing ten
1-CPU, thirty 2-CPU and twenty 4-CPU memory/boot iterations before network
and later inherited stages. Per-iteration logs are copied by that runner to
its timestamped stage7-6-artifacts directory.

Baseline interruption: origin/master was refreshed after startup and advanced
from 70b49c74 to d21807ba (148 commits). The old-revision run was deliberately
stopped before completion so it cannot qualify the new integration revision.
Its first ten 1-CPU and eight 2-CPU memory/boot iterations passed; no 4-CPU or
full Stage10.7 pass is claimed. Logs remain in the original timestamped audit
and stage7-6-artifacts/20261104-185905-556 directories. Resume baseline only
after synchronization and reinspection. This interruption is not a diagnosed
kernel defect. The final owned QEMU child is allowed to finish naturally.
