# Test results

Status: IN PROGRESS.

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
