# Experimental UEFI installer audit — 2026-10-25

The user requested a physical-machine installer and repeated that instruction
after the Stage 28 ordering conflict was explained. This authorizes prioritizing
the prototype, not certifying intervening stages or writing an unidentified
physical disk. All installation tests use disposable QEMU file-backed disks.

## Implementation and authority review

The independent UEFI app embeds the default development boot image and installs
its GPT, ESP, bootloader and kernel. Kernel sources, ABI and stage dependencies
are unchanged. The runtime remains the diagnostic shell and may use RAM VFS;
this is not a persistent desktop system claim.

All project installer Rust is safe Rust. Firmware ABI operations remain inside
the `uefi` library. Scoped protocol handles close on returns and errors. The
app has one thread, synchronous I/O, firmware-event keyboard waits and no new
kernel objects, asynchronous userspace pointers, IRQ locks or cross-CPU state.

Device-path ancestry excludes source media; malformed/empty/multi-instance
paths cannot establish target authority. Logical partitions, read-only/absent
media, undersized disks, non-512-byte sectors and incompatible buffer alignment
are excluded. Capacity is bounded to 16 displayed targets and 80 input characters.
Exact `ERASE N` confirmation is required. Device path, media ID, capacity, sector
size, alignment, presence and writability are checked again after confirmation.
Do not hot-swap disks during selection/install.

The embedded image cannot be swapped after selection. Primary/backup GPT header
and partition-table checksums are validated before writes. Only a canonical
single-ESP layout is accepted. The protective MBR and primary/backup geometry
are adjusted to the actual target capacity. Buffers are aligned to 4096 bytes.
Every written region is flushed, read back and compared byte for byte.

Errors stop installation; interrupted writes may leave an incomplete target.
There is no rollback, secure erase, dual boot or in-place upgrade guarantee.
Remaining space is unallocated. Boot uses the UEFI fallback path, without a new
NVRAM entry. Firmware disk access does not establish kernel-driver compatibility.

## Evidence and gates

The final bundle is `audit-artifacts/installer-20261025-final/`: bootable USB
image, ZIP, standalone EFI executable, embedded system image, instructions and
SHA-256 manifest. Earlier attempts and logs remain preserved.

| Check | Result / evidence under `audit-artifacts/` |
| --- | --- |
| Default kernel image and final UEFI executable build | PASS, builder completed |
| Independent installer host tests | PASS: 3, `installer-20261025-final/installer-host-tests.log` |
| Installer host Clippy, all targets, warnings denied | PASS, `installer-20261025-final/clippy-installer-host.log` |
| Installer UEFI Clippy, release, warnings denied | PASS, `installer-20261025-final/clippy-installer-uefi.log` |
| Repository host Clippy, all targets, warnings denied | PASS, `installer-20261025-v3/clippy-host.log` |
| Default kernel Clippy, freestanding target, warnings denied | PASS, `installer-20261025-v3/clippy-kernel.log` |
| Repository host test gate | INCOMPLETE: 138 tests in 22 integration executables passed; Windows blocked the binary test executable, error 4551. `installer-20261025-v3/host-tests.log`, `host-bin-retry.log` |
| Earlier installer QEMU 1/2/4 CPU matrix | PASS, `installer-qemu-20261025-v3/result.txt` |
| Final QEMU matrix plus keyboard/SMP preservation | PASS on 1/2/4 CPUs, `installer-qemu-20261025-final/result.txt` and per-VM serial logs |
| Physical installation | NOT RUN |
| Complete prior-stage acceptance chains | NOT RERUN; no full Stage 28 acceptance claim |

The QEMU gate checks wrong confirmation without writes, source exclusion,
unchanged source/unselected disk, selected-disk installation, GPT CRCs and backup
relocation, exact ESP contents, then boots a new VM with only the installed disk.
The final gate also types `smptest` through emulated keyboard input and requires
`[SMP] acknowledged TLB shootdowns: PASSED`. A no-eligible-target case excludes
read-only, undersized and 4Kn disks without writes. The harness only stops its
own QEMU processes and keeps each VM's commands, firmware variables and logs.

## Diagnosed failures

Windows initially blocked installer host tests and Rust's bundled linker, even
after escalation. Installer host tests later ran successfully without changing
assertions. The installed Microsoft linker supports the UEFI build with
`/NXCOMPAT:NO`, required for its EFI subsystem compatibility. The broader host
binary test remains blocked; it was not disabled or suppressed.

The first VM had no writable OVMF variables store. Firmware created `NvVars` on
the source FAT volume, causing the source hash assertion to fail after cancelled
installation. Bytewise comparison identified this firmware-created file.
Evidence remains in `installer-qemu-20261025/`. Each VM now has a separate
matching writable OVMF variables copy; tests use a copy of the distribution
image and retain source hash assertions.

An early UEFI Clippy check found a nested match guard; the guard was corrected
without lint suppression. No assertions were weakened, timeouts increased,
unrelated QEMU processes stopped or historical evidence deleted.

## Acceptance limits and next prerequisites

This is not full Stage 28 or Stage 29 acceptance. Graphical installation,
durable system-root policy, session startup, accounts, keyboard/timezone/network
setup, progress UX and recovery remain open. Resolve the Windows host-test block
and run the required complete preservation gate before accepting a full stage.
Physical qualification must establish firmware fallback discovery and runtime
storage/input/display compatibility on named hardware. Continue the remaining
roadmap in order except for this explicitly prioritized prototype.
