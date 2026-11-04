# Work ownership

Status: IN PROGRESS. BLD-A assigned to Codex on test/boot-login-baseline,
base 70b49c74215d7aecc0469ba7d677458ccaf7c10a. Owns this directory and its
coordination entry only. No kernel/app/bootloader implementation files claimed.
Integration refreshed to d21807ba through local merge aeb9d40a. The initial
old-revision chain was stopped; only the current-revision chain may run.
No other agent was contacted or started by this unit. Historical Claude notes
are context, not proof that a concurrent edit lock is free.

The local branch isolates commit history, not another process's file writes.
Recheck git status and ownership immediately before each implementation edit.
Only one acceptance chain may run because of shared ports and artifact paths.

Remote open-PR review on 2026-11-04 found overlapping proposals: #124 boot
identity (`console.rs`), #138 shell bottom-bar geometry, #133 Stage20.15 probes,
#131 wallpaper expectations, #136/#137 loader diagnostics/rollback, and
#134/#135 broad kernel recovery. Their descriptions explicitly report pending
verification. Do not overwrite or merge these proposals as accepted work.
The remote review does not establish that their authors hold live local locks.
WovenWeb PR #123 remains outside scope.
