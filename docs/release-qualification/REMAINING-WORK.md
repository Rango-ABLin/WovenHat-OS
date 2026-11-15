# Remaining work

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Current order

1. Finish signed image construction and interactive UEFI boot qualification.
2. Run build, warning-denying Clippy, Rust host tests and inherited Windows gate.
3. Reconcile and qualify Stage 11.5 loader repairs with retained failure evidence.
4. Assemble newer shell/oracle work while preserving accepted ABI and tools.
5. Implement and qualify missing application operations and persistence.
6. Close Stage 1-19 software requirements before formal Stage 20 acceptance.
7. Qualify installation, signed update, rollback and recovery on disposable disks.
8. Complete release provenance, independent review and hardware support records.

## External release prerequisites

Authorized production signing material, independent review and named physical
platform evidence remain blocked until available. All current tests use
virtual images and development credentials appropriate to their test harness.
