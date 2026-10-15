# Stage 19.8 timer/event wait-source increment — 2026-10-15

## Scope

The existing syscall 111 wait path covered input, mailbox, and compositor
damage. Timer and manual-reset event objects were available through the older
completion-handle API but could not be sources in one event-driven wait. This
increment adds syscall 206 (`WaitAnyObjects`) and the safe `libwoven` wrapper.

## Safety and ownership

- The kernel copies all 24-byte source records before registering a waiter.
- Records are bounded to eight, require unique one-bit masks, and validate the
  timer/event kind before touching an object.
- Object lookup remains owner- and generation-checked.
- An existing completion-handle waiter or another task's waiter is refused; a
  single task may register across multiple distinct objects without replacing
  any waiter.
- Timer expiry is consumed once; manual-reset events remain level-triggered.
- Every task registration is removed on success, timeout, or error.
- Object locks are released before task wakeups and no userspace pointer is
  retained while blocked.

## Evidence

- `cargo clippy -p wovenhat-os --all-targets -- -D warnings` — passed.
- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage19-8-test -- -D warnings` — passed.
- Same freestanding Clippy command for `stage19-13-test` — passed, protecting
  the neighboring feature gate from the new cfg-gated self-test.
- Stage 19.8 QEMU combined-wait gate, exit 33:
  - `target/hello-gui-multiplex-1cpu`
  - `target/hello-gui-multiplex-2cpu`
  - `target/hello-gui-multiplex-4cpu`

The userspace `hello-gui --timer-event` scenario creates a one-shot timer and
manual-reset event, waits on both, verifies the timer source, sets and waits on
the event source, and closes both objects. It passed on all three CPU counts.

Each serial log contains `[S19.8] timer/event object registration is level-safe
PASS` and `[S19.8] combined wait: PASSED`.

## Limit

This closes the timer/event source integration boundary for the new ABI. It
does not claim physical hardware qualification, multi-thread object waiter
fan-out, or completion-port replacement; each object still has one waiter
owner, while one task may wait on multiple objects.
