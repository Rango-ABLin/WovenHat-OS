# Combined prerequisite/status regression evidence — PASS

## Baseline and terminal result

Source implementation: `ad11e1f1`. Component acceptance decisions are recorded
separately in the COW, process-wait ownership and signed-status audits; this
document records shared regression evidence, not full Stage 1–19 readiness.

Session 94658 exited **0** at 2026-10-15 20:49:52 local with:
`=== EXIT STATUS AND EARLIER PREREQUISITES FULL ACCEPTANCE: PASS ===`.

Authoritative evidence:

- `audit-artifacts/exit-status-20261015/attempt-20261015-201016-441/`
- Immutable 500-input manifest and runner in `audit-artifacts/exit-status-20261015/`
- Exact full launcher: `audit-artifacts/acceptance-20261015-201703-183/`
- Per-run stress logs: `stage7-6-artifacts/20261015-201705-680/`

All 500 frozen inputs, including the compiled fixture and published manifest
supplied to feature Clippy, matched both the terminal check and independent
post-run inspection. Historical thread/IPC manifests and failed attempts were
not replaced. No competing foreign test/build process was found at inspection;
the reserved all-worktree serialization rule remained in force.

## Checked requirements

| Requirement | Retained evidence/result |
| --- | --- |
| Actual libwoven Ring-3 fixture, 1/2/4 CPUs | Three `focused-*cpu.txt` successes, QEMU exit 33; every new family and original Stage 11.2 marker required |
| Six signed process statuses | -1, -2, MIN, MAX, zero and 37; successful payload and double-reap assertions |
| Twelve signed thread cases | Same six statuses through natural exit and sibling cancellation; stale second joins rejected |
| Authority/error behavior | Real foreign join and foreign parent wait rejected; owned running child returns WouldBlock; unknown PID and duplicate waits fail; own-child signal termination observed |
| Ownership cleanup | Exact process/frame/IPC baselines plus no remaining owner thread records; original unjoined-sibling teardown gate retained |
| Host/build checks | Build, host all-targets Clippy, fixture release Clippy, production kernel Clippy; all Clippy invocations deny warnings |
| Host tests | All root Rust tests, nineteen libwoven tests, nineteen Python harness/builder tests |
| Kernel feature lint | Fifteen completed configurations including the mandatory-input signed-status feature; no missing-image result counted as passing |
| Inherited full preservation | Exact `RUN-STAGE10.7.ps1`; memory 1 CPU x10, 2 CPUs x30, 4 CPUs x20; live network and isolated async I/O/TCP on all three CPU counts |
| Existing runtime gates | Twenty-one Stage 10.8, 10.9, 11.1, 11.2, 11.3, 11.4, 11.5 boots, each exit 33 on 1/2/4 CPUs |

Independent inspection checked all 21 runtime-result files, all three fixture
results, all fifteen feature-lint completions, fixture/host/production lint and
build completion, and the success marker in every one of the 60 retained stress
logs. The entire chain stopped on any nonzero child exit or frozen-input mismatch.

## Limits and next prerequisites

These results accept only the explicitly recorded bounded corrections. They do
not qualify a new release package, merge Claude's different source tree, prove
physical firmware/storage/display behavior, or close full Stage 7/11 or 1–19
production readiness. Fixed-ET_EXEC placement in the graph loader, compiler
TLS/TCB and fresh child templates, general shared-library interoperability,
native hardware/recovery, persistent security and service-boundary requirements
remain open. No failure assertion, capability set, warning policy or timeout was
relaxed. Original signed-status host failures, COW page faults, and foreign-wait
failure remain retained in their individual audits.
