# Software architecture gap inventory (S00)

Date: 2026-10-24  
Source baseline: `f510ed549c881615847e2a003b33ebe360c2f7b0`  
Branch: `master`

This inventory applies the S00-S20 sequence in the Software Architecture
Completion Mandate to the checked-out source. “Verified-software” means the
current repository has actual-source tests and retained host/QEMU evidence for
the bounded contract named here. It does not mean native hardware support,
foreign-OS compatibility, production trust, or release approval.

## Classification

| Increment | Capability and source anchors | Current classification | Evidence / exact next prerequisite |
|---|---|---|---|
| S00 | Kernel, userspace, storage, security, drivers, graphics, network, SDK and services are represented by `kernel/src`, `libwoven`, `apps`, `tools`, `scripts`, and `tests`. | VERIFIED-SOFTWARE | This inventory, `checkpoint.json`, `m00-baseline-ledger.md`; refresh after every accepted increment. |
| S01 | Native syscall/IPC/capability boundaries and `libwoven` wrappers; package metadata and application launch paths. | VERIFIED-SOFTWARE | `libwoven::abi` and syscall 243 provide fail-closed major/minor/descriptor negotiation. The independently built exit-status fixture passed current, unsupported-major and malformed-reserved-byte cases at 1/2/4 CPUs; the inherited release chain passed 158/158. Physical, foreign-OS and production-trust claims remain separate. |
| S02 | Freestanding C entry/exit ABI and static ELF loader interoperability in `kernel/src`; Rust applications remain the supported application path. | VERIFIED-SOFTWARE (bounded) | The `WOVENHAT-S02-FREESTANDING-C-ABI-V1` fixture builds with LLVM/ld.lld and its independently guarded `s02-c-abi-test` uses the production loader, executes in Ring-3 with native exit 42, and verifies teardown at 1/2/4 CPUs (QEMU exit 33). Evidence: `audit-artifacts/s02-c-abi-rerun-{1,2,4}cpu-*`. libc/pthread/locale/math, destructors, TLS, async VFS/network and shared-library compatibility are explicitly outside S02. |
| S03 | Task/process lifecycle, IPC waits, terminal and input paths, shell/terminal applications. | PARTIAL | Safe `libwoven` signal/process-group wrappers pass the 193-test suite and warning-denying Clippy. The dedicated Ring-3 gate passes on 1/2/4 CPUs for fork/wait/reap, process-group SIGTERM teardown/status 143, and caught SIGUSR1 handler delivery/status 42. PTYs/termios, job control, default/stop/continue signal semantics, and complete zombie behavior remain open. |
| S03 | Task/process lifecycle, IPC waits, terminal and input paths, shell/terminal applications. | PARTIAL | Added safe `libwoven` wrappers for the existing bounded signal/process-group authorities; focused 193-test `libwoven` suite and warning-denying Clippy pass. Desktop and terminal journeys pass, but full signal disposition, process groups, PTYs/termios, job control and exact zombie/wait semantics still lack a dedicated complete acceptance matrix. |
| S04 | Scheduler, CPU migration/hotplug, capability/resource bounds and SMP tests in `kernel/src`. | VERIFIED-SOFTWARE (bounded) | Warning-denying lint and 1/2/4-CPU acceptance pass. Quota/priority/affinity accounting and topology-aware policy remain incomplete for S04 closure. |
| S05 | Paging, COW, allocator, swap and memory-pressure paths in `kernel/src`. | PARTIAL | Low-memory and allocator tests exist; deterministic reclaim/writeback/OOM policy and pressure-notification acceptance remain open. |
| S06 | WovenFS/VFS, FAT32 system path, journal/recovery and installer storage services. | VERIFIED-SOFTWARE (bounded) | Stage 28, WovenFS crash tests, installed-image boot and 1/2/4 QEMU evidence pass. Physical durability, full backup/restore and power-loss media qualification remain separate. |
| S07 | Service supervisor, configuration, logging and application service lifecycle. | VERIFIED-SOFTWARE (bounded) | Release chain and service-restart evidence pass. Persistent config migration, disk-full logging and privilege-separated service qualification need explicit S07 evidence. |
| S08 | PCI/NVMe/AHCI/xHCI/HDA/input driver ownership and restart paths. | VERIFIED-SOFTWARE (bounded) | Controller restart and USB/input matrices pass in QEMU. Generic crash supervision, hotplug breadth, power transitions and native hardware remain open. |
| S09 | smoltcp/network services, DHCP/DNS/ICMP, link loss, Wi-Fi/Bluetooth contracts and TLS policy. | VERIFIED-SOFTWARE (bounded) | Network/link-loss and radio-contract matrices pass. IPv6 socket lifecycle, routes/ND, profile service and full TLS certificate lifecycle remain incomplete. |
| S10 | WovenGuard capabilities, identity, package trust, TPM/Secure Boot boundary. | BLOCKED | Fail-closed trust preflight lacks x86_64 OVMF VARS, `swtpm`, OpenSSL and supplied development signing material; production custody and independent review are unavailable. |
| S11 | CPU renderer, compositor, surfaces and multi-output graphics under `apps/woven-shell` and graphics kernel paths. | VERIFIED-SOFTWARE (bounded) | GFX-02, single-output, dual-head and strict desktop matrices pass. Accelerated backend equivalence, reset/device-loss and native GPU qualification remain open. |
| S12 | Shell widgets, focus/input, clipboard, file dialogs and text/desktop surfaces. | PARTIAL | Strict desktop journey passes. Unicode shaping/fallback, localization, accessibility semantics/screen-reader and keyboard-only acceptance are not a complete closed contract. |
| S13 | Audio/video/printing/device services. | PARTIAL | HDA/device and input foundations exist; reusable audio mixing/capture, bounded media pipeline and IPP printing acceptance are not evidenced as complete. |
| S14 | WovenPackages, store, installer, update, rollback and recovery integration. | VERIFIED-SOFTWARE (bounded) | Release chain, package/update/security matrices and Stage 28/27.5 pass. Production trust, physical media and full transaction-boundary evidence remain open. |
| S15 | SDK/toolchain, examples, symbols, debugger workflow and independently built applications. | PARTIAL | Applications and tools build in-tree. Clean-environment reproducible SDK packaging and independent external build/run evidence remain open. |
| S16 | Sandbox profiles, isolated workload identities, filesystem views, IPC/network/resource limits. | PARTIAL | WovenGuard sandbox foundations and bounded capability tests exist. Adversarial isolation matrix and any approved Linux/Windows compatibility target are not closed. |
| S17 | Diagnostics, tracing, crash evidence, profiling and authenticated administration. | PARTIAL | Existing diagnostics and audit artifacts support local qualification. Versioned diagnostic bundle, authenticated remote administration, policy rollback and privacy acceptance remain open. |
| S18 | Sustained mixed workload, churn, restart, storage-fault and constrained-resource campaigns. | VERIFIED-SOFTWARE (bounded) | Seeded and storage-fault QEMU campaigns pass with retained evidence. This is not a production soak or native performance result; broader workload budgets remain open. |
| S19 | One-tree integrated closure, manifests, licenses/SBOM, provenance and release-image bypass audit. | BLOCKED | Current 158/158 release chain passes, but production trust/provenance, SBOM/clean-room evidence and independent review are not complete. |
| S20 | Honest software-qualified handoff and separate release decisions. | BLOCKED | Current release checkpoint remains NO-GO. Hardware, physical power-loss, trust/key custody, and independent security review remain external prerequisites. |

## S00 decision

S00 is `CLOSED` for the inventory increment. S01 is `CLOSED` for bounded
software qualification: the independent fixture exercised the real syscall
and fail-closed negative paths at 1/2/4 CPUs, and the current-head release
chain passed 158/158. S01 does not close any downstream capability. S02 is
`CLOSED` for its documented freestanding C ABI boundary: the exact C ELF built
and executed through the real loader on 1/2/4 CPUs. Hosted-C/POSIX runtime
compatibility remains outside S02.
S10, S13, S15, S17 and S19 also have independent software slices, but they are
not allowed to bypass the sequential prerequisite order.

## Safety and evidence boundary

This inventory used source inspection and retained host/QEMU evidence only. No
host reboot, raw-device access, physical-media write, firmware/TPM mutation,
production signing, or synthetic physical/independent-review evidence was
performed. QEMU/device-model results remain distinct from native qualification.
