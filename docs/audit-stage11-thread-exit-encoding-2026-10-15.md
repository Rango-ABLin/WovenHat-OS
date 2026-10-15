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
