# Stage 15.8 security-ledger ABI audit — 2026-10-13

Stage 15.8 now exposes a bounded, newest-relative security-ledger read ABI.
The kernel authorizes reads with `TaskInspect`, validates the userspace output
buffer, and returns a fixed 48-byte record without exposing internal ledger
layout. `libwoven` decodes the record into a typed `AuditEvent`; unknown action
values fail closed instead of being silently reinterpreted.

The read path is newest-relative and bounded by the existing ledger capacity.
Sequence, tick, CPU, action, allow/deny, actor, target, and detail fields are
copied only after the event is read under the ledger lock. The gate also checks
that a fresh permission-decision event can be observed through the ABI.

Validation:

- `cargo build --features stage15-8-test`: passed;
- freestanding kernel Clippy with `-D warnings`: passed;
- host/all-target Clippy and host tests were previously green before this
  gate; the shared checks are rerun after the Stage 15.8 documentation update;
- 1 CPU: `audit-artifacts/stage15.8-1cpu-1791899464254186200`;
- 2 CPUs: `audit-artifacts/stage15.8-2cpu-1791899481574683100`;
- 4 CPUs: `audit-artifacts/stage15.8-4cpu-1791899497818349600`;
- marker: `[S15.8] security ledger ABI: PASSED`.

This closes the kernel and userspace ledger-read boundary. A full security
center UI, durable export/retention policy, signed identity model, and secure
or measured boot remain product-hardening work; this gate does not claim those
features.
