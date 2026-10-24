# Stage 24.14 acceptance audit — bounded administration query UI

Date: 2026-10-21

## Delivered

This stage adds a bounded, read-only administration query ABI and integrates it
with WovenSecurity. An elevated owner-bound administrator session can query
non-secret user and group records, including IDs, account/group names, primary
GIDs, administrator flags, and bounded member IDs. The ABI has fixed maximum
record counts and explicit version/kind/count framing; it never returns salts,
password hashes, PINs, or session credentials. The kernel copies all request
and response buffers through the existing userspace validation path and records
the query as audit action 36.

WovenSecurity now provides a keyboard-driven administrator prompt and bounded
directory view. The session is closed and PIN storage is cleared on window
close or failed query. The hardware-demo fixture promotes only its first
volatile demo account to administrator so the UI path is testable; production
account loading does not promote accounts.

## Acceptance evidence

- `cargo test -p libwoven`: 88 passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- WovenSecurity freestanding Clippy and production system-apps kernel Clippy:
  passed.
- `git diff --check`: passed.
- Focused WovenSecurity 1-CPU QEMU acceptance passed; serial evidence contains
  `WovenSecurity: administrator directory ready`.
- Signed system-apps shell acceptance passed on 1, 2, and 4 CPUs.

Preserved logs are under
`audit-artifacts/stage24.14-admin-query-ui-20261021/`, with per-CPU shell
serial logs and the focused WovenSecurity serial log.

## Security and lifecycle review

The query requires an owner-bound live session and elevated administrator
state. The query syscall itself is not gated by the desktop AccountManage
capability because WovenSecurity is a normal signed application; the identity
authority remains the security boundary and requires the administrator
elevation proof. Mutation syscalls retain their existing capability and
elevation checks. Output is bounded before allocation and every UI-owned
session is reclaimed on close.

## Limits and next prerequisites

This is a query/directory surface, not the final mutation-management UI.
Configurable durable lockout, hardware-backed credentials, malformed-request
telemetry, release-key provisioning, physical USB qualification, and the
remaining production/release gates are still open. The attached USB device was
not written, reformatted, or otherwise modified.
