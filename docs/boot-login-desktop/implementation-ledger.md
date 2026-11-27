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

## BLD-B1 strict boot configuration

Status: TESTED — PASS (bounded unit). Branch: feature/boot-config-rejection.
Base: 224def05. Runtime commit: aaeaaf8f.
Owned files are recorded in agent-coordination.md. The version-matched UEFI
runtime is retained locally; configuration is checked before allocation and
parsed without heap allocation. Invalid policy terminates before handoff.
Valid legacy settings and version 1 remain supported. No kernel ABI, object,
capability or hand-written unsafe block is added. Firmware dependency versions
and checksums match the original package lock; provenance is retained.

Five actual-source parser tests, five host gate/provenance regressions, host
all-targets Clippy, UEFI-target Clippy and fixture-builder Clippy pass. An
initial QEMU case exposed a trailing-data error-class mismatch; fixed and
retained. The next 1-CPU run passes all nine cases. Malformed and oversized
framebuffer captures were visually checked. Final source-bound 1/2/4-CPU
matrix passes all 27 cases. Stage15.5 signed-handoff and the complete Windows
preservation chain pass on 1/2/4 CPUs. Full Phase B is not accepted.


## BLD-B2 bounded kernel image inspection

Status: TESTED - PASS (bounded unit). Runtime 72be85aa,
branch feature/boot-failure-validation. Pure safe Rust inspection precedes the
unchanged common mapper; disk/TFTP size checks precede allocation. Missing
kernel fallback then explicit rejection is preserved. No authority, kernel
object, ABI or hand-written unsafe change. Scope and limitations are recorded
in boot-image-validation.md, including dynamic relocation and trust exclusions.

All 45 actual UEFI cases pass on 1/2/4 CPUs using the same prepared source/image
hashes. Host 8 parser tests, 6 Python oracle tests, root/UEFI/producer Clippy and
format checks pass. Initial fixture-string length and Clippy defects were fixed
without suppressing checks; notes retained under audit-artifacts/boot-image-b2/.
Full Windows acceptance passes under acceptance-20261104-214457-697/:
214 Python tests (six skips), host Rust tests, lint/build, 60 memory repetitions,
network and isolated TCP 1/2/4 CPUs. Stage15.5 signed-handoff 1/2/4 and feature
Clippy pass. B2 is accepted; full Phase B remains open.

## BLD-B3 loader allocation failure

Status: TESTED - PASS (bounded unit). Runtime `952eda65` converts checked
loader-data allocation failure into static pre-handoff firmware-resource
diagnostics. All 16 source-bound UEFI cases pass at 1/2/4 CPUs, including a
real 256 MiB allocation-exhaustion case. The full Windows preservation gate
passes at `acceptance-20261105-074313-292/` (215 Python tests, six skips,
host Rust, 60 memory boots, network and Stage10.4-10.7 1/2/4 CPUs). Evidence
is preserved at `D:\WovenHat-OS-audit-boot-loader-b3-20261105` after intact
relocation to free workspace capacity. Full Phase B remains open.

## BLD-B resume / Phase C prerequisites (2026-11-09)

Status: IN PROGRESS. Base 5d1b1c09. User explicitly requests completing boot/recovery policy, firmware failure handling, then WovenInit service dependency/readiness integration. The first actual-source baseline `cargo test --test boot_config` fails with eight stale Policy field references and missing heapless dependency (target/boot-policy-resume-baseline.log). Repair preserves original assertions under Policy.boot and adds handoff/bounded command-line assertions. The initial repaired run also caught unread handoff under denied warnings (target/boot-policy-resume-repair.log); coverage now reads and checks the handoff rather than suppressing dead-code warnings.

The full Windows baseline is serialized in audit-artifacts/acceptance-20261109-082549-583. Runtime sources remain frozen while it runs. The source review must distinguish a stale checked-in UEFI lockfile from the actual nested install graph: nested install output confirms the local common mapper is compiled. No Phase B or Phase C closure is claimed. Independent read-only Phase B verification is requested under the roadmap acceptance loop; no second build/test chain is authorized.

## BLD-B Phase B boot/recovery closure (2026-11-09)

Status: ACCEPTED (software/QEMU scope). Boot policy is a versioned, bounded
BootInfo extension with a new serialized bootloader UUID, preserving the legacy
prefix. The UEFI parser validates and decodes bounded policy data, selects
default/recovery/diagnostic/alternate kernels without fallback downgrade, and
hands the exact read-only command line to the kernel. Recovery boots suppress
the durable root, desktop, accounts, and destructive recovery authority.

