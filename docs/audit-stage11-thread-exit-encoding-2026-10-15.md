# Process/thread exit-result encoding — bounded correction accepted

## Final acceptance — 2026-10-15

Following COW acceptance `79b2f5a` and wait-ownership acceptance `6c57a17`,
the signed-status correction in `ad11e1f1` is accepted. Session 94658 exited 0
with full combined acceptance PASS; all 500 frozen inputs matched at completion
and independent inspection. The expanded actual-libwoven fixture and original
thread gate passed on 1/2/4 CPUs. All six process statuses and twelve natural/
cancelled thread cases, stale/double joins, child ownership and genuine errors,
signal termination and exact resource baselines are covered. Full Stage 10.7,
all twenty-one Stage 10.8–11.5 runtime boots, all host tests, nineteen libwoven
and nineteen Python tests, build and warning-denying host/fixture/production/
fifteen-feature Clippy passed. See [combined evidence](audit-exit-status-combined-acceptance-2026-10-15.md)
for authoritative paths and independent audit checks.

The public wait type, syscall numbers and full-width errors are unchanged;
successful signed exit statuses now have an unambiguous low-32-bit payload.
This accepts the bounded encoding correction, not full Stage 11, combined-tree
integration, a new release or hardware readiness. Compiler TLS, fixed-ET_EXEC
placement and general loader interoperability are the next open prerequisites.
Pending/read-only statements below are the retained investigation timeline,
not the final acceptance decision. No original failing evidence was removed.

## Current executable correction — 2026-10-15

The original casts were extracted without changing behavior into the production
`exit_status::encode` helper, and both syscall branches were wired through it.
Actual-source host tests reproduced the kernel payload collision and both
libwoven decoder failures (exit 101):
`audit-artifacts/exit-status-20261015/kernel-before.txt` and `libwoven-before.txt`.
The helper now zero-extends through `u32`. Kernel payload tests and nineteen
libwoven tests pass (`kernel-after.txt`, `libwoven-after.txt`). The actual
thread decoder is shared with its test rather than copied. Syscall IDs, full-width
error constants, `Join` variants, and the public process wait return type remain
unchanged. A successful wait retains the status's low 32 bits; `as i32` recovers
the signed value. No unsafe code, object, capability, lock or wait was added
for the encoding correction.

`tools/exit-status-fixture` is a separate freestanding crate linked against the
actual libwoven library, using the established static ELF linker contract.
Its only assembly establishes CPU entry-stack alignment. It exercises six
signed process exits, six natural thread exits, six cancelled sibling statuses,
double reap/stale joins, cross-process authority, real WouldBlock and invalid
errors, and signal-terminated child cleanup. Workers park on timer events;
nonblocking process waits sleep between retries. It uses existing test process
capabilities, not an expanded capability set or unsigned production image path.

The independent `stage11-exit-status-test` subgate deliberately includes the
existing Stage 11.2 feature. Its guarded block runs first and then falls through
to the unchanged original thread assertions and exit. The runtime harness's
`11.2-status` choice builds the fixture, supplies its mandatory compile-time ELF
input, and requires every new family marker plus the original thread markers.
It cannot pass merely by reaching an earlier exit. Linting that new feature
requires `WOVENHAT_EXIT_STATUS_ELF`; missing input is not a pass.

Real execution exposed earlier COW ancestor-permission and wait-ownership
classification defects. They are documented separately in the Stage 7 COW and
process-wait ownership audits; all failing serial logs and source snapshots
are preserved. After their corrections, the full one-CPU fixture and original
Stage 11.2 gate passed in `stage11.2-status-1cpu-1792087223023180400/` with exact
resource baselines. A final revised 1/2/4-CPU matrix is running. Full lint,
host, inherited runtime and combined-source preservation are still required;
none of these integrated corrections is accepted yet. The original read-only
findings below remain investigation history.

### Final revised focused matrix and current full gate

All three revised focused boots passed, exit 33, with every libwoven family,
resource-baseline and original Stage 11.2 marker required by the harness:

- `stage11.2-status-1cpu-1792087580234175000/`
- `stage11.2-status-2cpu-1792087612734709000/`
- `stage11.2-status-4cpu-1792087650972914800/`

All nineteen Python harness/builder regressions pass. The builder uses a locked
dependency graph and shared project-local Cargo caches, records compiler and
input/output hashes, refuses existing evidence directories, and refuses to
publish a manifest if any input changes during compilation. Missing markers,
wrong exit, panic and failure strings remain fatal in the runtime harness.

Session 94658 is running the complete combined acceptance from
`audit-artifacts/exit-status-20261015/run-acceptance.ps1`, with the immutable
500-file manifest alongside it. This freeze also includes the ELF and its
published manifest supplied to new-feature Clippy. It is separate from all
historical thread/IPC manifests. Full build, host and fixture Clippy, all tests,
fifteen affected kernel-feature lint configurations, full Stage 10.7 and
Stage 10.8–11.5 runtime preservation remain required before acceptance.

Read-only review during the serialized runtime wait. No kernel/runtime input
was changed and no competing test chain was started for this finding.

