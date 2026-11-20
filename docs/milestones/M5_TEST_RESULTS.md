# M5 Test Results

Status: **M5_INCOMPLETE.** The first M5 slice (shared DMA ownership) passes its
own QEMU gate at 1, 2 and 4 CPUs. The M5 milestone as a whole is not
implemented. Only the DMA slice and one regression fix were done.

Branch: `m5/hardware-qualification`, based on `master` `10740a4f`. Changes are
uncommitted in the worktree `C:\Users\banda\Documents\GitHub\WovenHat-OS-m5`.

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
(commit `6d6a0b2b`). The NVMe migration is commit `6b8d793f`, the AHCI
migration is in progress (`git log -1 --format=%H -- kernel/src/ahci.rs`).

Not run in this slice: the 2 and 4 CPU runs of the Stage 13.12 gate,
host `cargo test` for the full suite, `cargo clippy --all-targets`,
`RUN-STAGE10.7.ps1`, and any device gate (AHCI, xHCI, network, Wi-Fi,
Bluetooth). No physical hardware was used.

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
