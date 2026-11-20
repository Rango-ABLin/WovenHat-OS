# M5 Test Results

Status: **M5_INCOMPLETE.** Implemented so far: shared DMA ownership (1/2/4 CPU
QEMU PASS), file-mmap ordering fix, NVMe on the shared DMA page, AHCI on the
shared DMA page with transfer ownership, and a PCI/MSI-X interrupt hardening
pass. The M5 milestone as a whole is not accepted; remaining slices and
deliverables are open (see `docs/milestones/M5_IMPLEMENTATION_REPORT.md` and
`docs/milestones/M5_RELEASE_READINESS.md`).

Branch: `m5/hardware-qualification`, based on `master` `10740a4f`, now at
commit `5ca5b9b3` on the branch worktree `C:\Users\banda\Documents\GitHub\WovenHat-OS-m5`.

## Results

| Check | Command | Result |
|---|---|---|
| Host DMA ownership and bounds tests | `cargo test --test dma_ownership` | PASS, 8 of 8 |
| M5 DMA QEMU gate, 1 CPU, 4 GiB | `python scripts/test-m5-dma-qemu.py --cpus 1 --memory 4G` | PASS, exit 33, marker `[M5 DMA] shared DMA ownership: PASSED`. Evidence: `audit-artifacts/m5-dma-1cpu-1795170920011644700/` |
| M5 DMA QEMU gate, 2 CPUs, 4 GiB | `... --cpus 2 --memory 4G` | PASS. Evidence: `audit-artifacts/m5-dma-2cpu-1795170978093493500/` |
| M5 DMA QEMU gate, 4 CPUs, 4 GiB | `... --cpus 4 --memory 4G` | PASS. Evidence: `audit-artifacts/m5-dma-4cpu-1795170996773351200/` |
| Existing `qemu-test` chain through FILE MMAP, 1 CPU | `python scripts/test-stage13-12-drivers.py --mode devices --cpus 1` | PASS, exit 33. Evidence: `audit-artifacts/stage13.12-devices-1cpu-1795171016784483200/` |
| Freestanding kernel check, Wi-Fi gate | `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage13-9-test` | PASS (exit 0) on `10740a4f` with the change applied |
| Warning-denying Clippy, M5 gate | `cargo clippy ... --features m5-dma-test -- -D warnings` | PASS |
| Warning-denying Clippy, kernel `stage13-14-test` gate | `cargo clippy -p wovenhat-kernel ... --features stage13-14-test -- -D warnings` | PASS |
| 1-CPU Stage 13.14 storage gate, NVMe | `python scripts/test-stage13-14-storage.py --cpus 1 --mode nvme` | PASS, exit 33, `[S13.14] NVMe/AHCI storage: PASSED`. Evidence: `audit-artifacts/stage13.14-nvme-1cpu-1795172062694030900/` |
| Stage 13.14 storage gate, AHCI, 1/2/4 CPUs | `python scripts/test-stage13-14-storage.py --mode ahci --cpus {1,2,4}` | PASS. Evidence: `audit-artifacts/stage13.14-ahci-{1,2,4}cpu-*` (1: `...1795173294135797700`, 2: `...1795173428000994800`, 4: `...1795173468690713700`) |
| M5 DMA QEMU gate, 1 / 2 / 4 CPUs | `python scripts/test-m5-dma-qemu.py --cpus {1,2,4} --memory 4G` | PASS |

The file-mmap regression on `master` is fixed in `kernel/src/userspace.rs`
(commit `6d6a0b2b`). The NVMe migration is commit `6b8d793f`, and the AHCI
migration plus transfer ownership are commits `da5bed49`/`c849b409`.

Not run in this slice: the 2 and 4 CPU runs of the Stage 13.12 gate,
`RUN-STAGE10.7.ps1`, and any physical-hardware gate. No physical hardware was
used.

## Results added by the PCI/MSI-X hardening pass (2026-11-20)

