# WovenSecurity

WovenSecurity is a bounded read-only Ring-3 client for the kernel security
ledger. It reports the caller identity and a capped newest-first audit view
when the process has the `TaskInspect` authority. It never grants, revokes, or
interprets policy and does not expose raw kernel pointers.

Modes are `--all`, `--identity`, `--audit`, and `--help`.
