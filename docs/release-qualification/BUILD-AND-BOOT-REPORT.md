# Build and boot report

## Large signed application boot repair

The production WovenImages package now loads and completes its Ring-3 decoder
gate on 1/2/4 CPUs. Its large writable/BSS mapping no longer collides with the
anonymous arena. Full inherited preservation passes in
`audit-artifacts/acceptance-20261116-073218-890`.

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Reproduced failure

The historical `target/desktop-interactive-20261113-083354-698/kernel-build.log`
reports a duplicate `--target-dir` in nested `cargo install`. The live master
build reproduced that exact error. An untracked `vendor/bootloader/uefi/target`
directory exists without a local Cargo manifest and triggers the local branch.
Evidence is retained in `reconciliation-baseline-build.log` at the original
repository root pending evidence consolidation.

## Repair and focused evidence

The local source condition now requires a manifest file. The duplicated local
`--target-dir` was removed, preserving the isolated OUT_DIR target/build paths.
Three tests compile the actual Rust build script and record its Cargo child
arguments. Original source fails two cases, repaired source passes all three.

## Image pipeline

A direct hardware-demo command passed the repaired nested build but failed at
required application-image and signing-key environment inputs. The established
`build_hardware_demo` harness built all eleven signed applications, both service
images, the kernel and the UEFI disk image successfully using its existing
development-only test key policy.
This is a test image build. Production signing remains a release prerequisite.

## Current classification

Implemented and focused-tested repair. Image construction succeeds. The
strict VirtIO-GPU desktop journey passes on WHPX at 1/2/4 CPUs, including
UEFI/kernel startup, test authentication, application launch and input.
The one-CPU TCG attempt fails waiting for WovenFiles launch. Its serial log
proves authentication and its captured desktop is retained. TCG remains open. QEMU and OVMF CODE were discovered under
`C:/Program Files/qemu`, with firmware in its `share` directory.

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

## Accepted loader-preservation build

The combined repair passes build, warning-denying host and affected kernel
Clippy, 218 root Rust tests, 243 libwoven tests and 211 Python tests. One
pre-existing locale fixture remains explicitly ignored. The fresh Windows
gate passes in `audit-artifacts/acceptance-20261115-162204-848`, including all
60 memory runs and inherited stages through 10.7 on 1/2/4 CPUs. The preceding
StorageFull attempt is preserved in `acceptance-20261115-155710-542`; only
regenerable incremental caches were removed before the successful retry.