| Check | Command | Result |
|---|---|---|
| Freestanding kernel checks, `stage13-12-test` / `stage13-13-test` | `cargo check -p wovenhat-kernel --target x86_64-unknown-none --features stage13-12-test` (and `stage13-13-test`) | PASS |
| Warning-denying Clippy, freestanding, `stage13-12-test` | `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-12-test -- -D warnings` | PASS |
| Warning-denying Clippy, all targets | `cargo clippy --all-targets -- -D warnings` | PASS (unused NVMe DMA imports removed) |
| Host test suite | `cargo test` | PASS |
| Stage 13.13 USB input, 1 CPU | `python scripts/test-stage13-13-usb-input.py --cpus 1` | PASS, exit 33. Evidence: `audit-artifacts/stage13.13-1cpu-1795174725331633400/` |
| Stage 13.12 driver bring-up, devices, 1 CPU | `python scripts/test-stage13-12-drivers.py --mode devices --cpus 1` | PASS, exit 33. Evidence: `audit-artifacts/stage13.12-devices-1cpu-1795175121814678400/` |
| Stage 13.14 storage, NVMe, 1 CPU | `python scripts/test-stage13-14-storage.py --mode nvme --cpus 1` | PASS, exit 33. Evidence: `audit-artifacts/stage13.14-nvme-1cpu-1795175407564438300/` |

## Regression fixed: FILE MMAP self-test

**Symptom.** On `master` `10740a4f`, every `qemu-test` build stopped at
`[FILE MMAP] regression tests: FAILED` and exited 35. The same failure happened
at 512 MiB, 2 GiB and 4 GiB guest RAM, and with none of this slice applied.

**Root cause (confirmed with serial checkpoints).**
`userspace::file_mmap_self_test` destroyed `root`, the test address space,
before its large-asset section (`d7f42637`, extended by `b763a8b4`). That
section maps into the same address space through `map_file_large_readonly`,
which then failed on freed page tables. The checkpoints showed every earlier
step `passed=true`, the large file written, and `map_file_large_readonly`
returning `None`.

**Fix.** The large-asset section now runs before `destroy_user_address_space`
and `remove(PATH)`, and both moved to the end of the test. The test's checks are
unchanged. `kernel/src/userspace.rs` is the only file changed for this fix.

## Gate placement bug found along the way

`m5-dma-test` first implied `qemu-test`. That compiled out the shell-first
block where the gate runs (`#[cfg(not(feature = "qemu-test"))]`), so the gate
was unreachable. It now has no `qemu-test` dependency, like
`stage31-driver-restart-test`. `qemu_test_exit_success`/`failure` also list
`m5-dma-test`.

## Build issues fixed on the way

- An unused `PhysAddr` import in `dma.rs` and unused accessors. The build
  denies warnings. The self-test now uses `len`, `is_empty` and
  `virtual_address`.

## PCI/MSI-X interrupt hardening pass (2026-11-20)

Findings and closed gaps from the M5 PCI/MSI-X baseline audit plus this pass:

- End-of-interrupt: LAPIC EOI via the local IDT handler (`smp::eoi`) is the
  delivery acknowledgement. `hal/pci/msi::ack_msi` no longer writes a PCI
  config/MMIO EOI from a worker context.
- MSI masking: added owned mask/unmask (`hal/pci/msi::mask_owned_msi`,
  `hal/pci/msi::unmask_owned_msi`) with ownership, capability and per-vector
  checks; capability-level enable mask for devices without per-vector masking.
- MSI-X: enable path already programs masked entries then unmasked, and
  disables/masks on quiesce; no code change required.
- xHCI interrupt lifecycle: `xhci::start_input` stores the acquired lease,
  unmask before enabling the interrupter, rolls back vector/waiter/lease on
  failure; `xhci::shutdown` masks delivery, clears waiter and lease before
  halting the controller.
- Lint fix: removed unused NVMe DMA page-size imports in `kernel/src/nvme.rs`.

## Environment limits observed

- C: drive ran out of space twice during builds. I removed only my own build
  output (`target/` in this worktree), temp copies I created, and incremental
  caches with `scripts/reclaim-build-space.py`. I did not touch another owner's
  `%LOCALAPPDATA%\Temp\opencode\wovenhat-pilot` (3.7 GiB).
- At one point a 4 GiB guest failed with `cannot set up guest memory 'pc.ram'`
  while another `qemu-system-x86_64` process was running. The same size worked
  later in the session.
- The M5 DMA gate script accepts `--memory`. It defaults to 4G to match the
  stage harnesses.
