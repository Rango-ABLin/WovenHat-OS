# Implementation ledger

## BLD-A

Status: COMPLETE (Phase A). Agent: Codex. Branch: test/boot-login-baseline.
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

Status: COMPLETE (bounded baseline repair). Agent: Codex. Base integration: d21807ba.
Owned files: tests/diagnostics.rs and only Volume::probe in kernel/src/wfs.rs.
The current baseline failed before QEMU on missing serial test dependency and
a nested-if warning. Added a thread-local recording serial double and exact
panic-output integration assertion. Rewrote the probe selection as let-else,
matching its neighboring mount path and supporting both Rust editions without
changing sector reads, superblock ranking or returned errors. No new kernel
object, lock, unsafe code, syscall, asynchronous pointer or capability exists.
Tests: diagnostics 10/10, WovenFS crash 20/20, host all-targets Clippy pass.
Default/Stage12.5/Stage15.2 target Clippy also pass with warnings denied.
Final runtime a980619e passes full Windows preservation and Stage12.1–12.5
QEMU matrices on 1/2/4 CPUs. Original formatting failure is retained; BLD-A3
resolves it. This is baseline repair acceptance, not a production OS claim.

## BLD-A2 persistent-account fixture

Status: TESTED — PASS (host fixture gate only). Agent: Codex.
Owned files: scripts/create-account-fixture.py; tests/test_account_fixture.py.
Disposable-image audit reproduced the obsolete SHA256(salt || PIN) record.
The generator now uses the production contract's PBKDF2-HMAC-SHA256 at
100000 iterations, preserving the existing public test identity/salt/PIN and
no-overwrite behavior. Host tests check the generated on-disk record against
a fixed known answer and verify an existing destination survives untouched.
Both tests pass. No production provisioning, credential migration, entropy,
authentication/session authority or actual login acceptance is claimed.

## BLD-A3 workspace formatting

Status: COMPLETE (formatting baseline). Branch: chore/boot-login-format.
Prepared in an isolated worktree at b75b9366 while the original Windows
chain runs. A plain cargo fmt followed by --check exposed conflicting import
ordering for shared block.rs compiled under editions 2021 and 2024. Explicit
workspace style_edition=2021 resolves that non-idempotence without disabling
any formatter rule. Cargo fmt --all followed by two --check runs passes.
The uniform style also normalizes imports in existing edition-2024 files;
all source edits are formatter output. Formatted host all-targets Clippy passed
offline. Integrated as a980619e; the serialized network-enabled Windows
acceptance-20261104-194935-034 run and the Stage12 matrix pass. No assertion,
feature guard, capability or timeout is changed.
