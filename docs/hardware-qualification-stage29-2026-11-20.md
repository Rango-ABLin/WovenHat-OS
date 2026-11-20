# Stage 29 — Real Hardware Qualification: design and read-only run

**Date**: 2026-11-20
**Author**: systems-architecture agent (safe read-only scope, user-authorized)
**Status**: DESIGN COMPLETE / READ-ONLY GATES RUN (results below)

## 1. Purpose

Stage 29 of the master roadmap is the *hardware compatibility laboratory*:
it is the only stage that can produce the **native** evidence the rest of the
roadmap repeatedly lists as the open release prerequisite ("physical
qualification", "hardware qualification"). Everything else in this repository is
QEMU/emulated/software evidence, and every accepted document says so explicitly.

This document records (a) the qualification architecture, (b) the read-only
gates that were actually run on this machine today, and (c) the deferred native
procedure that still requires explicit destructive authorization. It makes **no
native certification claim**: that is the point of the safety boundary.

## 2. Safety model: three zones

| Zone | What it may do | Native claim | Runs today |
|---|---|---|---|
| **Z1 — Read-only preflight** | stdlib/CIM inventory, TPM visibility, USB candidates, host identity. No disk handle, no write, no reboot, no TPM provision. | None | ✅ |
| **Z2 — Emulated/software matrix** | Deterministic virtual-hardware profiles, QEMU probe image, software fixtures. No device access. | None (always `PHYSICAL_NOT_TESTED`) | ✅ |
| **Z3 — Native destructive qualification** | Bare-metal boot, raw FAT32/DMA, power-loss, USB flash, TPM/Secure Boot provisioning. | Tier 1 only here | ❌ deferred |

Z3 requires, in order: an explicit owner decision naming a **spare non-boot
device**, **Administrator elevation**, and a signed authorization covering the
erase/power-cycle risk. None of those are in force for this run, so Z3 is
designed but not executed. The read-only preflight (Z1) exists precisely to
*gate* whether Z3 may be scheduled, and it currently reports **no**.

## 3. Qualification model

### 3.1 Categories (17)

`intel-pc, amd-pc, laptop, desktop, nvme, sata, usb, ethernet, wifi, audio,
intel-graphics, amd-graphics, nvidia-graphics, touchpad, webcam, bluetooth,
multi-monitor` — enforced verbatim by both
`scripts/physical-qualification-preflight.py` and
`scripts/qualify-hardware-report.py`.

### 3.2 Tiers (fail-closed)

- **Tier 1** — every category has a `passed` observation with
  `environment == "native"`. *Nothing in Z1/Z2 can ever produce this.*
- **Tier 2** — no native failure recorded, but native coverage is incomplete.
- **Tier 3** — at least one native failure recorded.

The classifier (`qualify-hardware-report.py`) hard-rejects a `tier1` label unless
the observation is both `passed` and `native`, and rejects evidence paths with
parent traversal or absolute paths.

### 3.3 Evidence schema

Two distinct, deliberately incompatible schemas:

- `wovenhat.hardware-qualification-preflight/v1` — the Z1 preflight report
  (`scripts/physical-qualification-preflight.py`), 17 × `PHYSICAL_NOT_TESTED`.
- `wovenhat.hardware-qualification/v1` — the Z3 classification report
  (`scripts/qualify-hardware-report.py`), observations carry
  `{id, category, device, result, environment, evidence, tier}`.

The preflight *cannot* accept native evidence; only a separate signed native
workflow may move a category out of `PHYSICAL_NOT_TESTED`.

## 4. Tooling inventory (existing, reused unchanged)

| Script | Zone | Role |
|---|---|---|
| `scripts/physical-qualification-preflight.py` | Z1 | stdlib-only host inventory + 17-category fail-closed report |
| `scripts/physical-host-preflight.ps1` | Z1 | CIM host/manufacturer/RAM/OS/adapters/disks/SecureBoot/bootcfg |
| `scripts/test-physical-tpm-readiness.ps1` | Z1 | TPM visibility/readiness, USB candidates, read-only |
| `scripts/test-physical-probe.py` | Z2 | build + boot the `physical-probe` read-only inventory image (CPUID/ACPI/PCI, no disk I/O) |
| `scripts/run-hardware-qualification-matrix.py` | Z2 | 108-job virtual matrix (12 profiles × 3 CPU × 3 RAM); hard-rejects any physical/raw flag |
| `scripts/hardware-qualification-stimulator.py` | Z2 | deterministic profile result generator, `PHYSICAL_NOT_TESTED` always |
| `scripts/qualify-hardware-report.py` | Z2/Z3 | report-only Tier classifier |
| `tests/test_hardware_qualification*.py`, `tests/test_physical_qualification_preflight.py` | Z1/Z2 | unit contracts for the above |

## 5. This run's plan (Z1 + Z2 only)

1. Z1 preflight — `physical-qualification-preflight.py` (17 categories, native not tested).
2. Z1 host inventory — `physical-host-preflight.ps1` (manufacturer/model/hypervisor/RAM/adapters/disks/SecureBoot).
3. Z1 TPM/USB readiness — `test-physical-tpm-readiness.ps1`.
4. Z2 virtual matrix — `run-hardware-qualification-matrix.py` (108 jobs, pure planning/emulation).
5. Z2 probe image — `test-physical-probe.py --cpus 1|2|4` (build `physical-probe` feature, boot under QEMU).
6. Z2 classifier — author a conservative `wovenhat.hardware-qualification/v1` report from the above and classify it (expected: **Tier 2**).

## 6. Native qualification procedure (Z3 — designed, deferred)

When the owner provides a spare non-boot device + elevation + signed
authorization, the lab runs four phases, each with its own evidence and
rollback:

1. **Storage** — native FAT32/WovenFS R/W, DMA/block alignment, I/O bench, on
   the *approved spare device only*.
2. **Boot** — UEFI handoff → kernel → display → console, repeatability ≥ 5 boots.
3. **Power integrity** — graceful shutdown/reboot and 5–10 controlled power-loss
   cycles with journal-replay verification.
4. **Firmware/trust** — Secure Boot prep, TPM2 provisioning/attestation, key
   storage — only on hardware that the Z1 preflight reports as present.

Each phase writes a signed observation into the `wovenhat.hardware-
qualification/v1` report; the classifier then computes the true Tier.

## 7. Results (2026-11-20, read-only)

All gates below are Z1/Z2: no device handle was opened, no media was written, no
firmware/TPM/boot entry was changed, no reboot was requested, and no native
qualification was claimed.

### 7.1 Z1 — preflight and host inventory

- `physical-qualification-preflight.py` → 17/17 categories `PHYSICAL_NOT_TESTED`,
  native evidence not supplied/accepted. Host: `Apeirova`, Windows 11
  (`10.0.26300`), `Intel64 Family 6 Model 165`, 12 logical CPUs, ~21.3 GB RAM.
- `physical-host-preflight.ps1` → **HP 290 G4 Microtower PC**, HypervisorPresent,
  **not elevated** (Secure Boot and `bcdedit` failed closed on access denied).
  Disks: `SK hynix BC901` 256 GB NVMe (boot/system), `ST500DM002` 500 GB
  (RAID/SATA), one 100 GB file-backed virtual disk. Adapters: Realtek RTL8821CE
  Wi-Fi (enabled) + Realtek GbE (disabled). No serial ports.
- `test-physical-tpm-readiness.ps1` → TPM **not visible / not ready**, no swtpm
  backend, no signed provisioning record, **0 USB targets**. Read-only.

### 7.2 Z2 — emulated/software matrix and probe

- `run-hardware-qualification-matrix.py` → **108/108 jobs PASS**, every result
  `PHYSICAL_NOT_TESTED`, physical/raw/native claims all `false`.
- `test-physical-probe.py --cpus 1|2|4` → inventory QEMU smoke **PASS** at all
  three CPU counts (read-only PCI/CPUID/ACPI inventory; halts before storage/
  network/audio; no disk I/O).
- Tooling unit tests → **18/18 pass**
  (`tests/test_hardware_qualification*.py`,
  `tests/test_physical_qualification_preflight.py`).

### 7.3 Classification

`qualify-hardware-report.py` over the authored
`wovenhat.hardware-qualification/v1` report (17 observations, environment
`emulated`, result `blocked`) → **valid, Tier 2**. Zero native-passed categories,
17 blocked/missing-native, zero failures. This is the honest result for a
read-only run and matches the repository's standing "native remains open" record.

### 7.4 Evidence

- `audit-artifacts/physical-qualification-preflight/report.json`
- `audit-artifacts/physical-host-20261120-164947-288/host.json`
- `audit-artifacts/physical-tpm-readiness-20261120-164932/readiness.json`
- `audit-artifacts/hardware-qualification-matrix-20261120/summary.json`
- `audit-artifacts/physical-probe-{1,2,4}cpu-*/result.json`
- `audit-artifacts/hardware-qualification-20261120/report.json` and `classification.json`

## 8. Open gates / next prerequisites

Native (Z3) qualification remains **open** until all of: an owner-selected spare
non-boot device, Administrator elevation, and a signed authorization for
erase/power-cycle risk are provided. Until then the release gate cannot reach
Tier 1, and this run does not move the release timeline.
