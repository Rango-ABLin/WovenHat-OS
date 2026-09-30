# Stage status

Production completion is tracked separately from bounded foundation acceptance
in [the Stage 1–12 gap audit](stage1-12-production-gap-audit.md). No stage is
fully complete while a roadmap requirement remains open.

Authoritative development sequence: [supplied Stage 10.7–36 roadmap](master-development-roadmap.md).

## TCP close/drain preservation repair - 2026-09-24

This-machine preflight exposed an intermittent 4-CPU Stage 10.7 failure:
last-reference release removed a TCP socket with 16 bytes still queued.
Graceful bounded retirement now preserves that transport. Ten corrected
4-CPU repeats passed, including queued-close cases; the full 75-boot Stage
10.7 preservation gate passed. Related 10.8/10.9/13.9 checks on 1/2/4 CPUs
and normal release shell/SMP smoke also passed. Assertions and existing test
timeouts are unchanged; cleanup now requires transport slots at baseline. See
[the TCP repair audit](audit-tcp-close-drain-2026-09-24.md).

## Physical integration request - 2026-09-24

Physical integration is incomplete. The latest user instruction selects this
HP Pavilion x360 as the actual test machine, superseding the earlier AX200
target choice. Its adapter is AX201 `8086:A0F0`, subsystem `8086:0074`, revision
`20`. Native AX201 startup and transport remain unimplemented; adding its PCI
ID to the AX200 activation list is not a valid implementation.

The authorized Kingston USB was cleared and written with the verified inventory
image. An elevated helper read back all 8,454,144 image bytes and matched the
SHA-256. Its new GPT layout contains an 8 MiB EFI system partition. The internal
system disk was not written. Secure Boot is enabled and the image has no PE
Authenticode certificate; native boot requires a local firmware decision.
Windows C: is fully encrypted with BitLocker protection on, so the recovery key
must be available before changing firmware security settings. Neither BitLocker
nor firmware settings were changed, and no reboot or physical test has run.
The inventory image passed 1/2/4-CPU configured QEMU boots plus a no-UART
framebuffer check. No AP startup or physical radio is tested by that mode.
See [the AX201 machine audit](audit-ax201-machine-2026-09-24.md).

The runtime firmware loader now validates the complete AX200 section layout
before DMA allocation and copies payloads into an unpublished owned context.
It skips separators and init-image records and preserves existing DMA bounds.
Build, warning-denying host/kernel/feature Clippy, all 35 Rust tests and all
54 Wi-Fi markers on 1/2/4 CPUs passed for this loader increment.
See [the runtime-loader audit](audit-ax200-runtime-loader-2026-09-24.md).

A real AX200 firmware container exposed parser defects. The corrected
allocation-free TLV parser passes seven new host tests and the real-image
probe; all 54 Wi-Fi markers still pass on 1/2/4 CPUs. Build, host-test and
kernel Clippy, and all 31 Rust tests pass. The full Stage 10.7 preservation
gate passed after the heap lint cleanup: nine Python tests and 75 QEMU
boots, plus three focused Wi-Fi boots (78 total for this prerequisite).
See [the physical preflight audit](audit-physical-wifi-preflight-2026-09-24.md).
This is prerequisite work, not physical firmware/IRQ/DMA/RF acceptance.

## Current continuation - 2026-09-24

Base commit `0563d0b` introduced the Stage 13.10AC deferred AX200 interrupt
service candidate. This continuation passed preservation and bounded QEMU
acceptance; it does not establish production completion of Stage 13.

- Ordinary build and host/freestanding-kernel Clippy passed with warnings
  denied; 24 Rust host tests and nine Python harness tests passed.
- The complete Stage 10.7 preservation gate passed, exit code 0: 75 QEMU boots
  covering 1/2/4 CPUs, live networking, and async block/file/UDP/TCP.
