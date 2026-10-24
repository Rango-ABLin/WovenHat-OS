# Process wait ownership classification — bounded correction accepted

## Final acceptance — 2026-10-15

After the earlier COW correction was accepted in `79b2f5a`, the wait-ownership
correction in source commit `ad11e1f1` is accepted on the same unchanged
500-input baseline. Session 94658 exited 0 with full combined acceptance PASS.
Real foreign process waits/joins and genuine owned-child errors passed on
1/2/4 CPUs, with exact resource baselines and the original thread gate intact.
Complete warning-denying/build/host and inherited Stage 10.7/10.8–11.5
preservation also passed. See [combined evidence](audit-exit-status-combined-acceptance-2026-10-15.md).

This accepts correct child-ownership classification and its existing-rank
lock sequencing, not full process/signals production closure, a new release,
combined-tree or physical qualification. Signed-status acceptance is recorded
next; compiler TLS and loader/hardware prerequisites remain open. Pending
statements below describe the preserved investigation timeline.

The real Ring-3 fixture's child called `libwoven::wait` on its live parent.
It expected the existing non-child error but received WouldBlock. Evidence:
`audit-artifacts/stage11.2-status-1cpu-1792087130955323000/`, child failure code
92 and enclosing fixture code 93, QEMU exit 35. The original task source is
retained there. This is distinct from both signed exit-result collisions and
the earlier COW stack fault, which had already been removed in this run.

`wait_process` inspected scheduler liveness before parent/child ownership.
It did not permit foreign reaping, but reported foreign live processes as
running children and disclosed liveness before validating the relationship.
The integrated correction first checks the child/parent pair under the existing
process-table guard, releases that guard, then inspects scheduler state. The
original final ownership/state check remains in the reap transaction. A sibling
waiter may reap between checks; the final transaction then returns NoSuchChild
instead of touching a reused/absent record. No scheduler lock is acquired under
the process-table lock, and no resource is reclaimed on the preliminary check.

The syscall number, u64 error sentinel, public libwoven return type and owned
running-child behavior are unchanged. This narrows classification to the
existing authority boundary; it grants no new permission, creates no object or
lock, and changes no cancellation, file/frame cleanup or asynchronous pointer.

The next one-CPU fixture passed foreign process wait and thread join rejection,
genuine owned-child WouldBlock, own-child signal termination, double reaping,
unknown PID errors, and exact process/frame/IPC/thread baselines:
`audit-artifacts/stage11.2-status-1cpu-1792087223023180400/`.
All six signed process statuses and natural/cancelled thread statuses also
passed, as did the original Stage 11.2 assertions. This is focused evidence,
**not acceptance**. The final revised focused 1/2/4-CPU matrix now also passes.
Session 94658 is running combined acceptance under the new 500-file source
freeze; full inherited warning-denying/build/host/runtime preservation remains
mandatory.
