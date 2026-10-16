# Audit: IPC message receive race (2026-10-16)

**Baseline:** `phase2/userspace-platform` `9c3e6fc`.
**Result:** defect fixed in source. Host and lint validation pass; every
boot's IPC self-test now covers the race. QEMU evidence comes from CI.

## Defect

`sys_message_receive` (syscall 12) delivers a message in three steps:
1. `ipc::peek` copies the head of the caller's mailbox.
2. The payload is copied to user memory with no lock held, because the
   copy can fault.
3. `ipc::receive` dequeues the head.

Peeking first is deliberate: if the copy-out fails, the message stays
queued and is not lost.

Mailboxes belong to a process, and a process can now have several threads
(Stage 11). If two threads of one process receive at the same time:

```
thread A: peek -> M1        thread B: peek -> M1
thread A: copy M1           thread B: copy M1
thread A: receive (M1)      thread B: receive (M2)   <- M2 dequeued, never delivered
```

M1 is delivered twice and M2 is silently lost. No privilege is crossed, so
this is a data-integrity defect, not a security one. It affects any
multi-threaded program that receives messages from more than one thread.

## Fix

- **Dequeue counter.** `LegacyEndpoint` counts its dequeues
  (`dequeued`, wrapping). `peek` returns `(Message, serial)`.
- **Conditional dequeue.** The new `receive_if(receiver, serial)` dequeues
  only if no dequeue happened since that peek; otherwise it returns
  `QueueEmpty`.
- **Retry.** `sys_message_receive` loops: peek, copy out, `receive_if`. A
  thread that loses the race copies the *next* message instead. It returns
  `SYSCALL_ERROR` only when the mailbox is empty, the buffer is too small,
  or the copy-out fails.
- **Termination.** Each retry requires another receiver to have dequeued,
  so the loop ends once the queue drains.
- **Cleanup.** The unused, race-prone public `ipc::receive` wrapper is
  removed. `State::receive` remains for the self-test.

ABI unchanged. The handle-based endpoint IPC (Stage 8.x) dequeues
atomically and was not affected.

## Evidence

- **Boot self-test.** `ipc::self_test()` runs on every boot and halts on
  failure. It now simulates the race on the real global IPC state: two
  peeks of the same head, the first dequeue succeeds, the stale serial gets
  `QueueEmpty` without taking or dropping the next message, and a fresh
  peek returns that next message.
- **Kernel Clippy (`-D warnings`).** Clean for default, `qemu-test`,
  `system-apps`, `stage20-15-test` and `stage19-1`–`stage19-4-test`.
- **Host.** `--all-targets` Clippy is clean; `cargo test` passes 154
  tests.
- **Not run.** No local QEMU (acceptance hold). CI runs every gate on
  1/2/4 CPUs, and every boot runs the self-test.
