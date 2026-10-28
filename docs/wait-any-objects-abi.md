# Timer/event unified wait ABI

Syscall `206` (`WaitAnyObjects`) extends the existing Stage 19.8 wait path
without changing syscall `111`. Its arguments are:

```text
rdi = absolute timer deadline, or UINT64_MAX
rsi = readable userspace pointer to an array of records
rdx = record count, 1..8
```

Each record is 24 little-endian bytes:

```text
u64 object   generation-tagged timer/event object
u64 kind     0 = timer, 1 = manual-reset event
u64 mask     one unique bit in bits 3..63
```

The kernel copies the complete array before registering any waiter. It then
registers the current task with each object, rechecks readiness, and sleeps
event-driven until a source, the deadline, or an error occurs. All task
registrations are removed before return. A timer expiry is consumed once;
manual-reset events remain ready until cleared. A live completion-handle waiter
makes that object unavailable rather than replacing the existing owner. Manual-
reset events accept up to four distinct task waiters and wake all of them;
one-shot timers retain a single consuming task waiter so expiry is not
duplicated.

The safe `libwoven::wait_any_objects` wrapper bounds the source array to eight
records and performs the same packed encoding. The ABI deliberately does not
change the existing input/message/damage mask contract, and object handles are
still owner- and generation-checked by the kernel.
