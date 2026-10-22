# WovenRecovery

`wovenhat-recovery` is the bounded Stage 27.5 recovery-environment
foundation. It provides `status`, `check`, and `logs` read-only commands and
explicit `reset`/`reinstall` admission probes.

The kernel requires both an explicitly delegated `Recovery` capability and an
authenticated recovery boot-mode handoff. The current kernel has no production
boot-mode handoff yet, so ordinary boots fail closed. Even with both gates,
reset and reinstall report “not implemented”; this app does not erase disks,
restore snapshots, repair boot records, or install an image.
