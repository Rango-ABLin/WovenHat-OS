# External review and native hardware record

Recorded 2026-11-20 for current `master`.

## Repository-verifiable provenance

The isolated `git archive` clean-room build completed with locked, offline
Cargo resolution. Its report is retained at
`audit-artifacts/clean-room-build-20261120.json`; 256 libwoven tests passed and
one already-declared source-fixture test remained ignored. Hardware-report and
release-package policy contracts pass. This demonstrates reproducible in-tree
source consumption, not an independent organization or release signature.

## Independent review

**BLOCKED — no independent reviewer attestation is present.** Repository agents,
including Codex and Claude Code, are not independent external organizations.
No reviewer identity, signed review scope, finding disposition, or detached
attestation has been supplied. This record must not be promoted until those
artifacts are provided and verified against an exact commit.

## Production signing custody

**BLOCKED PENDING HARDWARE CEREMONY — the repository now supports non-exportable
PKCS#11 signing, but no production token or public-key evidence is provisioned.**
The release tools require separate boot, application and SDK public keys plus
exactly one external hardware signer; ambiguous seed-and-token authority fails
closed. Development signatures and test keys do not establish release custody.
See `KEY-CUSTODY.md` for the witnessed generation, verification and rotation
procedure.

## Native hardware support

**BLOCKED — no complete native lab matrix is authorized or present.** The
companion `hardware-support-record.json` enumerates every required category as
blocked. QEMU and emulated-device results are retained elsewhere but cannot be
relabelled as native Tier 1 evidence. No physical device, firmware setting,
boot entry, or storage medium was accessed during this pass.

To advance this record, a named lab owner must provide repository-relative
evidence for each tested device, including exact identifiers, firmware,
configuration, result, logs, destructive-test authorization where applicable,
and the source commit. The independent reviewer must then sign the resulting
scope and unresolved findings.
