# Test qualification report

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Environment and source

Windows PowerShell, Python 3.15.0, Rust nightly-2026-07-10
(rustc 1.99.0-nightly af3d95584). Project-local Cargo intermediate and target
directories are reused for focused root builds. Build/QEMU chains are serial.

## Executed evidence

| Command or check | Result | Evidence at original repository root |
| --- | --- | --- |
| Live hardware-demo direct build on master | Failed, duplicate target directory | reconciliation-baseline-build.log |
| Same direct command with repaired script | Passed nested build, failed missing package/key inputs | reconciliation-repaired-build.log |
| Actual-script regression on original source | 2 failures / 3 cases | reconciliation-regression-before.log |
| Actual-script regression on repaired source | 3 passes | reconciliation-regression-after.log |
| Python unittest discovery | 210 passes, 1 failure | reconciliation-python-tests.log |
| Offline Cargo metadata reproduction | Missing bumpalo 3.20.3 | reconciliation-metadata-error.log |
| cargo fetch --locked --config net.offline=false | Exit 0 | reconciliation-dependency-fetch.log |

The Python failure originates from offline metadata needing uncached locked
registry packages. Complete locked prefetch restored the cache. The complete
Python suite rerun passes all 211 tests with exit 0. The signed image build
completed successfully under
`audit-artifacts/master-reconciliation-worktree/target/reconciliation-desktop-20261115-092029`.

## Pending acceptance

The strict desktop journey passes with exit 0 on WHPX at 1/2/4 CPUs.
Directories are `audit-artifacts/reconciliation-desktop-whpx-{1,2,4}cpu-20261115`
inside the isolated worktree. The one-CPU TCG journey fails waiting for Files
launch in `audit-artifacts/reconciliation-desktop-1cpu-20261115`.
Build, warning-denying host/kernel Clippy, Rust host tests and inherited
QEMU stage gates remain in progress or pending.

## Completed build checks

`cargo build --locked`, `cargo clippy --all-targets -- -D warnings`, kernel
Clippy for the default and hardware-demo features with warnings denied, and
`cargo test --all-targets` all exit 0. Exact commands, UTC start/end times,
environment inputs, source base/patch hash and individual logs are retained in
`audit-artifacts/reconciliation-build-checks-20261115/result.json` in the
isolated worktree. The inherited Stage 10.7 Windows chain is the next gate.