- Stages 1-5, 10.8-10.9, 11.1-11.5, 12.1-12.5, and 13.1-13.9 passed
  per-feature freestanding Clippy and 1/2/4-CPU QEMU (66 additional boots).
  Wi-Fi required all 54 markers through 13.10AC on every CPU configuration.
- CPU hotplug passed on 2/4 CPUs. The normal release build and 4-CPU
  shell/PS2/IOAPIC/SMP smoke passed. Total: 144 successful QEMU boots.

The continuation corrected feature boundaries, a misplaced audio check in
USB HID's error branch, and a nested Cargo release-build artifact-lock
conflict in the shell harness. Failed/interrupted attempts remain preserved.
See [the continuation audit](audit-stage13-10ac-continuation-2026-09-23.md)
for exact evidence directories and validation scope.

Next prerequisites remain production Wi-Fi worker/lifecycle integration,
secure entropy provisioning, physical AX201 firmware/IRQ/DMA/RF qualification,
and the earlier unfulfilled roadmap requirements. Do not advance to a later
stage or treat these synthetic Wi-Fi results as working physical Wi-Fi.

## Stage 6 — accepted on 2026-09-16

The bounded SMP foundation passed the complete release matrix: warning-denying
kernel/host Clippy, all Rust host tests, 1/2/4-CPU memory/storage/network
gates, legacy PIC fallback, 4-CPU release gates, and shell/SMP smoke. See
[the Stage 6 audit](audit-stage6-2026-09-16.md).

General multicore userspace, unrestricted concurrent device/filesystem service
throughput, multi-node NUMA page-placement qualification, non-contiguous
hotplug hardware qualification, APIC-ID-above-255 hardware qualification, and
broader cross-layer lock-path qualification and priority inheritance
remain explicit deferred requirements in the Stage 6 contract. CPU-domain
placement, audited Ready-state I/O service migration, and the x2APIC MSR transport are
implemented; bounded contiguous-prefix AP offline/re-online control is also
implemented; interrupt-safe scheduler, IPC, WovenGuard, teardown, and worker
locks now enforce bounded rank and nesting
checks, and VirtIO network DMA uses an allocator-reserved physically
contiguous arena; see the [NUMA/x2APIC audit](audit-stage6-numa-2026-09-16.md),
[hotplug audit](audit-stage6-hotplug-2026-09-16.md), and
[lock-order audit](audit-stage6-lock-order-2026-09-16.md) and
[DMA audit](audit-stage6-dma-2026-09-16.md).

The CPU lifecycle follow-up supports a non-contiguous online mask in QEMU:
CPU 1 can park while CPU 3 continues scheduling work and acknowledging TLB
shootdowns, then rejoin its stable slot. Offline preparation validates every
task move before committing any; draining CPUs reject new placement. See the
[non-contiguous hotplug audit](audit-stage6-noncontiguous-hotplug-2026-09-17.md).
Physical non-contiguous hotplug, high APIC-ID, and multi-node NUMA hardware
qualification remain open.
The final source passed the complete release matrix and dedicated twice-cycled
2/4-CPU hotplug gates after this follow-up.
The cancellation follow-up distinguishes unclaimed requests from AP-owned
transitions, rearms rejected states, and keeps further hotplug disabled if an
AP never finishes a claimed transition. Forced AP rejections and unclaimed
timeouts recover and retry in the 2/4-CPU QEMU gates; broad concurrent-work
and physical-fault stress remains open.

