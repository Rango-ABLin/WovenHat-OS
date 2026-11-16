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

## Inherited Windows acceptance completed

`RUN-STAGE10.7.ps1` exits 0 at source `a5eec13b4de5b1669642947958740fe1ae5d93ca`
with host network access. Evidence is retained in
`audit-artifacts/acceptance-20261115-100412-082`. All 10 one-CPU, 30 two-CPU
and 20 four-CPU memory runs pass, together with host checks and the inherited
network, IPC, security and asynchronous I/O/TCP gates at 1/2/4 CPUs.
The earlier restricted-network chain failed at DNS and remains preserved in
`audit-artifacts/acceptance-20261115-094032-061`. A focused network rerun with
host access passed before the complete chain was rerun.

## Stage 11.5 loader repair investigation

The one-CPU baseline at `a5eec13b` fails after the five signed-dependency
negative checks, before the first valid graph load. Evidence is
`audit-artifacts/stage11.5-1cpu-1794731337892442300`. The trampoline extent
incorrectly included the high shared-library arena. The working repair uses
the main executable's parsed segments and registers the trampoline before
fallible initialization. Standard Stage 11.5 then passes at 1/2/4 CPUs.

A compiler-produced shared ELF exposed duplicate TLS mapping destruction.
Static TLS descriptors borrow subranges of the initial TLS allocation. An
explicit ownership bit now prevents treating those borrowed descriptors as
separate page allocations. The compiler fuzz helper also exited before the
inherited Stage 11.5 block. Successful fuzz completion now returns to it.
The full compiler-plus-loader gate passes at one CPU in
`audit-artifacts/stage11.5-compiler-1cpu-1794732481244384300`. Multicore and TLS
revalidation was then expanded into the combined matrix recorded below.

The runtime preservation matrix passes Stages 10.8, 10.9 and 11.1 through
11.4 on 1/2/4 CPUs. The forced ASLR guard probe fails with status 98 before
the guard write because ordinary mmap is limited to the slots preceding the
dynamic-TLS reservation. This reservation predates the current changes.
Evidence is `audit-artifacts/stage11.2-aslr-1cpu-1794732082310194700`. The
sixteen-slot assertion is retained. The conflict needs a production repair
before this increment can receive complete acceptance.

## Combined loader checks, 2026-11-15 follow-up

`audit-artifacts/loader-preservation-combined-20261115-1422/result.json`
records passing build, warning-denying host and all affected-feature kernel
Clippy, 218 host tests, 243 libwoven unit tests, and all 211 Python tests.
The libwoven suite retains one previously ignored locale fixture, with its
existing ignore reason recorded in the test log. The runtime matrix passes
Stages 10.8, 10.9 and 11.1-11.5 at 1/2/4 CPUs on the combined kernel repair.

The added exit-status fixture gate stopped before boot because its lockfile
omitted current libwoven dependencies. Offline lockfile regeneration added
the missing graph without changing existing locked versions. Its next build
exposed a core-only build-std configuration conflicting with alloc dependencies.
The fixture now rebuilds both core and alloc. The unchanged locked builder
passes the one-CPU exit-status gate. Remaining status/ASLR/compiler/TLS/staging
runs are active under `audit-artifacts/loader-preservation-resumed-20261115-1552`.

Dynamic-TLS displacement and fork probes reproduced fixed placement and omitted
child mappings before correction. The strengthened ASLR gate passes at zero
and nonzero process displacement on 1/2/4 CPUs, with all sixteen mmap slots,
child COW isolation, guard faults and exact reclamation. Failure artifacts and
partial acceptance attempts remain preserved. Full post-repair Windows
acceptance and the complete resumed matrix were required before commitment and
are satisfied by the final evidence below.

## Completed combined runtime verification

`audit-artifacts/loader-combined-summary-20261115.json` reconciles both runs
by command name and verifies an identical kernel patch SHA-256. All 54 checks
pass, including 36 real QEMU runtime boots across 1/2/4 CPUs. They cover
Stages 10.8, 10.9, 11.1-11.6, signed exit status, ASLR, compiler shared PIE and
compiler TLS lifecycle. The failed fixture build attempts remain preserved.
The first post-repair full Windows chain was launched with output at the
original repository root in `reconciliation-stage10-7-loader-repair-launch.log`.
That inherited gate completed successfully as recorded below.

## Host capacity failure and recovery

The post-repair Windows chain in `audit-artifacts/acceptance-20261115-155710-542`
passed all sixty memory runs, live networking and gates through Stage 10.4.
Stage 10.5 packaging then failed with Windows error 112, StorageFull, while
writing the FAT image. The full transcript and failure remain preserved.
Approximately 10 GiB of regenerable Rust incremental caches were removed from
this isolated worktree after resolving both paths beneath its root. Source,
images, logs and historical evidence were retained. The one-CPU Stage 10.5
retry passed.

## Loader-preservation increment accepted

The fresh complete chain passes with `CARGO_INCREMENTAL=0` in
`audit-artifacts/acceptance-20261115-162204-848`. All 60 memory runs and the
inherited stages through 10.7 pass on 1/2/4 CPUs. Together with
`audit-artifacts/loader-combined-summary-20261115.json`, accepted evidence is
54 focused checks, 36 focused QEMU boots and the full inherited chain. The
pre-repair, interrupted, restricted-network and StorageFull failures remain
preserved; no assertion, warning policy or timeout was weakened.
