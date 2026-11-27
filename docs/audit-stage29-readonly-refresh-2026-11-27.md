# Stage 29 read-only refresh — post-CI-repair (2026-11-27)

## Scope

This bounded Stage 29 bit re-ran the safe Z1/Z2 preflight and classifier at
commit `308a31f2`, after the CI repair. It is read-only and fail-closed. No
reboot, firmware change, boot entry, TPM operation, raw disk open, USB write,
power control or native boot was performed. No QEMU result was relabelled as
native evidence.

## Tooling defect fixed

`scripts/qualify-hardware-report.py` wrote its `--output` file without creating
the parent directory, so a fresh evidence path failed with `FileNotFoundError`.
The script now creates the parent directory before writing and still emits the
same classification document. `tests/test_hardware_qualification.py` gained a
subprocess regression test for the fresh nested output path; a duplicate
identical test method was removed without changing assertions.

## Evidence

| Check | Result |
| --- | --- |
| `python scripts/physical-qualification-preflight.py` | exit 0; 17 required native categories remain `PHYSICAL_NOT_TESTED`; all safety flags false |
| `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\physical-host-preflight.ps1` | exit 0; HP 290 G4 Microtower PC, Windows 11 `10.0.26300`, not elevated, `PhysicalTestPerformed=False`; Secure Boot and BCD queries fail closed on access denied; no serial ports |
| `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\test-physical-tpm-readiness.ps1` | exit 0; TPM not visible/ready, no swtpm/live TPM path, no signed provisioning record, 0 USB targets, all safety flags false |
| `python scripts/qualify-hardware-report.py --input docs/release-qualification/hardware-support-record.json --output audit-artifacts/hardware-qualification-20261127-post-ci/classification.json` | exit 0; valid, Tier 2, 17 blocked/missing native categories, 0 native passes |
| `python -X utf8=0 -m unittest tests.test_hardware_qualification -v` | 11 tests, OK |

Artifacts:

- `audit-artifacts/physical-qualification-preflight/report.json`
- `audit-artifacts/physical-host-20261127-175136-660/host.json`
- `audit-artifacts/physical-tpm-readiness-20261127-175140/readiness.json`
- `audit-artifacts/hardware-qualification-20261127-post-ci/classification.json`

The host inventory at this refresh does not list the previously authorized
VendorCo spare USB; this is a current-readiness observation only and does not
change that device's prior bounded Phase 1 record.

## Gate result and next prerequisites

The read-only Z1/Z2 bit remains complete at the repaired commit. Z3 native
qualification is still blocked. It requires owner-controlled elevation, a
visible ready TPM plus signed provisioning record for trust work, physical
console/serial capture and power control for boot phases 2-4, and a freshly
confirmed spare non-boot device. Classification remains valid **Tier 2** with
17 blocked native categories and 0 native passes. No Tier 1 or production
hardware claim is made.
