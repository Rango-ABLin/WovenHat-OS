# Stage 13.12 boot-time driver bring-up audit — 2026-10-14

## Finding that opened this stage

Every Stage 13 driver was compiled only into its own test feature
(`#[cfg(feature = "stage13-N-test")] mod nvme;` and so on). A normal boot
contained no driver manager, NVMe, AHCI, xHCI or HDA code. Each driver had
passing QEMU gates, but none was ever bound by a running system.

## What changed

**Bring-up (`kernel/src/drivers.rs`).** It runs in every normal boot, after
storage and networking.
1. It registers a PCI class matcher with the WovenDriver manager for each
   driver:
   - NVMe: 01/08/02
   - AHCI: 01/06/01
   - xHCI: 0c/03/30
   - HDA: 04/03
2. For each driver, it takes the first PCI function the manager matches to
   it.
3. It binds that function through `driver::bind_pci`. Binding claims the
   function and every programmed MMIO BAR for the boot owner token.
4. It calls the driver's new `init_at(device)` with the function re-read
   after admission, which may move BARs.

Initialization failure runs `driver::unbind_pci_resources`, so a failed
controller keeps no ownership. Ready drivers are entered in the device
registry and listed by the shell's `devices` command.

**Drivers.** `nvme`, `ahci`, `xhci` and `hda` gain `init_at`. `init()` keeps
finding the first controller itself.

**Driver manager.** `driver.rs` gains:
- `pci_binding(name)`: the function a driver is bound to;
- `pci_bar_leases(name)`: how many BAR leases it holds;
- `PciMatch::matches`, now crate-visible.

**Build gating.** The driver modules, the driver manager and the PCI
helpers that binding uses now compile in production (no `qemu-test`) and
in `stage13-12-test`.
- Production compiles only what it uses. The PCI hotplug functions stay
  gated to the Stage 13 tests.
- Each driver module has one `expect(dead_code, reason = …)`, active only
  in those two configurations. The reason names the APIs still waiting for
  production callers.
- Shared PCI, MSI, MSI-X and paging items that the live binding path now
  uses had their dead-code expectations narrowed so they do not apply in
  those builds.

## Gate

`[S13.12] driver bring-up: PASSED`. The gate runs the same `bring_up()` a
normal boot runs. It does not trust bring-up's report on its own; it checks
each driver against the driver manager and the device registry.
- **Ready:**
  - the bound function's class matches the driver's matcher;
  - `pci_binding` equals the reported address;
  - at least one BAR lease is held;
  - the registry lists the device with the right kind.
- **Absent, InitFailed, BindFailed:** no binding, and no registry entry.
- **Block I/O:** NVMe and AHCI each write, flush, read back and restore the
  last sector through the bound controller.
- **Idempotence:** a second bring-up binds nothing twice. Ready drivers
  report `AlreadyBound` at the same address, and the rest repeat their
  outcome.

The harness, `scripts/test-stage13-12-drivers.py`, fixes the expected
outcome of every driver per machine:
- **`devices`:** q35 with NVMe, a disk on the chipset AHCI controller, xHCI
  with a USB keyboard, and HDA. All four drivers must be Ready.
- **`nodisk`:** q35 with no disk. AHCI must be InitFailed, so the rollback
  path runs for real, and the others absent.
- **`absent`:** i440FX `pc`, which has none of these controllers. All four
  must be absent.
- **`production`:** a kernel built with no test feature, on the `devices`
  machine. All four drivers must be Ready, and the boot must reach
  `[BOOT] shell-first runtime ready`.

## Evidence (`audit-artifacts/`)

Stage 13.12, every mode on 1/2/4 CPUs, all passing:
- `devices`: `stage13.12-devices-1cpu-1791991503667577800`, `stage13.12-devices-2cpu-1791990748265435800`, `stage13.12-devices-4cpu-1791990800577141600`
- `nodisk`: `stage13.12-nodisk-1cpu-1791991302563673200`, `stage13.12-nodisk-2cpu-1791990899979693700`, `stage13.12-nodisk-4cpu-1791990949150002000`
- `absent`: `stage13.12-absent-1cpu-1791990999411980500`, `stage13.12-absent-2cpu-1791991051625832800`, `stage13.12-absent-4cpu-1791991105619152400`
- `production`: `stage13.12-production-1cpu-1791993293840764300`, `stage13.12-production-2cpu-1791991237755855900`, `stage13.12-production-4cpu-1791991270219159200`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. Each one failed the gate as intended:
- **Init failure keeps its binding** (`nodisk`): fails at "ahci0 InitFailed
  matches the driver manager and registry".
- **Init without binding** (`devices`): fails at "nvme0 Ready matches the
  driver manager and registry".
- **Registry not updated** (`devices`): fails at the same check.
- **Production boot compiles bring-up but never runs it**
  (`production`): fails with all four drivers missing.
  - The first version of this control removed the call. That only failed
    to compile, because of dead code under warning denial, so it proved
    nothing and was redone.

After the restore, `devices` passes again on 1 CPU.

Prior Stage 13 gates, rerun because their modules' gating changed, all
passing on 1/2/4 CPUs with exit 33:
- 13.1: `stage13.1-1cpu-1791991564231310000`, `stage13.1-2cpu-1791991637059693200`, `stage13.1-4cpu-1791991689376412700`
- 13.2: `stage13.2-1cpu-1791991735805966200`, `stage13.2-2cpu-1791991803606816100`, `stage13.2-4cpu-1791991852038745100`
- 13.3: `stage13.3-1cpu-1791991910277480900`, `stage13.3-2cpu-1791991983189973600`, `stage13.3-4cpu-1791992033063998500`
- 13.4: `stage13.4-1cpu-1791992082266661900`, `stage13.4-2cpu-1791992144276219400`, `stage13.4-4cpu-1791992185591944800`
- 13.5: `stage13.5-1cpu-1791992228265296100`, `stage13.5-2cpu-1791992296143722300`, `stage13.5-4cpu-1791992343240542000`
- 13.6: `stage13.6-1cpu-1791992388773827400`, `stage13.6-2cpu-1791992458686005800`, `stage13.6-4cpu-1791992505974391000`
- 13.7: `stage13.7-1cpu-1791992553411053400`, `stage13.7-2cpu-1791992629052095800`, `stage13.7-4cpu-1791992684607507200`
- 13.8: `stage13.8-1cpu-1791992736166240000`, `stage13.8-2cpu-1791992809983741000`, `stage13.8-4cpu-1791992857002094700`
- 13.9: `stage13.9-1cpu-1791992900875924000`, `stage13.9-2cpu-1791992988846889200`, `stage13.9-4cpu-1791993044100267500`
- 13.10: `stage13.10-1cpu-1791993100935930400`, `stage13.10-2cpu-1791993172706277700`, `stage13.10-4cpu-1791993226844159800`

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-12-test`;
- each of `stage13-1-test` through `stage13-10-test`;
- `stage12-1-test`, `stage14-5d-test` and `stage1-5-test`;
- host `--all-targets`.

## Limits

- **One function per driver.** A second controller of the same class
  stays unbound.
- **xHCI needs a connected device.** Its initialization requires a
  connected root port, so a controller with nothing plugged in reports
  InitFailed.
- **Initialization still polls.** The controllers poll synchronously
  under their own locks during initialization. Interrupt-driven
  completion is the next step for xHCI.
- **Not yet consumed by anything else.** Storage is not mounted from NVMe
  or AHCI, USB keyboards and mice are not yet read, and audio is not
  exposed to userspace. These are the remaining Stage 13 closure steps.
- **QEMU only.** None of this is physical-hardware qualification.