The source-bound 26-case policy/firmware matrix passed at 1/2/4 CPUs, including
strict GOP rejection, disk I/O, alternate selection, and invalid policy cases.
Real page-table/mapping exhaustion, stale-key ExitBootServices retry, and forced
ExitBootServices failure fixtures also passed at 1/2/4 CPUs. Host policy tests,
Python harness tests, host/kernel/UEFI warning-denying Clippy, and format checks
pass. The complete Windows preservation chain passed at
`audit-artifacts/acceptance-20261109-094329-057`. Physical firmware, PXE/NIC,
Secure Boot provisioning, and device recovery remain hardware/trust work.

## BLD-C Phase C WovenInit readiness closure (2026-11-09)

Status: ACCEPTED. `kernel/src/woven_init.rs` replaces unordered production
service startup with a bounded four-node dependency graph. Package readiness
precedes update, installer, and search. Readiness requires the expected live
PID to own the expected service with nonzero client rights; waits are bounded
to 500 ticks. Any dependency, spawn, registration, or readiness failure rolls
back every process started by the transaction in reverse order.

Normal WovenInit-to-desktop acceptance passed at
`target/shell-regression/system-apps-1cpu-1794222517041520300`,
`system-apps-2cpu-1794222656818206300`, and
`system-apps-4cpu-1794222770523822300`. The feature-isolated failure case kills
update after registration but before readiness publication; its evidence is
`system-apps-1cpu-1794222602511550200`,
`system-apps-2cpu-1794222715234098900`, and
`system-apps-4cpu-1794222829517617100` under the same directory. All three CPU
counts reject the barrier, verify every graph process exited, and emit
`[PHASE C] WovenInit readiness failure rollback: PASSED`. Production
and failure-feature target Clippy, Python syntax/oracles, formatting, and diff
checks pass. The complete Windows preservation chain passed in
`audit-artifacts/acceptance-20261109-131738-437` (218 host tests, warning-clean
build/lints, 60 repeated memory boots, network and Stage 10.4-10.7 at 1/2/4
CPUs). The preceding failed run is retained at
`audit-artifacts/acceptance-20261109-131508-542`; it exposed and led to repair
of a stale licensed-font host oracle, not a runtime relaxation.

## BLD-D Phase D splash/ownership closure (2026-11-09)

Status: ACCEPTED. Existing splash rendering is preserved; incorrect
`BOOT LOADER` kernel-stage labels are corrected to `KERNEL BOOT`. Stage 1 is
truthfully classified as pre-timer, and monotonic delta/total measurements
begin only after PIT initialization. The compositor records its first
terminal-to-desktop handoff after the owner-authorized frame has been presented
and after releasing the compositor lock. No ABI, capability, kernel object,
lock, unsafe block, deadline, or framebuffer ownership rule was added.

The source-bound QMP gate passed at 1/2/4 CPUs with 1280x800 captures:
`target/shell-regression/phase-d-1cpu-1794226887479458400`,
`phase-d-2cpu-1794227010862864400`, and
`phase-d-4cpu-1794227079535988900`. Every run validated the splash palette,
monotonic post-timer stages, WovenInit barrier, exclusive desktop handoff, and
all 1,024,000 pixels changing between splash and desktop. The 1-CPU splash and
desktop frames were visually inspected. Ten focused host oracles, workspace
all-targets Clippy, production kernel Clippy, formatting, and diff checks pass.
The full Windows preservation chain passed at
`audit-artifacts/acceptance-20261109-142532-913`.

## BLD-E Phase E authentication closure (2026-11-10)

Status: ACCEPTED. Production credential creation and rotation use the bounded
secure-entropy API and fail closed without RDRAND. Verifier-bearing account and
WID2 paths are protected from userspace; WovenShell consumes a separate
names-only catalog and sends bounded plaintext credentials to the kernel
authority. Digest-replay calls are retired fail-closed without renumbering the
ABI. Missing or invalid production account state stops before the desktop, and
the existing installer is the first-account provisioning path.

The missing-account, unavailable-entropy, first-user installer, Stage 20.12
sign-in/lock, Stage 24.4 identity/lockout, Stage 24.5 ABI and Stage 24.59
Change-PIN gates passed at 1/2/4 CPUs. Direct feature and host-target Clippy
passed with warnings denied. The complete Windows preservation chain passed at
`audit-artifacts/acceptance-20261110-123957-212` with 220 host harness tests,
host Rust tests, 60 repeated memory boots, live networking, and Stage 10.4-10.7
on 1/2/4 CPUs. Phase F is next; no graphical-login, session/desktop or physical
hardware gate is claimed by this entry.

## BLD-F Phase F graphical login closure (2026-11-10)

