# Stage 13.20: ACPI power-off and reset audit — 2026-10-15

The kernel could neither turn a machine off nor restart it. This stage
does both, for live hardware testing, where a tester must be able to shut
down and the stick's log must get a last flush. It also delivers the first
two items of roadmap Stage 25, power management.

## What changed

**ACPI (`hal/acpi.rs`, published as `acpi::power()`).**
- The FADT's PM1a and PM1b control blocks. The extended (GAS) addresses
  are preferred when they are system I/O.
- The SMI command port and ACPI_ENABLE.
- The reset register, when RESET_REG_SUP is set.
- The `\_S5` sleep types, from the DSDT or else an SSDT:
  `Name (_S5, Package)` with Zero, One, byte, word or dword elements.

**`power.rs`.**
- `power_off()`:
  1. hands the machine to ACPI if the firmware left it in legacy mode;
  2. writes `SLP_TYP | SLP_EN` to PM1b, then PM1a;
  3. returns only if the machine is still running, with the reason.
- `reboot()` tries each reset method in turn:
  1. the FADT reset register (system I/O);
  2. the 8042 reset line;
  3. the 0xCF9 reset control;
  4. a triple fault.
- Both give the live-test log a last flush first.

**Kernel shell.** `poweroff`/`shutdown` and `reboot`/`restart`.

**Live-test log.** The `WOVENLOG` label is now also accepted from the
FAT32 root directory, where Windows' format writes it. A 13.19 gate mode,
`windows-label`, covers it.

## Gate

`scripts/test-stage13-20-power.py`:
- **poweroff:** the stage13-20-test kernel enters S5, and QEMU must exit
  by itself with status 0.
- **reboot:** the stage13-20b-test kernel resets, and QEMU (`-no-reboot`)
  must exit likewise.

No failure line may follow the request. In QEMU, the FADT reset register
(port 0xCF9) reset the machine on the first attempt. `\_S5` read as
(0, 0), with PM1a at port 0x604.

## Evidence (`audit-artifacts/`)

- **13.20 power-off, 1/2/4 CPUs:** `stage13.20-poweroff-1cpu-1792074113568239300`, `stage13.20-poweroff-2cpu-1792073744690430900`, `stage13.20-poweroff-4cpu-1792073853280290100`.
- **13.20 reset, 1/2/4 CPUs:** `stage13.20-reboot-1cpu-1792074383549659200`, `stage13.20-reboot-2cpu-1792073765647322300`, `stage13.20-reboot-4cpu-1792073894760101800`.
- **13.19 windows-label, 1/2/4 CPUs:** `stage13.19-windows-label-1cpu-1792074649172553700`, `stage13.19-windows-label-2cpu-1792073802466461400`, `stage13.19-windows-label-4cpu-1792073918705552400`.
- **Release gate:** `release-validation-1792076670/`.

Chain results:

```
lint-all-features: exit 0 []
host tests: ok
13.20 poweroff 1cpu: PASS (57s) 
13.20 reboot 1cpu: PASS (34s) 
13.19 windows-label 1cpu: PASS (37s) 
13.20 poweroff 2cpu: PASS (18s) 
13.20 reboot 2cpu: PASS (29s) 
13.19 windows-label 2cpu: PASS (47s) 
13.20 poweroff 4cpu: PASS (30s) 
13.20 reboot 4cpu: PASS (20s) 
13.19 windows-label 4cpu: PASS (32s) 
13.19 test 1cpu: PASS (99s) 
13.19 production 1cpu: PASS (30s) 
13.19 unlabelled 1cpu: PASS (24s) 
negative control 13.20 S5 sleep type ignored (suspend instead of off): caught ['timeout: the machine neither turned off nor reset', 'QEMU exit status 1, expected 0 (stopped by the guest)', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.20-poweroff-1cpu-1792074113568239300']
negative control 13.20 every reset method skipped: caught ['timeout: the machine neither turned off nor reset', 'QEMU exit status 1, expected 0 (stopped by the guest)', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.20-reboot-1cpu-1792074383549659200']
negative control 13.19 root-directory label ignored: caught ['FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-windows-label-1cpu-1792074649172553700']
13.12 production 1cpu: PASS (21s) 
13.2 1cpu: PASS (27s) 
13.12 production 2cpu: PASS (6s) 
13.2 2cpu: PASS (13s) 
13.12 production 4cpu: PASS (6s) 
13.2 4cpu: PASS (13s) 
13.17 ahci 1cpu: PASS (37s) 
13.18 1cpu: PASS (34s) 
20.1 1cpu: PASS (27s) 
release: PASS (1753s) 
DONE
```

**Negative controls**, each caught, on 1 CPU with the source restored byte
for byte:
- **Sleep type altered:** QEMU suspends instead of turning off.
- **Every reset method skipped.**
- **Root-directory label ignored:** the windows-label stick is not
  logged.

## Limits

- **Reset registers.** Only system-I/O reset registers are used directly;
  memory and PCI-configuration ones fall through to the 8042 and 0xCF9.
- **Not yet done (roadmap Stage 25):** sleep, wake, CPU idle and frequency
  management, battery, thermal and lid.