Reviewed SHA-256:

- `kernel/src/syscall.rs`: `da08696e03b020409cb876aac4406c58b5ce57a2eaceb1d74e0b1bdbb6848af1`
- `libwoven/src/lib.rs`: `67a4f819ca3988f17ce225b5dbd9e9907d06fdfd95a3f2718d397267ac0e335b`

## Static counterexample

Thread exit and cancellation accept an `i32` status. The registry and task join
return that signed status successfully; the `ThreadJoin` syscall branch encodes
`Ok(exit)` as `exit as u64`. `libwoven::syscall::thread_join` reserves the exact
64-bit `ERROR` and `WOULD_BLOCK` values before decoding other results as `i32`.

| Successful thread status | Kernel's current result | Userspace interpretation |
| --- | --- | --- |
| `-1` | `u64::MAX` | `Join::Invalid`, not `Exited(-1)` |
| `-2` | `u64::MAX - 1` | `Join::Running`, not `Exited(-2)` |
| `i32::MIN` | Sign-extended 64-bit value, not either sentinel | `Exited(i32::MIN)` |
| `0`, positive `i32` | Nonnegative 64-bit value | `Exited(status)` |

The syscall blocks for a running thread, so a completed `-2` status is not an
actual would-block response. Registry join can already have reclaimed the
record/resources before userspace misclassifies the result. Cancellation has
the same return contract because its supplied `i32` status is joined normally.
This counterexample follows the inspected casts/decoder; it is not yet an
executed host or QEMU regression.

A separate concern about high-bit thread IDs was checked and not substantiated:
the ABI uses exact sentinels, not a generic signed-negative check. Valid thread
slots are 0–63 in the low 32 bits, so generation-tagged IDs cannot equal either
all-ones sentinel. Do not unnecessarily reduce the generation domain or alter
the handle format to fix a nonexistent sign-bit restriction.

## Required correction and evidence

After the current generation correction and earlier IPC gate, make successful
32-bit exit payloads unambiguous from 64-bit error sentinels. Zero-extending the
status's `u32` representation is a candidate: the existing `as i32` decoder can
recover the original signed status while `u64::MAX`/`MAX-1` stay distinct. Review
all raw syscall consumers before selecting that encoding. Preserve syscall
numbers, owner checks, blocking semantics, ordinary statuses and teardown.

Require actual encode/decode host regressions and Ring-3/libwoven join tests for
`-1`, `-2`, `i32::MIN`, `i32::MAX`, zero and an ordinary positive status, including
both natural thread exit and requested sibling cancellation. Invalid/stale or
foreign-owner joins must still fail, and joins must reclaim stack/TLS/registry
resources exactly once. Run warning-denying affected checks, Stage 11.2 and
inherited 1/2/4-CPU preservation without weakening assertions or timeouts.

This finding does not change the frozen generation correction or certify
Stage 11.2/Stages 1–19. The ABI correction remains a separate unaccepted gate.

## Earlier process wait path has the same collision — source review

Read-only follow-up during IPC session 63244; no new test chain or source
edit. `task::wait_process` returns `Result<i32, WaitError>`, and the `Waitpid`
syscall encodes `Ok(exit_code)` with the same sign-extending `as u64` cast.
Its explicit errors are `u64::MAX - 1` for `StillRunning` and `u64::MAX` for
`NoSuchChild`. `libwoven::wait` passes the result through `check`, which maps
those exact values to `Error::WouldBlock` and `Error::Failed` respectively.
Successful child statuses `-2` and `-1` are therefore indistinguishable from
those errors even after the kernel has consumed the zombie and reclaimed its
resources. This is a source counterexample, not executed process regression
evidence. The reviewed syscall/libwoven hashes above still match; task source
SHA-256 is `11b2cd9d1c61903f2624fae10e0eb2b7e670bdd57530f555ccc3c474d1c16a3c`.

The current shell's `reap` treats `WouldBlock` as still running; it can thus
temporarily retain a completed `-2` child until a subsequent wait reports no
child. That does not prove a kernel resource leak or indefinite shell retention.
The reviewed shell SHA-256 is
`59514e1e23ae695a0db9774fd7c5ae4a3cec4b35d9a631f4262fbd4ea0a90127`.
Existing boot wait/reap tests exercise successful status zero and double-wait
rejection; those assertions do not cover this signed boundary.

Resolve this earlier process-wait encoding together with the thread-status
prerequisite after IPC acceptance, before advancing loader/TLS work. Preserve
syscall 5, the existing public `Result<u64, Error>` wait signature, actual-running
and no-child errors, parent ownership, zombie/resource reclamation and all
ordinary positive statuses. Review raw consumers and explicitly document the
32-bit status payload representation before sharing an encoding helper; do not
change the type or invent a new wait ABI silently. Require host production
encode/decode tests and real Ring-3 parent/child waits for `-1`, `-2`, `i32::MIN`,
`i32::MAX`, zero and positive status, actual-running/no-child/foreign-child
errors, double reaping and exact resource baselines. Original thread natural
exit/cancellation coverage remains required as well.
