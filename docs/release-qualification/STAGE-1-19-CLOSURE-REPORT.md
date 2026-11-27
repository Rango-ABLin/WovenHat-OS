# Stage 1-19 closure report

## Current-head bounded software closure — 2026-11-27

Source baseline: `685e9cbed05fd8f975f7cc14f4a6cf8eedcaeec9` on
`phase-b/bottom-taskbar-core`.

Decision for the in-repository acceptance boundary: **all Stage 1-19
software/QEMU gates are verified on the current source**. Evidence includes the
mandatory serialized `RUN-STAGE10.7.ps1` exit 0, warning-denying host/all-target
Clippy and affected feature Clippy, workspace tests, the 1/2/4-CPU inherited
matrix, and the expanded 8/16-CPU USB HID, compositor, window/input, damage,
scale and WovenShell input-isolation matrix. The xHCI handoff now fails closed
and a stalled WovenShell is terminated after its bounded compositor wait instead
of being accepted as a live desktop session.

This is not a collective production-ready declaration. The remaining acceptance
predicates are external to software/QEMU and therefore cannot be closed from a
local checkout:

- fresh physical USB keyboard/mouse and bare-metal compositor evidence;
- native storage/controller/radio/display qualification and physical power-loss
  or torn-media behavior;
- production non-exportable key custody, witnessed ceremony and actual signed
  exact-commit release artifacts;
- independent organization rebuild attestation and independent security review;
- physical Secure Boot/TPM and named-device certification evidence.

Until those submissions are received and verified, the repository-wide Stage
1-19 production decision remains **BLOCKED**, while no bounded in-repository
software acceptance gate remains unverified on this baseline.

Recorded 2026-11-15 on `integration/master-reconciliation-20261115`.
Source baseline `541dd70832b680e3fc4f248377c194611f71cc2d` plus the bootloader build repair described below.
Current acceptance applies only to explicitly executed checks.

## Stage 1-5 current-source software verification

The bounded Stage 1-5 storage-journal gate passes on 1/2/4 CPUs at current
source, together with affected warning-denying Clippy and six FAT32/page-cache
host tests. The inherited full Windows chain also passes at the same kernel
source. Stage 1-5 remains blocked from closure by native storage/DMA, physical
power-loss/torn-write evidence and independent review; no hardware claim is
inferred from the software marker.

## Stage 6 current-source bounded software closure

The dedicated Stage 6 feature lint and twice-cycled 2/4-CPU hotplug matrix pass
on current source. Cancellation, evacuation, rejected/timeout recovery,
offline/online masks and middle-CPU lifecycle are directly required. This
closes the bounded software/QEMU contract only; physical hotplug, high APIC
IDs, native NUMA and broader stress remain production blockers.

## Stage 7 current-source bounded software closure

An explicit runtime-harness route now requires the Stage 7 environment marker
and SMP profile. Harness contracts, affected feature lint and dedicated 1/2/4
CPU boots pass on current source. This closes the bounded environment ABI only;
complete signals, dynamic userspace runtime, unrestricted sizing and broader
fault/teardown qualification remain open Stage 7-10 software work.

## Stage 8 current-source bounded software closure

An explicit fail-closed harness contract now requires all Stage 8 IPC, shared-
memory, capability-transfer, service-discovery and multicore-stress markers.
Harness tests, warning-denying Clippy and 1/2/4-CPU boots pass. This closes the
bounded IPC software gate only; broader integration, stress and hardware
qualification remain open.

## Implemented and historical evidence

The authoritative `docs/stage1-19-closure-gate.md` records bounded foundation,
SMP, isolation, IPC, loader, WovenFS, drivers, networking, security, application
and compositor implementations with their historical software acceptance.
`docs/stage-status.md` contains later increments and evidence references.

## Tested and accepted here

The build, warning-denying Clippy, 218 Rust tests and 211 Python tests pass.
The strict WHPX desktop journey passes on 1/2/4 CPUs. The first Windows chain
passed all 60 memory runs, then failed at DNS under restricted host networking.
The focused network rerun passed with host network access. The full Windows
rerun passes with exit 0 in `audit-artifacts/acceptance-20261115-100412-082`.
The accepted loader-preservation matrix and fresh inherited chain are below.
Historical evidence has not been promoted beyond its measured scope.

## Blocked and remaining implementation

Stage 11.5 loader mapping ownership and address placement require focused
reconciliation with PRs 137/143. Storage persistence/crash recovery, broader
SMP stress, security and networking require current-source qualification.
Native hardware, physical power loss, production trust provisioning and
independent review retain the closure ledger's open release gates.

## Stage 12 source reconciliation

Native WovenFS mounting and system-volume dispatch are implemented in
`kernel/src/storage.rs`. `mount_ata_root` probes direct or GPT-backed WovenFS
and mounts `/mnt` through the native VFS adapter. Native `df`/`fscheck` and
root-aware service persistence are present. Historical 1/2/4-CPU installed
image evidence is recorded on 2026-11-04 in stage-status.

The dedicated installed-volume gates now cold-boot the same writable image
three times on 1/2/4 CPUs. The normal gate proves payload, shorter overwrite,
native consistency checks and cleanup across boots. The isolated interruption
gate stops after the flushed journal commit but before target writes; later
boots replay, verify, overwrite and clean up the same transaction. This closes
that bounded virtual-disk gap. Other cut positions, volatile-cache/torn-sector
behavior, physical power loss, native media, and broader application data
migration remain required.

## Stage 11 loader-preservation increment accepted

The pre-repair graph, dynamic-TLS displacement and missing fork mapping
failures remain preserved. The repair passes the 54-check combined matrix,
including 36 QEMU boots on 1/2/4 CPUs, and the complete fresh Windows chain in
`audit-artifacts/acceptance-20261115-162204-848`. Ownership, rollback, fork COW
isolation, all sixteen mmap slots, ASLR displacement and exact teardown are
directly exercised. Full Stage 1-19 closure remains open for the inventory's
remaining storage, application, integration, hardware and trust gates.

## Stage 11 pure-BSS TLS increment accepted

At commit `b9829f62`, a real compiler/LLD local-exec TLS image with zero
file-backed TLS bytes and eight zero-initialized runtime bytes executes through
the production loader on every online CPU. The dedicated gate passes on 1/2/4
CPUs with exact teardown. Build, warning-denying host and feature Clippy, Rust
tests, 212 Python tests and the complete serialized Stage 10.7 Windows chain
pass. This removes the ledger's pure-BSS TLS software gap only. Broader TLS
models, toolchain/entropy stress, storage recovery, hardware, trust and
independent review continue to block collective Stage 1-19 closure.
