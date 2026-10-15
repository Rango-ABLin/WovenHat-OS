# Stage 11.2 thread exit-result encoding — source finding, unaccepted

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
