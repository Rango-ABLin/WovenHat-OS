# Test results

Status: IN PROGRESS. Phases A-I are accepted; Phase J is next.

## Windows baseline attempts

All directories below are under audit-artifacts/ and retain transcript/output.

| Evidence directory | Source | Result |
|---|---|---|
| acceptance-20261104-185726-462 | 70b49c74 | Deliberately interrupted after discovering origin/master was 148 commits newer; not current-source acceptance |
| acceptance-20261104-190707-667 | aeb9d40a, runtime d21807ba | TESTED — FAIL before QEMU: missing diagnostics serial double and WovenFS collapsible-if warning |
| acceptance-20261104-191149-393 | 6b4cf263 runtime; fixture added separately as b75b9366 | TESTED — FAIL: ten 1-CPU and nineteen 2-CPU boots passed; build before 2-CPU iteration 20 attempted inaccessible crates.io after a concurrent worktree check invalidated shared Cargo artifacts; QEMU did not launch for that iteration |
| acceptance-20261104-192520-691 | a980619e | TESTED — FAIL: all 60 memory repetitions passed; first network guest passed DHCP but DNS timed out under restricted networking |

The initial stale-source run also passed ten single-CPU repetitions and its
final child boot completed naturally after runner interruption. Those results
are retained only as historical evidence. The previous repeat logs are in
stage7-6-artifacts/20261104-191357-707/, including the build-failure snapshot.
No kernel failure is inferred from the registry-access build error. No retry
count, timeout, assertion, guest marker, or QEMU exit requirement was relaxed.

## Focused checks

Evidence directory: audit-artifacts/boot-login-baseline-repair-20261104/.

| Check | Result / limits |
|---|---|
| Diagnostics host tests | TESTED — PASS, 10/10, including actual emitted panic registers and newline |
| WovenFS crash tests | TESTED — PASS, 20/20 |
| Host all-targets Clippy after BLD-A1 | TESTED — PASS, warnings denied |
| Default/Stage12.5/Stage15.2 kernel Clippy after BLD-A1 | TESTED — PASS, warnings denied; before formatter integration |
| Original workspace formatting | TESTED — FAIL, 417 diff hunks in 41 files; workspace-format.log retained |
| BLD-A3 workspace formatting | TESTED — PASS: explicit shared style; two consecutive worktree checks and integrated root check |
| Formatted host all-targets Clippy | TESTED — PASS offline; formatted-host-clippy-offline.log. Original network failure also retained |
| Persistent account fixture audit | Original generated record did not match current PBKDF2 KDF; account-fixture-audit.json |
| Corrected fixture host tests | TESTED — PASS, 2/2; fixed known-answer record and no-overwrite; account-fixture-tests-final.log |
| Virtual trust preflight | BLOCKED / NOT_QUALIFIED, exit 2; swtpm missing, image/key/certificate not supplied, no QEMU execution requested; virtual-trust-preflight.json |

The fixture tests were added after the 191149 chain's Python discovery and
were run explicitly. The fresh 192520 chain includes them in discovery.
Host-test mocked desktop/session PASS lines are not runtime QEMU evidence.
No new authenticated graphical journey, secure first-run provisioning, firmware
Secure Boot, physical TPM, or native hardware qualification is claimed.

## Network restriction diagnosis and current gate

The unchanged one-CPU live network harness passed outside the restricted
sandbox: DHCP, DNS, ICMP, UDP, TCP and final kernel/SMP markers all passed,
exit 33. No assertion, timeout, guest code or DNS name was changed. Evidence:
network-outside-sandbox-1cpu.log and copied serial/QEMU logs in the repair
evidence directory. This isolates the restricted-network execution boundary
as the cause of the preceding DNS failure, rather than proving a kernel DNS
regression.

Current full run: audit-artifacts/acceptance-20261104-194935-034/ on a980619e,
with cached Cargo dependencies and approved network access. IN PROGRESS.
No concurrent Cargo or runtime acceptance command may execute.
## Phase A closure

Status: COMPLETE for inventory/baseline, runtime source a980619e. The full
network-enabled RUN-STAGE10.7.ps1 run acceptance-20261104-194935-034 exited 0
with its final acceptance marker. Host Python: 208 tests, six skips. Build,
host/kernel warning-denying Clippy, Cargo host tests, 60 repeated memory/boot
runs, network 1/2/4 CPUs and isolated Stage10.4–10.7 1/2/4 matrices all pass.
All eight final feature lints (Stage12.1–12.5, Stage15.2, Stage32.1/32.2) pass.
Stage12.1–12.5 QEMU also passes all 15 cases; final-stage12-matrix.log retains
individual evidence paths. This does not accept later startup/security phases
or substitute mocked graphics output for real graphical qualification.

Phase A supplemental package gate: libwoven host units pass 212/212. Its
package-specific all-targets Clippy initially found an existing identity_op
in the shadow test's pixels[1 * 4 + 1]. Replacing that with pixels[5] preserves
the exact assertion; repaired Clippy and all 212 tests pass. Only cfg(test)
code changed after the accepted runtime source; no runtime matrix repetition
is claimed for that test-only spelling change. Original lint output retained.

