# WovenHat OS

A 64-bit x86 operating system written from scratch in Rust: a preemptive SMP
microkernel with capability-based security, an async completion-port I/O model,
a live IPv4 network stack, and a bounded-by-construction driver stack covering
NVMe, AHCI, xHCI/USB, HD Audio, Wi-Fi and Bluetooth.

**Status: pre-release (`0.8.0`). Boots and runs under QEMU/UEFI on 1, 2 and 4
CPUs.** Physical-hardware qualification is not complete — see
[What is and isn't done](#what-is-and-isnt-done).

## Why this repository looks the way it does

WovenHat is built in numbered stages, and a stage is only "accepted" once it has
passed a recorded gate: a warning-denying Clippy run, host unit tests, and QEMU
boots on 1/2/4 CPUs that each exit with the expected status and emit required
serial markers. Evidence for every accepted stage is kept in `docs/` as a dated
audit, and the serial logs are preserved rather than discarded.

That discipline produces two habits visible throughout the codebase:

- **Bounded by construction.** Kernel objects use fixed-capacity tables, not
  growth. Handles are generation-tagged so a stale handle cannot address a
  reused slot. Locks carry explicit ranks checked at acquisition, so lock-order
  inversions panic instead of deadlocking in the field.
- **Fail closed, and say so.** A foundation that validates a protocol without
  driving real hardware is documented as exactly that. `docs/stage-status.md`
  records what each stage does *not* yet cover, by name.

If you are evaluating this project, `docs/stage-status.md` is the honest
summary and `docs/architecture-and-codebase-guide.md` is the architecture tour.

## Layout

| Path | What it is |
| --- | --- |
| `kernel/` | The kernel (`no_std`, target `x86_64-unknown-none`). ~81k lines. |
| `libwoven/` | Userspace API boundary for Ring-3 programs; still a thin stub. |
| `src/` | Host-side build driver; prints the built UEFI image path. |
| `build.rs` | Links the kernel artifact into a bootable UEFI disk image. |
| `tests/` | Host unit tests (Rust) and harness tests (Python). |
| `scripts/` | QEMU acceptance harnesses and image tooling. |
| `docs/` | Roadmap, architecture guide, stage status, and dated audits. |
| `vendor/bootloader/` | Patched `bootloader` crate used for UEFI boot. |

Kernel subsystems of note: `task.rs` (scheduler), `smp.rs`, `paging.rs`,
`heap.rs`, `ipc.rs`, `wovenguard.rs` (capability enforcement), `audit.rs`
(security ledger), `vfs.rs` + `fat32.rs` + `wovenfs.rs`, `network.rs`,
`completion_port.rs` + `async_*.rs` (async I/O), `hal/pci/`, and the `wifi_*`
and `bluetooth_hci.rs` stacks.

## Building and running

Prerequisites: the pinned nightly in `rust-toolchain.toml`
(`nightly-2026-07-10`, which rustup installs automatically), Python 3, and QEMU
with OVMF firmware.

The acceptance launchers keep Cargo's build and target directories inside the
project, because Windows `%TEMP%` cleanup has previously deleted Cargo's
intermediates mid-build and failed an entire chain. Use the same settings for
one-off commands:

```powershell
$env:CARGO_BUILD_BUILD_DIR = Join-Path (Get-Location) ".cargo-build"
$env:CARGO_TARGET_DIR      = Join-Path (Get-Location) "target"
```

Boot a release build under QEMU:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-release.ps1 -Cpus 4
```

Build just the kernel, or produce the bootable image:

```powershell
cargo build -p wovenhat-kernel --target x86_64-unknown-none
cargo run --release -- --print-image   # prints the UEFI image path
```

## Validating a change

Run these before proposing a change; CI (`.github/workflows/kernel.yml`) runs
the same gates on Linux.

```powershell
# Warning-denying lint, kernel and host
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings
cargo clippy --all-targets -- -D warnings

# Host unit and harness tests
cargo test

# A feature-gated QEMU gate, on each CPU count
python .\scripts\test-network-qemu.py --feature stage14-6-test --cpus 1 --timeout 300
```

Stage self-tests live behind Cargo features named after the stage
(`stage14-6-test`, `stage1-5-test`, …); see `kernel/Cargo.toml`. A feature
builds the stage's self-test into the kernel, which emits a serial marker such
as `[S14.6] live socket firewall admission: PASSED` and then exits with the
harness's expected status. A gate fails if the marker is missing *or* the exit
status is wrong.

The full preservation chain — 75 QEMU boots covering the accepted Stage 10.7
surface — is:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\RUN-STAGE10.7.ps1
```

Run only one acceptance chain at a time; the harnesses bind fixed host ports and
write to fixed output paths.

## What is and isn't done

Accepted under deterministic QEMU validation:

- Preemptive SMP scheduling with CPU-domain placement, hotplug of a bounded
  CPU prefix, and TLB shootdown acknowledgement.
- Paging, a growable mapped heap with coalescing free lists, frame reclamation
  via per-range bitmaps, swap, and file-backed mappings with a page cache.
- Capability-based security: lineage tracking, recursive revocation, sandbox
  profiles, task authority enforcement, and a fixed-capacity security ledger.
- IPC: handle messaging, shared memory, capability transfer, service discovery.
- Async I/O: completion ports, timers, deadlines, cancellation, and async
  block/file/UDP/TCP paths reachable from Ring 3.
- Processes and threads: PID/TID lifecycles, join semantics, TLS, W^X ELF
  loading with ASLR, and a loader that rejects program-header types whose
  semantics are not implemented.
- Storage: FAT32 with a bounded on-volume prepared/committed journal that
  recovers interrupted multi-file batches; WovenFS metadata, snapshots, and
  AEAD volume encryption with a revocable key vault.
- Networking: live DHCP/DNS/ICMP/UDP/TCP over VirtIO, a bounded routing table,
  and a WovenGuard firewall policy bound to live socket admission.
- Drivers: PCIe topology with generation-tagged ownership, NVMe, AHCI, xHCI,
  USB HID, HD Audio, and protocol stacks for Wi-Fi (WPA2/CCMP) and Bluetooth
  (HCI, L2CAP, ATT/GATT, BR/EDR and LE security).

Explicitly **not** done, and not claimed:

- **Physical hardware qualification.** Driver stacks are validated against
  deterministic software models and QEMU, not against real radios, disks or
  controllers. The Wi-Fi and Bluetooth stacks in particular have never
  transmitted on real hardware; the intended AX201 adapter has no native
  startup path.
- Unclean-shutdown and power-loss testing on physical storage; physical DMA
  qualification.
- Multi-node NUMA, non-contiguous physical CPU hotplug, and APIC IDs above 255.
- IPv6 sockets (the IPv6 protocol layers exist and are validated, but no live
  socket path), connection tracking, NAT, and packet-level firewall filtering.
- Dynamic linking, shared libraries, and TLS image allocation in the ELF
  loader.
- Measured key provisioning, persistent key storage, and key rotation policy
  for encrypted volumes.

`docs/stage-status.md` carries the per-stage version of this list, and
`docs/stage1-12-production-gap-audit.md` tracks production completion
separately from bounded foundation acceptance.

## Contributing

`AGENTS.md` holds the development rules this project is built under — stage
ordering, preservation of accepted ABI, lock and concurrency review
requirements, and the rule that a failing test is a defect to diagnose rather
than an assertion to weaken. Read it before changing kernel code.

Audit artifacts and generated build output are intentionally Git-ignored;
historical evidence under `audit-artifacts/` is not to be deleted.
