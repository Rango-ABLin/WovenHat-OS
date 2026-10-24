# Stage 19 WovenSecurity audit — 2026-10-14

WovenSecurity is a separate static Ring-3 application. It provides bounded,
read-only process identity and newest-first security-ledger reporting through
the existing `libwoven` audit ABI. If `TaskInspect` is unavailable it reports
that the ledger is unavailable; it never grants, revokes, or interprets
security policy.

The two host parser tests and warning-denying freestanding Clippy passed. The
signed external-application harness passed on 1, 2, and 4 CPUs with exit 33;
serial/QEMU evidence is preserved in
`audit-artifacts/woven-security-{1,2,4}cpu`.

The six-application Stage 19 catalog gate and 4-CPU production system-app
boot consume this package. Persistent security-policy UI and user-facing
consent workflows remain separate desktop/service work.
