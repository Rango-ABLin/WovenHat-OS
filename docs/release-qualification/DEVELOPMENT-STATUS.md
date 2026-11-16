# Development status

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Implemented

The UEFI build script selects local sources only when `uefi/Cargo.toml` exists
and gives either source mode exactly one isolated Cargo target directory.

## Tested

Three actual-script regression cases pass. The original script fails the
leftover-directory and local-manifest cases. Full Python discovery found
210 passing tests and one failure caused by an uncached locked dependency.
Locked dependency prefetch completed successfully and all 211 tests pass on rerun.

## Accepted

The bounded command-construction regression passes. The signed image builds
and the strict desktop journey passes on WHPX at 1/2/4 CPUs. Build,
warning-denying Clippy, Rust tests and inherited stage gates pass.
TCG at one CPU fails waiting for WovenFiles launch and remains open.

## Blocked

Release closure depends on the remaining software gates, production signing,
independent review and physical qualification recorded in the closure ledger.

## Not implemented

Remaining application and release requirements have yet to be reconciled
against an assembled, qualified source revision. See REMAINING-WORK.md.

## Completed build checks

`cargo build --locked`, `cargo clippy --all-targets -- -D warnings`, kernel
Clippy for the default and hardware-demo features with warnings denied, and
`cargo test --all-targets` all exit 0. Exact commands, UTC start/end times,
environment inputs, source base/patch hash and individual logs are retained in
`audit-artifacts/reconciliation-build-checks-20261115/result.json` in the
isolated worktree. The inherited Stage 10.7 Windows chain passes as recorded below.

## Inherited Windows acceptance completed

`RUN-STAGE10.7.ps1` exits 0 at source `a5eec13b4de5b1669642947958740fe1ae5d93ca`
with host network access. Evidence is retained in
`audit-artifacts/acceptance-20261115-100412-082`. All 10 one-CPU, 30 two-CPU
and 20 four-CPU memory runs pass, together with host checks and the inherited
network, IPC, security and asynchronous I/O/TCP gates at 1/2/4 CPUs.
The earlier restricted-network chain failed at DNS and remains preserved in
`audit-artifacts/acceptance-20261115-094032-061`. A focused network rerun with
host access passed before the complete chain was rerun.

## Stage 11 loader-preservation repair accepted

The baseline graph failure is preserved in
`audit-artifacts/stage11.5-1cpu-1794731337892442300`. The accepted repair
separates main-image trampoline placement from the shared-library arena,
registers trampoline ownership before fallible initialization, distinguishes
borrowed static TLS subranges from owned dynamic mappings, and gives dynamic
TLS a bounded ASLR-relative arena outside the sixteen ordinary mmap slots.
Fork shares owned dynamic pages through COW and accounts them in write-fault
and teardown paths. Compiler and staging acceptance reach the intended
inherited and rollback checks.

`audit-artifacts/loader-combined-summary-20261115.json` records 54 successful
checks and verifies one production-patch SHA-256 across the resumed run. Its
36 QEMU boots cover Stages 10.8, 10.9 and 11.1-11.6 plus exit-status, ASLR,
compiler and TLS variants on 1/2/4 CPUs. The fresh Windows preservation chain
passes in `audit-artifacts/acceptance-20261115-162204-848`.

This accepts the bounded loader-preservation increment. Full Stages 1-19 and
release qualification remain open for reboot persistence, complete native app
journeys, branch reconciliation, hardware and production trust gates.
