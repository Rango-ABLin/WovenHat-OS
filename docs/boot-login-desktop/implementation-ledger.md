# Implementation ledger

## BLD-A

Status: IN PROGRESS. Agent: Codex. Branch: test/boot-login-baseline.
Initial base: 70b49c74215d7aecc0469ba7d677458ccaf7c10a.
Current integration: d21807ba, merged locally as aeb9d40a. Checkpoint fb7e8c2
contains the initial audit, not accepted runtime implementation.
Scope: baseline execution, source inventory and startup/security gap audit.
Owned files: this directory and the BLD-A entry in docs/agent-coordination.md.
Dependencies: existing roadmap and baseline acceptance; no later gate
promotion. Initial audit introduced no runtime changes; see BLD-A1 for repairs.
Tests: prescribed Windows
gate running; results pending. Next: diagnose baseline, finish source trace,
then select the smallest prerequisite integration unit.

The full user specification is archived verbatim in directive.md. Its bottom
taskbar/no-top-bar requirement and paused WovenWeb scope govern this work.

## BLD-A1 baseline repair

Status: IN PROGRESS. Agent: Codex. Base integration: d21807ba.
Owned files: tests/diagnostics.rs and only Volume::probe in kernel/src/wfs.rs.
The current baseline failed before QEMU on missing serial test dependency and
a nested-if warning. Added a thread-local recording serial double and exact
panic-output integration assertion. Rewrote the probe selection as let-else,
matching its neighboring mount path and supporting both Rust editions without
changing sector reads, superblock ranking or returned errors. No new kernel
object, lock, unsafe code, syscall, asynchronous pointer or capability exists.
Tests: diagnostics 10/10, WovenFS crash 20/20, host all-targets Clippy pass.
Default/Stage12.5/Stage15.2 target Clippy also pass with warnings denied.
Next: complete preservation gate. No phase acceptance yet. Workspace format
check exposes inherited formatting debt, retained separately from this repair.