## B1 configuration acceptance

Runtime aaeaaf8f: TESTED — PASS for the bounded B1 unit. All 27 real UEFI
configuration cases passed with the same prepared image/source hashes across
1/2/4 CPUs. Final case logs: audit-artifacts/boot-config-b1/final-{1,2,4}cpu.log.
The individual timestamped directories retain commands, source/image/firmware
provenance, serial/QEMU logs, and rejection framebuffer captures. Malformed
and oversized failure screens were visually inspected.

Existing Stage15.5 signed-handoff passed on 1/2/4 CPUs. The complete Windows
preservation run audit-artifacts/acceptance-20261104-205802-369/ exited 0:
213 Python tests (six skips), Rust host tests, warning-denying host/kernel
Clippy, 60 memory repetitions, network 1/2/4 and isolated Stage10.4–10.7.
Five actual-source parser tests and five host oracle/provenance tests pass;
standalone UEFI target and fixture-builder Clippy pass. No warning or assertion
was suppressed. The original trailing-data mismatch and initial upstream lint
failures remain retained. This does not accept full Phase B or authenticated
firmware boot. Next: missing/corrupt/unsupported kernel-image failure handling.

## Phase E authentication acceptance

Source: `1ff84dcc3dd7a34ae172b95ce7f01c26f3a1a75f`.

| Gate | Evidence | Result |
|---|---|---|
| Missing/invalid account withholds desktop and session | `target/phase-e-missing-account-{1,2,4}cpu` | PASS |
| Secure entropy unavailable/zeroized failure | `target/phase-e-entropy-failure-{1,2,4}cpu` | PASS |
| First-user installer provisioning | `target/phase-e-first-user-{1,2,4}cpu` | PASS |
| Protected verifier storage and real sign-in/lock | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| Owner binding, known/unknown lockout | `audit-artifacts/stage24.4-{1,2,4}cpu-*` | PASS |
| Identity syscall ABI / retired replay calls | `audit-artifacts/stage24.5-{1,2,4}cpu-*` | PASS |
| Durable Change-PIN flow | `target/woven-shell-change-pin-{1,2,4}cpu` | PASS |
| Full Windows preservation | `audit-artifacts/acceptance-20261110-123957-212` | PASS |

Affected kernel feature Clippy and workspace `cargo clippy --all-targets -- -D
warnings` passed. The full gate ran 220 host harness tests, host Rust tests,
warning-denying build/lints, 10 one-CPU, 30 two-CPU and 20 four-CPU memory boots,
live network 1/2/4 and Stage 10.4-10.7 1/2/4. Its read-only physical/trust
preflight correctly remained not qualified and made no device or firmware
change. No timeout, retry budget, marker or assertion was relaxed.

## Phase F graphical login acceptance

| Gate | Evidence | Result |
|---|---|---|
| Graphical sign-in, account navigation, wrong/correct PIN and lock/unlock | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| Normal/high-contrast visual and performance qualification | `target/phase-f-login-whpx-1cpu-20261110-171909` | PASS |
| WovenShell no-std target Clippy | `cargo clippy --manifest-path apps/woven-shell/Cargo.toml --target x86_64-unknown-none -- -D warnings` | PASS |
| Stage 20.12 kernel target Clippy | Stage 20.12 feature, warnings denied | PASS |
| Workspace host-target Clippy | `cargo clippy --all-targets -- -D warnings` | PASS |
| Full Windows preservation | `audit-artifacts/acceptance-20261110-172935-815` | PASS |

The final visual run captured and visually inspected `woven-login.ppm` and
`woven-login-high-contrast.ppm`. It met the existing 20-second input and
5-second screendump-frame budgets. The earlier retained runs
`target/phase-f-login-1cpu-20261110-134803` and
`target/phase-f-login-whpx-1cpu-20261110-{135314,135614,140242}` failed before
the harness separated input-to-render latency from QMP screendump latency; no
deadline was increased. The full gate ran 220 host harness tests, host Rust
tests, warning-denying build/lints, 60 memory boots, live network 1/2/4 and
Stage 10.4-10.7 on 1/2/4 CPUs. Physical/trust preflight remained read-only and
not qualified.

## Phase G session-management acceptance

Source: `7e9136e1e14981464da8f757e0c9166e40486b03`.

| Gate | Evidence | Result |
|---|---|---|
| Authenticated launch and exact inherited kernel identity | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| Lock suspends and valid unlock restores inherited authority | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| Logout revokes credentials, kills child, removes surface/tray and returns to sign-in | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| WovenShell no-std target Clippy | warnings denied | PASS |
| Stage 20.12 kernel target Clippy | warnings denied | PASS |
| Workspace host-target Clippy | warnings denied | PASS |
| Full Windows preservation | `audit-artifacts/acceptance-20261111-032505-125` | PASS |

The full gate ran 220 host harness tests, host Rust tests, warning-denying
build/lints, 10 one-CPU, 30 two-CPU and 20 four-CPU memory boots, live network
1/2/4 and Stage 10.4-10.7 on 1/2/4 CPUs. Its physical and virtual-trust
preflights were read-only and correctly remained not qualified. The earlier
incomplete run `audit-artifacts/acceptance-20261110-191838-383` is retained; it
ended during host/network setup without a PASS or FAIL marker and was not used
as acceptance evidence. No timeout, retry budget or assertion was relaxed.