The 2026-09-17 lock follow-up made the file-frame cache IRQ-safe, moved
fork/dup reference retention outside the process-table lock, and added
generation-tagged VFS and pipe handles to reject stale slot reuse. The VFS
read and materialization paths now release registry/open-description guards
for disk I/O and revalidate node identity, version, backing, and shared seek
position before committing results. Both VFS tables now use ranked IRQ-safe
locks. See the
[lock-order audit](audit-stage6-lock-order-2026-09-16.md). The full release
matrix and twice-cycled 2/4-CPU hotplug gates passed after these changes.
The pipe follow-up also uses scheduler-latched wakeups and rejects full waiter
tables, closing a lost-wakeup window under concurrent readers and writers.
The global heap metadata lock is now rank-50 IRQ-safe; page mapping happens
before that guard is taken during boot. Its earlier full release and 2/4-CPU
hotplug gates passed. The follow-up makes boot capacity RAM-scaled from
256 KiB to 8 MiB, raises live-allocation metadata to 2,048 entries, recovers
alignment padding, and coalesces freed intervals. A partial kernel map now
rolls back. Runtime growth beyond the eager mapping remains open.
See the [Stage 6 heap audit](audit-stage6-heap-2026-09-17.md).
The final heap change passed the full release matrix and twice-cycled 2/4-CPU
hotplug gates.
The heap metadata follow-up replaces the fixed live-object and free-interval
tables with per-span headers and an in-place coalescing free list. Host tests
and a 4,096-object QEMU probe cover the former 2,048-object ceiling; mapped
runtime growth and broad allocator throughput remain open. See the
[heap metadata audit](audit-stage6-heap-metadata-2026-09-17.md).
The final metadata source passed the full 33-check release matrix and the
twice-cycled 2/4-CPU hotplug gates.
Runtime heap growth now maps and publishes 256 KiB chunks outside the heap
guard. Rank-free, IRQ-enabled allocations can grow on failure; BSP idle maps
ahead for guarded callers when free space is low. QEMU exercises growth after
SMP startup and reserve maintenance. Sudden large guarded allocations,
low-memory behavior, and multicore throughput still need qualification; see
the [heap growth audit](audit-stage6-heap-growth-2026-09-17.md).
The final growth source passed the complete 33-check release matrix and
twice-cycled 2/4-CPU hotplug gates.
The frame allocator now reserves a bitmap in each usable physical range and
uses it to track allocations, reject double frees, and recover returned
frames beyond its 4,096-entry hot cache. See the
[frame reclamation audit](audit-stage6-frame-reclamation-2026-09-17.md).
The QEMU memory gate exercises 4,352 returned frames; physical high-RAM and
NUMA latency qualification remains open.
The final frame-reclamation source passed the full 33-check release matrix
and dedicated twice-cycled 2/4-CPU hotplug gates.
The bounded device, keyboard decoder, journal, swap-state, mount-record, and
key-vault tables now use IRQ-safe locks. Keyboard input preserves the caller's
pre-lock interrupt state for early-boot polling; Stage 1-5 journal, Stage 12.3
key-vault, Stage 12.5 mount-record, and normal PS/2 shell gates passed.
The full release matrix and dedicated twice-cycled 2/4-CPU hotplug gates also
passed after the keyboard interrupt-state correction.
The FAT32 clean-page cache now uses a rank-20 IRQ mutex only for short metadata
and page-copy sections; physical page loads run after releasing it, with an
invalidation epoch preventing stale publication. Host, full release, and
2/4-CPU hotplug gates passed. Cross-layer FAT32 mutation transactions remain
open for unrestricted concurrent filesystem service.
The shell current-directory state now has a short rank-10 IRQ guard, with a
fixed-buffer copy released before console output or allocation. The terminal
has a rank-10 preemption guard that keeps device IRQs live during rendering,
and syscall writes restore live IRQs around full-frame operations. Lock-order
tracker updates remain interrupt-atomic. This closes the audited terminal and
shell compatibility-lock gap, but not the other Stage 6 production gaps above.
The final source passed the complete release matrix and twice-cycled 2/4-CPU
hotplug gates; details are in the lock-order audit.
The network runtime and VirtIO transport now declare ranks 20 and 30 for
their existing runtime-to-transport nesting. No kernel `IrqMutex::new` call
sites remain; this is rank coverage, not proof of every cross-subsystem path.
The ranked network change passed the full release matrix and twice-cycled
2/4-CPU hotplug gates.

