# Remaining work

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Current order

1. ~~Persistent installed-settings/service migration coverage.~~ Bounded v1
   to v2 migration, startup write-back and signed 1/2/4-CPU service evidence
   are complete; future schemas require another explicit migration.
2. ~~Broader virtual WovenFS crash-recovery boundaries.~~ Descriptor, staged
   payload, commit, target, target-flush, checkpoint and retirement pass on
   1/2/4 CPUs with replay, overwrite and cleanup evidence.
3. Reconcile the next dependency-ordered Stage 11 executable-layout gap with
   retained failure evidence and the accepted loader ABI.
4. Qualify volatile physical-controller cache and torn-sector behavior on
   authorized native media; physical power-loss/native-media evidence remains
   external.
5. Assemble newer shell/application work while preserving accepted ABI and tools.
6. Implement and qualify the remaining application operations and service data
   migrations now that the bounded native WovenFS reboot/replay gate passes.
7. Close Stage 1-19 software requirements before formal Stage 20 acceptance.
8. Extend the accepted disposable-image installation, signed update, rollback
   and recovery gates to physical media, firmware integration and power-loss
   qualification; the bounded regular-file/QEMU scope passes on current source.
9. Obtain the external inputs documented in
   `EXTERNAL-REVIEW-AND-HARDWARE.md`: signed independent review, production
   signing custody and named native-lab results. Local clean-room provenance
   and conservative blocked-category hardware records now pass validation.

## External release prerequisites

Authorized production signing material, independent review and named physical
platform evidence remain blocked until available. All current tests use
virtual images and development credentials appropriate to their test harness.
