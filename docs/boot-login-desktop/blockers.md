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
6. The synchronized baseline failed host all-targets Clippy before QEMU.
   BLD-A1 repairs now pass focused host tests and default/Stage12.5/Stage15.2
   target lints; full rerun is pending. Original failure retained at
   audit-artifacts/acceptance-20261104-190707-667/.
7. Original workspace formatting failed on inherited code (417 diff hunks); report is
   audit-artifacts/boot-login-baseline-repair-20261104/workspace-format.log.
   BLD-A3 now resolves this in a980619e with one explicit mixed-edition style.
   Formatting and host Clippy pass; the integrated full runtime gate is pending.
8. Actual virtual-trust preflight exits 2: swtpm is unavailable, and a signed
   disk image/development key/certificate have not been supplied. Evidence:
   audit-artifacts/boot-login-baseline-repair-20261104/virtual-trust-preflight.json.
   The preflight used installed CODE and VARS files and requested no QEMU run;
   it cannot qualify a trust chain or certify matching key enrollment.

These are prerequisites/findings, not claims that all feasible work is blocked.