## Stage 10.7 — accepted on 2026-09-16

The imported recovery source is preserved in Git commit `49e789d`. That commit
records the baseline only and does not certify acceptance.

The audit corrected worker starvation, local-preemption lock safety, readiness
observation, host-exchange validation and evidence retention. The complete gate
ended with `=== STAGE 10.7 ACCEPTANCE: PASS ===` and host exit status 0.

| Gate | Result |
| --- | --- |
| `cargo build` | Passed |
| Host and freestanding-kernel Clippy with `-D warnings` | Passed |
| Rust host regressions | 13 passed |
| Python host-harness regressions | 4 passed |
| Memory/scheduler/pager/IPC/security preservation | 1 CPU: 10/10; 2 CPUs: 30/30; 4 CPUs: 20/20 |
| Live DHCP/DNS/ICMP/UDP/TCP | 1/2/4 CPUs passed |
| Stage 10.4 asynchronous block I/O | 1/2/4 CPUs passed |
| Stage 10.5 asynchronous file I/O | 1/2/4 CPUs passed |
| Stage 10.6 asynchronous UDP | 1/2/4 CPUs passed |
| Strengthened Stage 10.7 asynchronous TCP | 1/2/4 CPUs passed |

Total: 75 QEMU boots passed in the final full chain. Each boot required debug-exit
status 33 and its required markers; network harnesses also verified host traffic.
Final build/lint/host checks also passed and are recorded under
`audit-artifacts/final-static/`. The accepted-stage commit follows the imported
baseline as a separate Git commit; use `git log --oneline` to identify it.

## Stage 10.8 — accepted on 2026-09-16

Completion ports, operation association, batch poll/wait, bounded reservations,
cancellation events, generation retirement and timeout-aware scheduler blocking
are implemented. The full gate passed with exit code 0: all 75 Stage 10.7 boots
plus completion-port boots on 1/2/4 CPUs (78 total). The latter include Ring-3
copyout retry, measured timeout, teardown, SMP producers and consumer wakeup.
Build, warning-denying host/kernel/probe Clippy, 18 Rust tests and four Python
harness tests passed. Final host tests include waiter notification and port quota.

Full transcript: `audit-artifacts/acceptance-20260916-085314-575/acceptance-output.txt`.
Focused serial evidence is retained in timestamped `audit-artifacts/stage10.8-*`
directories. See [ABI contract](completion-port-abi.md) and
[audit](audit-stage10-8-2026-09-16.md).

## Stage 10.9 — accepted on 2026-09-16

Timers, deadlines, asynchronous event objects, cancellation and sleep-until
are implemented under bounded process ownership. The 1/2/4-CPU QEMU gate,
build, warning-denying Clippy and Rust regressions passed. See
[audit](audit-stage10-9-2026-09-16.md).

## Stage 10.10 — accepted on 2026-09-16

The aggregate gate validates the Stage 10.8 completion architecture together
with Stage 10.9 timers/events on 1, 2, and 4 CPUs. All six boots passed with
exit 33 and retained serial evidence. Stage 10 is closed; Stage 11 follows.

## Stage 11.1 — accepted on 2026-09-16

PID lifecycle, parent/child ownership, exit status, wait semantics, process
groups, isolated resources and teardown passed build, Clippy, host tests, and
1/2/4-CPU QEMU validation. See [audit](audit-stage11-1-2026-09-16.md).

Next: Stage 11.2 per-process threads and join/TLS semantics.

## Stage 11.2 — accepted on 2026-09-16

Generation-safe thread IDs, owner-checked join, termination status, TLS and
thread-local errno state passed build, Clippy, host tests, and 1/2/4-CPU QEMU
validation. See [audit](audit-stage11-2-2026-09-16.md).

Next: Stage 11.3 structured notifications and lifecycle events.

## Stages 11.3–11.5 — accepted on 2026-09-16