Status: ACCEPTED. The existing WovenShell pre-authentication state now renders
the complete WovenLogin interaction: responsive account selection, masked PIN
entry, explicit graphical submission, keyboard/pointer navigation, visible
authentication failure, high-contrast mode, and power/restart controls. Every
credential submission still calls the accepted kernel identity authority; no
verifier access, local authentication, new syscall, capability, kernel object,
unsafe block, or deadline was added.

The enhanced Stage 20.12 sign-in/lock journey passed at
`target/woven-shell-sign-in-{1,2,4}cpu`. The final WHPX visual qualification
passed at `target/phase-f-login-whpx-1cpu-20261110-171909`; its normal and
high-contrast 1280x800 frames were visually inspected and met the unchanged
20-second input and 5-second frame budgets. Earlier TCG/WHPX runs remain
retained at `target/phase-f-login-1cpu-20261110-134803` and
`target/phase-f-login-whpx-1cpu-20261110-{135314,135614,140242}`; they exposed
a performance-metric classification defect, which was corrected without
raising a budget. App target, affected kernel target and workspace host Clippy
pass with warnings denied. The complete Windows preservation chain passed at
`audit-artifacts/acceptance-20261110-172935-815`. Phase G is next; no session
management or physical hardware gate is claimed by this entry.

## BLD-G Phase G session-management closure (2026-11-11)

Status: ACCEPTED. `task::Process` now carries kernel-only identity-session
membership and active state. Authenticated spawn/fork inherit the parent's
credentials and tag; userspace cannot set either. The production primary shell
is refused command spawn before authentication. Lock suspends every matching
member before the authority transition, unlock restores them only after valid
verification, and close revokes all inherited credentials before retiring the
token. Natural and remote owner teardown release the binding.

WovenShell's Quick Settings now includes Log out. It sends SIGKILL to its
bounded child table, closes the kernel session, wipes typed and Change-PIN
material, resets account state and returns to sign-in. The strengthened Stage
20.12 journey passed at `target/woven-shell-sign-in-{1,2,4}cpu`, proving a
launched child's identity equality, lock suspension, unlock restoration,
logout revocation, child surface/process cleanup and display return. App,
Stage20.12 target and host-target Clippy passed with warnings denied. The full
Windows preservation chain passed at
`audit-artifacts/acceptance-20261111-032505-125`. Phase H is next; no complete
desktop integration or physical hardware gate is claimed by this entry.

## BLD-H Phase H WovenShell integration closure (2026-11-12)

Status: ACCEPTED. The production shell uses the directive's bottom taskbar and
a bounded WovenStart menu rather than a full-screen Activities/workspace
preview. Search, selection, application launch, focus restoration, window
management and an explicit localized empty-result state share bounded geometry
and input hit regions. The current 220-pixel diagonal wallpaper, system font,
gradient title chrome and rounded translucent cards are reflected by independent
kernel acceptance oracles. Flat toast, Quick Settings and notification-center
cards receive one rounded compositor blend, avoiding the former double blend.

Stage 20.1-20.4 pass at `target/woven-shell-{1,2,4}cpu`,
`target/woven-shell-apps-{1,2,4}cpu`,
`target/woven-shell-launcher-{1,2,4}cpu` and
`target/woven-shell-windows-{1,2,4}cpu`. Production Stage 20.15 passes in
`audit-artifacts/stage20.15-{1,2,4}cpu-*`. The Phase G Stage 20.12 journey was
rerun at `target/woven-shell-sign-in-{1,2,4}cpu` and preserves authentication,
identity inheritance, lock/unlock, logout, child cleanup and display return.
Warning-denying target/host Clippy and formatting pass. The complete Windows
chain passes at `audit-artifacts/acceptance-20261112-153757-168`. Phase I is
next; physical firmware, trust and device qualification remain deferred.

## BLD-I Phase I full-system integration closure (2026-11-12)

Status: ACCEPTED. Stage 20.12 now executes two complete login/logout cycles in
one production shell and independently validates the translucent taskbar/font
readiness pixels. Warm reboot, supervisor crash/restart, WovenInit rollback,
installed WovenFS persistence, repeated abrupt-shutdown recovery and input
isolation all pass on 1/2/4 CPUs.

The pass corrected test-only pre-session account-gate ordering and moved remote
identity-owner revocation ahead of scheduler acquisition. This both prevents a
last authenticated operation racing termination and removes the observed
rank-10 to rank-5 lock inversion. Affected feature and host Clippy pass with
warnings denied. The complete Windows chain passes at
`audit-artifacts/acceptance-20261112-164814-197`. Phase J is next; physical
firmware, trust and device qualification remain deferred.
