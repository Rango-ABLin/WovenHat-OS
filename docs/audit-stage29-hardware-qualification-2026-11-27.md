# Stage 29 hardware qualification — Z1/Z2 pass, Z3 still blocked (2026-11-27)

**Scope:** read-only host preflight, read-only TPM/USB readiness, virtual hardware
matrix, and the QEMU `physical-probe` inventory smoke. No boot entry, firmware
setting, TPM, physical disk, radio or USB target was modified. No native device
was booted and no physical evidence was accepted.

## Repairs made to unblock the bit-by-bit run

1. `kernel/src/physical_probe.rs` previously declared its two probe entry points
   as `-> !`. That made the remaining normal boot path unreachable to Rust's
   lints under `--features physical-probe`, producing `unused variable:
   maintenance_boot` and dead-code errors. The probes still halt the machine,
   but now return unit so the rest of the boot image remains type-reachable and
   warning-denying stage-feature lints remain honest. No `allow`/`expect` was
   added.
2. `scripts/qualify-hardware-report.py` had a strict cross-platform helper,
   `_repository_relative_evidence_path`, that rejects backslashes, NUL bytes,
   drive/UNC anchors and `..`; `validate()` bypassed it with an inline check.
   A NUL-containing evidence path was therefore accepted. Validation now uses
   the helper, so the fail-closed contract is enforced on Linux and Windows.

## Z1 — host and trust preflight (read-only)

- `python scripts/physical-qualification-preflight.py` → schema
  `wovenhat.hardware-qualification-preflight/v1`, 17/17 native categories
  `PHYSICAL_NOT_TESTED`, all safety flags false.
- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
  scripts\physical-host-preflight.ps1` → HP 290 G4 Microtower PC, Windows 11
  `10.0.26300`, 12 logical CPUs, ~21.3 GB RAM, HypervisorPresent, **not
  elevated**. Secure Boot and boot-configuration queries failed closed on
  access denied. Disks: SK hynix BC901 NVMe (boot/system), ST500DM002 SATA/RAID,
  one 100 GB file-backed virtual disk. Wi-Fi enabled, GbE disabled, no serial
  ports. `PhysicalTestPerformed=False`.
- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
  scripts\test-physical-tpm-readiness.ps1` → TPM not visible/not ready, no QEMU
  TPM backend, no signed provisioning record, 0 USB candidates. Read-only.

Evidence: `audit-artifacts/physical-host-20261127-155316-375/host.json`,
`audit-artifacts/physical-tpm-readiness-20261127-155329/readiness.json`.

## Z2 — virtual matrix and probe image

- `python scripts/run-hardware-qualification-matrix.py` → exit 0; the existing
  `audit-artifacts/hardware-qualification-matrix-*` summary records the 108-job
  virtual matrix with `PHYSICAL_NOT_TESTED` children and no raw/physical flags.
- `python scripts/test-physical-probe.py --cpus 1|2|4` → inventory QEMU smoke
  **PASS** at 1, 2 and 4 configured CPUs. The image printed the required
  read-only PCI/CPUID/ACPI inventory markers, halting before storage/network
  runtime, AP startup and candidate QEMU tests. Each result explicitly records
  `physical_wifi_acceptance: false`.
- `python -m unittest discover -s tests -p 'test_hardware_qualification*.py'` →
  **18/18 pass** after the validator repair.
- `python -m unittest discover -s tests -p
  'test_physical_qualification_preflight.py'` → pass.
- `python scripts/qualify-hardware-report.py --input
  docs/release-qualification/hardware-support-record.json --output
  audit-artifacts/hardware-qualification-20261127/classification.json` → valid,
  **Tier 2**: 17 blocked observations, 17 missing native categories, zero
  native-passed categories.

Evidence: `audit-artifacts/physical-probe-1cpu-1795788187705896500/`,
`audit-artifacts/physical-probe-2cpu-1795788272878951900/`,
`audit-artifacts/physical-probe-4cpu-1795788284314476000/`,
`audit-artifacts/hardware-qualification-20261127/classification.json`.

## Gate result

The Z1/Z2 bit is **complete for this checkout**. Z3 native qualification remains
**BLOCKED** and cannot be advanced by more QEMU evidence. The next native step
needs all of: an owner-selected spare non-boot device, Administrator elevation
(this host is not elevated), and signed authorization for erase/power-cycle
risk. TPM/Secure Boot work additionally needs a visible, ready TPM and a signed
provisioning record; this host reports neither. No release or Tier 1 claim is
made.
