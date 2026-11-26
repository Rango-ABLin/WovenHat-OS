# WovenHat OS — Stage 29 Zone 3: Native Qualification Authorization

**Template only — not executed. No native claim is made by this document.**

## 1. Authorization Request

The device owner authorizes Stage 29 Zone 3 (Z3) native hardware qualification on a designated spare non-boot device. This authorization covers:

- Erase/format of the designated device
- Native FAT32/WovenFS read/write testing
- UEFI boot handoff and kernel boot testing (≥ 5 cold boots)
- Controlled power-loss cycles (5–10) with journal-replay verification
- Secure Boot preparation and TPM2 provisioning/attestation (only if Z1 reports hardware present)

## 2. Device Designation

| Field | Value |
|-------|-------|
| Device model | VendorCo ProductCode USB Device |
| Device interface | USB (removable media) |
| Device size | ~15.7 GB (15728640000 bytes) |
| Device number / index | Disk 3 |
| Serial number | 4045141272666606378 |
| Is boot device? | No |
| Is system drive? | No |
| Contains data to preserve? | **Owner must confirm NO** |
| Preflight safe_for_destructive_gate | true |
| Alternative candidate | ST500DM002-1BD142 (Disk 0, 500 GB SATA/RAID) |

**Safety rule**: The designated device must NOT be the boot/system drive (SK hynix BC901 NVMe, Disk 1). If the device contains any data to preserve, stop — Z3 will erase it.

## 3. Operator

| Field | Value |
|-------|-------|
| Operator name |Anthony George Banda |
| Elevation | Administrator (required — current session is NOT elevated) |
| Date | 11-25-2026 |
| Host machine | Apeirova (HP 290 G4 Microtower PC) |

## 4. Z3 Test Phases

Each phase produces a signed observation written into the `wovenhat.hardware-qualification/v1` report.

### Phase 1 — Storage

Native FAT32/WovenFS read/write, DMA/block alignment, I/O bench on the designated spare device only.

### Phase 2 — Boot

UEFI handoff → kernel → display → console. Repeatability ≥ 5 cold boots. Each boot must reach the `[BOOT] shell-first runtime ready` marker or equivalent.

### Phase 3 — Power Integrity

Graceful shutdown/reboot, then 5–10 controlled power-loss cycles. After each cycle, journal-replay verification must confirm filesystem consistency.

### Phase 4 — Firmware/Trust

Secure Boot preparation, TPM2 provisioning/attestation, key storage. **SKIPPED** — Z1 preflight reports TPM not visible/not ready (`Access denied`), Secure Boot cannot be queried without elevation.

## 5. Rollback

Each phase is independently rollback-safe:
- Device partition table is restored to pre-test state
- Any WovenFS metadata written during test is cleared
- Boot entries modified during testing are restored

## 6. Tier Determination

After Z3 completes, `qualify-hardware-report.py` classifies observations:

| Observation condition | Tier |
|----------------------|------|
| All 17 categories: `passed` + `environment == native` | Tier 1 |
| No native failure, but incomplete native coverage | Tier 2 |
| At least one native failure | Tier 3 |

## 7. Revocation

This authorization may be revoked at any time by the operator. Revocation halts all Z3 activity immediately.

---

**To execute**:
1. Fill in Operator name and Date above
2. Confirm the USB device has no data to preserve
3. Run PowerShell as Administrator
4. Execute the Z3 procedure in `docs/hardware-qualification-stage29-2026-11-20.md` section 6, following `docs/release-qualification/Z3-NATIVE-QUALIFICATION-CHECKLIST.md`
