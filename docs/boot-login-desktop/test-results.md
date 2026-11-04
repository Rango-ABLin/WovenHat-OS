# Test results

Status: IN PROGRESS. No full directive phase is accepted by this unit.

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
runs, network 1/2/4 CPUs and isolated Stage10.4�10.7 1/2/4 matrices all pass.
All eight final feature lints (Stage12.1�12.5, Stage15.2, Stage32.1/32.2) pass.
Stage12.1�12.5 QEMU also passes all 15 cases; final-stage12-matrix.log retains
individual evidence paths. This does not accept later startup/security phases
or substitute mocked graphics output for real graphical qualification.

Phase A supplemental package gate: libwoven host units pass 212/212. Its
package-specific all-targets Clippy initially found an existing identity_op
in the shadow test's pixels[1 * 4 + 1]. Replacing that with pixels[5] preserves
the exact assertion; repaired Clippy and all 212 tests pass. Only cfg(test)
code changed after the accepted runtime source; no runtime matrix repetition
is claimed for that test-only spelling change. Original lint output retained.

## B1 configuration acceptance

Runtime aaeaaf8f: TESTED � PASS for the bounded B1 unit. All 27 real UEFI
configuration cases passed with the same prepared image/source hashes across
1/2/4 CPUs. Final case logs: audit-artifacts/boot-config-b1/final-{1,2,4}cpu.log.
The individual timestamped directories retain commands, source/image/firmware
provenance, serial/QEMU logs, and rejection framebuffer captures. Malformed
and oversized failure screens were visually inspected.

Existing Stage15.5 signed-handoff passed on 1/2/4 CPUs. The complete Windows
preservation run audit-artifacts/acceptance-20261104-205802-369/ exited 0:
213 Python tests (six skips), Rust host tests, warning-denying host/kernel
Clippy, 60 memory repetitions, network 1/2/4 and isolated Stage10.4�10.7.
Five actual-source parser tests and five host oracle/provenance tests pass;
standalone UEFI target and fixture-builder Clippy pass. No warning or assertion
was suppressed. The original trailing-data mismatch and initial upstream lint
failures remain retained. This does not accept full Phase B or authenticated
firmware boot. Next: missing/corrupt/unsupported kernel-image failure handling.