Structured notifications, the `libwoven` userspace API boundary, and hardened
ELF/W^X loader validation passed their individual 1/2/4-CPU QEMU gates,
freestanding Clippy, and host tests. See [audit](audit-stage11-3-5-2026-09-16.md).

The follow-up ASLR pass now randomizes production ELF, stack and mmap bases
with a deterministic `qemu-test` switch; see [audit](audit-aslr-2026-09-16.md).

## Stage 12.1 — accepted on 2026-09-16

The VFS 2.0 typed interface and SystemVfs adapter passed build, freestanding
Clippy, and QEMU validation on 1/2/4 CPUs. See
[audit](audit-stage12-1-2026-09-16.md).

Next: Stage 12.2 WovenFS metadata and crash-consistency design.

## Stage 13.1 — accepted on 2026-09-16

The WovenDriver manager passed device matching, binding, suspend/resume,
build, freestanding Clippy, and 1/2/4-CPU QEMU validation. See
[audit](audit-stage13-1-2026-09-16.md).

## Stage 13.2 — topology/ownership increment implemented; full acceptance open (2026-09-27)

The PCIe foundation now includes bounded generation-tagged function/bridge
topology, explicit single-owner claims, generation-tagged logical MMIO BAR
leases, teardown-before-reuse invalidation, and bridge bus-window parentage.
The dedicated preservation gate requires focused host tests, warning-denying
kernel Clippy, runtime-harness regressions, and 1/2/4-CPU QEMU evidence.

Full Stage 13.2 acceptance remains blocked on BAR size probing/allocation and
rebalance, bridge I/O/MMIO/prefetchable resource routing, generic MSI/MSI-X
vector allocation/programming/teardown, PCIe hotplug, and WovenDriver
binding/unbinding integrated with generation ownership. The current paging
layer also lacks a physical MMIO unmap/revoke primitive; topology teardown
therefore revokes logical lease authority but must not be described as page
table mapping revocation.

Stage 13.3 NVMe is intentionally not advanced by this work.

## Stages 12.2–12.5 — accepted on 2026-09-16

WovenFS metadata/integrity, volume-integrity envelope boundaries, snapshots,
and storage-management inventory each passed their 1/2/4-CPU gates. See
[audit](audit-stage12-2-5-2026-09-16.md). Production AEAD now passes the
isolated 1/2/4-CPU gate with a bounded revocable key vault; see
[audit](audit-volume-crypto-2026-09-16.md). Measured key provisioning,
persistent key storage, rotation policy, and encrypted-volume mount integration
remain open before encrypted volumes are security-complete.

## Stage 7.1.5 - accepted on 2026-09-16

The scheduler/pager watchdog and termination lifecycle gate passed in full:
50 one-CPU memory runs, 100 two-CPU runs, 50 four-CPU runs, and live
DHCP/DNS/ICMP plus host-verified UDP/TCP at 1/2/4 CPUs. See
audit-stage7-1-5-timeout-2026-09-16.md for the bounded lifecycle handoff
correction and retained serial evidence.

## Roadmap Stage 13.10A — Bluetooth HCI core accepted on 2026-09-30

The hardware-independent Bluetooth HCI foundation is accepted. Bounded command encoding, Command Complete parsing, controller command-credit state, opcode/status validation, and deterministic self-tests passed the dedicated 1/2/4-CPU QEMU gate with exit 33 in GitHub Actions run 36718370038. See [acceptance audit](../AUDIT-STAGE13.10A-BLUETOOTH-HCI-ACCEPTANCE.md).

Historical Wi-Fi labels 13.10A–AC are Stage 13.9 AX200/WovenWiFi extensions; roadmap Stage 13.10 is Bluetooth. Stage 13.10A does not claim USB transport or physical Bluetooth hardware support.

Next: Stage 13.10B Bluetooth USB HCI transport over xHCI.


## Roadmap Stage 13.10B — Bluetooth USB HCI transport contract accepted on 2026-09-30

