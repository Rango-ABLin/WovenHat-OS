# Release readiness

Installed WovenFS now passes normal cold-reboot persistence and deterministic
post-commit/pre-target journal recovery on 1/2/4-CPU disposable images. This is
focused virtual-disk evidence; physical storage behavior and remaining
application migrations keep the release decision unchanged.

The disposable installer, A/B rollback, durable update-record and signed
WovenRecovery gates pass on current source, including 1/2/4-CPU execution.
They access regular-file images only. Physical installation, firmware state,
power-loss durability and production signing custody remain NO-GO items.

Local clean-room provenance and hardware-record policy validation pass. The
native support record intentionally contains zero passed categories, and
production provenance verification refuses absent custody inputs. Independent
review, production signatures and complete native lab evidence therefore keep
the release decision at NO-GO.

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Decision

NO-GO. Full current-source software acceptance and release qualification remain
open. The bounded bootloader and Stage 11 loader-preservation repairs have
focused acceptance evidence.

## Implemented and tested

Source-mode selection and isolated nested Cargo arguments are repaired and
covered by actual build-script execution. Locked dependency prefetch succeeds.

## Accepted

The loader-preservation increment passes 54 focused checks, 36 QEMU boots and
the full inherited Windows chain. This is bounded acceptance, not a newly
accepted full stage or release candidate.

## Blocked and not implemented

WHPX build-to-desktop qualification passes on 1/2/4 CPUs. TCG application
launch, application persistence and required CI need further acceptance on the
assembled branch. Production trust, physical hardware,
power-loss qualification and independent review remain open. The remaining
software scope is tracked in REMAINING-WORK.md.