## B2 accepted bounded unit at 72be85aa

All 45 source-bound configuration/kernel image cases pass on 1/2/4 CPUs:
- audit-artifacts/boot-config-1cpu-1793821233234070200/
- audit-artifacts/boot-config-2cpu-1793821318621793500/
- audit-artifacts/boot-config-4cpu-1793821396550297200/

The first two runs began before the checkpoint commit; their source hashes
match the checkpoint's normalized runtime sources. Logs are copied in
boot-image-b2/{1,2,4}cpu.log. Missing-kernel framebuffer was visually inspected.
Host tests, Clippy and formatting pass as recorded in qualification-commands.json.
Full Windows preservation passes: acceptance-20261104-214457-697/ (exit 0),
214 Python tests, six skips, host Rust tests, build/lints, 60 memory boots,
network and isolated Stage10.4-10.7 on 1/2/4 CPUs. Stage15.5 software signed
handoff and its kernel feature Clippy pass; boot-image-b2/ retains the logs.
Full Phase B and firmware Secure Boot qualification remain open.

## Phase H WovenShell integration acceptance

Runtime implementation checkpoint: `4558dc28` plus the current focused kernel
acceptance-oracle corrections.

| Gate | Evidence | Result |
|---|---|---|
| Desktop, bottom taskbar, readiness and production clock | `target/woven-shell-{1,2,4}cpu` | PASS |
| Application surfaces, modern chrome and rounded translucent notification | `target/woven-shell-apps-{1,2,4}cpu` | PASS |
| Bounded WovenStart, keyboard/pointer search, launch, empty-result feedback and focus return | `target/woven-shell-launcher-{1,2,4}cpu` | PASS |
| Window focus, close and rendering behavior | `target/woven-shell-windows-{1,2,4}cpu` | PASS |
| Production desktop session, F12 ownership transition and Quick Settings | `audit-artifacts/stage20.15-{1,2,4}cpu-*` | PASS |
| Phase G sign-in/lock/unlock/logout preservation | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| WovenShell target, affected kernel features and workspace host Clippy | warnings denied | PASS |
| Full Windows preservation | `audit-artifacts/acceptance-20261112-153757-168` | PASS |

The final source removes the obsolete full-screen Activities/workspace-preview
path rather than hiding it, renders an explicit localized `No matches` state,
and uses one rounded compositor blend for flat toast, Quick Settings and
notification-center cards. Stage 20.12 was updated to use the accepted bottom
geometry and deterministic absolute-pointer selection for system-menu rows;
its complete identity and teardown journey passes at 1/2/4 CPUs. The full gate
ran 220 host harness tests, warning-denying build/lints and host Rust tests, 60
repeated memory boots, live networking and Stage 10.4-10.7 on 1/2/4 CPUs. No
timeout, retry budget, security assertion or capability boundary was relaxed.

## Phase I full-system integration acceptance

Source: `41a78305`.

| Gate | Evidence | Result |
|---|---|---|
| Two consecutive authenticated login/logout cycles | `target/woven-shell-sign-in-{1,2,4}cpu` | PASS |
| Warm reboot | `audit-artifacts/stage13.20-reboot-{1,2,4}cpu-*` | PASS |
| Service crash/restart | `target/shell-regression/system-apps-{1,2,4}cpu-{1794493799031614800,1794493857107820100,1794493916080375200}` | PASS |
| WovenInit readiness-failure rollback | `target/shell-regression/system-apps-{1,2,4}cpu-{1794493974404661900,1794494031692067800,1794494111514118200}` | PASS |
| Installed WovenFS persistence | `target/installed-disk-release-20261112T143613Z/{1,2,4}cpu` | PASS |
| Three abrupt shutdowns plus recovery | `audit-artifacts/stage31-sudden-shutdown-{1,2,4}cpu-20261112T14*` | PASS |
| Cross-window input isolation and teardown | `target/woven-shell-input-isolation-{1,2,4}cpu` | PASS |
| Affected-feature and workspace Clippy | warnings denied | PASS |
| Full Windows preservation | `audit-artifacts/acceptance-20261112-164814-197` | PASS |

The integration pass exposed and fixed two real defects. Pre-session lifecycle
fixtures were incorrectly stopped by the production missing-account gate; only
the two test-only fixtures now defer that gate and still terminate before a
desktop session. Remote process termination also released identity authority
while holding the scheduler lock; revocation now occurs before scheduler
acquisition, closing the final-operation race and preserving lock order. A
failed pre-fix supervisor run is retained at
`target/shell-regression/system-apps-1cpu-1794493446151989400`.

The final full gate ran 220 host harness tests, warning-denying build/lints,
host Rust and WovenFS crash tests, 10 one-CPU, 30 two-CPU and 20 four-CPU
boots, live networking and Stage 10.4-10.7 on 1/2/4 CPUs. Physical and virtual
trust preflights remained read-only and not qualified.