The xHCI-owned Bluetooth USB transport contract is accepted at the descriptor/protocol boundary. Stage 13.10B recognizes the USB Wireless Controller / RF Controller / Bluetooth Primary Controller interface (E0/01/01), validates the interrupt-IN HCI event endpoint plus bulk-IN/bulk-OUT ACL endpoints, and constructs the class control-request setup packet used to carry HCI commands. The hardware-independent `bluetooth_hci.rs` protocol layer remains separate from USB transport semantics.

GitHub Actions run 36722602921 passed at source SHA `0ba3ce257f93775afe67ed975f8549fcdbb78506`, including lint/host regressions, the 1/2/4-core boot matrix, Stage 13.3–13.9 regression gates, and the Stage 13.10 runtime gate. The runtime harness requires `[S13.10B] Bluetooth USB HCI transport contract: PASSED`; the workflow step label still says Stage 13.10A and should be renamed in the next acceptance-maintenance change.

This acceptance is a transport-contract milestone, not a physical-controller claim. Live xHCI endpoint/ring ownership, HCI command/event transfer execution, ACL transfer execution, controller initialization, discovery, L2CAP, pairing/security, ATT/GATT, lifecycle/hotplug, and physical hardware qualification remain open.

Next: Stage 13.10C live USB HCI transfer execution and controller initialization boundary.


## Roadmap Stage 13.10C — bounded HCI transfer execution contract accepted on 2026-09-30

The Bluetooth command/event transaction boundary is accepted. The HCI core now serializes one outstanding command against controller credits, rejects a second command while one is in flight, requires the matching Command Complete opcode, and restores readiness only after successful completion. The xHCI Bluetooth layer also exposes bounded command, interrupt-event, ACL-IN, and ACL-OUT transfer plans derived from the accepted Stage 13.10B interface/endpoint contract.

GitHub Actions run 36729724615 passed at source SHA `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`. The dedicated “Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance” gate and the complete Stage 13.3–13.9 regression chain passed. Preserved release-validation artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`.

This milestone does not yet claim a physical Bluetooth controller or live USB transfer completion. Dedicated xHCI Bluetooth endpoint contexts/rings and DMA buffers, live control-OUT HCI command submission, interrupt-IN event reception, ACL execution, Reset/Read Local Version against hardware, teardown/recovery, and physical qualification remain open.

Next: bind the accepted transfer plans to dedicated xHCI rings/DMA ownership and execute the controller-initialization sequence.


## Roadmap Stage 13.10D — live USB HCI mechanics and controller initialization accepted on 2026-09-30

Stage 13.10D materializes the Bluetooth USB transport contract as xHCI-owned state: endpoint-zero control-OUT-with-data for HCI commands, dedicated interrupt-IN event and bulk-IN/bulk-OUT ACL endpoint contexts, separate transfer rings and DMA buffers, direction-aware DCI mapping, and bounded event/ACL execution primitives. The hardware-independent HCI layer adds a fail-closed controller initializer that requires Reset Command Complete before Read Local Version and reaches Ready only after the matching second completion.

GitHub Actions run 36747805657 passed at exact source SHA `41e009543e01afc6a348122ff3a2c4f0be16e351`. The dedicated “Stage 13.10D Bluetooth controller init 1/2/4-core acceptance” step passed together with the complete regression chain. Preserved release-validation artifact digest: `sha256:a245914f91143eb8f4d4b5aa6747d3d5bd07230c6dfa6d650455eb27ba52cae1`.

This acceptance proves the bounded transport/initialization architecture under deterministic CI; it does not claim physical Bluetooth-radio qualification because the QEMU acceptance device is not a Bluetooth controller. Physical USB-controller execution, teardown/recovery, hotplug, and radio qualification remain explicit hardware work.

Next: Stage 13.10E Bluetooth discovery/scanning and controller capability discovery above the accepted HCI initialization boundary.
