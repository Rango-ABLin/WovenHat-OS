# Blockers and prerequisites

Status: IN PROGRESS.

1. Baseline execution and source/security audit must finish before runtime
   edits. Current Windows evidence path is in test-results.md.
2. Firmware implementation is resolved external bootloader-x86_64-uefi 0.11.17,
   not the local UefiBoot wrapper. Its source was located and traced; config
   parsing silently defaults after errors and needs a gated rejection change.
3. Secure production entropy for account provisioning is not established.
4. No-account startup, credential-record disclosure/replay, diagnostic console
   and pre-authentication capabilities need negative security qualification.
5. Native firmware/Secure Boot/TPM/physical media qualification cannot be inferred
   from QEMU. Do not mutate host media or firmware during software acceptance.

These are prerequisites/findings, not claims that all feasible work is blocked.
