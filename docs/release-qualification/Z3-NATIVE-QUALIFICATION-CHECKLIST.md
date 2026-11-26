# WovenHat OS — Stage 29 Zone 3: Native Qualification Execution Checklist

**Prerequisites (all three must be met before starting):**
- [ ] A spare non-boot device is designated (not SK hynix BC901 NVMe boot drive)
- [ ] Administrator elevation is active
- [ ] Signed authorization document is complete

---

## Phase 0 — Pre-Flight (Z1 Confirmation)

Before any Z3 activity, re-run Z1 to confirm nothing has changed:

- [ ] `python scripts/physical-qualification-preflight.py` → 17/17 `PHYSICAL_NOT_TESTED`
- [ ] `powershell scripts/physical-host-preflight.ps1` → host inventory current
- [ ] `powershell scripts/test-physical-tpm-readiness.ps1` → TPM/USB status current
- [ ] Confirm designated device is NOT boot/system

**Stop here if any Z1 result changed.**

---

## Phase 1 — Storage (Native FAT32/WovenFS R/W)

**Device**: [designated spare device path/serial]

- [ ] Native FAT32 format on spare device
- [ ] WovenFS write test (create files, directories, metadata)
- [ ] WovenFS read-back verification (checksums match)
- [ ] DMA/block alignment test
- [ ] I/O bench (sequential + random read/write)
- [ ] Device restored to pre-test partition state

**Evidence**: Signed observation → `wovenhat.hardware-qualification/v1` report

---

## Phase 2 — Boot (UEFI → Kernel → Console)

**Target**: ≥ 5 cold boots, each must reach runtime-ready marker

- [ ] Boot 1: UEFI handoff → kernel → display → console — PASS/FAIL
- [ ] Boot 2: UEFI handoff → kernel → display → console — PASS/FAIL
- [ ] Boot 3: UEFI handoff → kernel → display → console — PASS/FAIL
- [ ] Boot 4: UEFI handoff → kernel → display → console — PASS/FAIL
- [ ] Boot 5: UEFI handoff → kernel → display → console — PASS/FAIL
- [ ] (Optional additional boots for statistical confidence)

**Success criterion**: All ≥ 5 boots reach `[BOOT] shell-first runtime ready` or equivalent marker.

**Evidence**: Serial logs from each boot + signed observation

---

## Phase 3 — Power Integrity (Power-Loss + Journal Replay)

- [ ] Graceful shutdown — PASS/FAIL
- [ ] Graceful reboot — PASS/FAIL
- [ ] Power-loss cycle 1 → journal replay verification — PASS/FAIL
- [ ] Power-loss cycle 2 → journal replay verification — PASS/FAIL
- [ ] Power-loss cycle 3 → journal replay verification — PASS/FAIL
- [ ] Power-loss cycle 4 → journal replay verification — PASS/FAIL
- [ ] Power-loss cycle 5 → journal replay verification — PASS/FAIL
- [ ] (Optional cycles 6–10 for extended confidence)

**Success criterion**: Filesystem consistent after every power-loss cycle; journal replay recovers cleanly.

**Evidence**: Power-loss log + journal-replay output + signed observation

---

## Phase 4 — Firmware/Trust (Secure Boot / TPM2)

**Only if Z1 reports the hardware as present.**

- [ ] Secure Boot preparation (if TPM visible and ready)
- [ ] TPM2 provisioning (if TPM available)
- [ ] TPM2 attestation quote (if TPM available)
- [ ] Key storage verification (if TPM available)

**Skip criterion**: If Z1 reports TPM not visible/not ready (current state), this phase is skipped and recorded as `blocked` with reason.

**Evidence**: TPM quotes + signed observation

---

## Phase 5 — Classification

- [ ] Author `wovenhat.hardware-qualification/v1` report with all observations
- [ ] Run `python scripts/qualify-hardware-report.py --input <report> --output <classification>`
- [ ] Record tier result:
  - [ ] Tier 1: all 17 categories `passed` + `native`
  - [ ] Tier 2: no native failure, incomplete coverage
  - [ ] Tier 3: at least one native failure

**Evidence**: `classification.json`

---

## Rollback Confirmation

- [ ] Device partition table restored to pre-test state
- [ ] WovenFS test metadata cleared from device
- [ ] Boot entries restored (if modified)
- [ ] No residual test state on designated device

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Operator | | | |
| Device owner | | | |

**Final tier**: ________ (Tier 1 / Tier 2 / Tier 3)

---

**Remember**: Z3 is destructive. It erases the designated device. Do not proceed on any device containing data to preserve.
