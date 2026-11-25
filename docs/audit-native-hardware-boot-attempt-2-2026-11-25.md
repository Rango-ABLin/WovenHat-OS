# Native hardware boot attempt 2 — 2026-11-25

Image and preparation: `docs/audit-native-hardware-boot-preparation-2026-11-25.md`.
Observed fallback context: `docs/audit-native-hardware-boot-attempt-1-2026-11-25.md`.

## Requested tester actions

The user confirmed they retried the same native boot and performed the requested
terminal commands:

```text
note native boot reached diagnostic shell; F1 fallback only
savelog
tasks
```

The user then confirmed the exact screen and command replies:

- Under `WOVENHAT DIAGNOSTIC SHELL (F1 TO OPEN DESKTOP)`, the status line was
  `SESSION: pid N has not shown the desktop yet (still running)`. This selects
  the hung-before-display outcome in `kernel/src/main.rs:1904-1911`, not a
  spawn failure or early exit.
- Both `note` and `savelog` replied that there was **no live-test stick**.
  This selects the `flush_now() == false` path in
  `kernel/src/shell.rs:156-180`: the live-log object was never initialized on
  this native boot.

A photograph was not supplied; the exact text answers are preserved above.

## Returned media

The `WOVENLOG` FAT32 volume came back healthy but empty. A recursive read-only
listing found no entries, including no `LOGS` directory and no
`BOOTnnnn.TXT`. Therefore neither `note` nor `savelog` produced a durable file
on this host, and the deferred live-log worker again failed to initialize.

Evidence boundary:

- `\\?\Volume{4b5cfdc7-c97b-4356-b8a1-6ab87023657c}\` — no entries.
- The volume remains FAT32, healthy, removable, and correctly labelled.

## Interpretation

This confirms two native failures on this host:

1. WovenShell is spawned and still alive but never reaches display activation
   within the bounded wait; the diagnostic fallback remains active.
2. The native USB/WOVENLOG live-log object is absent, so tester notes and
   `savelog` cannot be persisted.

No native category is passed by this attempt. These are recorded as native
failed observations in the Stage 29 manifest, so the fail-closed classification
becomes Tier 3 until the defects are diagnosed and a fresh native boot passes.
