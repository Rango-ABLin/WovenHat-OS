# Stage 15.3 permission broker audit — 2026-10-13

Stage 15.3 now has a bounded runtime permission broker. Requests carry a
generation-tagged handle, owner task, requested authority, and reason digest;
the state machine distinguishes pending, resolving, granted, denied, and
revoked. Capacity is fixed at 32 entries and stale handles fail closed.

Decisions require `TaskControl`, and successful grants/revokes still traverse
the existing WovenGuard lineage and sandbox checks. Request, decision, and
revoke events are recorded in the security ledger. The userspace ABI exposes
request, poll, decide, and revoke operations as syscall numbers 122–125, with
typed `libwoven` wrappers.

Validation:

- freestanding Stage 15.3 Clippy: passed with `-D warnings`;
- host/all-target Clippy: passed with `-D warnings`;
- 1 CPU: `audit-artifacts/stage15.3-1cpu-1791899076247234100`;
- 2 CPUs: `audit-artifacts/stage15.3-2cpu-1791899112419732700`;
- 4 CPUs: `audit-artifacts/stage15.3-4cpu-1791899123374724100`;
- marker: `[S15.3] permission broker: PASSED`.

This closes the kernel broker and ABI boundary. A graphical permission UI,
application identity/signature policy, and persistent user consent database
remain later Stage 15 requirements.
