# Repository reconciliation report

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Baseline decision

Fetched all remote branches and tags successfully. Local and remote master are
`541dd70832b680e3fc4f248377c194611f71cc2d`. The isolated integration branch
starts there to preserve the current ABI, accepted source and release tools.
The original checkout had a clean working tree before the coordination entry.

The Phase J branch `5a9e6f9f9e4a84333851de99f30ef4ca2a6ca4d5` diverges by
64 master-only and 52 branch-only commits. Its total diff touches 2,196 files,
including tracked build output and 16 deleted script/tool files. Deletions
include diagnostic export, keyboard-only desktop qualification, clean-room SDK
verification, SDK signing, and the TLS service tools. It also removes existing
libwoven audio, clipboard, accessibility and network APIs. These changes need
individual reconciliation before adoption. Its worktree contains an uncommitted
settings-policy change and report outlines, which remain available there.

## Unmerged implementations and conflicts

- WovenEditor and WovenSettings GUI sources match master in the local updated-app
  and Phase J branches. Their lockfiles differ. Reapplying those GUI changes
  would add no source functionality.
- WovenShell bottom-taskbar/start-menu work exists in PR 138 and the Phase J
  lineage. Master retains its previously accepted shell. Adopting the newer
  shell requires its renderer oracles and session integration to be qualified
  together after the build gate.
- WovenFiles PR 141 changes `apps/woven-files/src/main.rs` into a read-only
  browser. Mutation operations required by the directive remain an integration
  gap.
- PR 137 contains a focused trampoline mapping-ownership repair in
  `kernel/src/userspace/linker.rs`. Master still registers that mapping after
  fallible initialization. Review and qualify the focused patch at Stage 11.5.
- PR 143 also contains loader placement, feature guards, renderer corrections
  and CI prefetch changes. It requires commit-level review because its scope
  extends beyond dependency fetching. PR 136 adds loader diagnostics. PR 131
  carries renderer oracle corrections requiring comparison to the chosen shell.
- WovenFS and storage code differ across the Phase J lineage. Existing master
  storage contracts remain the starting point for disposable-disk qualification.

## Live PR and CI evidence

The GitHub API returned 49 open PRs. Exact heads and all 49 open PR check
inventories are retained in the root audit directory as
`reconciliation-open-prs-20261115.json` and
`reconciliation-pr-checks-20261115.json` plus
`reconciliation-other-pr-checks-20261115.json`. Checks include repeated attempts.
PRs 5, 6 and 7 have zero check runs. The full inventories retain the result
for every other open head.
Every named PR has failing checks at its current head. Green auxiliary
workflows therefore provide partial evidence only.

| PR | Exact head prefix | Check count | Failed check names |
| --- | --- | --- | --- |
| 143 | `cf924830efd7` | 47 | desktop-userspace, drivers-radio |
| 142 | `c470e67dede9` | 47 | core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie |
| 141 | `8939f3ce875b` | 31 | core-foundation, desktop-userspace, drivers-radio, maturity-release-blockers, network-security, runtime-storage-pcie, wovenfs-lint |
| 140 | `42948e95fa87` | 26 | CodeQL, Stage 28 and GFX-02 QEMU matrix (1 CPU), Stage 28 and GFX-02 QEMU matrix (2 CPU), Stage 28 and GFX-02 QEMU matrix (4 CPU), core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie, wovenfs-lint |
| 139 | `8c3fad730754` | 42 | core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie |
| 138 | `ea7ebdee2b61` | 29 | desktop-userspace |
| 137 | `17acacd913bc` | 47 | core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie |
| 136 | `c4c59b5ac75b` | 47 | core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie |
| 131 | `4b53313172a1` | 47 | core-foundation, desktop-userspace, drivers-radio, runtime-storage-pcie |

## Recovery sequence

Finish the nested bootloader build repair and full image/boot gate first.
Then qualify focused loader fixes, reconcile shell plus oracle changes, and
extend applications and storage in roadmap dependency order. Preserve all
historical failures and qualify each assembled source revision afresh.

## Qualified revision

Full-system qualification is pending. The exact baseline above and working
repair are under test. No assembled release revision has been accepted.
